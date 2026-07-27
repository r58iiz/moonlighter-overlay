use super::draw_list::{Argb, DebugRect, ItemCard, color};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};

pub struct Renderer<'a> {
    pub buf: &'a mut [u32],
    pub width: usize,
    pub height: usize,
    pub font: &'a FontArc,
}

impl<'a> Renderer<'a> {
    pub fn new(buf: &'a mut [u32], width: usize, height: usize, font: &'a FontArc) -> Self {
        Self {
            buf,
            width,
            height,
            font,
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Argb) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            let src_a = (color >> 24) & 0xFF;
            if src_a == 255 {
                self.buf[idx] = color;
            } else if src_a > 0 {
                let dst = self.buf[idx];
                let dst_a = (dst >> 24) & 0xFF;
                let dst_r = (dst >> 16) & 0xFF;
                let dst_g = (dst >> 8) & 0xFF;
                let dst_b = dst & 0xFF;

                let src_r = (color >> 16) & 0xFF;
                let src_g = (color >> 8) & 0xFF;
                let src_b = color & 0xFF;

                let inv_a = 255 - src_a;
                let out_r = (src_r * src_a + dst_r * inv_a) / 255;
                let out_g = (src_g * src_a + dst_g * inv_a) / 255;
                let out_b = (src_b * src_a + dst_b * inv_a) / 255;
                let out_a = src_a.max(dst_a);

                self.buf[idx] = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
            }
        }
    }

    /// Draws a solid rounded rectangle card with optional border and drop shadow.
    pub fn draw_rounded_rect(
        &mut self,
        rx: usize,
        ry: usize,
        rw: usize,
        rh: usize,
        radius: usize,
        fill_color: Argb,
        border_color: Option<Argb>,
    ) {
        if rw == 0 || rh == 0 {
            return;
        }

        // Draw subtle drop shadow first
        let shadow_color = 0x66_00_00_00;
        let shadow_off = 3;
        for y in 0..rh {
            for x in 0..rw {
                let px = rx + x + shadow_off;
                let py = ry + y + shadow_off;
                if is_inside_rounded(x, y, rw, rh, radius) {
                    self.set_pixel(px, py, shadow_color);
                }
            }
        }

        // Fill main card
        for y in 0..rh {
            for x in 0..rw {
                let px = rx + x;
                let py = ry + y;
                if is_inside_rounded(x, y, rw, rh, radius) {
                    self.set_pixel(px, py, fill_color);
                }
            }
        }

        // Optional border outline
        if let Some(bc) = border_color {
            for y in 0..rh {
                for x in 0..rw {
                    let px = rx + x;
                    let py = ry + y;
                    let on_edge = is_inside_rounded(x, y, rw, rh, radius)
                        && (!is_inside_rounded(x + 1, y, rw, rh, radius)
                            || x == 0
                            || !is_inside_rounded(x.saturating_sub(1), y, rw, rh, radius)
                            || !is_inside_rounded(x, y + 1, rw, rh, radius)
                            || y == 0
                            || !is_inside_rounded(x, y.saturating_sub(1), rw, rh, radius));
                    if on_edge {
                        self.set_pixel(px, py, bc);
                    }
                }
            }
        }
    }

    /// Renders a beautiful floating item card tooltip.
    pub fn draw_item_card(&mut self, card: &ItemCard) {
        let title_text = format_item_name(&card.name);
        let pop_text = card.popularity.to_uppercase();
        let price_text = match card.price {
            Some(p) => format!("{p}g"),
            None => "---".to_string(),
        };

        let font_size = 14.0;
        let text_w =
            (title_text.len() * 8 + pop_text.len() * 8 + price_text.len() * 8 + 40).max(120);
        let card_w = text_w;
        let card_h = 42;

        // Position card centered above item rectangle
        let card_x = (card.x as usize + card.width as usize / 2).saturating_sub(card_w / 2);
        let card_y = (card.y as usize).saturating_sub(card_h + 8);

        let pop_color = match card.popularity.to_lowercase().as_str() {
            "high" => color::GREEN,
            "low" => color::RED,
            _ => color::ORANGE,
        };

        // Glassmorphic dark card body with vibrant green accent border
        self.draw_rounded_rect(
            card_x,
            card_y,
            card_w,
            card_h,
            8,
            color::DARK_BG,
            Some(color::GREEN),
        );

        // Popularity badge pill inside card
        let badge_x = card_x + 8;
        let badge_y = card_y + 10;
        let badge_w = 40;
        let badge_h = 22;
        self.draw_rounded_rect(badge_x, badge_y, badge_w, badge_h, 4, pop_color, None);

        // Badge text inside chip
        self.draw_text_smooth(badge_x + 6, badge_y + 4, &pop_text, color::WHITE, 11.0);

        // Item title
        self.draw_text_smooth(
            card_x + 54,
            card_y + 4,
            &title_text,
            color::WHITE,
            font_size,
        );

        // Price text with gold accent color
        let gold_color = 0xFF_FF_D7_00;
        let price_display = if card.is_ng_plus {
            format!("[NG+] {price_text}")
        } else {
            price_text
        };
        self.draw_text_smooth(card_x + 54, card_y + 22, &price_display, gold_color, 12.0);
    }

    /// Renders a wireframe rectangle for debug mode.
    pub fn draw_debug_rect(&mut self, debug: &DebugRect) {
        let x = debug.rect.x as usize;
        let y = debug.rect.y as usize;
        let w = debug.rect.width as usize;
        let h = debug.rect.height as usize;
        let t = debug.thickness as usize;

        let x2 = (x + w).min(self.width);
        let y2 = (y + h).min(self.height);

        for row in y..(y + t).min(y2) {
            for col in x..x2 {
                self.set_pixel(col, row, debug.color);
            }
        }
        for row in y2.saturating_sub(t).max(y)..y2 {
            for col in x..x2 {
                self.set_pixel(col, row, debug.color);
            }
        }
        for row in y..y2 {
            for col in x..(x + t).min(x2) {
                self.set_pixel(col, row, debug.color);
            }
            for col in x2.saturating_sub(t).max(x)..x2 {
                self.set_pixel(col, row, debug.color);
            }
        }

        if !debug.label.is_empty() {
            self.draw_text_smooth(x + t + 2, y + t + 2, &debug.label, debug.color, 12.0);
        }
    }

    /// Anti-aliased text rendering using ab_glyph with subtle drop shadow for readability.
    pub fn draw_text_smooth(
        &mut self,
        px: usize,
        py: usize,
        text: &str,
        color: Argb,
        font_px: f32,
    ) {
        // Draw drop shadow first
        self.render_text_pass(px + 1, py + 1, text, 0xAA_00_00_00, font_px);
        // Main text
        self.render_text_pass(px, py, text, color, font_px);
    }

    fn render_text_pass(&mut self, px: usize, py: usize, text: &str, color: Argb, font_px: f32) {
        let scale = PxScale::from(font_px);
        let scaled = self.font.as_scaled(scale);

        let fg_r = ((color >> 16) & 0xFF) as u8;
        let fg_g = ((color >> 8) & 0xFF) as u8;
        let fg_b = (color & 0xFF) as u8;
        let fg_a = (color >> 24) & 0xFF;

        let mut cursor_x = px as f32;
        let baseline_y = py as f32 + scaled.ascent();
        let mut prev_glyph_id = None;

        for ch in text.chars() {
            let glyph_id = scaled.glyph_id(ch);
            if let Some(prev) = prev_glyph_id {
                cursor_x += scaled.kern(prev, glyph_id);
            }
            prev_glyph_id = Some(glyph_id);

            let glyph =
                glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));
            cursor_x += scaled.h_advance(glyph_id);

            if let Some(outlined) = self.font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    if coverage < 1.0 / 255.0 {
                        return;
                    }
                    let sx = bounds.min.x as isize + gx as isize;
                    let sy = bounds.min.y as isize + gy as isize;
                    if sx < 0 || sy < 0 {
                        return;
                    }
                    let sx = sx as usize;
                    let sy = sy as usize;
                    if sx >= self.width || sy >= self.height {
                        return;
                    }

                    let cov_a = (coverage * fg_a as f32 + 0.5) as u32;
                    let blended_color = (cov_a << 24)
                        | ((fg_r as u32) << 16)
                        | ((fg_g as u32) << 8)
                        | (fg_b as u32);
                    self.set_pixel(sx, sy, blended_color);
                });
            }
        }
    }
}

fn is_inside_rounded(x: usize, y: usize, w: usize, h: usize, r: usize) -> bool {
    if r == 0 {
        return true;
    }
    let r = r.min(w / 2).min(h / 2);
    if x < r && y < r {
        let dx = r - x;
        let dy = r - y;
        return dx * dx + dy * dy <= r * r;
    }
    if x >= w - r && y < r {
        let dx = x - (w - r - 1);
        let dy = r - y;
        return dx * dx + dy * dy <= r * r;
    }
    if x < r && y >= h - r {
        let dx = r - x;
        let dy = y - (h - r - 1);
        return dx * dx + dy * dy <= r * r;
    }
    if x >= w - r && y >= h - r {
        let dx = x - (w - r - 1);
        let dy = y - (h - r - 1);
        return dx * dx + dy * dy <= r * r;
    }
    true
}

pub fn format_item_name(name: &str) -> String {
    name.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct EguiRenderer;

impl EguiRenderer {
    pub fn render_item_cards(ctx: &egui::Context, cards: &[ItemCard]) {
        for (idx, card) in cards.iter().enumerate() {
            let card_x = (card.x as f32 + card.width as f32 / 2.0) - 60.0;
            let card_y = (card.y as f32 - 48.0).max(10.0);

            egui::Area::new(egui::Id::new(format!("item_card_{idx}")))
                .fixed_pos(egui::pos2(card_x, card_y))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(egui::Color32::from_black_alpha(235))
                        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(48, 209, 91)))
                        .corner_radius(8.0)
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let pop_color = match card.popularity.to_lowercase().as_str() {
                                    "high" => egui::Color32::from_rgb(48, 209, 91),
                                    "low" => egui::Color32::from_rgb(255, 69, 58),
                                    _ => egui::Color32::from_rgb(255, 159, 10),
                                };

                                ui.colored_label(pop_color, card.popularity.to_uppercase());

                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(format_item_name(&card.name))
                                            .strong()
                                            .color(egui::Color32::WHITE),
                                    );
                                    let price_str = match card.price {
                                        Some(p) => format!("{p}g"),
                                        None => "---".to_string(),
                                    };
                                    let price_display = if card.is_ng_plus {
                                        format!("[NG+] {price_str}")
                                    } else {
                                        price_str
                                    };
                                    ui.colored_label(
                                        egui::Color32::from_rgb(255, 215, 0),
                                        price_display,
                                    );
                                });
                            });
                        });
                });
        }
    }

    pub fn render_status_hud(ctx: &egui::Context, state: &super::draw_list::DrawListState) {
        egui::Area::new(egui::Id::new("status_hud"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(220))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(100, 100, 100),
                    ))
                    .corner_radius(6.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Overlay Mode:");
                            let mode_text = format!("{:?}", state.mode);
                            let mode_color = match state.mode {
                                super::draw_list::OverlayMode::Passive => egui::Color32::GREEN,
                                super::draw_list::OverlayMode::Marking => egui::Color32::YELLOW,
                                super::draw_list::OverlayMode::Search => egui::Color32::LIGHT_BLUE,
                                super::draw_list::OverlayMode::Paused => egui::Color32::RED,
                            };
                            ui.colored_label(mode_color, mode_text);

                            if state.is_ng_plus {
                                ui.separator();
                                ui.colored_label(
                                    egui::Color32::from_rgb(180, 100, 255),
                                    "[NG+ Mode]",
                                );
                            }
                        });
                    });
            });
    }
}
