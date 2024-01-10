use crate::{Error, Result};
use std::{env, sync::OnceLock};

pub fn config() -> &'static Config {
    static INSTANCE: OnceLock<Config> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Config::load_from_env().unwrap_or_else(|ex| panic!("Failed to load config: {ex:?}"))
    })
}
pub struct Config {
    pub database_url: String,
}

impl Config {
    fn load_from_env() -> Result<Config> {
        Ok(Config {
            database_url: get_env("DATABASE_URL")?,
        })
    }
}

fn get_env(name: &'static str) -> Result<String> {
    env::var(name).map_err(|_| Error::ConfigMissingEnv(name))
}
