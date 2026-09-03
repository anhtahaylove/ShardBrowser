use crate::store;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// Absolute path to the ShardX executable.
    pub browser_path: Option<String>,
    /// Theme: "dark" (default) or "light".
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Geo-IP checker provider used by the proxy "Test" button.
    /// One of "ip-api.com" | "ipapi.co" | "ipwho.is".
    #[serde(default)]
    pub geo_checker: Option<String>,
    /// "fingerprint" (use the screen from the bound fingerprint) or
    /// "real" (let ShardX use the host's real screen).
    #[serde(default)]
    pub screen_resolution_mode: Option<String>,
    /// Offer to fill fields a generated identity fits. Never applies to a
    /// synchronised launch — input is already mirrored there.
    #[serde(default = "default_true")]
    pub helper_enabled: bool,
    /// Profile's camera is ShardX's rather than the machine's. On by default:
    /// the host's real camera contradicts the fingerprint and links profiles.
    #[serde(default = "default_true")]
    pub camera_enabled: bool,
    /// Field kinds the helper reacts to, as the engine names them. Empty = all.
    #[serde(default)]
    pub helper_triggers: Vec<String>,
    /// Hide the launcher to the system tray on close instead of quitting.
    #[serde(default = "default_minimize_to_tray")]
    pub minimize_to_tray: bool,
    /// Appended to every launch, one per line. Applied last, so a repeat wins.
    #[serde(default)]
    pub extra_args: String,
    /// Where profiles, user-data, extensions and the trash live. None = the
    /// config dir. Changed through `data_root_migrate`, never by hand.
    #[serde(default)]
    pub data_root: Option<String>,

    // ---- Local automation HTTP API (axum + JWT bearer) ----
    /// Whether the local API server listens on 127.0.0.1:`api_port`.
    #[serde(default = "default_api_enabled")]
    pub api_enabled: bool,
    /// Port the API binds on 127.0.0.1.
    #[serde(default = "default_api_port")]
    pub api_port: u16,
    /// HS256 signing key for API JWTs.  Auto-generated on first run
    /// (see `ensure_secret`); rotating it invalidates issued tokens.
    #[serde(default)]
    pub api_secret: String,
}

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "dark".into()
}

fn default_minimize_to_tray() -> bool {
    true
}

fn default_api_enabled() -> bool {
    true
}

fn default_api_port() -> u16 {
    40325
}

pub fn load() -> Result<Settings> {
    let path = store::settings_path()?;
    if !path.exists() {
        return Ok(Settings {
            browser_path: None,
            theme: default_theme(),
            geo_checker: Some("ip-api.com".into()),
            screen_resolution_mode: Some("fingerprint".into()),
            helper_enabled: default_true(),
            camera_enabled: default_true(),
            helper_triggers: Vec::new(),
            minimize_to_tray: default_minimize_to_tray(),
            extra_args: String::new(),
            data_root: None,
            api_enabled: default_api_enabled(),
            api_port: default_api_port(),
            api_secret: String::new(),
        });
    }
    let body = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&body).unwrap_or_default())
}

/// Load settings, generating + persisting the API JWT secret if it's
/// still empty.  Call once at startup before the server reads it.
pub fn ensure_secret() -> Result<Settings> {
    let mut s = load()?;
    if s.api_secret.is_empty() {
        s.api_secret = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        save(&s)?;
    }
    Ok(s)
}

/// Split into switches on whitespace; quoted runs survive.
pub fn parse_extra_args(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in raw.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, c @ ('"' | '\'')) => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn save(s: &Settings) -> Result<()> {
    let body = serde_json::to_string_pretty(s)?;
    fs::write(store::settings_path()?, body)?;
    Ok(())
}
