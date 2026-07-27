use crate::matchers::{ExecutionMode, MatcherAlgorithm, Rect};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub const CONFIG_PATH: &str = "moonlighter_overlay_config.toml";

#[derive(Debug, Clone)]
pub struct TitleMatcher {
    pattern: String,
    regex: Option<regex::Regex>,
}

impl TitleMatcher {
    pub fn new(pattern: &str) -> Self {
        let pattern = pattern.trim().to_string();
        let regex = regex::RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
            .ok();
        Self { pattern, regex }
    }

    pub fn is_match(&self, window_title: &str) -> bool {
        if self.pattern.is_empty() {
            return true;
        }
        if let Some(ref re) = self.regex {
            re.is_match(window_title)
        } else {
            window_title
                .to_lowercase()
                .contains(&self.pattern.to_lowercase())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSlotConfig {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CustomSlotConfig {
    pub fn to_rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn from_rect(r: Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub debug_mode: bool,
    pub match_algorithm: MatcherAlgorithm,
    pub use_simd: bool,
    pub sample_step: u32,
    pub detection_delay_ms: u64,
    pub render_delay_ms: u64,
    pub debounce_ms: u64,
    pub leader_key: String,
    pub mark_region_hotkey: String,
    pub toggle_debug_hotkey: String,
    pub detect_window_hotkey: String,
    pub exit_app_hotkey: String,
    pub toggle_ng_plus_hotkey: String,
    pub dump_templates_hotkey: String,
    pub manual_search_hotkey: String,
    pub pause_overlay_hotkey: String,
    pub target_window_title: String,
    pub ng_plus_mode: bool,
    pub marked_slots: Vec<CustomSlotConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            debug_mode: false,
            match_algorithm: MatcherAlgorithm::ZNCC,
            use_simd: true,
            sample_step: 1,
            detection_delay_ms: 500,
            render_delay_ms: 33,
            debounce_ms: 100,
            leader_key: "O".to_string(),
            mark_region_hotkey: "M".to_string(),
            toggle_debug_hotkey: "D".to_string(),
            detect_window_hotkey: "R".to_string(),
            exit_app_hotkey: "X".to_string(),
            toggle_ng_plus_hotkey: "N".to_string(),
            dump_templates_hotkey: "T".to_string(),
            manual_search_hotkey: "S".to_string(),
            pause_overlay_hotkey: "P".to_string(),
            target_window_title: "^Moonlighter$".to_string(),
            ng_plus_mode: false,
            marked_slots: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn execution_mode(&self) -> ExecutionMode {
        if self.use_simd {
            ExecutionMode::Simd
        } else {
            ExecutionMode::Normal
        }
    }

    pub fn load_or_default() -> Self {
        if Path::new(CONFIG_PATH).exists() {
            match fs::read_to_string(CONFIG_PATH) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(cfg) => {
                        println!("[config] loaded config from {}", CONFIG_PATH);
                        return cfg;
                    }
                    Err(e) => {
                        eprintln!("[config] error parsing {CONFIG_PATH}: {e}, using defaults")
                    }
                },
                Err(e) => eprintln!("[config] error reading {CONFIG_PATH}: {e}, using defaults"),
            }
        }
        let default_cfg = Self::default();
        let _ = default_cfg.save();
        default_cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(CONFIG_PATH, content).map_err(|e| e.to_string())?;
        println!("[config] saved settings to {}", CONFIG_PATH);
        Ok(())
    }
}

pub type SharedConfig = Arc<Mutex<AppConfig>>;

pub fn new_shared_config() -> SharedConfig {
    Arc::new(Mutex::new(AppConfig::load_or_default()))
}
