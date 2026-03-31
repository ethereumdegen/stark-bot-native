use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_agent")]
    pub default_agent: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_poll_timeout")]
    pub poll_timeout_secs: u64,
}

fn default_agent() -> String { "general".into() }
fn default_base_url() -> String { "https://starflask.com/api".into() }
fn default_poll_interval() -> u64 { 3 }
fn default_poll_timeout() -> u64 { 600 }

impl Default for Config {
    fn default() -> Self {
        Self {
            default_agent: default_agent(),
            project_id: None,
            base_url: default_base_url(),
            poll_interval_secs: default_poll_interval(),
            poll_timeout_secs: default_poll_timeout(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let dir = config_dir();
        std::fs::create_dir_all(&dir).ok();

        // Load .env
        let env_path = dir.join(".env");
        dotenvy::from_path(&env_path).ok();

        // Load config.yaml
        let config_path = dir.join("config.yaml");
        if config_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&config_path) {
                if let Ok(cfg) = serde_yaml::from_str(&contents) {
                    return cfg;
                }
            }
        }

        // Write default config
        let cfg = Config::default();
        if let Ok(yaml) = serde_yaml::to_string(&cfg) {
            std::fs::write(&config_path, yaml).ok();
        }
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_dir().join("config.yaml");
        let yaml = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(path, yaml).map_err(|e| e.to_string())
    }

    pub fn api_key(&self) -> Option<String> {
        std::env::var("STARFLASK_API_KEY").ok().filter(|k| !k.is_empty())
    }

    pub fn save_api_key(key: &str) -> Result<(), String> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let env_path = dir.join(".env");

        // Read existing content, replace or append the key
        let mut lines: Vec<String> = if env_path.exists() {
            std::fs::read_to_string(&env_path)
                .map_err(|e| e.to_string())?
                .lines()
                .filter(|l| !l.starts_with("STARFLASK_API_KEY="))
                .map(|l| l.to_string())
                .collect()
        } else {
            Vec::new()
        };
        lines.push(format!("STARFLASK_API_KEY={}", key));

        std::fs::write(&env_path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;

        // Set in current process so it takes effect immediately
        // SAFETY: We're single-threaded at this point in the setup flow
        unsafe { std::env::set_var("STARFLASK_API_KEY", key); }
        Ok(())
    }

    /// Wipe all config: .env (API key) and config.yaml.
    pub fn reset() -> Result<(), String> {
        let dir = config_dir();
        let env_path = dir.join(".env");
        let config_path = dir.join("config.yaml");
        if env_path.exists() {
            std::fs::remove_file(&env_path).map_err(|e| e.to_string())?;
        }
        if config_path.exists() {
            std::fs::remove_file(&config_path).map_err(|e| e.to_string())?;
        }
        // Clear from current process
        // SAFETY: single-threaded at this point
        unsafe { std::env::remove_var("STARFLASK_API_KEY"); }
        Ok(())
    }

    pub fn base_url(&self) -> String {
        std::env::var("STARFLASK_BASE_URL")
            .ok()
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| self.base_url.clone())
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("starkbot")
}

