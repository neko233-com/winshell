use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomShell {
    pub id: String,
    pub name: String,
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub font_family: String,
    pub font_size: f32,
    pub scrollback: usize,
    pub default_shell: String,
    pub bash_path: Option<PathBuf>,
    pub working_directory: Option<PathBuf>,
    pub suggestions: bool,
    pub sidebar: bool,
    pub env: BTreeMap<String, String>,
    pub shells: Vec<CustomShell>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_family: "Cascadia Mono".into(),
            font_size: 15.,
            scrollback: 10_000,
            default_shell: "bash".into(),
            bash_path: None,
            working_directory: None,
            suggestions: true,
            sidebar: true,
            env: BTreeMap::new(),
            shells: Vec::new(),
        }
    }
}

pub fn directory() -> PathBuf {
    directories::ProjectDirs::from("", "", "winshell")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".winshell"))
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = directory().join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut config: Self = toml::from_str(&std::fs::read_to_string(&path)?)
            .with_context(|| format!("Invalid configuration: {}", path.display()))?;
        if !config.font_size.is_finite() {
            anyhow::bail!("font_size must be a finite number");
        }
        config.font_size = config.font_size.clamp(10., 32.);
        config.scrollback = config.scrollback.clamp(100, 100_000);
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let directory = directory();
        std::fs::create_dir_all(&directory)?;
        let temporary = directory.join(format!("config.{}.tmp", std::process::id()));
        std::fs::write(&temporary, toml::to_string_pretty(self)?)?;
        std::fs::rename(temporary, directory.join("config.toml"))?;
        Ok(())
    }

    pub fn cwd(&self) -> PathBuf {
        self.working_directory
            .clone()
            .filter(|path| path.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .or_else(|| directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("C:\\"))
    }
}
