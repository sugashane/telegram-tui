use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("telegram-tui")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn state_path() -> PathBuf {
    config_dir().join("state.json")
}

pub fn session_path() -> PathBuf {
    config_dir().join("telegram.session")
}

// ---------------------------------------------------------------------------
// AppConfig – user preferences (TOML)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Telegram API id from https://my.telegram.org
    pub api_id: Option<i32>,
    /// Telegram API hash from https://my.telegram.org
    pub api_hash: Option<String>,
    /// Phone number with country code (e.g. "+15551234567")
    pub phone: Option<String>,
    /// IANA timezone name (e.g. "America/New_York")
    pub timezone: String,
    /// Active theme name
    pub theme: Theme,
    /// Background poll interval in seconds (for presence, etc.)
    pub poll_interval_secs: u64,
    /// Show message timestamps
    pub show_timestamps: bool,
    /// Desktop notification mode
    pub notifications: NotifyMode,
    /// Enable spell-check via aspell
    pub spell_check: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_id: None,
            api_hash: None,
            phone: None,
            timezone: "UTC".into(),
            theme: Theme::Default,
            poll_interval_secs: 60,
            show_timestamps: true,
            notifications: NotifyMode::DmOnly,
            spell_check: false,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        match fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = config_dir();
        fs::create_dir_all(&dir)?;
        let s = toml::to_string_pretty(self)?;
        fs::write(config_path(), s)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AppState – runtime state (JSON, persisted frequently)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub selected_chat_id: Option<i64>,
    pub favorite_chat_ids: HashSet<i64>,
    pub muted_chat_ids: HashSet<i64>,
    pub starred_message_keys: HashSet<String>,
    pub filter: String,
    pub compose_draft: String,
    /// Per-chat read watermark: chat_id -> last_read_message_id
    pub read_watermarks: HashMap<i64, i32>,
}

impl AppState {
    pub fn load() -> Self {
        let path = state_path();
        match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = config_dir();
        fs::create_dir_all(&dir)?;
        let s = serde_json::to_string_pretty(self)?;
        let path = state_path();
        fs::write(&path, s)?;
        // Restrict permissions on Unix (contains user state)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Notification mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyMode {
    Off,
    All,
    DmOnly,
    DmAndMentions,
}

impl Default for NotifyMode {
    fn default() -> Self {
        Self::DmOnly
    }
}

// ---------------------------------------------------------------------------
// Theme system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    Default,
    Dracula,
    Gruvbox,
    Nord,
    Solarized,
    Monokai,
    Tokyo,
    Catppuccin,
    Everforest,
    OneDark,
    Kanagawa,
    Rose,
}

impl Theme {
    pub const ALL: &'static [Theme] = &[
        Theme::Default,
        Theme::Dracula,
        Theme::Gruvbox,
        Theme::Nord,
        Theme::Solarized,
        Theme::Monokai,
        Theme::Tokyo,
        Theme::Catppuccin,
        Theme::Everforest,
        Theme::OneDark,
        Theme::Kanagawa,
        Theme::Rose,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Dracula => "dracula",
            Self::Gruvbox => "gruvbox",
            Self::Nord => "nord",
            Self::Solarized => "solarized",
            Self::Monokai => "monokai",
            Self::Tokyo => "tokyo",
            Self::Catppuccin => "catppuccin",
            Self::Everforest => "everforest",
            Self::OneDark => "onedark",
            Self::Kanagawa => "kanagawa",
            Self::Rose => "rose",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.iter().find(|t| t.name() == s).copied()
    }

    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Default => ThemePalette::default_theme(),
            Self::Dracula => ThemePalette::dracula(),
            Self::Gruvbox => ThemePalette::gruvbox(),
            Self::Nord => ThemePalette::nord(),
            Self::Solarized => ThemePalette::solarized(),
            Self::Monokai => ThemePalette::monokai(),
            Self::Tokyo => ThemePalette::tokyo(),
            Self::Catppuccin => ThemePalette::catppuccin(),
            Self::Everforest => ThemePalette::everforest(),
            Self::OneDark => ThemePalette::onedark(),
            Self::Kanagawa => ThemePalette::kanagawa(),
            Self::Rose => ThemePalette::rose(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub muted: Color,
    pub border: Color,
    pub border_focus: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub mode_normal: Color,
    pub mode_insert: Color,
    pub mode_command: Color,
    pub mode_visual: Color,
    pub mode_search: Color,
    pub unread: Color,
    pub mention: Color,
    pub error: Color,
    /// Author name colour rotation
    pub authors: [Color; 7],
}

impl ThemePalette {
    pub fn default_theme() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::White,
            accent: Color::Cyan,
            muted: Color::DarkGray,
            border: Color::DarkGray,
            border_focus: Color::Cyan,
            selection_bg: Color::Rgb(40, 40, 60),
            selection_fg: Color::White,
            mode_normal: Color::Blue,
            mode_insert: Color::Green,
            mode_command: Color::Yellow,
            mode_visual: Color::Magenta,
            mode_search: Color::Yellow,
            unread: Color::Green,
            mention: Color::Rgb(255, 200, 50),
            error: Color::Red,
            authors: [
                Color::Cyan,
                Color::Green,
                Color::Yellow,
                Color::Magenta,
                Color::Blue,
                Color::Red,
                Color::LightCyan,
            ],
        }
    }

    pub fn dracula() -> Self {
        Self {
            bg: Color::Rgb(40, 42, 54),
            fg: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(139, 233, 253),
            muted: Color::Rgb(98, 114, 164),
            border: Color::Rgb(68, 71, 90),
            border_focus: Color::Rgb(189, 147, 249),
            selection_bg: Color::Rgb(68, 71, 90),
            selection_fg: Color::Rgb(248, 248, 242),
            mode_normal: Color::Rgb(139, 233, 253),
            mode_insert: Color::Rgb(80, 250, 123),
            mode_command: Color::Rgb(241, 250, 140),
            mode_visual: Color::Rgb(189, 147, 249),
            mode_search: Color::Rgb(241, 250, 140),
            unread: Color::Rgb(80, 250, 123),
            mention: Color::Rgb(255, 184, 108),
            error: Color::Rgb(255, 85, 85),
            authors: [
                Color::Rgb(139, 233, 253),
                Color::Rgb(80, 250, 123),
                Color::Rgb(241, 250, 140),
                Color::Rgb(189, 147, 249),
                Color::Rgb(255, 121, 198),
                Color::Rgb(255, 184, 108),
                Color::Rgb(255, 85, 85),
            ],
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            bg: Color::Rgb(40, 40, 40),
            fg: Color::Rgb(235, 219, 178),
            accent: Color::Rgb(131, 165, 152),
            muted: Color::Rgb(146, 131, 116),
            border: Color::Rgb(80, 73, 69),
            border_focus: Color::Rgb(214, 93, 14),
            selection_bg: Color::Rgb(80, 73, 69),
            selection_fg: Color::Rgb(235, 219, 178),
            mode_normal: Color::Rgb(131, 165, 152),
            mode_insert: Color::Rgb(184, 187, 38),
            mode_command: Color::Rgb(250, 189, 47),
            mode_visual: Color::Rgb(211, 134, 155),
            mode_search: Color::Rgb(250, 189, 47),
            unread: Color::Rgb(184, 187, 38),
            mention: Color::Rgb(254, 128, 25),
            error: Color::Rgb(204, 36, 29),
            authors: [
                Color::Rgb(131, 165, 152),
                Color::Rgb(184, 187, 38),
                Color::Rgb(250, 189, 47),
                Color::Rgb(211, 134, 155),
                Color::Rgb(214, 93, 14),
                Color::Rgb(204, 36, 29),
                Color::Rgb(69, 133, 136),
            ],
        }
    }

    pub fn nord() -> Self {
        Self {
            bg: Color::Rgb(46, 52, 64),
            fg: Color::Rgb(216, 222, 233),
            accent: Color::Rgb(136, 192, 208),
            muted: Color::Rgb(76, 86, 106),
            border: Color::Rgb(59, 66, 82),
            border_focus: Color::Rgb(136, 192, 208),
            selection_bg: Color::Rgb(67, 76, 94),
            selection_fg: Color::Rgb(229, 233, 240),
            mode_normal: Color::Rgb(136, 192, 208),
            mode_insert: Color::Rgb(163, 190, 140),
            mode_command: Color::Rgb(235, 203, 139),
            mode_visual: Color::Rgb(180, 142, 173),
            mode_search: Color::Rgb(235, 203, 139),
            unread: Color::Rgb(163, 190, 140),
            mention: Color::Rgb(208, 135, 112),
            error: Color::Rgb(191, 97, 106),
            authors: [
                Color::Rgb(136, 192, 208),
                Color::Rgb(163, 190, 140),
                Color::Rgb(235, 203, 139),
                Color::Rgb(180, 142, 173),
                Color::Rgb(129, 161, 193),
                Color::Rgb(208, 135, 112),
                Color::Rgb(191, 97, 106),
            ],
        }
    }

    pub fn solarized() -> Self {
        Self {
            bg: Color::Rgb(0, 43, 54),
            fg: Color::Rgb(131, 148, 150),
            accent: Color::Rgb(38, 139, 210),
            muted: Color::Rgb(88, 110, 117),
            border: Color::Rgb(7, 54, 66),
            border_focus: Color::Rgb(38, 139, 210),
            selection_bg: Color::Rgb(7, 54, 66),
            selection_fg: Color::Rgb(147, 161, 161),
            mode_normal: Color::Rgb(38, 139, 210),
            mode_insert: Color::Rgb(133, 153, 0),
            mode_command: Color::Rgb(181, 137, 0),
            mode_visual: Color::Rgb(108, 113, 196),
            mode_search: Color::Rgb(181, 137, 0),
            unread: Color::Rgb(133, 153, 0),
            mention: Color::Rgb(203, 75, 22),
            error: Color::Rgb(220, 50, 47),
            authors: [
                Color::Rgb(38, 139, 210),
                Color::Rgb(133, 153, 0),
                Color::Rgb(181, 137, 0),
                Color::Rgb(108, 113, 196),
                Color::Rgb(42, 161, 152),
                Color::Rgb(203, 75, 22),
                Color::Rgb(211, 54, 130),
            ],
        }
    }

    pub fn monokai() -> Self {
        Self {
            bg: Color::Rgb(39, 40, 34),
            fg: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(102, 217, 239),
            muted: Color::Rgb(117, 113, 94),
            border: Color::Rgb(62, 61, 50),
            border_focus: Color::Rgb(166, 226, 46),
            selection_bg: Color::Rgb(62, 61, 50),
            selection_fg: Color::Rgb(248, 248, 242),
            mode_normal: Color::Rgb(102, 217, 239),
            mode_insert: Color::Rgb(166, 226, 46),
            mode_command: Color::Rgb(230, 219, 116),
            mode_visual: Color::Rgb(174, 129, 255),
            mode_search: Color::Rgb(230, 219, 116),
            unread: Color::Rgb(166, 226, 46),
            mention: Color::Rgb(253, 151, 31),
            error: Color::Rgb(249, 38, 114),
            authors: [
                Color::Rgb(102, 217, 239),
                Color::Rgb(166, 226, 46),
                Color::Rgb(230, 219, 116),
                Color::Rgb(174, 129, 255),
                Color::Rgb(249, 38, 114),
                Color::Rgb(253, 151, 31),
                Color::Rgb(102, 217, 239),
            ],
        }
    }

    pub fn tokyo() -> Self {
        Self {
            bg: Color::Rgb(26, 27, 38),
            fg: Color::Rgb(169, 177, 214),
            accent: Color::Rgb(125, 207, 255),
            muted: Color::Rgb(65, 72, 104),
            border: Color::Rgb(41, 46, 66),
            border_focus: Color::Rgb(125, 207, 255),
            selection_bg: Color::Rgb(41, 46, 66),
            selection_fg: Color::Rgb(192, 202, 245),
            mode_normal: Color::Rgb(125, 207, 255),
            mode_insert: Color::Rgb(158, 206, 106),
            mode_command: Color::Rgb(224, 175, 104),
            mode_visual: Color::Rgb(187, 154, 247),
            mode_search: Color::Rgb(224, 175, 104),
            unread: Color::Rgb(158, 206, 106),
            mention: Color::Rgb(255, 158, 100),
            error: Color::Rgb(247, 118, 142),
            authors: [
                Color::Rgb(125, 207, 255),
                Color::Rgb(158, 206, 106),
                Color::Rgb(224, 175, 104),
                Color::Rgb(187, 154, 247),
                Color::Rgb(247, 118, 142),
                Color::Rgb(255, 158, 100),
                Color::Rgb(115, 218, 202),
            ],
        }
    }

    pub fn catppuccin() -> Self {
        Self {
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            accent: Color::Rgb(137, 180, 250),
            muted: Color::Rgb(88, 91, 112),
            border: Color::Rgb(49, 50, 68),
            border_focus: Color::Rgb(203, 166, 247),
            selection_bg: Color::Rgb(49, 50, 68),
            selection_fg: Color::Rgb(205, 214, 244),
            mode_normal: Color::Rgb(137, 180, 250),
            mode_insert: Color::Rgb(166, 227, 161),
            mode_command: Color::Rgb(249, 226, 175),
            mode_visual: Color::Rgb(203, 166, 247),
            mode_search: Color::Rgb(249, 226, 175),
            unread: Color::Rgb(166, 227, 161),
            mention: Color::Rgb(250, 179, 135),
            error: Color::Rgb(243, 139, 168),
            authors: [
                Color::Rgb(137, 180, 250),
                Color::Rgb(166, 227, 161),
                Color::Rgb(249, 226, 175),
                Color::Rgb(203, 166, 247),
                Color::Rgb(245, 194, 231),
                Color::Rgb(250, 179, 135),
                Color::Rgb(148, 226, 213),
            ],
        }
    }

    pub fn everforest() -> Self {
        Self {
            bg: Color::Rgb(39, 51, 44),
            fg: Color::Rgb(211, 198, 170),
            accent: Color::Rgb(127, 187, 179),
            muted: Color::Rgb(113, 120, 107),
            border: Color::Rgb(52, 63, 56),
            border_focus: Color::Rgb(163, 190, 140),
            selection_bg: Color::Rgb(52, 63, 56),
            selection_fg: Color::Rgb(211, 198, 170),
            mode_normal: Color::Rgb(127, 187, 179),
            mode_insert: Color::Rgb(167, 192, 128),
            mode_command: Color::Rgb(219, 188, 127),
            mode_visual: Color::Rgb(214, 153, 182),
            mode_search: Color::Rgb(219, 188, 127),
            unread: Color::Rgb(167, 192, 128),
            mention: Color::Rgb(230, 152, 117),
            error: Color::Rgb(230, 126, 128),
            authors: [
                Color::Rgb(127, 187, 179),
                Color::Rgb(167, 192, 128),
                Color::Rgb(219, 188, 127),
                Color::Rgb(214, 153, 182),
                Color::Rgb(230, 152, 117),
                Color::Rgb(230, 126, 128),
                Color::Rgb(131, 192, 158),
            ],
        }
    }

    pub fn onedark() -> Self {
        Self {
            bg: Color::Rgb(40, 44, 52),
            fg: Color::Rgb(171, 178, 191),
            accent: Color::Rgb(97, 175, 239),
            muted: Color::Rgb(92, 99, 112),
            border: Color::Rgb(50, 56, 66),
            border_focus: Color::Rgb(97, 175, 239),
            selection_bg: Color::Rgb(50, 56, 66),
            selection_fg: Color::Rgb(192, 202, 213),
            mode_normal: Color::Rgb(97, 175, 239),
            mode_insert: Color::Rgb(152, 195, 121),
            mode_command: Color::Rgb(229, 192, 123),
            mode_visual: Color::Rgb(198, 120, 221),
            mode_search: Color::Rgb(229, 192, 123),
            unread: Color::Rgb(152, 195, 121),
            mention: Color::Rgb(209, 154, 102),
            error: Color::Rgb(224, 108, 117),
            authors: [
                Color::Rgb(97, 175, 239),
                Color::Rgb(152, 195, 121),
                Color::Rgb(229, 192, 123),
                Color::Rgb(198, 120, 221),
                Color::Rgb(224, 108, 117),
                Color::Rgb(209, 154, 102),
                Color::Rgb(86, 182, 194),
            ],
        }
    }

    pub fn kanagawa() -> Self {
        Self {
            bg: Color::Rgb(31, 31, 40),
            fg: Color::Rgb(220, 215, 186),
            accent: Color::Rgb(126, 156, 216),
            muted: Color::Rgb(84, 84, 109),
            border: Color::Rgb(54, 54, 70),
            border_focus: Color::Rgb(149, 127, 184),
            selection_bg: Color::Rgb(54, 54, 70),
            selection_fg: Color::Rgb(220, 215, 186),
            mode_normal: Color::Rgb(126, 156, 216),
            mode_insert: Color::Rgb(152, 187, 108),
            mode_command: Color::Rgb(226, 194, 120),
            mode_visual: Color::Rgb(149, 127, 184),
            mode_search: Color::Rgb(226, 194, 120),
            unread: Color::Rgb(152, 187, 108),
            mention: Color::Rgb(255, 160, 102),
            error: Color::Rgb(195, 64, 67),
            authors: [
                Color::Rgb(126, 156, 216),
                Color::Rgb(152, 187, 108),
                Color::Rgb(226, 194, 120),
                Color::Rgb(149, 127, 184),
                Color::Rgb(210, 126, 153),
                Color::Rgb(255, 160, 102),
                Color::Rgb(122, 168, 159),
            ],
        }
    }

    pub fn rose() -> Self {
        Self {
            bg: Color::Rgb(25, 23, 36),
            fg: Color::Rgb(224, 222, 244),
            accent: Color::Rgb(156, 207, 216),
            muted: Color::Rgb(110, 106, 134),
            border: Color::Rgb(38, 35, 53),
            border_focus: Color::Rgb(196, 167, 231),
            selection_bg: Color::Rgb(57, 53, 82),
            selection_fg: Color::Rgb(224, 222, 244),
            mode_normal: Color::Rgb(156, 207, 216),
            mode_insert: Color::Rgb(156, 207, 216),
            mode_command: Color::Rgb(246, 193, 119),
            mode_visual: Color::Rgb(196, 167, 231),
            mode_search: Color::Rgb(246, 193, 119),
            unread: Color::Rgb(156, 207, 216),
            mention: Color::Rgb(234, 154, 151),
            error: Color::Rgb(235, 111, 146),
            authors: [
                Color::Rgb(156, 207, 216),
                Color::Rgb(246, 193, 119),
                Color::Rgb(196, 167, 231),
                Color::Rgb(235, 111, 146),
                Color::Rgb(49, 116, 143),
                Color::Rgb(234, 154, 151),
                Color::Rgb(62, 143, 176),
            ],
        }
    }
}
