use config::{Config as ConfigLoader, Environment, File};
use serde::{Deserialize, Serialize};
// use std::fs;
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub device_ip: String,
    pub port: u16,
    pub timeout: u64,
    pub ping_interval: u64,
    pub reconnect: u64,
}

static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();

impl Config {
    pub fn init() -> anyhow::Result<()> {
        let settings = ConfigLoader::builder()
            .add_source(File::with_name("config.toml"))
            .add_source(Environment::with_prefix("APP").separator("__"))
            .build()?;

        let config: Config = settings.try_deserialize()?;

        CONFIG
            .set(RwLock::new(config))
            .map_err(|_| anyhow::anyhow!("Config has already been initialized"))?;

        Ok(())
    }

    pub fn global() -> std::sync::RwLockReadGuard<'static, Config> {
        CONFIG
            .get()
            .expect("Config is not initialized. Call Config::init() first.")
            .read()
            .expect("Config RwLock was poisoned")
    }

    // pub fn update_and_save<F>(modify: F) -> anyhow::Result<()>
    // where
    //     F: FnOnce(&mut Config),
    // {
    //     let lock = CONFIG
    //         .get()
    //         .expect("Config is not initialized. Call Config::init() first.");

    //     let mut config = lock
    //         .write()
    //         .map_err(|_| anyhow::anyhow!("RwLock poisoned"))?;

    //     // Apply in-memory modifications
    //     modify(&mut config);

    //     // Serialize back to TOML format
    //     let toml_string = toml::to_string_pretty(&*config)?;

    //     // Save to file
    //     fs::write("config.toml", toml_string)?;

    //     Ok(())
    // }
}
