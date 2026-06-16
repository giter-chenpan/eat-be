use rocket::Config;
use std::sync::OnceLock;

pub struct AppConfig {
    pub translation_url: String,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub chat_rate_limit_per_minute: u32,
    pub chat_max_tool_iterations: u32,
    pub mcp_server_url: String,
}

impl AppConfig {
    pub fn new() -> Self {
        let translation_url = Config::figment()
            .extract_inner::<String>("translation_url")
            .expect("translation_url is not set");
        let openrouter_api_key = Config::figment()
            .extract_inner::<String>("openrouter_api_key")
            .expect("OPENROUTER_API_KEY is not set (config: openrouter_api_key)");
        let openrouter_model = Config::figment()
            .extract_inner::<String>("openrouter_model")
            .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());
        let chat_rate_limit_per_minute = Config::figment()
            .extract_inner::<u32>("chat_rate_limit_per_minute")
            .unwrap_or(20);
        let chat_max_tool_iterations = Config::figment()
            .extract_inner::<u32>("chat_max_tool_iterations")
            .unwrap_or(5);
        let mcp_server_url = Config::figment()
            .extract_inner::<String>("mcp_server_url")
            .unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
        Self {
            translation_url,
            openrouter_api_key,
            openrouter_model,
            chat_rate_limit_per_minute,
            chat_max_tool_iterations,
            mcp_server_url,
        }
    }
}

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();

pub fn get_config() -> &'static AppConfig {
    APP_CONFIG.get_or_init(|| AppConfig::new())
}
