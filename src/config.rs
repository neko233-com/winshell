use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

pub fn set_directory(path: PathBuf) -> Result<()> {
    CONFIG_DIRECTORY
        .set(path)
        .map_err(|_| anyhow::anyhow!("Configuration directory is already initialized"))
}

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
    pub language: String,
    pub theme: String,
    pub custom_theme: crate::theme::ThemeOverrides,
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
            language: "system".into(),
            theme: "midnight".into(),
            custom_theme: Default::default(),
            font_family: if cfg!(target_os = "macos") {
                "Menlo"
            } else {
                "Cascadia Mono"
            }
            .into(),
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
    if let Some(path) = CONFIG_DIRECTORY.get() {
        return path.clone();
    }
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
        anyhow::ensure!(
            crate::theme::THEMES
                .iter()
                .any(|(id, _)| *id == config.theme),
            "Unknown theme: {}",
            config.theme
        );
        for color in [
            &config.custom_theme.background,
            &config.custom_theme.foreground,
            &config.custom_theme.accent,
            &config.custom_theme.panel,
            &config.custom_theme.border,
            &config.custom_theme.selection,
        ]
        .into_iter()
        .flatten()
        {
            crate::theme::parse_color(color)?;
        }
        for shell in &config.shells {
            anyhow::ensure!(
                !shell.id.trim().is_empty() && !shell.name.trim().is_empty(),
                "Shell profiles require an id and name"
            );
        }
        for (key, value) in config
            .env
            .iter()
            .chain(config.shells.iter().flat_map(|shell| shell.env.iter()))
        {
            anyhow::ensure!(
                !key.is_empty() && !key.contains(['=', '\0']) && !value.contains('\0'),
                "Invalid environment variable configuration"
            );
        }
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
