use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Tabs, Wrap,
};
use ratatui::Frame;

use crate::app::{App, ConversationFilter, FocusPane, LoginPhase, Mode, Screen};
use crate::config::ThemePalette;
use crate::telegram::DialogKind;

// ---------------------------------------------------------------------------
// Main render entry point
// ---------------------------------------------------------------------------

pub fn render(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Login => render_login(frame, app),
        Screen::Main => render_main(frame, app),
    }
}

fn render_main(frame: &mut Frame, app: &App) {
    let pal = &app.palette;
    let size = frame.area();

    // Top-level horizontal split: chat list (30%) | right panel (70%)
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(size);

    // Right panel: messages | compose (5 lines) | status (1 line)
    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(h_chunks[1]);

    render_chat_list(frame, app, pal, h_chunks[0]);
    render_messages(frame, app, pal, v_chunks[0]);
    render_compose(frame, app, pal, v_chunks[1]);
    render_status_bar(frame, app, pal, v_chunks[2]);

    // Overlays
    if app.show_help {
        render_help_overlay(frame, app, pal, size);
    }
    if app.mode == Mode::ThemePicker {
        render_theme_picker(frame, app, pal, size);
    }
}

// ---------------------------------------------------------------------------
// Login screen
// ---------------------------------------------------------------------------

fn render_login(frame: &mut Frame, app: &App) {
    let pal = &app.palette;
    let size = frame.area();

    // Clear background
    frame.render_widget(Clear, size);
    let bg = Block::default().style(Style::default().bg(pal.bg));
    frame.render_widget(bg, size);

    // Center a box on screen — 50 wide, 16 tall
    let box_w = 50u16.min(size.width.saturating_sub(4));
    let box_h = 16u16.min(size.height.saturating_sub(2));
    let x = (size.width.saturating_sub(box_w)) / 2;
    let y = (size.height.saturating_sub(box_h)) / 2;
    let area = Rect::new(x, y, box_w, box_h);

    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(pal.border_focus))
        .title(" telegram-tui ")
        .title_alignment(Alignment::Center);
    frame.render_widget(border, area);

    let inner = Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), area.height.saturating_sub(2));

    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        "Welcome to telegram-tui",
        Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    match app.login_phase {
        LoginPhase::EnteringPhone => {
            lines.push(Line::from(Span::styled(
                "Enter your phone number (with country code):",
                Style::default().fg(pal.fg),
            )));
            lines.push(Line::from(""));

            // Phone input with cursor
            let phone_display = render_input_with_cursor(
                &app.login_phone, app.login_cursor, pal, false,
            );
            lines.push(phone_display);

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Example: +15551234567",
                Style::default().fg(pal.muted),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Enter", Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" to continue  ", Style::default().fg(pal.fg)),
                Span::styled("Esc", Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" to quit", Style::default().fg(pal.fg)),
            ]));
        }
        LoginPhase::WaitingForCode => {
            lines.push(Line::from(Span::styled(
                "Requesting login code...",
                Style::default().fg(pal.accent),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "A code will be sent to your Telegram app.",
                Style::default().fg(pal.fg),
            )));
        }
        LoginPhase::EnteringCode => {
            lines.push(Line::from(Span::styled(
                "Enter the code from your Telegram app:",
                Style::default().fg(pal.fg),
            )));
            lines.push(Line::from(""));

            let code_display = render_input_with_cursor(
                &app.login_code, app.login_cursor, pal, false,
            );
            lines.push(code_display);

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Check your other Telegram app for the code.",
                Style::default().fg(pal.muted),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Enter", Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" to submit  ", Style::default().fg(pal.fg)),
                Span::styled("Esc", Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" to go back", Style::default().fg(pal.fg)),
            ]));
        }
        LoginPhase::EnteringPassword => {
            lines.push(Line::from(Span::styled(
                "Two-factor authentication required.",
                Style::default().fg(pal.accent),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Enter your 2FA password:",
                Style::default().fg(pal.fg),
            )));
            lines.push(Line::from(""));

            // Show password as dots
            let pw_display = render_input_with_cursor(
                &app.login_password, app.login_cursor, pal, true,
            );
            lines.push(pw_display);

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Enter", Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" to submit  ", Style::default().fg(pal.fg)),
                Span::styled("Esc", Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" to go back", Style::default().fg(pal.fg)),
            ]));
        }
        LoginPhase::WaitingForAuth => {
            lines.push(Line::from(Span::styled(
                "Signing in...",
                Style::default().fg(pal.accent),
            )));
        }
    }

    // Error display
    if let Some(err) = &app.login_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            err.as_str(),
            Style::default().fg(pal.error),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Render an input field with a block cursor at the given position.
fn render_input_with_cursor<'a>(
    text: &str,
    cursor: usize,
    pal: &ThemePalette,
    mask: bool,
) -> Line<'a> {
    let display: String = if mask {
        "*".repeat(text.len())
    } else {
        text.to_string()
    };

    let (before, cursor_char, after) = if cursor < display.len() {
        let before = display[..cursor].to_string();
        let c = display[cursor..].chars().next().unwrap_or(' ');
        let after = if cursor + c.len_utf8() < display.len() {
            display[cursor + c.len_utf8()..].to_string()
        } else {
            String::new()
        };
        (before, c.to_string(), after)
    } else {
        (display.clone(), " ".to_string(), String::new())
    };

    Line::from(vec![
        Span::styled(before, Style::default().fg(pal.fg)),
        Span::styled(
            cursor_char,
            Style::default().fg(pal.bg).bg(pal.fg),
        ),
        Span::styled(after, Style::default().fg(pal.fg)),
    ])
}

// ---------------------------------------------------------------------------
// Chat list (left pane)
// ---------------------------------------------------------------------------

fn render_chat_list(frame: &mut Frame, app: &App, pal: &ThemePalette, area: Rect) {
    let focused = app.focus == FocusPane::Chats;
    let border_color = if focused { pal.border_focus } else { pal.border };

    // Split: filter tabs at bottom (1 line) + list above
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    // Filter tabs
    let tab_titles: Vec<Span> = ConversationFilter::ALL_FILTERS
        .iter()
        .map(|f| {
            let style = if *f == app.filter {
                Style::default().fg(pal.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(pal.muted)
            };
            Span::styled(f.label(), style)
        })
        .collect();
    let tabs = Tabs::new(tab_titles)
        .style(Style::default().fg(pal.muted))
        .highlight_style(Style::default().fg(pal.accent))
        .select(app.filter.index())
        .divider(Span::raw(" | "));
    frame.render_widget(tabs, chunks[1]);

    // Dialog list
    let visible = app.visible_dialogs();
    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, dialog)| {
            let is_fav = app.favorite_chat_ids.contains(&dialog.id);
            let is_muted = app.muted_chat_ids.contains(&dialog.id);

            let prefix = match dialog.kind {
                DialogKind::User => "",
                DialogKind::Group => "#",
                DialogKind::Channel => ">>",
            };

            let star = if is_fav { "*" } else { "" };
            let mute = if is_muted { " [M]" } else { "" };
            let unread = if dialog.unread_count > 0 {
                format!(" ({})", dialog.unread_count)
            } else {
                String::new()
            };

            let title_style = if dialog.unread_count > 0 {
                Style::default().fg(pal.unread).add_modifier(Modifier::BOLD)
            } else if i == app.selected_chat && focused {
                Style::default().fg(pal.selection_fg)
            } else {
                Style::default().fg(pal.fg)
            };

            let line = Line::from(vec![
                Span::styled(star, Style::default().fg(pal.accent)),
                Span::styled(
                    format!("{prefix}{}", dialog.title),
                    title_style,
                ),
                Span::styled(unread, Style::default().fg(pal.unread)),
                Span::styled(mute, Style::default().fg(pal.muted)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            " Chats ",
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        ));

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(pal.selection_bg)
                .fg(pal.selection_fg),
        );

    let mut list_state = ListState::default();
    if focused || app.loaded_chat_id.is_some() {
        list_state.select(Some(app.selected_chat));
    }
    frame.render_stateful_widget(list, chunks[0], &mut list_state);
}

// ---------------------------------------------------------------------------
// Messages pane
// ---------------------------------------------------------------------------

fn render_messages(frame: &mut Frame, app: &App, pal: &ThemePalette, area: Rect) {
    let focused = app.focus == FocusPane::Messages;
    let border_color = if focused { pal.border_focus } else { pal.border };

    let title = if let Some(chat_id) = app.loaded_chat_id {
        app.dialogs
            .iter()
            .find(|d| d.id == chat_id)
            .map(|d| format!(" {} ", d.title))
            .unwrap_or_else(|| " Messages ".into())
    } else {
        " Messages ".into()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        ));

    if app.messages.is_empty() {
        let empty = Paragraph::new(if app.loaded_chat_id.is_some() {
            "No messages yet."
        } else {
            "Select a chat to view messages. Press Enter on a chat."
        })
        .style(Style::default().fg(pal.muted))
        .block(block);
        frame.render_widget(empty, area);
        return;
    }

    // Build message lines
    let inner = block.inner(area);
    let mut lines: Vec<Line> = Vec::new();
    let mut msg_line_starts: Vec<usize> = Vec::new(); // index into `lines` for each message

    for (i, msg) in app.messages.iter().enumerate() {
        msg_line_starts.push(lines.len());

        let is_selected = focused && i == app.selected_message;
        let in_visual = if app.mode == Mode::Visual {
            if let Some(anchor) = app.visual_anchor {
                let lo = anchor.min(app.selected_message);
                let hi = anchor.max(app.selected_message);
                i >= lo && i <= hi
            } else {
                false
            }
        } else {
            false
        };

        let is_search_match = app.search_matches_set.contains(&i);

        // Author header
        let author_color = pal.authors[i % pal.authors.len()];
        let timestamp = if app.config.show_timestamps {
            format!(
                " [{}]",
                msg.date.format("%H:%M")
            )
        } else {
            String::new()
        };
        let edited = if msg.edit_date.is_some() { " (edited)" } else { "" };
        let reply_indicator = if msg.reply_to_msg_id.is_some() { " -> reply" } else { "" };

        let mut header_spans = vec![
            Span::styled(
                &msg.sender_name,
                Style::default().fg(author_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(timestamp, Style::default().fg(pal.muted)),
            Span::styled(edited, Style::default().fg(pal.muted)),
            Span::styled(reply_indicator, Style::default().fg(pal.muted)),
        ];

        if msg.outgoing {
            header_spans.push(Span::styled(" (you)", Style::default().fg(pal.muted)));
        }

        lines.push(Line::from(header_spans));

        // Message body
        let body_style = if is_selected && !in_visual {
            Style::default().fg(pal.selection_fg).bg(pal.selection_bg)
        } else if in_visual {
            Style::default().fg(pal.selection_fg).bg(pal.mode_visual)
        } else if is_search_match {
            Style::default().fg(pal.fg).bg(pal.accent)
        } else {
            Style::default().fg(pal.fg)
        };

        for text_line in msg.text.lines() {
            lines.push(Line::styled(text_line.to_string(), body_style));
        }
        if msg.text.is_empty() {
            lines.push(Line::styled("[no text]", Style::default().fg(pal.muted)));
        }

        // Blank separator
        lines.push(Line::raw(""));
    }

    // Scroll calculation — keep selected message visible
    let visible_height = inner.height as usize;
    let selected_line_start = msg_line_starts
        .get(app.selected_message)
        .copied()
        .unwrap_or(0);
    let selected_line_end = msg_line_starts
        .get(app.selected_message + 1)
        .copied()
        .unwrap_or(lines.len());

    let scroll = {
        let mut s = app.message_scroll_offset;
        // Ensure selected message is visible
        if selected_line_start < s {
            s = selected_line_start;
        }
        if selected_line_end > s + visible_height {
            s = selected_line_end.saturating_sub(visible_height);
        }
        s
    };

    let para = Paragraph::new(lines.clone())
        .block(block)
        .scroll((scroll as u16, 0));
    frame.render_widget(para, area);

    // Scrollbar
    if lines.len() > visible_height {
        let mut scrollbar_state = ScrollbarState::new(lines.len())
            .position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

// ---------------------------------------------------------------------------
// Compose pane
// ---------------------------------------------------------------------------

fn render_compose(frame: &mut Frame, app: &App, pal: &ThemePalette, area: Rect) {
    let focused = app.focus == FocusPane::Compose;
    let border_color = if focused { pal.border_focus } else { pal.border };

    let title = if app.pending_edit_msg_id.is_some() {
        " Edit Message "
    } else if let Some(author) = &app.pending_reply_author {
        &format!(" Reply to {} ", author)
    } else {
        " Compose (Alt+Enter to send) "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(pal.accent),
        ));

    let compose_style = if focused {
        Style::default().fg(pal.fg)
    } else {
        Style::default().fg(pal.muted)
    };

    let text = if app.compose_input.is_empty() && !focused {
        "Press i to start typing..."
    } else {
        &app.compose_input
    };

    let para = Paragraph::new(text.to_string())
        .style(compose_style)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);

    // Show cursor in insert mode
    if focused && app.mode == Mode::Insert {
        let inner = Block::default()
            .borders(Borders::ALL)
            .inner(area);
        let (cx, cy) = cursor_position(&app.compose_input, app.compose_cursor, inner.width as usize);
        let x = inner.x + cx as u16;
        let y = inner.y + cy as u16;
        if y < inner.y + inner.height {
            frame.set_cursor_position((x, y));
        }
    }
}

fn cursor_position(text: &str, cursor: usize, width: usize) -> (usize, usize) {
    let before = &text[..cursor.min(text.len())];
    let mut x = 0usize;
    let mut y = 0usize;
    let w = if width == 0 { 80 } else { width };

    for ch in before.chars() {
        if ch == '\n' {
            x = 0;
            y += 1;
        } else {
            x += 1;
            if x >= w {
                x = 0;
                y += 1;
            }
        }
    }
    (x, y)
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

fn render_status_bar(frame: &mut Frame, app: &App, pal: &ThemePalette, area: Rect) {
    let mode_color = match app.mode {
        Mode::Normal => pal.mode_normal,
        Mode::Insert => pal.mode_insert,
        Mode::Command => pal.mode_command,
        Mode::Search => pal.mode_search,
        Mode::Visual => pal.mode_visual,
        Mode::ThemePicker => pal.mode_command,
    };

    let mode_span = Span::styled(
        format!(" {} ", app.mode.label()),
        Style::default()
            .fg(pal.bg)
            .bg(mode_color)
            .add_modifier(Modifier::BOLD),
    );

    let connection = if app.connected {
        Span::styled(" [OK] ", Style::default().fg(pal.unread))
    } else {
        Span::styled(" [--] ", Style::default().fg(pal.error))
    };

    let middle = match app.mode {
        Mode::Command => {
            Span::styled(
                format!(":{}", app.command_input),
                Style::default().fg(pal.fg),
            )
        }
        Mode::Search => {
            let prefix = if app.search_direction == crate::app::SearchDirection::Forward {
                "/"
            } else {
                "?"
            };
            Span::styled(
                format!("{prefix}{}", app.search_input),
                Style::default().fg(pal.fg),
            )
        }
        _ => Span::styled(&app.status, Style::default().fg(pal.muted)),
    };

    let right_info = format!(
        " {}:{} ",
        match app.focus {
            FocusPane::Chats => "chats",
            FocusPane::Messages => "msgs",
            FocusPane::Compose => "compose",
        },
        match app.focus {
            FocusPane::Chats => format!("{}/{}", app.selected_chat + 1, app.visible_dialogs().len()),
            FocusPane::Messages => {
                if app.messages.is_empty() {
                    "0/0".into()
                } else {
                    format!("{}/{}", app.selected_message + 1, app.messages.len())
                }
            }
            FocusPane::Compose => format!("{}c", app.compose_input.len()),
        }
    );
    let right_span = Span::styled(right_info, Style::default().fg(pal.muted));

    let bar = Line::from(vec![mode_span, Span::raw(" "), connection, middle, Span::raw(" "), right_span]);
    let para = Paragraph::new(bar).style(Style::default().bg(pal.bg));
    frame.render_widget(para, area);
}

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

fn render_help_overlay(frame: &mut Frame, app: &App, pal: &ThemePalette, area: Rect) {
    let popup = centered_rect(70, 80, area);
    frame.render_widget(Clear, popup);

    let help_lines = help_text(pal);
    let title = if app.help_search_active {
        format!(" Help — Search: {} ", app.help_search_input)
    } else {
        " Help (q/Esc to close, / to search) ".into()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(pal.accent))
        .title(Span::styled(
            title,
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        ));

    let para = Paragraph::new(help_lines)
        .block(block)
        .scroll((app.help_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, popup);
}

fn help_text<'a>(pal: &ThemePalette) -> Vec<Line<'a>> {
    let h = Style::default().fg(pal.accent).add_modifier(Modifier::BOLD);
    let k = Style::default().fg(pal.unread).add_modifier(Modifier::BOLD);
    let d = Style::default().fg(pal.fg);

    vec![
        Line::styled("telegram-tui — Vim-style Telegram Client", h),
        Line::raw(""),
        Line::styled("== Navigation ==", h),
        Line::from(vec![Span::styled("  j / Down   ", k), Span::styled("Move down", d)]),
        Line::from(vec![Span::styled("  k / Up     ", k), Span::styled("Move up", d)]),
        Line::from(vec![Span::styled("  gg         ", k), Span::styled("Go to top", d)]),
        Line::from(vec![Span::styled("  G          ", k), Span::styled("Go to bottom", d)]),
        Line::from(vec![Span::styled("  Ctrl+d     ", k), Span::styled("Half-page down", d)]),
        Line::from(vec![Span::styled("  Ctrl+u     ", k), Span::styled("Half-page up", d)]),
        Line::from(vec![Span::styled("  Tab        ", k), Span::styled("Cycle focus: Chats → Messages → Compose", d)]),
        Line::from(vec![Span::styled("  Shift+Tab  ", k), Span::styled("Cycle focus backwards", d)]),
        Line::from(vec![Span::styled("  h / l      ", k), Span::styled("Previous / next filter tab (Chats pane)", d)]),
        Line::from(vec![Span::styled("  Enter      ", k), Span::styled("Open chat / mark message read", d)]),
        Line::raw(""),
        Line::styled("== Modes ==", h),
        Line::from(vec![Span::styled("  i/I/a/A    ", k), Span::styled("Enter Insert mode (compose)", d)]),
        Line::from(vec![Span::styled("  Esc        ", k), Span::styled("Return to Normal mode", d)]),
        Line::from(vec![Span::styled("  :          ", k), Span::styled("Enter Command mode", d)]),
        Line::from(vec![Span::styled("  /          ", k), Span::styled("Search forward", d)]),
        Line::from(vec![Span::styled("  ?          ", k), Span::styled("Search backward", d)]),
        Line::from(vec![Span::styled("  v / V      ", k), Span::styled("Visual / Visual-Line mode (messages)", d)]),
        Line::raw(""),
        Line::styled("== Message Actions ==", h),
        Line::from(vec![Span::styled("  y          ", k), Span::styled("Yank (copy) message text", d)]),
        Line::from(vec![Span::styled("  p          ", k), Span::styled("Paste yank register into compose", d)]),
        Line::from(vec![Span::styled("  r          ", k), Span::styled("Reply to selected message", d)]),
        Line::from(vec![Span::styled("  e          ", k), Span::styled("Edit own message", d)]),
        Line::from(vec![Span::styled("  D          ", k), Span::styled("Delete own message", d)]),
        Line::from(vec![Span::styled("  f          ", k), Span::styled("Toggle favorite on chat", d)]),
        Line::from(vec![Span::styled("  M          ", k), Span::styled("Toggle mute on chat", d)]),
        Line::raw(""),
        Line::styled("== Compose ==", h),
        Line::from(vec![Span::styled("  Alt+Enter  ", k), Span::styled("Send message", d)]),
        Line::from(vec![Span::styled("  Enter      ", k), Span::styled("New line (Insert mode)", d)]),
        Line::from(vec![Span::styled("  dd         ", k), Span::styled("Delete line (Normal mode in compose)", d)]),
        Line::from(vec![Span::styled("  dw         ", k), Span::styled("Delete word (Normal mode in compose)", d)]),
        Line::from(vec![Span::styled("  u / U      ", k), Span::styled("Undo / Redo (Normal mode in compose)", d)]),
        Line::from(vec![Span::styled("  w / b      ", k), Span::styled("Next / previous word", d)]),
        Line::from(vec![Span::styled("  0 / $      ", k), Span::styled("Start / end of line", d)]),
        Line::raw(""),
        Line::styled("== Commands ==", h),
        Line::from(vec![Span::styled("  :q         ", k), Span::styled("Quit", d)]),
        Line::from(vec![Span::styled("  :r         ", k), Span::styled("Refresh", d)]),
        Line::from(vec![Span::styled("  :theme     ", k), Span::styled("Open theme picker", d)]),
        Line::from(vec![Span::styled("  :help      ", k), Span::styled("This help", d)]),
        Line::from(vec![Span::styled("  :all       ", k), Span::styled("Show all chats", d)]),
        Line::from(vec![Span::styled("  :unread    ", k), Span::styled("Filter unread", d)]),
        Line::from(vec![Span::styled("  :fav       ", k), Span::styled("Filter favorites", d)]),
        Line::from(vec![Span::styled("  :dms       ", k), Span::styled("Filter DMs", d)]),
        Line::from(vec![Span::styled("  :groups    ", k), Span::styled("Filter groups", d)]),
        Line::from(vec![Span::styled("  :channels  ", k), Span::styled("Filter channels", d)]),
        Line::from(vec![Span::styled("  :readall   ", k), Span::styled("Mark all as read", d)]),
        Line::from(vec![Span::styled("  :ts        ", k), Span::styled("Toggle timestamps", d)]),
        Line::from(vec![Span::styled("  :open N    ", k), Span::styled("Open chat #N", d)]),
        Line::raw(""),
        Line::styled("== Other ==", h),
        Line::from(vec![Span::styled("  Ctrl+c     ", k), Span::styled("Quit", d)]),
        Line::from(vec![Span::styled("  Ctrl+r     ", k), Span::styled("Refresh", d)]),
        Line::from(vec![Span::styled("  Ctrl+h/l   ", k), Span::styled("Previous / next filter tab", d)]),
        Line::from(vec![Span::styled("  n / N      ", k), Span::styled("Repeat search / reverse", d)]),
        Line::from(vec![Span::styled("  F1         ", k), Span::styled("Open this help", d)]),
    ]
}

// ---------------------------------------------------------------------------
// Theme picker overlay
// ---------------------------------------------------------------------------

fn render_theme_picker(frame: &mut Frame, app: &App, pal: &ThemePalette, area: Rect) {
    let popup = centered_rect(40, 60, area);
    frame.render_widget(Clear, popup);

    let filtered = app.filtered_themes();
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, theme)| {
            let style = if i == app.selected_theme {
                Style::default()
                    .fg(pal.selection_fg)
                    .bg(pal.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(pal.fg)
            };
            ListItem::new(Span::styled(theme.name(), style))
        })
        .collect();

    let title = if app.picker_filter.is_empty() {
        " Theme (type to filter, Enter to select) ".into()
    } else {
        format!(" Theme: {} ", app.picker_filter)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(pal.accent))
        .title(Span::styled(
            title,
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        ));

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    list_state.select(Some(app.selected_theme));
    frame.render_stateful_widget(list, popup, &mut list_state);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
