use crate::matchers::Rect;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type Argb = u32;

pub mod color {
    use super::Argb;
    pub const RED: Argb = 0xFF_FF_45_3A;
    pub const GREEN: Argb = 0xFF_30_D1_5B;
    pub const BLUE: Argb = 0xFF_0A_84_FF;
    pub const YELLOW: Argb = 0xFF_FF_D6_0A;
    pub const TEAL: Argb = 0xFF_64_D2_FF;
    pub const WHITE: Argb = 0xFF_F2_F2_F7;
    pub const ORANGE: Argb = 0xFF_FF_9F_0A;
    pub const PURPLE: Argb = 0xFF_BF_5A_F2;
    pub const DARK_BG: Argb = 0xEE_1C_1C_1E;
    pub const CARD_BORDER: Argb = 0xFF_3A_3A_3C;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayMode {
    #[default]
    Passive,
    Marking,
    Search,
    Paused,
}

impl OverlayMode {
    pub fn toggle(self, target: OverlayMode) -> OverlayMode {
        if self == target {
            OverlayMode::Passive
        } else {
            target
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemCard {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub name: String,
    pub popularity: String,
    pub price: Option<u32>,
    pub match_score: f32,
    pub is_ng_plus: bool,
}

#[derive(Debug, Clone)]
pub struct DebugRect {
    pub rect: Rect,
    pub color: Argb,
    pub label: String,
    pub thickness: u32,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub prices: HashMap<String, u32>,
    pub ng_plus_prices: Option<HashMap<String, u32>>,
}

#[derive(Debug, Clone, Default)]
pub struct DrawListState {
    pub mode: OverlayMode,
    pub item_cards: Vec<ItemCard>,
    pub debug_rects: Vec<DebugRect>,
    pub marked_slots: Vec<Rect>,
    pub current_drag: Option<(u32, u32, u32, u32)>,
    pub redetect_requested: bool,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub is_ng_plus: bool,
}

pub type SharedDrawList = Arc<Mutex<DrawListState>>;

pub fn new_draw_list() -> SharedDrawList {
    Arc::new(Mutex::new(DrawListState::default()))
}
