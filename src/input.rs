/// Command parser for `:` command mode.

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Quit,
    Refresh,
    Help,
    Theme(Option<String>),
    Timestamps,
    SpellCheck,
    ReadAll,
    Filter(FilterKind),
    Open(usize),
    Dm(String),
    Export(Option<String>),
    Attach(String),
    Forward(usize),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterKind {
    All,
    Unread,
    Favorites,
    Dms,
    Groups,
    Channels,
}

/// Tab-completable command names.
pub const COMMAND_NAMES: &[&str] = &[
    "quit", "q", "refresh", "r", "help", "?", "theme", "timestamps", "ts",
    "spell", "spellcheck", "readall", "read-all", "all", "unread", "fav",
    "favorites", "dms", "groups", "channels", "open", "dm", "export",
    "attach", "file", "forward", "fw",
];

pub fn parse_command(input: &str) -> Command {
    let input = input.trim();
    let (cmd, arg) = match input.split_once(char::is_whitespace) {
        Some((c, a)) => (c, Some(a.trim())),
        None => (input, None),
    };

    match cmd {
        "q" | "quit" => Command::Quit,
        "r" | "refresh" => Command::Refresh,
        "help" | "?" => Command::Help,
        "theme" => Command::Theme(arg.map(|s| s.to_string())),
        "timestamps" | "ts" => Command::Timestamps,
        "spell" | "spellcheck" => Command::SpellCheck,
        "readall" | "read-all" => Command::ReadAll,
        "all" => Command::Filter(FilterKind::All),
        "unread" => Command::Filter(FilterKind::Unread),
        "fav" | "favorites" => Command::Filter(FilterKind::Favorites),
        "dms" => Command::Filter(FilterKind::Dms),
        "groups" => Command::Filter(FilterKind::Groups),
        "channels" => Command::Filter(FilterKind::Channels),
        "open" => {
            let n = arg.and_then(|s| s.parse().ok()).unwrap_or(0);
            Command::Open(n)
        }
        "dm" => Command::Dm(arg.unwrap_or("").to_string()),
        "export" => Command::Export(arg.map(|s| s.to_string())),
        "attach" | "file" => Command::Attach(arg.unwrap_or("").to_string()),
        "forward" | "fw" => {
            let n = arg.and_then(|s| s.parse().ok()).unwrap_or(0);
            Command::Forward(n)
        }
        _ => Command::Unknown(input.to_string()),
    }
}

/// Return tab-complete candidates that start with `prefix`.
pub fn complete(prefix: &str) -> Vec<&'static str> {
    COMMAND_NAMES
        .iter()
        .filter(|name| name.starts_with(prefix))
        .copied()
        .collect()
}
