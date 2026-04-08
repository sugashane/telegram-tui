/// Telegram client wrapper around `grammers` 0.9.
///
/// Defines our own data types so the rest of the app doesn't depend on
/// grammers directly. Communication between the TUI and the client task
/// happens through tokio mpsc channels.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use grammers_client::client::UpdateStream;
use grammers_client::client::UpdatesConfiguration;
use grammers_client::message::InputMessage;
use grammers_client::peer::Peer;
use grammers_client::update::Update;
use grammers_client::Client;
use grammers_client::SignInError;
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use grammers_session::types::PeerRef;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Our domain types — independent of grammers internals
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TgDialog {
    pub id: i64,
    pub peer_ref: PeerRef,
    pub title: String,
    pub kind: DialogKind,
    pub unread_count: i32,
    pub last_message_text: String,
    pub last_message_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    User,
    Group,
    Channel,
}

#[derive(Debug, Clone)]
pub struct TgMessage {
    pub id: i32,
    pub chat_id: i64,
    pub sender_name: String,
    pub sender_id: Option<i64>,
    pub text: String,
    pub date: DateTime<Utc>,
    pub reply_to_msg_id: Option<i32>,
    pub outgoing: bool,
    pub edit_date: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Actions (UI → client) and Events (client → UI)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum TelegramAction {
    // Auth actions
    RequestLoginCode { phone: String },
    SubmitLoginCode { code: String },
    SubmitTwoFaPassword { password: String },

    // Normal actions
    LoadDialogs,
    LoadHistory { chat_id: i64, limit: usize },
    SendMessage { chat_id: i64, text: String, reply_to: Option<i32> },
    EditMessage { chat_id: i64, message_id: i32, text: String },
    DeleteMessages { chat_id: i64, message_ids: Vec<i32> },
    MarkRead { chat_id: i64 },
    Shutdown,
}

#[derive(Debug)]
pub enum TelegramEvent {
    // Auth events
    AuthRequired,
    LoginCodeSent,
    LoginSuccess,
    LoginNeedPassword,
    LoginError(String),

    // Normal events
    DialogsLoaded(Vec<TgDialog>),
    HistoryLoaded { chat_id: i64, messages: Vec<TgMessage> },
    NewMessage(TgMessage),
    MessageEdited(TgMessage),
    MessagesDeleted { chat_id: i64, message_ids: Vec<i32> },
    MessageSent(TgMessage),
    Error(String),
    Connected,
}

pub type ActionTx = mpsc::UnboundedSender<TelegramAction>;
pub type ActionRx = mpsc::UnboundedReceiver<TelegramAction>;
pub type EventTx = mpsc::UnboundedSender<TelegramEvent>;
pub type EventRx = mpsc::UnboundedReceiver<TelegramEvent>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a stable i64 id from a Peer, using Telegram Bot API dialog-id format.
fn peer_to_i64(peer: &Peer) -> i64 {
    peer.id().bot_api_dialog_id()
}

fn peer_kind(peer: &Peer) -> DialogKind {
    match peer {
        Peer::User(_) => DialogKind::User,
        Peer::Group(_) => DialogKind::Group,
        Peer::Channel(_) => DialogKind::Channel,
    }
}

fn convert_message(msg: &grammers_client::message::Message) -> TgMessage {
    let sender_name = msg
        .sender()
        .and_then(|p| p.name().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown".into());
    let sender_id = msg
        .sender_id()
        .map(|pid| pid.bot_api_dialog_id());
    // Use peer_id() which always resolves, not peer() which requires cache
    let chat_id = msg.peer_id().bot_api_dialog_id();
    TgMessage {
        id: msg.id(),
        chat_id,
        sender_name,
        sender_id,
        text: msg.text().to_string(),
        date: msg.date(),
        reply_to_msg_id: msg.reply_to_message_id(),
        outgoing: msg.outgoing(),
        edit_date: msg.edit_date(),
    }
}

// ---------------------------------------------------------------------------
// Peer cache — maps our i64 chat_id → PeerRef for API calls
// ---------------------------------------------------------------------------

struct PeerCache {
    refs: std::collections::HashMap<i64, PeerRef>,
}

impl PeerCache {
    fn new() -> Self {
        Self {
            refs: std::collections::HashMap::new(),
        }
    }

    fn insert(&mut self, id: i64, peer_ref: PeerRef) {
        self.refs.insert(id, peer_ref);
    }

    fn get(&self, id: i64) -> Option<PeerRef> {
        self.refs.get(&id).copied()
    }
}

// ---------------------------------------------------------------------------
// Connect (no authentication — login is handled via TUI actions)
// ---------------------------------------------------------------------------

/// Start everything: connect, create client, spawn pool runner and client task.
/// Returns (Client, is_authorized, ActionTx, EventRx).
/// If `is_authorized` is false, the TUI should show a login screen and
/// send auth actions (RequestLoginCode, SubmitLoginCode, etc.).
pub async fn start(
    api_id: i32,
    api_hash: String,
    session_path: &Path,
) -> Result<(Client, bool, ActionTx, EventRx)> {
    // Open session
    let session = Arc::new(
        SqliteSession::open(session_path)
            .await
            .context("failed to open session database")?,
    );

    // Create sender pool
    let SenderPool {
        runner,
        handle,
        updates,
    } = SenderPool::new(Arc::clone(&session) as Arc<_>, api_id);

    let client = Client::new(handle);

    // Spawn the network I/O runner
    tokio::spawn(runner.run());

    // Check auth status but don't authenticate here
    let authorized = client.is_authorized().await?;

    // Set up action/event channels
    let (action_tx, action_rx) = mpsc::unbounded_channel::<TelegramAction>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<TelegramEvent>();

    // Create update stream
    let update_stream = client
        .stream_updates(updates, UpdatesConfiguration::default())
        .await;

    // Spawn our client task (holds api_hash for login actions)
    let client_for_task = client.clone();
    tokio::spawn(async move {
        run_client_task(client_for_task, api_hash, update_stream, action_rx, event_tx).await;
    });

    Ok((client, authorized, action_tx, event_rx))
}

// ---------------------------------------------------------------------------
// Background task
// ---------------------------------------------------------------------------

async fn run_client_task(
    client: Client,
    api_hash: String,
    mut update_stream: UpdateStream,
    mut action_rx: ActionRx,
    event_tx: EventTx,
) {
    use grammers_client::client::{LoginToken, PasswordToken};

    let mut cache = PeerCache::new();
    let mut login_token: Option<LoginToken> = None;
    let mut password_token: Option<PasswordToken> = None;

    let _ = event_tx.send(TelegramEvent::Connected);

    loop {
        tokio::select! {
            action = action_rx.recv() => {
                match action {
                    // ── Auth actions ────────────────────────────────
                    Some(TelegramAction::RequestLoginCode { phone }) => {
                        match client.request_login_code(&phone, &api_hash).await {
                            Ok(token) => {
                                login_token = Some(token);
                                let _ = event_tx.send(TelegramEvent::LoginCodeSent);
                            }
                            Err(e) => {
                                let _ = event_tx.send(TelegramEvent::LoginError(
                                    format!("Failed to request code: {e}")
                                ));
                            }
                        }
                    }
                    Some(TelegramAction::SubmitLoginCode { code }) => {
                        if let Some(token) = login_token.take() {
                            match client.sign_in(&token, &code).await {
                                Ok(_) => {
                                    let _ = event_tx.send(TelegramEvent::LoginSuccess);
                                }
                                Err(SignInError::PasswordRequired(pt)) => {
                                    password_token = Some(pt);
                                    let _ = event_tx.send(TelegramEvent::LoginNeedPassword);
                                }
                                Err(SignInError::SignUpRequired) => {
                                    let _ = event_tx.send(TelegramEvent::LoginError(
                                        "This phone number is not registered with Telegram.".into()
                                    ));
                                }
                                Err(e) => {
                                    let _ = event_tx.send(TelegramEvent::LoginError(
                                        format!("Sign in failed: {e}")
                                    ));
                                }
                            }
                        } else {
                            let _ = event_tx.send(TelegramEvent::LoginError(
                                "No login code was requested. Please enter your phone number first.".into()
                            ));
                        }
                    }
                    Some(TelegramAction::SubmitTwoFaPassword { password }) => {
                        if let Some(pt) = password_token.take() {
                            match client.check_password(pt, &password).await {
                                Ok(_) => {
                                    let _ = event_tx.send(TelegramEvent::LoginSuccess);
                                }
                                Err(e) => {
                                    let _ = event_tx.send(TelegramEvent::LoginError(
                                        format!("2FA check failed: {e}")
                                    ));
                                }
                            }
                        }
                    }

                    // ── Normal actions ──────────────────────────────
                    Some(TelegramAction::LoadDialogs) => {
                        match load_dialogs(&client, &mut cache).await {
                            Ok(dialogs) => {
                                let _ = event_tx.send(TelegramEvent::DialogsLoaded(dialogs));
                            }
                            Err(e) => {
                                let _ = event_tx.send(TelegramEvent::Error(
                                    format!("Failed to load dialogs: {e}")
                                ));
                            }
                        }
                    }
                    Some(TelegramAction::LoadHistory { chat_id, limit }) => {
                        match load_history(&client, &cache, chat_id, limit).await {
                            Ok(messages) => {
                                let _ = event_tx.send(TelegramEvent::HistoryLoaded {
                                    chat_id,
                                    messages,
                                });
                            }
                            Err(e) => {
                                let _ = event_tx.send(TelegramEvent::Error(
                                    format!("Failed to load history: {e}")
                                ));
                            }
                        }
                    }
                    Some(TelegramAction::SendMessage { chat_id, text, reply_to }) => {
                        if let Some(peer_ref) = cache.get(chat_id) {
                            let mut input = InputMessage::new().text(&text);
                            if let Some(reply_id) = reply_to {
                                input = input.reply_to(Some(reply_id));
                            }
                            match client.send_message(peer_ref, input).await {
                                Ok(msg) => {
                                    let _ = event_tx.send(TelegramEvent::MessageSent(
                                        convert_message(&msg),
                                    ));
                                }
                                Err(e) => {
                                    let _ = event_tx.send(TelegramEvent::Error(
                                        format!("Failed to send: {e}")
                                    ));
                                }
                            }
                        }
                    }
                    Some(TelegramAction::EditMessage { chat_id, message_id, text }) => {
                        if let Some(peer_ref) = cache.get(chat_id) {
                            let input = InputMessage::new().text(&text);
                            if let Err(e) = client.edit_message(peer_ref, message_id, input).await {
                                let _ = event_tx.send(TelegramEvent::Error(
                                    format!("Failed to edit: {e}")
                                ));
                            }
                        }
                    }
                    Some(TelegramAction::DeleteMessages { chat_id, message_ids }) => {
                        if let Some(peer_ref) = cache.get(chat_id) {
                            if let Err(e) = client.delete_messages(peer_ref, &message_ids).await {
                                let _ = event_tx.send(TelegramEvent::Error(
                                    format!("Failed to delete: {e}")
                                ));
                            }
                        }
                    }
                    Some(TelegramAction::MarkRead { chat_id }) => {
                        if let Some(peer_ref) = cache.get(chat_id) {
                            let _ = client.mark_as_read(peer_ref).await;
                        }
                    }
                    Some(TelegramAction::Shutdown) | None => {
                        break;
                    }
                }
            }
            update = update_stream.next() => {
                match update {
                    Ok(Update::NewMessage(msg)) => {
                        let tg_msg = convert_message(&msg);
                        if let Some(pr) = msg.peer_ref().await {
                            cache.insert(tg_msg.chat_id, pr);
                        }
                        let _ = event_tx.send(TelegramEvent::NewMessage(tg_msg));
                    }
                    Ok(Update::MessageEdited(msg)) => {
                        let _ = event_tx.send(TelegramEvent::MessageEdited(
                            convert_message(&msg),
                        ));
                    }
                    Ok(Update::MessageDeleted(deletion)) => {
                        let _ = event_tx.send(TelegramEvent::MessagesDeleted {
                            chat_id: 0,
                            message_ids: deletion.messages().to_vec(),
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let _ = event_tx.send(TelegramEvent::Error(
                            format!("Update error: {e}")
                        ));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// API operations
// ---------------------------------------------------------------------------

async fn load_dialogs(
    client: &Client,
    cache: &mut PeerCache,
) -> Result<Vec<TgDialog>> {
    let mut dialogs_iter = client.iter_dialogs();
    let mut result = Vec::new();

    while let Some(dialog) = dialogs_iter.next().await? {
        let peer = dialog.peer();
        let id = peer_to_i64(peer);
        let peer_ref = dialog.peer_ref();
        cache.insert(id, peer_ref);

        let unread = match &dialog.raw {
            grammers_client::tl::enums::Dialog::Dialog(d) => d.unread_count,
            grammers_client::tl::enums::Dialog::Folder(_) => 0,
        };

        let last_msg = dialog.last_message.as_ref();
        let tg_dialog = TgDialog {
            id,
            peer_ref,
            title: peer.name().unwrap_or("(no name)").to_string(),
            kind: peer_kind(peer),
            unread_count: unread,
            last_message_text: last_msg
                .map(|m| m.text().to_string())
                .unwrap_or_default(),
            last_message_date: last_msg.map(|m| m.date()),
        };
        result.push(tg_dialog);
    }

    Ok(result)
}

async fn load_history(
    client: &Client,
    cache: &PeerCache,
    target_chat_id: i64,
    limit: usize,
) -> Result<Vec<TgMessage>> {
    let peer_ref = cache
        .get(target_chat_id)
        .context("chat not in cache — open it from the chat list first")?;

    let mut iter = client.iter_messages(peer_ref);
    iter = iter.limit(limit);

    let mut messages = Vec::new();
    while let Some(msg) = iter.next().await? {
        messages.push(convert_message(&msg));
    }
    messages.reverse();
    Ok(messages)
}
