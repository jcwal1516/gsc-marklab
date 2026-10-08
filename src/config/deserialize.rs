use std::path::Path;

use crate::errors::{MarklabError, Result};

use super::model::AnalysisConfig;

impl AnalysisConfig {
    pub fn from_toml_path(path: impl AsRef<Path>) -> Result<Self> {
        let path_ref = path.as_ref();
        let text = std::fs::read_to_string(path_ref)
            .map_err(|source| MarklabError::io(path_ref, source))?;
        let config = deserialize_toml(&text)?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_toml_overrides(text: &str) -> Result<Self> {
        let config = if text.trim().is_empty() {
            Self::default()
        } else {
            let default_text = toml::to_string(&Self::default())
                .map_err(|error| MarklabError::Config(error.to_string()))?;
            let mut merged = default_text
                .parse::<toml::Value>()
                .map_err(|error| MarklabError::Config(error.to_string()))?;
            let overrides = text
                .parse::<toml::Value>()
                .map_err(|error| MarklabError::Config(error.to_string()))?;
            merge_toml_value(&mut merged, overrides);
            let merged_text = toml::to_string(&merged)
                .map_err(|error| MarklabError::Config(error.to_string()))?;
            deserialize_toml(&merged_text)?
        };
        config.validate()?;
        Ok(config)
    }
}

fn deserialize_toml(text: &str) -> Result<AnalysisConfig> {
    let deserializer = toml::de::Deserializer::new(text);
    serde_path_to_error::deserialize(deserializer).map_err(|error| {
        let path = error.path().to_string();
        let detail = error.inner();
        if path.is_empty() {
            MarklabError::Config(detail.to_string())
        } else {
            MarklabError::Config(format!("{path}: {detail}"))
        }
    })
}

fn merge_toml_value(target: &mut toml::Value, source: toml::Value) {
    match (target, source) {
        (toml::Value::Table(target), toml::Value::Table(source)) => {
            for (key, value) in source {
                if let Some(existing) = target.get_mut(&key) {
                    merge_toml_value(existing, value);
                } else {
                    target.insert(key, value);
                }
            }
        }
        (target, source) => *target = source,
    }
}
