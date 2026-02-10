/// Server configuration loaded from environment variables.
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_origin: String,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("NVISY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("NVISY_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            cors_origin: std::env::var("NVISY_CORS_ORIGIN").unwrap_or_else(|_| "*".to_string()),
        }
    }
}
