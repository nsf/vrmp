use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_favorite_directories")]
    pub favorite_directories: Vec<PathBuf>,
    #[serde(default = "default_show_video_files_only")]
    pub show_video_files_only: bool,
    #[serde(default = "default_show_hidden_files")]
    pub show_hidden_files: bool,
    #[serde(default = "default_ui_distance")]
    pub ui_distance: f32,
    #[serde(default = "default_ui_angle")]
    pub ui_angle: f32,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_camera_movement_speed")]
    pub camera_movement_speed: f32,
    #[serde(default = "default_camera_sensitivity")]
    pub camera_sensitivity: f32,
    #[serde(default = "default_cursor_sensitivity")]
    pub cursor_sensitivity: f32,
}

fn default_favorite_directories() -> Vec<PathBuf> {
    Default::default()
}

fn default_show_video_files_only() -> bool {
    true
}

fn default_show_hidden_files() -> bool {
    Default::default()
}

fn default_ui_distance() -> f32 {
    0.45
}

fn default_ui_angle() -> f32 {
    35.0
}

fn default_ui_scale() -> f32 {
    0.45
}

fn default_camera_movement_speed() -> f32 {
    5.0
}

fn default_camera_sensitivity() -> f32 {
    0.05
}

fn default_cursor_sensitivity() -> f32 {
    1.0
}

impl Config {
    pub fn load() -> Result<Config, anyhow::Error> {
        let dirs = xdg::BaseDirectories::with_prefix("vrmp")?;
        if let Some(file) = dirs.find_config_file("config.ron") {
            let bytes = std::fs::read(&file)?;
            let s = String::from_utf8(bytes)?;
            Ok(ron::from_str(&s)?)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        let dirs = xdg::BaseDirectories::with_prefix("vrmp")?;
        let path = dirs.place_config_file("config.ron")?;
        let s = ron::to_string(self)?;
        Ok(std::fs::write(path, s)?)
    }
}

pub struct ConfigSyncer {
    config: Config,
    dirty: bool,
}

impl ConfigSyncer {
    pub fn new(config: Config) -> ConfigSyncer {
        ConfigSyncer { config, dirty: false }
    }

    pub fn get(&self) -> &Config {
        &self.config
    }

    /// When you request config for writing, it's assumed to be modified and therefore will be synced to disk.
    /// Thus for read only access use get().
    pub fn get_mut(&mut self) -> &mut Config {
        self.dirty = true;
        &mut self.config
    }

    pub fn save_maybe(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        match self.config.save() {
            Ok(_) => log::info!("saved config file"),
            Err(e) => log::error!("failed saving config file: {}", e),
        }
    }
}
