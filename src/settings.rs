use serde::{Deserialize, Serialize};

const SETTINGS_PATH: &str = "delve_settings.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct UiSettings {
    pub reduced_flash: bool,
    pub high_contrast: bool,
    pub color_filter: u8,
    pub camera_easing: u8,
    pub head_bob: bool,
    pub low_motion: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            reduced_flash: false,
            high_contrast: false,
            color_filter: 0,
            camera_easing: 1,
            head_bob: true,
            low_motion: false,
        }
    }
}

impl UiSettings {
    pub fn load() -> Self {
        std::fs::read_to_string(SETTINGS_PATH)
            .ok()
            .and_then(|json| serde_json::from_str::<Self>(&json).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(self) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&self).map_err(std::io::Error::other)?;
        std::fs::write(SETTINGS_PATH, json)
    }

    fn sanitized(mut self) -> Self {
        self.color_filter %= crate::visual::PALETTE_FAMILY_COUNT;
        self.camera_easing %= 3;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_use_accessible_defaults() {
        let settings: UiSettings = serde_json::from_str(r#"{"reduced_flash":true}"#).unwrap();
        assert!(settings.reduced_flash);
        assert!(settings.head_bob);
        assert!(!settings.low_motion);
        assert_eq!(settings.camera_easing, 1);
    }

    #[test]
    fn invalid_cycle_values_are_sanitized() {
        let settings = UiSettings {
            color_filter: 9,
            camera_easing: 8,
            ..UiSettings::default()
        }
        .sanitized();
        assert_eq!(settings.color_filter, 3);
        assert_eq!(settings.camera_easing, 2);
    }
}
