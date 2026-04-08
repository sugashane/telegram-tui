use std::collections::{HashSet, VecDeque};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::{AppConfig, AppState, Theme, ThemePalette};
use crate::input::{self, Command, FilterKind};
use crate::telegram::{
    ActionTx, TelegramAction, TelegramEvent, TgDialog, TgMessage, DialogKind,
};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginPhase {
    EnteringPhone,
    WaitingForCode,
    EnteringCode,
    EnteringPassword,
    WaitingForAuth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Search,
    Visual,
    ThemePicker,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Command => "COMMAND",
            Self::Search => "SEARCH",
            Self::Visual => "VISUAL",
            Self::ThemePicker => "THEME",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Chats,
    Messages,
    Compose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationFilter {
    All,
    Unread,
    Favorites,
    Dms,
    Groups,
    Channels,
}

impl ConversationFilter {
    pub const ALL_FILTERS: &'static [Self] = &[
        Self::All,
        Self::Unread,
        Self::Favorites,
        Self::Dms,
        Self::Groups,
        Self::Channels,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Unread => "Unread",
            Self::Favorites => "Favs",
            Self::Dms => "DMs",
            Self::Groups => "Groups",
            Self::Channels => "Chans",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL_FILTERS.iter().position(|&f| f == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        let i = (self.index() + 1) % Self::ALL_FILTERS.len();
        Self::ALL_FILTERS[i]
    }

    pub fn prev(self) -> Self {
        let i = if self.index() == 0 {
            Self::ALL_FILTERS.len() - 1
        } else {
            self.index() - 1
        };
        Self::ALL_FILTERS[i]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Reverse,
}

// ---------------------------------------------------------------------------
// App – single source of truth for all UI state
// ---------------------------------------------------------------------------

pub struct App {
    // -- Telegram client channel --
    pub action_tx: ActionTx,

    // -- Screen state --
    pub screen: Screen,
    pub login_phase: LoginPhase,
    pub login_phone: String,
    pub login_code: String,
    pub login_password: String,
    pub login_cursor: usize,
    pub login_error: Option<String>,

    // -- Core state --
    pub mode: Mode,
    pub focus: FocusPane,
    pub filter: ConversationFilter,
    pub config: AppConfig,
    pub state: AppState,
    pub palette: ThemePalette,
    pub should_quit: bool,
    pub connected: bool,

    // -- Chat list --
    pub dialogs: Vec<TgDialog>,
    pub selected_chat: usize,
    pub loaded_chat_id: Option<i64>,
    pub favorite_chat_ids: HashSet<i64>,
    pub muted_chat_ids: HashSet<i64>,

    // -- Messages --
    pub messages: Vec<TgMessage>,
    pub selected_message: usize,
    pub message_scroll_offset: usize,

    // -- Compose --
    pub compose_input: String,
    pub compose_cursor: usize,
    pub compose_undo_stack: VecDeque<(String, usize)>,
    pub compose_redo_stack: VecDeque<(String, usize)>,
    pub pending_reply_msg_id: Option<i32>,
    pub pending_reply_author: Option<String>,
    pub pending_edit_msg_id: Option<i32>,

    // -- Visual mode --
    pub visual_anchor: Option<usize>,
    pub visual_line_mode: bool,
    pub yank_register: String,

    // -- Search --
    pub search_input: String,
    pub search_direction: SearchDirection,
    pub last_search_query: Option<String>,
    pub last_search_direction: Option<SearchDirection>,
    pub search_matches: Vec<usize>,
    pub search_matches_set: HashSet<usize>,
    pub search_match_cursor: Option<usize>,

    // -- Command mode --
    pub command_input: String,

    // -- Help overlay --
    pub show_help: bool,
    pub help_scroll: u16,
    pub help_search_input: String,
    pub help_search_active: bool,
    pub help_search_match: Option<usize>,

    // -- Theme picker --
    pub selected_theme: usize,
    pub theme_before_picker: Option<Theme>,
    pub picker_filter: String,

    // -- UI state --
    pub status: String,
    pub pending_g: bool,
    pub pending_count: String,
    pub last_poll_ok: Option<bool>,

    // -- Compose normal-mode state --
    pub compose_pending_delete: bool,
}

impl App {
    pub fn new(action_tx: ActionTx, is_authorized: bool) -> Self {
        let config = AppConfig::load();
        let state = AppState::load();
        let palette = config.theme.palette();

        // Pre-fill phone from config if available
        let login_phone = config.phone.clone().unwrap_or_default();
        let phone_len = login_phone.len();

        let screen = if is_authorized { Screen::Main } else { Screen::Login };

        let app = Self {
            action_tx,
            screen,
            login_phase: LoginPhase::EnteringPhone,
            login_phone,
            login_code: String::new(),
            login_password: String::new(),
            login_cursor: phone_len,
            login_error: None,

            mode: Mode::Normal,
            focus: FocusPane::Chats,
            filter: ConversationFilter::All,
            config,
            state: state.clone(),
            palette,
            should_quit: false,
            connected: false,

            dialogs: Vec::new(),
            selected_chat: 0,
            loaded_chat_id: None,
            favorite_chat_ids: state.favorite_chat_ids.clone(),
            muted_chat_ids: state.muted_chat_ids.clone(),

            messages: Vec::new(),
            selected_message: 0,
            message_scroll_offset: 0,

            compose_input: state.compose_draft.clone(),
            compose_cursor: state.compose_draft.len(),
            compose_undo_stack: VecDeque::new(),
            compose_redo_stack: VecDeque::new(),
            pending_reply_msg_id: None,
            pending_reply_author: None,
            pending_edit_msg_id: None,

            visual_anchor: None,
            visual_line_mode: false,
            yank_register: String::new(),

            search_input: String::new(),
            search_direction: SearchDirection::Forward,
            last_search_query: None,
            last_search_direction: None,
            search_matches: Vec::new(),
            search_matches_set: HashSet::new(),
            search_match_cursor: None,

            command_input: String::new(),

            show_help: false,
            help_scroll: 0,
            help_search_input: String::new(),
            help_search_active: false,
            help_search_match: None,

            selected_theme: 0,
            theme_before_picker: None,
            picker_filter: String::new(),

            status: String::new(),
            pending_g: false,
            pending_count: String::new(),
            last_poll_ok: None,

            compose_pending_delete: false,
        };

        // Only load dialogs if already authorized
        if is_authorized {
            let _ = app.action_tx.send(TelegramAction::LoadDialogs);
        }
        app
    }

    // -----------------------------------------------------------------------
    // Persistence
    // -----------------------------------------------------------------------

    pub fn persist_state(&self) {
        let s = AppState {
            selected_chat_id: self.current_chat_id(),
            favorite_chat_ids: self.favorite_chat_ids.clone(),
            muted_chat_ids: self.muted_chat_ids.clone(),
            starred_message_keys: self.state.starred_message_keys.clone(),
            filter: match self.filter {
                ConversationFilter::All => "all",
                ConversationFilter::Unread => "unread",
                ConversationFilter::Favorites => "favorites",
                ConversationFilter::Dms => "dms",
                ConversationFilter::Groups => "groups",
                ConversationFilter::Channels => "channels",
            }
            .into(),
            compose_draft: self.compose_input.clone(),
            read_watermarks: self.state.read_watermarks.clone(),
        };
        let _ = s.save();
    }

    // -----------------------------------------------------------------------
    // Filtered dialog list
    // -----------------------------------------------------------------------

    pub fn visible_dialogs(&self) -> Vec<&TgDialog> {
        self.dialogs
            .iter()
            .filter(|d| match self.filter {
                ConversationFilter::All => true,
                ConversationFilter::Unread => d.unread_count > 0,
                ConversationFilter::Favorites => self.favorite_chat_ids.contains(&d.id),
                ConversationFilter::Dms => d.kind == DialogKind::User,
                ConversationFilter::Groups => d.kind == DialogKind::Group,
                ConversationFilter::Channels => d.kind == DialogKind::Channel,
            })
            .collect()
    }

    pub fn current_chat_id(&self) -> Option<i64> {
        let visible = self.visible_dialogs();
        visible.get(self.selected_chat).map(|d| d.id)
    }

    fn chat_count(&self) -> usize {
        self.visible_dialogs().len()
    }

    fn message_count(&self) -> usize {
        self.messages.len()
    }

    fn take_count(&mut self) -> usize {
        let n = self.pending_count.parse::<usize>().unwrap_or(1);
        self.pending_count.clear();
        n
    }

    // -----------------------------------------------------------------------
    // Telegram event handling
    // -----------------------------------------------------------------------

    pub fn handle_telegram_event(&mut self, event: TelegramEvent) {
        match event {
            // ── Auth events ────────────────────────────────────────
            TelegramEvent::AuthRequired => {
                self.screen = Screen::Login;
                self.login_phase = LoginPhase::EnteringPhone;
                self.login_error = None;
            }
            TelegramEvent::LoginCodeSent => {
                self.login_phase = LoginPhase::EnteringCode;
                self.login_code.clear();
                self.login_cursor = 0;
                self.login_error = None;
            }
            TelegramEvent::LoginSuccess => {
                self.screen = Screen::Main;
                self.login_error = None;
                self.connected = true;
                self.status = "Logged in successfully".into();
                // Save phone to config for next time
                self.config.phone = Some(self.login_phone.clone());
                let _ = self.config.save();
                // Load dialogs now that we're authenticated
                let _ = self.action_tx.send(TelegramAction::LoadDialogs);
            }
            TelegramEvent::LoginNeedPassword => {
                self.login_phase = LoginPhase::EnteringPassword;
                self.login_password.clear();
                self.login_cursor = 0;
                self.login_error = None;
            }
            TelegramEvent::LoginError(msg) => {
                self.login_error = Some(msg);
                // Go back to the appropriate input phase
                match self.login_phase {
                    LoginPhase::WaitingForCode => {
                        self.login_phase = LoginPhase::EnteringPhone;
                        self.login_cursor = self.login_phone.len();
                    }
                    LoginPhase::WaitingForAuth => {
                        // Could be code or password failure
                        if !self.login_code.is_empty() {
                            self.login_phase = LoginPhase::EnteringCode;
                            self.login_cursor = self.login_code.len();
                        } else {
                            self.login_phase = LoginPhase::EnteringPhone;
                            self.login_cursor = self.login_phone.len();
                        }
                    }
                    _ => {}
                }
            }

            // ── Normal events ──────────────────────────────────────
            TelegramEvent::Connected => {
                self.connected = true;
                self.status = "Connected to Telegram".into();
                self.last_poll_ok = Some(true);
            }
            TelegramEvent::DialogsLoaded(dialogs) => {
                self.dialogs = dialogs;
                self.status = format!("{} chats loaded", self.dialogs.len());
                self.last_poll_ok = Some(true);
                // Restore selected chat
                if let Some(saved_id) = self.state.selected_chat_id {
                    if let Some(pos) = self.visible_dialogs().iter().position(|d| d.id == saved_id) {
                        self.selected_chat = pos;
                    }
                }
            }
            TelegramEvent::HistoryLoaded { chat_id, messages } => {
                if self.loaded_chat_id == Some(chat_id) {
                    self.messages = messages;
                    if !self.messages.is_empty() {
                        self.selected_message = self.messages.len() - 1;
                    }
                    self.message_scroll_offset = 0;
                }
            }
            TelegramEvent::NewMessage(mut msg) => {
                if self.loaded_chat_id == Some(msg.chat_id) {
                    // Deduplicate: our own sent messages arrive via both
                    // MessageSent and the update stream
                    if !self.messages.iter().any(|m| m.id == msg.id) {
                        // Resolve "Unknown" sender from dialog list
                        if msg.sender_name == "Unknown" {
                            if let Some(dialog) = self.dialogs.iter().find(|d| d.id == msg.chat_id) {
                                if dialog.kind == DialogKind::User {
                                    msg.sender_name = dialog.title.clone();
                                }
                            }
                        }
                        // Auto-scroll if user was at the bottom
                        let was_at_bottom = self.messages.is_empty()
                            || self.selected_message >= self.messages.len().saturating_sub(1);
                        self.messages.push(msg.clone());
                        if was_at_bottom {
                            self.selected_message = self.messages.len() - 1;
                        }
                    }
                }
                // Update dialog's last message
                if let Some(dialog) = self.dialogs.iter_mut().find(|d| d.id == msg.chat_id) {
                    dialog.last_message_text = msg.text.clone();
                    dialog.last_message_date = Some(msg.date);
                    if !msg.outgoing {
                        dialog.unread_count += 1;
                    }
                }
            }
            TelegramEvent::MessageEdited(msg) => {
                if self.loaded_chat_id == Some(msg.chat_id) {
                    if let Some(existing) = self.messages.iter_mut().find(|m| m.id == msg.id) {
                        existing.text = msg.text;
                        existing.edit_date = msg.edit_date;
                    }
                }
            }
            TelegramEvent::MessagesDeleted { chat_id, message_ids } => {
                if self.loaded_chat_id == Some(chat_id) || chat_id == 0 {
                    self.messages.retain(|m| !message_ids.contains(&m.id));
                    let len = self.messages.len();
                    if self.selected_message >= len && len > 0 {
                        self.selected_message = len - 1;
                    }
                }
            }
            TelegramEvent::MessageSent(msg) => {
                if self.loaded_chat_id == Some(msg.chat_id) {
                    self.messages.push(msg);
                    self.selected_message = self.messages.len().saturating_sub(1);
                }
                self.compose_input.clear();
                self.compose_cursor = 0;
                self.pending_reply_msg_id = None;
                self.pending_reply_author = None;
                self.pending_edit_msg_id = None;
            }
            TelegramEvent::Error(err) => {
                self.status = format!("Error: {err}");
                self.last_poll_ok = Some(false);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Login screen input
    // -----------------------------------------------------------------------

    fn on_login_key(&mut self, key: KeyEvent) {
        // Get mutable ref to the active input field and cursor
        let (input, cursor) = match self.login_phase {
            LoginPhase::EnteringPhone => (&mut self.login_phone, &mut self.login_cursor),
            LoginPhase::EnteringCode => (&mut self.login_code, &mut self.login_cursor),
            LoginPhase::EnteringPassword => (&mut self.login_password, &mut self.login_cursor),
            LoginPhase::WaitingForCode | LoginPhase::WaitingForAuth => return,
        };

        match key.code {
            KeyCode::Enter => {
                match self.login_phase {
                    LoginPhase::EnteringPhone => {
                        if !self.login_phone.trim().is_empty() {
                            self.login_phase = LoginPhase::WaitingForCode;
                            self.login_error = None;
                            let _ = self.action_tx.send(TelegramAction::RequestLoginCode {
                                phone: self.login_phone.trim().to_string(),
                            });
                        }
                    }
                    LoginPhase::EnteringCode => {
                        if !self.login_code.trim().is_empty() {
                            self.login_phase = LoginPhase::WaitingForAuth;
                            self.login_error = None;
                            let _ = self.action_tx.send(TelegramAction::SubmitLoginCode {
                                code: self.login_code.trim().to_string(),
                            });
                        }
                    }
                    LoginPhase::EnteringPassword => {
                        if !self.login_password.is_empty() {
                            self.login_phase = LoginPhase::WaitingForAuth;
                            self.login_error = None;
                            let _ = self.action_tx.send(TelegramAction::SubmitTwoFaPassword {
                                password: self.login_password.clone(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char(c) => {
                input.insert(*cursor, c);
                *cursor += 1;
            }
            KeyCode::Backspace => {
                if *cursor > 0 {
                    input.remove(*cursor - 1);
                    *cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if *cursor < input.len() {
                    input.remove(*cursor);
                }
            }
            KeyCode::Left => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            KeyCode::Right => {
                if *cursor < input.len() {
                    *cursor += 1;
                }
            }
            KeyCode::Home => {
                *cursor = 0;
            }
            KeyCode::End => {
                *cursor = input.len();
            }
            KeyCode::Esc => {
                // On phone screen, quit. On code/password, go back to phone.
                match self.login_phase {
                    LoginPhase::EnteringPhone => {
                        self.should_quit = true;
                    }
                    LoginPhase::EnteringCode | LoginPhase::EnteringPassword => {
                        self.login_phase = LoginPhase::EnteringPhone;
                        self.login_cursor = self.login_phone.len();
                        self.login_error = None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Key dispatch
    // -----------------------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        // Global quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        // Login screen has its own input handling
        if self.screen == Screen::Login {
            self.on_login_key(key);
            return;
        }

        // Help overlay handling
        if self.show_help {
            self.handle_help_key(key);
            return;
        }

        // Ctrl shortcuts in Normal/Visual mode
        if matches!(self.mode, Mode::Normal | Mode::Visual)
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            match key.code {
                KeyCode::Char('d') => {
                    self.move_down(self.page_step());
                    return;
                }
                KeyCode::Char('u') => {
                    self.move_up(self.page_step());
                    return;
                }
                KeyCode::Char('h') if self.focus == FocusPane::Chats && self.mode == Mode::Normal => {
                    self.filter = self.filter.prev();
                    self.selected_chat = 0;
                    return;
                }
                KeyCode::Char('l') if self.focus == FocusPane::Chats && self.mode == Mode::Normal => {
                    self.filter = self.filter.next();
                    self.selected_chat = 0;
                    return;
                }
                KeyCode::Char('r') if self.mode == Mode::Normal => {
                    let _ = self.action_tx.send(TelegramAction::LoadDialogs);
                    self.status = "Refreshing...".into();
                    return;
                }
                _ => {}
            }
        }

        match self.mode {
            Mode::Normal => {
                if self.focus == FocusPane::Compose && self.handle_compose_normal_key(key) {
                    return;
                }
                self.handle_normal_mode(key);
            }
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Command => self.handle_command_mode(key),
            Mode::Search => self.handle_search_mode(key),
            Mode::Visual => self.handle_visual_mode(key),
            Mode::ThemePicker => self.handle_theme_picker_mode(key),
        }
    }

    fn page_step(&self) -> usize {
        10 // half-page; could be dynamic based on terminal height
    }

    // -----------------------------------------------------------------------
    // Help overlay
    // -----------------------------------------------------------------------

    fn handle_help_key(&mut self, key: KeyEvent) {
        if self.help_search_active {
            match key.code {
                KeyCode::Esc => {
                    self.help_search_active = false;
                    self.help_search_input.clear();
                }
                KeyCode::Enter => {
                    self.help_search_active = false;
                }
                KeyCode::Backspace => {
                    self.help_search_input.pop();
                }
                KeyCode::Char(c) => {
                    self.help_search_input.push(c);
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = false;
                self.help_scroll = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.help_scroll = self.help_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.help_scroll = self.help_scroll.saturating_add(10);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.help_scroll = self.help_scroll.saturating_sub(10);
            }
            KeyCode::Char('/') => {
                self.help_search_active = true;
                self.help_search_input.clear();
            }
            KeyCode::Char('n') => {
                self.help_search_match = self.help_search_match.map(|m| m + 1);
            }
            KeyCode::Char('N') => {
                self.help_search_match = self.help_search_match.and_then(|m| m.checked_sub(1));
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Normal mode
    // -----------------------------------------------------------------------

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        // Count prefix accumulation
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_digit() && !(c == '0' && self.pending_count.is_empty()) {
                self.pending_count.push(c);
                return;
            }
        }

        // gg motion
        if self.pending_g {
            self.pending_g = false;
            if key.code == KeyCode::Char('g') {
                match self.focus {
                    FocusPane::Chats => self.selected_chat = 0,
                    FocusPane::Messages => {
                        self.selected_message = 0;
                        self.message_scroll_offset = 0;
                    }
                    FocusPane::Compose => {}
                }
            }
            self.pending_count.clear();
            return;
        }

        match key.code {
            KeyCode::Char('g') => {
                self.pending_g = true;
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::F(1) => {
                self.show_help = true;
                self.help_scroll = 0;
            }
            KeyCode::Char('?') => {
                // ? = reverse search (also opens help if no prior search)
                self.mode = Mode::Search;
                self.search_direction = SearchDirection::Reverse;
                self.search_input.clear();
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.search_direction = SearchDirection::Forward;
                self.search_input.clear();
            }
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                let count = self.take_count();
                self.move_down(count);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let count = self.take_count();
                self.move_up(count);
            }
            KeyCode::Char('G') => {
                match self.focus {
                    FocusPane::Chats => {
                        let len = self.chat_count();
                        if len > 0 {
                            self.selected_chat = len - 1;
                        }
                    }
                    FocusPane::Messages => {
                        let len = self.message_count();
                        if len > 0 {
                            self.selected_message = len - 1;
                        }
                    }
                    FocusPane::Compose => {}
                }
                self.pending_count.clear();
            }
            // Focus cycling
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusPane::Chats => FocusPane::Messages,
                    FocusPane::Messages => FocusPane::Compose,
                    FocusPane::Compose => FocusPane::Chats,
                };
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    FocusPane::Chats => FocusPane::Compose,
                    FocusPane::Messages => FocusPane::Chats,
                    FocusPane::Compose => FocusPane::Messages,
                };
            }
            // Filter tabs (chats pane)
            KeyCode::Char('h') if self.focus == FocusPane::Chats => {
                self.filter = self.filter.prev();
                self.selected_chat = 0;
            }
            KeyCode::Char('l') if self.focus == FocusPane::Chats => {
                self.filter = self.filter.next();
                self.selected_chat = 0;
            }
            // Toggle favorite
            KeyCode::Char('f') => {
                if let Some(id) = self.current_chat_id() {
                    if !self.favorite_chat_ids.remove(&id) {
                        self.favorite_chat_ids.insert(id);
                    }
                }
            }
            // Toggle mute
            KeyCode::Char('M') => {
                if let Some(id) = self.current_chat_id() {
                    if !self.muted_chat_ids.remove(&id) {
                        self.muted_chat_ids.insert(id);
                    }
                }
            }
            // Search repeat
            KeyCode::Char('n') => self.repeat_search(false),
            KeyCode::Char('N') => self.repeat_search(true),
            // Enter insert mode
            KeyCode::Char('i') | KeyCode::Char('I') | KeyCode::Char('a') | KeyCode::Char('A') => {
                self.mode = Mode::Insert;
                self.focus = FocusPane::Compose;
            }
            // Command mode
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_input.clear();
            }
            // Visual mode (messages pane)
            KeyCode::Char('v') if self.focus == FocusPane::Messages && !self.messages.is_empty() => {
                self.mode = Mode::Visual;
                self.visual_anchor = Some(self.selected_message);
                self.visual_line_mode = false;
            }
            KeyCode::Char('V') if self.focus == FocusPane::Messages && !self.messages.is_empty() => {
                self.mode = Mode::Visual;
                self.visual_anchor = Some(self.selected_message);
                self.visual_line_mode = true;
            }
            // Reply
            KeyCode::Char('r') if self.focus == FocusPane::Messages && !self.messages.is_empty() => {
                let msg = &self.messages[self.selected_message];
                self.pending_reply_msg_id = Some(msg.id);
                self.pending_reply_author = Some(msg.sender_name.clone());
                self.mode = Mode::Insert;
                self.focus = FocusPane::Compose;
            }
            // Yank
            KeyCode::Char('y') if self.focus == FocusPane::Messages && !self.messages.is_empty() => {
                let text = self.messages[self.selected_message].text.clone();
                self.yank_register = text;
                self.status = "Yanked message".into();
            }
            // Paste yank into compose
            KeyCode::Char('p') => {
                if !self.yank_register.is_empty() {
                    let reg = self.yank_register.clone();
                    self.compose_push_undo();
                    self.compose_input.insert_str(self.compose_cursor, &reg);
                    self.compose_cursor += reg.len();
                }
            }
            // Edit own message
            KeyCode::Char('e') if self.focus == FocusPane::Messages && !self.messages.is_empty() => {
                let msg = &self.messages[self.selected_message];
                if msg.outgoing {
                    self.pending_edit_msg_id = Some(msg.id);
                    self.compose_input = msg.text.clone();
                    self.compose_cursor = self.compose_input.len();
                    self.mode = Mode::Insert;
                    self.focus = FocusPane::Compose;
                    self.status = "Editing message...".into();
                } else {
                    self.status = "Can only edit own messages".into();
                }
            }
            // Delete own message
            KeyCode::Char('D') if self.focus == FocusPane::Messages && !self.messages.is_empty() => {
                let msg = &self.messages[self.selected_message];
                if msg.outgoing {
                    if let Some(chat_id) = self.loaded_chat_id {
                        let _ = self.action_tx.send(TelegramAction::DeleteMessages {
                            chat_id,
                            message_ids: vec![msg.id],
                        });
                    }
                } else {
                    self.status = "Can only delete own messages".into();
                }
            }
            // Enter = load chat (chats pane) or open message context (messages)
            KeyCode::Enter => {
                match self.focus {
                    FocusPane::Chats => {
                        if let Some(id) = self.current_chat_id() {
                            self.loaded_chat_id = Some(id);
                            self.messages.clear();
                            self.selected_message = 0;
                            let _ = self.action_tx.send(TelegramAction::LoadHistory {
                                chat_id: id,
                                limit: 50,
                            });
                            self.focus = FocusPane::Messages;
                        }
                    }
                    FocusPane::Messages => {
                        // Mark as read (best-effort)
                        if let Some(chat_id) = self.loaded_chat_id {
                            let _ = self.action_tx.send(TelegramAction::MarkRead { chat_id });
                        }
                    }
                    FocusPane::Compose => {}
                }
            }
            _ => {
                self.pending_count.clear();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Insert mode
    // -----------------------------------------------------------------------

    fn handle_insert_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            // Alt+Enter = send message
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.send_compose();
            }
            // Enter = newline in compose
            KeyCode::Enter => {
                self.compose_push_undo();
                self.compose_input.insert(self.compose_cursor, '\n');
                self.compose_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.compose_cursor > 0 {
                    self.compose_push_undo();
                    let prev = self.prev_char_boundary(self.compose_cursor);
                    self.compose_input.drain(prev..self.compose_cursor);
                    self.compose_cursor = prev;
                }
            }
            KeyCode::Left => {
                self.compose_cursor = self.prev_char_boundary(self.compose_cursor);
            }
            KeyCode::Right => {
                self.compose_cursor = self.next_char_boundary(self.compose_cursor);
            }
            KeyCode::Char(c) => {
                self.compose_push_undo();
                self.compose_input.insert(self.compose_cursor, c);
                self.compose_cursor += c.len_utf8();
            }
            KeyCode::Tab => {
                self.compose_push_undo();
                self.compose_input.insert(self.compose_cursor, '\t');
                self.compose_cursor += 1;
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Command mode
    // -----------------------------------------------------------------------

    fn handle_command_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_input.clear();
            }
            KeyCode::Enter => {
                let cmd = input::parse_command(&self.command_input);
                self.execute_command(cmd);
                self.command_input.clear();
                if self.mode == Mode::Command {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Backspace => {
                self.command_input.pop();
                if self.command_input.is_empty() {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Tab => {
                let candidates = input::complete(&self.command_input);
                if candidates.len() == 1 {
                    self.command_input = candidates[0].to_string();
                }
            }
            KeyCode::Char(c) => {
                self.command_input.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => self.should_quit = true,
            Command::Refresh => {
                let _ = self.action_tx.send(TelegramAction::LoadDialogs);
                self.status = "Refreshing...".into();
            }
            Command::Help => {
                self.show_help = true;
                self.help_scroll = 0;
            }
            Command::Theme(arg) => {
                match arg.as_deref() {
                    None | Some("list") => {
                        self.mode = Mode::ThemePicker;
                        self.theme_before_picker = Some(self.config.theme);
                        self.selected_theme = Theme::ALL
                            .iter()
                            .position(|&t| t == self.config.theme)
                            .unwrap_or(0);
                        self.picker_filter.clear();
                    }
                    Some(name) => {
                        if let Some(theme) = Theme::from_name(name) {
                            self.config.theme = theme;
                            self.palette = theme.palette();
                            let _ = self.config.save();
                            self.status = format!("Theme: {name}");
                        } else {
                            self.status = format!("Unknown theme: {name}");
                        }
                    }
                }
            }
            Command::Timestamps => {
                self.config.show_timestamps = !self.config.show_timestamps;
                let _ = self.config.save();
            }
            Command::SpellCheck => {
                self.config.spell_check = !self.config.spell_check;
                let _ = self.config.save();
            }
            Command::ReadAll => {
                for d in &mut self.dialogs {
                    d.unread_count = 0;
                }
                self.status = "All marked as read".into();
            }
            Command::Filter(kind) => {
                self.filter = match kind {
                    FilterKind::All => ConversationFilter::All,
                    FilterKind::Unread => ConversationFilter::Unread,
                    FilterKind::Favorites => ConversationFilter::Favorites,
                    FilterKind::Dms => ConversationFilter::Dms,
                    FilterKind::Groups => ConversationFilter::Groups,
                    FilterKind::Channels => ConversationFilter::Channels,
                };
                self.selected_chat = 0;
            }
            Command::Open(n) => {
                let visible = self.visible_dialogs();
                if n > 0 && n <= visible.len() {
                    self.selected_chat = n - 1;
                    if let Some(id) = self.current_chat_id() {
                        self.loaded_chat_id = Some(id);
                        self.messages.clear();
                        let _ = self.action_tx.send(TelegramAction::LoadHistory {
                            chat_id: id,
                            limit: 50,
                        });
                        self.focus = FocusPane::Messages;
                    }
                }
            }
            Command::Unknown(s) => {
                self.status = format!("Unknown command: {s}");
            }
            // Stubs for features to implement later
            Command::Dm(_) | Command::Export(_) | Command::Attach(_) | Command::Forward(_) => {
                self.status = "Not yet implemented".into();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Search mode
    // -----------------------------------------------------------------------

    fn handle_search_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_input.clear();
                self.search_matches.clear();
                self.search_matches_set.clear();
                self.search_match_cursor = None;
            }
            KeyCode::Enter => {
                if !self.search_input.is_empty() {
                    self.last_search_query = Some(self.search_input.clone());
                    self.last_search_direction = Some(self.search_direction);
                    self.execute_search();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
            }
            _ => {}
        }
    }

    fn execute_search(&mut self) {
        let query = self.search_input.to_lowercase();
        self.search_matches.clear();
        self.search_matches_set.clear();

        match self.focus {
            FocusPane::Messages => {
                for (i, msg) in self.messages.iter().enumerate() {
                    if msg.text.to_lowercase().contains(&query)
                        || msg.sender_name.to_lowercase().contains(&query)
                    {
                        self.search_matches.push(i);
                        self.search_matches_set.insert(i);
                    }
                }
            }
            FocusPane::Chats => {
                // Collect matches first to avoid borrow conflict with visible_dialogs()
                let matches: Vec<usize> = self
                    .visible_dialogs()
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.title.to_lowercase().contains(&query))
                    .map(|(i, _)| i)
                    .collect();
                for i in matches {
                    self.search_matches.push(i);
                    self.search_matches_set.insert(i);
                }
            }
            _ => {}
        }

        // Jump to first match
        if !self.search_matches.is_empty() {
            let start = match self.focus {
                FocusPane::Messages => self.selected_message,
                FocusPane::Chats => self.selected_chat,
                _ => 0,
            };
            let idx = if self.search_direction == SearchDirection::Forward {
                self.search_matches.iter().position(|&m| m >= start).unwrap_or(0)
            } else {
                self.search_matches.iter().rposition(|&m| m <= start).unwrap_or(0)
            };
            self.search_match_cursor = Some(idx);
            let target = self.search_matches[idx];
            match self.focus {
                FocusPane::Messages => self.selected_message = target,
                FocusPane::Chats => self.selected_chat = target,
                _ => {}
            }
            self.status = format!("[{}/{}]", idx + 1, self.search_matches.len());
        } else {
            self.status = "No matches".into();
            self.search_match_cursor = None;
        }
    }

    fn repeat_search(&mut self, reverse: bool) {
        if let Some(query) = self.last_search_query.clone() {
            self.search_input = query;
            if reverse {
                self.search_direction = match self.last_search_direction.unwrap_or(SearchDirection::Forward) {
                    SearchDirection::Forward => SearchDirection::Reverse,
                    SearchDirection::Reverse => SearchDirection::Forward,
                };
            } else {
                self.search_direction = self.last_search_direction.unwrap_or(SearchDirection::Forward);
            }
            // Advance cursor before searching
            match self.focus {
                FocusPane::Messages => {
                    if self.search_direction == SearchDirection::Forward {
                        self.selected_message = self.selected_message.saturating_add(1)
                            .min(self.messages.len().saturating_sub(1));
                    } else {
                        self.selected_message = self.selected_message.saturating_sub(1);
                    }
                }
                FocusPane::Chats => {
                    if self.search_direction == SearchDirection::Forward {
                        self.selected_chat = (self.selected_chat + 1).min(self.chat_count().saturating_sub(1));
                    } else {
                        self.selected_chat = self.selected_chat.saturating_sub(1);
                    }
                }
                _ => {}
            }
            self.execute_search();
        }
    }

    // -----------------------------------------------------------------------
    // Visual mode (messages)
    // -----------------------------------------------------------------------

    fn handle_visual_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.visual_anchor = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected_message + 1 < self.messages.len() {
                    self.selected_message += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_message = self.selected_message.saturating_sub(1);
            }
            KeyCode::Char('G') => {
                if !self.messages.is_empty() {
                    self.selected_message = self.messages.len() - 1;
                }
            }
            KeyCode::Char('y') => {
                // Yank visual selection
                if let Some(anchor) = self.visual_anchor {
                    let start = anchor.min(self.selected_message);
                    let end = anchor.max(self.selected_message);
                    let text: String = self.messages[start..=end]
                        .iter()
                        .map(|m| m.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.yank_register = text;
                    self.status = format!("Yanked {} messages", end - start + 1);
                }
                self.mode = Mode::Normal;
                self.visual_anchor = None;
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Compose pane normal mode
    // -----------------------------------------------------------------------

    fn handle_compose_normal_key(&mut self, key: KeyEvent) -> bool {
        // Handle pending delete (dd, dw)
        if self.compose_pending_delete {
            self.compose_pending_delete = false;
            match key.code {
                KeyCode::Char('d') => {
                    // dd — delete current line
                    self.compose_push_undo();
                    let (line_start, line_end) = self.compose_current_line_range();
                    self.compose_input.drain(line_start..line_end);
                    self.compose_cursor = line_start.min(self.compose_input.len());
                    return true;
                }
                KeyCode::Char('w') => {
                    // dw — delete next word
                    self.compose_push_undo();
                    let end = self.compose_next_word_boundary();
                    self.compose_input.drain(self.compose_cursor..end);
                    return true;
                }
                _ => return true,
            }
        }

        match key.code {
            KeyCode::Esc => {
                // Just clear any pending state
                return false; // fall through to normal mode
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.compose_cursor = self.prev_char_boundary(self.compose_cursor);
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.compose_cursor = self.next_char_boundary(self.compose_cursor);
                true
            }
            KeyCode::Char('w') => {
                self.compose_cursor = self.compose_next_word_boundary();
                true
            }
            KeyCode::Char('b') => {
                self.compose_cursor = self.compose_prev_word_boundary();
                true
            }
            KeyCode::Char('0') if self.pending_count.is_empty() => {
                let (line_start, _) = self.compose_current_line_range();
                self.compose_cursor = line_start;
                true
            }
            KeyCode::Char('$') => {
                let (_, line_end) = self.compose_current_line_range();
                // Position before newline
                let end = if line_end > 0
                    && self.compose_input.as_bytes().get(line_end - 1) == Some(&b'\n')
                {
                    line_end - 1
                } else {
                    line_end
                };
                self.compose_cursor = end;
                true
            }
            KeyCode::Char('d') => {
                self.compose_pending_delete = true;
                true
            }
            KeyCode::Char('u') => {
                self.compose_pop_undo();
                true
            }
            KeyCode::Char('U') => {
                self.compose_pop_redo();
                true
            }
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                true
            }
            KeyCode::Char('I') => {
                let (line_start, _) = self.compose_current_line_range();
                self.compose_cursor = line_start;
                self.mode = Mode::Insert;
                true
            }
            KeyCode::Char('a') => {
                self.compose_cursor = self.next_char_boundary(self.compose_cursor);
                self.mode = Mode::Insert;
                true
            }
            KeyCode::Char('A') => {
                let (_, line_end) = self.compose_current_line_range();
                let end = if line_end > 0
                    && self.compose_input.as_bytes().get(line_end - 1) == Some(&b'\n')
                {
                    line_end - 1
                } else {
                    line_end
                };
                self.compose_cursor = end;
                self.mode = Mode::Insert;
                true
            }
            // Send with Alt+Enter
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.send_compose();
                true
            }
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // Theme picker mode
    // -----------------------------------------------------------------------

    fn handle_theme_picker_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if let Some(orig) = self.theme_before_picker.take() {
                    self.config.theme = orig;
                    self.palette = orig.palette();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let filtered = self.filtered_themes();
                if !filtered.is_empty() {
                    self.selected_theme = (self.selected_theme + 1) % filtered.len();
                    let theme = filtered[self.selected_theme];
                    self.palette = theme.palette();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let filtered = self.filtered_themes();
                if !filtered.is_empty() {
                    self.selected_theme = if self.selected_theme == 0 {
                        filtered.len() - 1
                    } else {
                        self.selected_theme - 1
                    };
                    let theme = filtered[self.selected_theme];
                    self.palette = theme.palette();
                }
            }
            KeyCode::Enter => {
                let filtered = self.filtered_themes();
                if let Some(&theme) = filtered.get(self.selected_theme) {
                    self.config.theme = theme;
                    self.palette = theme.palette();
                    let _ = self.config.save();
                    self.status = format!("Theme: {}", theme.name());
                }
                self.theme_before_picker = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.picker_filter.pop();
                self.selected_theme = 0;
            }
            KeyCode::Char(c) if c != 'j' && c != 'k' && c != 'q' => {
                self.picker_filter.push(c);
                self.selected_theme = 0;
            }
            _ => {}
        }
    }

    pub fn filtered_themes(&self) -> Vec<Theme> {
        if self.picker_filter.is_empty() {
            Theme::ALL.to_vec()
        } else {
            let f = self.picker_filter.to_lowercase();
            Theme::ALL
                .iter()
                .filter(|t| t.name().contains(&f))
                .copied()
                .collect()
        }
    }

    // -----------------------------------------------------------------------
    // Movement helpers
    // -----------------------------------------------------------------------

    fn move_down(&mut self, count: usize) {
        match self.focus {
            FocusPane::Chats => {
                let max = self.chat_count().saturating_sub(1);
                self.selected_chat = (self.selected_chat + count).min(max);
            }
            FocusPane::Messages => {
                let max = self.message_count().saturating_sub(1);
                self.selected_message = (self.selected_message + count).min(max);
            }
            FocusPane::Compose => {}
        }
    }

    fn move_up(&mut self, count: usize) {
        match self.focus {
            FocusPane::Chats => {
                self.selected_chat = self.selected_chat.saturating_sub(count);
            }
            FocusPane::Messages => {
                self.selected_message = self.selected_message.saturating_sub(count);
            }
            FocusPane::Compose => {}
        }
    }

    // -----------------------------------------------------------------------
    // Compose helpers
    // -----------------------------------------------------------------------

    fn send_compose(&mut self) {
        let text = self.compose_input.trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(chat_id) = self.loaded_chat_id {
            if let Some(edit_id) = self.pending_edit_msg_id.take() {
                let _ = self.action_tx.send(TelegramAction::EditMessage {
                    chat_id,
                    message_id: edit_id,
                    text,
                });
                self.compose_input.clear();
                self.compose_cursor = 0;
                self.mode = Mode::Normal;
            } else {
                let _ = self.action_tx.send(TelegramAction::SendMessage {
                    chat_id,
                    text,
                    reply_to: self.pending_reply_msg_id.take(),
                });
                self.pending_reply_author = None;
                // compose_input cleared on MessageSent event
            }
        }
        self.mode = Mode::Normal;
    }

    fn compose_push_undo(&mut self) {
        self.compose_undo_stack
            .push_back((self.compose_input.clone(), self.compose_cursor));
        if self.compose_undo_stack.len() > 100 {
            self.compose_undo_stack.pop_front();
        }
        self.compose_redo_stack.clear();
    }

    fn compose_pop_undo(&mut self) {
        if let Some((text, cursor)) = self.compose_undo_stack.pop_back() {
            self.compose_redo_stack
                .push_back((self.compose_input.clone(), self.compose_cursor));
            self.compose_input = text;
            self.compose_cursor = cursor;
        }
    }

    fn compose_pop_redo(&mut self) {
        if let Some((text, cursor)) = self.compose_redo_stack.pop_back() {
            self.compose_undo_stack
                .push_back((self.compose_input.clone(), self.compose_cursor));
            self.compose_input = text;
            self.compose_cursor = cursor;
        }
    }

    fn prev_char_boundary(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let mut p = pos - 1;
        while p > 0 && !self.compose_input.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    fn next_char_boundary(&self, pos: usize) -> usize {
        if pos >= self.compose_input.len() {
            return self.compose_input.len();
        }
        let mut p = pos + 1;
        while p < self.compose_input.len() && !self.compose_input.is_char_boundary(p) {
            p += 1;
        }
        p
    }

    fn compose_current_line_range(&self) -> (usize, usize) {
        let bytes = self.compose_input.as_bytes();
        let start = bytes[..self.compose_cursor]
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        let end = bytes[self.compose_cursor..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|p| self.compose_cursor + p + 1)
            .unwrap_or(self.compose_input.len());
        (start, end)
    }

    fn compose_next_word_boundary(&self) -> usize {
        let bytes = self.compose_input.as_bytes();
        let mut p = self.compose_cursor;
        // Skip current word chars
        while p < bytes.len() && !bytes[p].is_ascii_whitespace() {
            p += 1;
        }
        // Skip whitespace
        while p < bytes.len() && bytes[p].is_ascii_whitespace() {
            p += 1;
        }
        p
    }

    fn compose_prev_word_boundary(&self) -> usize {
        let bytes = self.compose_input.as_bytes();
        let mut p = self.compose_cursor;
        if p > 0 {
            p -= 1;
        }
        // Skip whitespace backwards
        while p > 0 && bytes[p].is_ascii_whitespace() {
            p -= 1;
        }
        // Skip word chars backwards
        while p > 0 && !bytes[p - 1].is_ascii_whitespace() {
            p -= 1;
        }
        p
    }
}
