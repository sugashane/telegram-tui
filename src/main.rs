mod app;
mod config;
mod input;
mod telegram;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{Event, EventStream};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::config::{config_dir, session_path, AppConfig};
use crate::telegram::{TelegramAction, TelegramEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load();

    // ── Check for API credentials ──────────────────────────────────────
    let (api_id, api_hash) = match (config.api_id, config.api_hash.as_deref()) {
        (Some(id), Some(hash)) if !hash.is_empty() => (id, hash.to_string()),
        _ => {
            eprintln!("telegram-tui requires Telegram API credentials.");
            eprintln!();
            eprintln!("This is a one-time developer setup:");
            eprintln!("  1. Go to https://my.telegram.org and log in");
            eprintln!("  2. Navigate to 'API development tools'");
            eprintln!("  3. Create an application to get your api_id and api_hash");
            eprintln!("  4. Add them to your config file:");
            eprintln!();
            let cfg_path = config::config_path();
            eprintln!("   {}", cfg_path.display());
            eprintln!();
            eprintln!("   api_id = 12345");
            eprintln!("   api_hash = \"your_api_hash_here\"");
            eprintln!();
            eprintln!("After this, login is just phone number + code from Telegram.");

            // Create config dir and default config if it doesn't exist
            let dir = config_dir();
            std::fs::create_dir_all(&dir)?;
            if !cfg_path.exists() {
                let default_config = AppConfig::default();
                default_config.save()?;
                eprintln!();
                eprintln!("   (Created default config file for you to edit)");
            }

            std::process::exit(1);
        }
    };

    // ── Connect to Telegram ────────────────────────────────────────────
    let sess_path = session_path();
    let (client, is_authorized, action_tx, mut event_rx) =
        telegram::start(api_id, api_hash, &sess_path)
            .await
            .context("Failed to connect to Telegram")?;

    // ── Set up terminal ────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ── Run TUI event loop ─────────────────────────────────────────────
    // If not authorized, App starts on the login screen automatically
    let mut app = App::new(action_tx.clone(), is_authorized);
    let result = run_app(&mut terminal, &mut app, &mut event_rx).await;

    // ── Cleanup ────────────────────────────────────────────────────────
    app.persist_state();

    // Signal shutdown to background task
    let _ = action_tx.send(TelegramAction::Shutdown);

    // Disconnect gracefully
    client.disconnect();

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        cursor::Show
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<TelegramEvent>,
) -> Result<()> {
    let mut event_stream = EventStream::new();
    let mut persist_interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        // Draw
        terminal.draw(|frame| ui::render(frame, app))?;

        // Set cursor style based on mode/screen
        let cursor_style = match app.screen {
            app::Screen::Login => cursor::SetCursorStyle::BlinkingBar,
            app::Screen::Main => match app.mode {
                app::Mode::Insert => cursor::SetCursorStyle::BlinkingBar,
                app::Mode::Normal => cursor::SetCursorStyle::SteadyBlock,
                _ => cursor::SetCursorStyle::SteadyBlock,
            },
        };
        execute!(terminal.backend_mut(), cursor_style)?;

        if app.should_quit {
            break;
        }

        // Multiplex events
        tokio::select! {
            // Terminal events (keyboard, resize)
            event = event_stream.next() => {
                match event {
                    Some(Ok(Event::Key(key))) => {
                        app.on_key(key);
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        // Terminal will redraw on next loop
                    }
                    Some(Err(_)) | None => break,
                    _ => {}
                }
            }
            // Telegram events
            event = event_rx.recv() => {
                match event {
                    Some(tg_event) => app.handle_telegram_event(tg_event),
                    None => break, // channel closed
                }
            }
            // Periodic state persistence
            _ = persist_interval.tick() => {
                app.persist_state();
            }
        }
    }

    Ok(())
}
