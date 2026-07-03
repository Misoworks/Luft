use crate::ConfigError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub primary: Option<String>,
    pub default_scale: f64,
    #[serde(flatten)]
    pub outputs: BTreeMap<String, OutputConfig>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            primary: None,
            default_scale: 1.0,
            outputs: BTreeMap::new(),
        }
    }
}

impl DisplayConfig {
    pub fn output_scale(&self, output: &str) -> f64 {
        self.outputs
            .get(output)
            .and_then(|config| config.scale)
            .unwrap_or(self.default_scale)
            .clamp(0.5, 4.0)
    }

    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        validate_scale("display.default_scale", self.default_scale)?;
        for (name, output) in &self.outputs {
            output.validate(name)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub enabled: bool,
    pub scale: Option<f64>,
    pub x: i32,
    pub y: i32,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub refresh_millihertz: Option<i32>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scale: None,
            x: 0,
            y: 0,
            width: None,
            height: None,
            refresh_millihertz: None,
        }
    }
}

impl OutputConfig {
    fn validate(&self, name: &str) -> Result<(), ConfigError> {
        if let Some(scale) = self.scale {
            validate_scale(&format!("display.{name}.scale"), scale)?;
        }
        if self.width.is_some_and(|width| width <= 0) {
            return Err(ConfigError::Validation(format!(
                "display.{name}.width must be greater than zero"
            )));
        }
        if self.height.is_some_and(|height| height <= 0) {
            return Err(ConfigError::Validation(format!(
                "display.{name}.height must be greater than zero"
            )));
        }
        if self.refresh_millihertz.is_some_and(|refresh| refresh <= 0) {
            return Err(ConfigError::Validation(format!(
                "display.{name}.refresh_millihertz must be greater than zero"
            )));
        }
        Ok(())
    }
}

fn validate_scale(field: &str, scale: f64) -> Result<(), ConfigError> {
    if !(0.5..=4.0).contains(&scale) {
        return Err(ConfigError::Validation(format!(
            "{field} must be between 0.5 and 4.0"
        )));
    }
    Ok(())
}
