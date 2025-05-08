use rocket::Config;
use std::sync::OnceLock;

pub struct AppConfig {
   pub translation_url: String,
}

impl AppConfig {
    pub fn new() -> Self{
        let translation_url = Config::figment().extract_inner::<String>("translation_url").expect("translation_url is not set");
        Self {
            translation_url,
        }
    }
}


 static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();

pub fn get_config() -> &'static AppConfig {
    APP_CONFIG.get_or_init(|| AppConfig::new())
}
