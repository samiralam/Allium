use std::fs::{self, File};
use std::io::Write;

use anyhow::Result;
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::constants::ALLIUM_CLOCK_SETTINGS;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClockSettings {
    #[serde(default)]
    pub twelve_hour: bool,
}

impl ClockSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load() -> Result<Self> {
        if ALLIUM_CLOCK_SETTINGS.exists() {
            debug!("found state, loading from file");
            if let Ok(json) = fs::read_to_string(ALLIUM_CLOCK_SETTINGS.as_path())
                && let Ok(json) = serde_json::from_str(&json)
            {
                return Ok(json);
            }
            warn!("failed to read state file, removing");
            fs::remove_file(ALLIUM_CLOCK_SETTINGS.as_path())?;
        }
        Ok(Self::new())
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string(&self).unwrap();
        File::create(ALLIUM_CLOCK_SETTINGS.as_path())?.write_all(json.as_bytes())?;
        Ok(())
    }
}
