use std::fs::{self, File};
use std::time::Duration;

use anyhow::Result;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use strum::FromRepr;

use crate::constants::ALLIUM_POWER_SETTINGS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSettings {
    pub power_button_action: PowerButtonAction,
    pub lid_close_action: PowerButtonAction,
    pub auto_sleep_when_charging: bool,
    pub auto_sleep_duration_minutes: i32,
    #[serde(default)]
    pub auto_shutdown_delay: AutoShutdownDelay,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, FromRepr, Default)]
pub enum PowerButtonAction {
    #[default]
    Suspend,
    Shutdown,
    Nothing,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, FromRepr, Default)]
pub enum AutoShutdownDelay {
    Secs10,
    Secs30,
    Secs60,
    Mins2,
    #[default]
    Mins5,
    Mins10,
    Mins30,
    Mins60,
    Never,
}

impl AutoShutdownDelay {
    pub fn to_duration(&self) -> Option<Duration> {
        match self {
            AutoShutdownDelay::Secs10 => Some(Duration::from_secs(10)),
            AutoShutdownDelay::Secs30 => Some(Duration::from_secs(30)),
            AutoShutdownDelay::Secs60 => Some(Duration::from_secs(60)),
            AutoShutdownDelay::Mins2 => Some(Duration::from_secs(2 * 60)),
            AutoShutdownDelay::Mins5 => Some(Duration::from_secs(5 * 60)),
            AutoShutdownDelay::Mins10 => Some(Duration::from_secs(10 * 60)),
            AutoShutdownDelay::Mins30 => Some(Duration::from_secs(30 * 60)),
            AutoShutdownDelay::Mins60 => Some(Duration::from_secs(60 * 60)),
            AutoShutdownDelay::Never => None,
        }
    }
}

impl Default for PowerSettings {
    fn default() -> Self {
        Self {
            lid_close_action: PowerButtonAction::Shutdown,
            power_button_action: PowerButtonAction::Suspend,
            auto_sleep_when_charging: true,
            auto_sleep_duration_minutes: 5,
            auto_shutdown_delay: AutoShutdownDelay::Mins5,
        }
    }
}

impl PowerSettings {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load() -> Result<Self> {
        if ALLIUM_POWER_SETTINGS.exists() {
            debug!("found state, loading from file");
            let file = File::open(ALLIUM_POWER_SETTINGS.as_path())?;
            if let Ok(json) = serde_json::from_reader(file) {
                return Ok(json);
            }
            warn!("failed to read power file, removing");
            fs::remove_file(ALLIUM_POWER_SETTINGS.as_path())?;
        }
        Ok(Self::new())
    }

    pub fn save(&self) -> Result<()> {
        let file = File::create(ALLIUM_POWER_SETTINGS.as_path())?;
        serde_json::to_writer(file, &self)?;
        Ok(())
    }
}
