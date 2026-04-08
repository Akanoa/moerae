use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_max_indexable_tokens")]
    pub max_indexable_tokens: usize,
    #[serde(default = "default_segment_capacity")]
    pub segment_capacity: usize,
    #[serde(default = "default_segment_staleness")]
    pub segment_staleness: u64,
    #[serde(default = "default_max_segments")]
    pub max_segments: usize,
    #[serde(default = "default_max_persist_segments")]
    pub max_persist_segments: usize,
    #[serde(default = "default_max_cached_indexes")]
    pub max_cached_indexes: usize,
    #[serde(default = "default_base_score")]
    pub base_score: f32,
    #[serde(default = "default_hit_boost")]
    pub hit_boost: f32,
    #[serde(default = "default_promotion_threshold")]
    pub promotion_threshold: f32,
    #[serde(default = "default_search_limit")]
    pub default_search_limit: usize,
}

fn default_max_indexable_tokens() -> usize { 128 }
fn default_segment_capacity() -> usize { 100 }
fn default_segment_staleness() -> u64 { 3600 }
fn default_max_segments() -> usize { 50 }
fn default_max_persist_segments() -> usize { 100 }
fn default_max_cached_indexes() -> usize { 20 }
fn default_base_score() -> f32 { 0.3 }
fn default_hit_boost() -> f32 { 0.1 }
fn default_promotion_threshold() -> f32 { 0.7 }
fn default_search_limit() -> usize { 10 }

impl Default for Config {
    fn default() -> Self {
        Self {
            max_indexable_tokens: default_max_indexable_tokens(),
            segment_capacity: default_segment_capacity(),
            segment_staleness: default_segment_staleness(),
            max_segments: default_max_segments(),
            max_persist_segments: default_max_persist_segments(),
            max_cached_indexes: default_max_cached_indexes(),
            base_score: default_base_score(),
            hit_boost: default_hit_boost(),
            promotion_threshold: default_promotion_threshold(),
            default_search_limit: default_search_limit(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_indexable_tokens == 0 {
            return Err("max_indexable_tokens must be > 0".into());
        }
        if self.segment_capacity == 0 {
            return Err("segment_capacity must be > 0".into());
        }
        if self.segment_staleness == 0 {
            return Err("segment_staleness must be > 0".into());
        }
        if self.max_segments == 0 {
            return Err("max_segments must be > 0".into());
        }
        if self.max_persist_segments == 0 {
            return Err("max_persist_segments must be > 0".into());
        }
        if self.max_cached_indexes == 0 {
            return Err("max_cached_indexes must be > 0".into());
        }
        if !(0.0..=1.0).contains(&self.base_score) {
            return Err("base_score must be in 0.0..=1.0".into());
        }
        if self.hit_boost <= 0.0 {
            return Err("hit_boost must be > 0.0".into());
        }
        if !(0.0..=1.0).contains(&self.promotion_threshold) {
            return Err("promotion_threshold must be in 0.0..=1.0".into());
        }
        if self.default_search_limit == 0 {
            return Err("default_search_limit must be > 0".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn serde_roundtrip_with_defaults() {
        let json = "{}";
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_indexable_tokens, 128);
        assert_eq!(config.segment_capacity, 100);
        assert!((config.base_score - 0.3).abs() < f32::EPSILON);
        config.validate().unwrap();
    }

    #[test]
    fn serde_roundtrip_partial() {
        let json = r#"{"max_segments": 25}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_segments, 25);
        assert_eq!(config.segment_capacity, 100); // default
        config.validate().unwrap();
    }

    #[test]
    fn serde_full_roundtrip() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_indexable_tokens, restored.max_indexable_tokens);
        assert_eq!(config.segment_capacity, restored.segment_capacity);
    }

    #[test]
    fn validation_catches_zero_segment_capacity() {
        let mut config = Config::default();
        config.segment_capacity = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_catches_invalid_base_score() {
        let mut config = Config::default();
        config.base_score = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_catches_invalid_promotion_threshold() {
        let mut config = Config::default();
        config.promotion_threshold = -0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_catches_zero_hit_boost() {
        let mut config = Config::default();
        config.hit_boost = 0.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_catches_zero_staleness() {
        let mut config = Config::default();
        config.segment_staleness = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_catches_zero_search_limit() {
        let mut config = Config::default();
        config.default_search_limit = 0;
        assert!(config.validate().is_err());
    }
}
