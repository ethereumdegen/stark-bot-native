use crate::config::Config;

pub fn run(key: Option<String>, value: Option<String>) -> Result<(), String> {
    let mut config = Config::load();

    match (key, value) {
        (None, _) => {
            // Show all config
            println!("default_agent: {}", config.default_agent);
            println!("base_url: {}", config.base_url);
            println!("poll_interval_secs: {}", config.poll_interval_secs);
            println!("poll_timeout_secs: {}", config.poll_timeout_secs);
            println!();
            println!("config dir: {}", crate::config::config_dir().display());
            if let Some(key) = config.api_key() {
                println!("api_key: {}...{}", &key[..4.min(key.len())], &key[key.len().saturating_sub(4)..]);
            } else {
                println!("api_key: (not set)");
            }
        }
        (Some(key), None) => {
            // Get single value
            match key.as_str() {
                "default_agent" => println!("{}", config.default_agent),
                "base_url" => println!("{}", config.base_url),
                "poll_interval_secs" => println!("{}", config.poll_interval_secs),
                "poll_timeout_secs" => println!("{}", config.poll_timeout_secs),
                _ => return Err(format!("Unknown config key: {}", key)),
            }
        }
        (Some(key), Some(val)) => {
            // Set value
            match key.as_str() {
                "default_agent" => config.default_agent = val,
                "base_url" => config.base_url = val,
                "poll_interval_secs" => config.poll_interval_secs = val.parse().map_err(|_| "Invalid number")?,
                "poll_timeout_secs" => config.poll_timeout_secs = val.parse().map_err(|_| "Invalid number")?,
                _ => return Err(format!("Unknown config key: {}", key)),
            }
            config.save()?;
            println!("Set {} = {}", key, config.default_agent);
        }
    }

    Ok(())
}
