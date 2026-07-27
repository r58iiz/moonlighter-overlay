use image::GrayImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MatcherAlgorithm {
    SAD,
    ZNCC,
    Chamfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionMode {
    Simd,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> u32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    pub rect: Rect,
    /// Normalised similarity score in [0.0, 1.0].
    /// 1.0 represents a perfect match.
    pub score: f32,
}

pub struct MatchTemplateParams<'a> {
    pub algorithm: MatcherAlgorithm,
    pub mode: ExecutionMode,
    pub mask: Option<&'a GrayImage>,
    pub sample_step: u32,
}

pub trait TemplateMatcher {
    fn match_at(
        &self,
        image: &[u8],
        img_stride: usize,
        tpl_data: &[u8],
        mask_data: Option<&[u8]>,
        tpl_w: usize,
        tpl_h: usize,
        ox: usize,
        oy: usize,
        step: usize,
        best_score: f32,
    ) -> f32;
}
