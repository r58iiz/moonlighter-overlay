use crate::matchers::Rect;
use std::collections::HashMap;

pub const SHARED_ITEM: SubRect = SubRect {
    x: 111,
    y: 30,
    w: 75,
    h: 75,
};
pub const SHARED_POP: SubRect = SubRect {
    x: 214,
    y: 1,
    w: 54,
    h: 54,
};
pub const SHARED_PRICE: SubRect = SubRect {
    x: 205,
    y: 75,
    w: 90,
    h: 22,
};

#[derive(Debug, Clone, Copy)]
pub struct SubRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl SubRect {
    pub fn to_rect(self, slot: Rect) -> Rect {
        Rect {
            x: slot.x + self.x,
            y: slot.y + self.y,
            width: self.w,
            height: self.h,
        }
    }

    pub fn scaled(self, scale: f32) -> Self {
        Self {
            x: (self.x as f32 * scale).round() as u32,
            y: (self.y as f32 * scale).round() as u32,
            w: (self.w as f32 * scale).round() as u32,
            h: (self.h as f32 * scale).round() as u32,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CellRegions {
    pub slot: Rect,
    pub item: Rect,
    pub popularity: Rect,
    pub price: Rect,
}

impl CellRegions {
    pub fn from_slot(slot: Rect, scale: f32) -> Self {
        Self {
            slot,
            item: SHARED_ITEM.scaled(scale).to_rect(slot),
            popularity: SHARED_POP.scaled(scale).to_rect(slot),
            price: SHARED_PRICE.scaled(scale).to_rect(slot),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShopCoords {
    pub slots: Vec<Rect>,
    pub cells: Vec<CellRegions>,
    pub bbox: Rect,
}

impl ShopCoords {
    pub fn new(slots: Vec<Rect>, bbox: Rect, dpi_scale: f32) -> Self {
        let cells = slots
            .iter()
            .map(|&s| CellRegions::from_slot(s, dpi_scale))
            .collect();
        Self { slots, cells, bbox }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Popularity {
    Low,
    #[default]
    Normal,
    High,
}

impl Popularity {
    pub fn from_template_name(name: &str) -> Option<Self> {
        match name {
            "low" => Some(Self::Low),
            "normal" => Some(Self::Normal),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    pub fn as_key(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

impl std::fmt::Display for Popularity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_key())
    }
}

pub struct DetectedItem {
    pub slot_index: usize,
    pub name: String,
    pub match_score: f32,
    pub popularity: Popularity,
    pub detected_price: Option<u32>,
    pub prices: HashMap<String, u32>,
    pub item_rect: Rect,
    pub popularity_rect: Rect,
    pub price_rect: Rect,
}
