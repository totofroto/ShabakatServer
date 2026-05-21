use std::env;

pub struct Config {
    pub port: u16,
    pub scan_interval_secs: u64,
    pub data_dir: String,
    pub web_dir: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_redirect_uri: Option<String>,
    pub jwt_secret: String,
    pub disable_auth: bool,
    pub auth_bypass_local: bool,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env_parse("SHABAKAT_PORT", 8080),
            scan_interval_secs: env_parse("SHABAKAT_SCAN_INTERVAL", 600),
            data_dir: env::var("SHABAKAT_DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
            web_dir: env::var("SHABAKAT_WEB_DIR").ok().filter(|s| !s.is_empty()),
            telegram_bot_token: env::var("SHABAKAT_TELEGRAM_BOT_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            telegram_chat_id: env::var("SHABAKAT_TELEGRAM_CHAT_ID")
                .ok()
                .filter(|s| !s.is_empty()),
            google_client_id: env::var("GOOGLE_CLIENT_ID").ok().filter(|s| !s.is_empty()),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET").ok().filter(|s| !s.is_empty()),
            google_redirect_uri: env::var("GOOGLE_REDIRECT_URI").ok().filter(|s| !s.is_empty()),
            jwt_secret: env::var("JWT_SECRET").expect("CRITICAL ERROR: JWT_SECRET environment variable is missing! The server cannot start safely."),
            disable_auth: true, // DEBUG: hardcoded — remove before production deploy
            auth_bypass_local: env::var("SHABAKAT_AUTH_BYPASS_LOCAL").map(|v| v == "true").unwrap_or(false),
        }
    }
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
