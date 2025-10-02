use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub timings: bool,
    pub root_url: Option<String>,
    pub math: MathConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timings: false,
            root_url: None,
            math: MathConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MathConfig {
    pub prefer_persistent: bool,
    pub command: Option<String>,
}

impl Default for MathConfig {
    fn default() -> Self {
        Self {
            prefer_persistent: false,
            command: None,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("failed to read config {}: {}", display(path), e))?;
        let mut config: Config = toml::from_str(&contents)
            .map_err(|e| format!("failed to parse config {}: {}", display(path), e))?;
        config.normalize();
        Ok(config)
    }

    fn normalize(&mut self) {
        if let Some(root) = &mut self.root_url {
            if root != "/" {
                *root = root.trim_end_matches('/').to_string();
            }
        }
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn default_config_path(input_path: &Path) -> PathBuf {
    let dir = input_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join("dllup.toml")
}
