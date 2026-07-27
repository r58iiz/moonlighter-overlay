use super::draw_list::{DrawListState, SharedDrawList, color};
use super::renderer::Renderer;
use crate::config::{CustomSlotConfig, SharedConfig};
use crate::matchers::Rect;

pub struct MarkingOverlay;

impl MarkingOverlay {
    pub fn render(renderer: &mut Renderer<'_>, state: &DrawListState) {
        let dim_bg = 0x55_00_00_00;
        for y in 0..renderer.height {
            for x in 0..renderer.width {
                renderer.set_pixel(x, y, dim_bg);
            }
        }

        let banner_w = 680;
        let banner_h = 44;
        let banner_x = (renderer.width.saturating_sub(banner_w)) / 2;
        let banner_y = 20;

        renderer.draw_rounded_rect(
            banner_x,
            banner_y,
            banner_w,
            banner_h,
            10,
            color::DARK_BG,
            Some(color::ORANGE),
        );

        let msg = format!(
            "MARKING MODE: {} slot(s) marked | Drag to Add | Right-Click to Remove | ENTER / F8 Save | C Clear | ESC Cancel",
            state.marked_slots.len()
        );
        renderer.draw_text_smooth(banner_x + 16, banner_y + 12, &msg, color::WHITE, 13.0);

        for (idx, slot) in state.marked_slots.iter().enumerate() {
            let label = format!("Slot {}", idx + 1);
            let debug = super::draw_list::DebugRect {
                rect: *slot,
                color: color::TEAL,
                label,
                thickness: 3,
            };
            renderer.draw_debug_rect(&debug);
        }

        if let Some((sx, sy, cx, cy)) = state.current_drag {
            let min_x = sx.min(cx);
            let min_y = sy.min(cy);
            let w = sx.abs_diff(cx);
            let h = sy.abs_diff(cy);

            if w > 0 && h > 0 {
                let drag_rect = Rect::new(min_x, min_y, w, h);
                let label = format!("{w}x{h}");
                let debug = super::draw_list::DebugRect {
                    rect: drag_rect,
                    color: color::YELLOW,
                    label,
                    thickness: 2,
                };
                renderer.draw_debug_rect(&debug);
            }
        }
    }

    pub fn save_marked_slots(draw_list: &SharedDrawList, config: &SharedConfig) {
        let slots = {
            let dl = draw_list.lock().unwrap();
            dl.marked_slots.clone()
        };

        {
            let mut cfg = config.lock().unwrap();
            cfg.marked_slots = slots.into_iter().map(CustomSlotConfig::from_rect).collect();
            if let Err(e) = cfg.save() {
                eprintln!("[marking] error saving config: {e}");
            } else {
                println!(
                    "[marking] saved {} slot(s) to config",
                    cfg.marked_slots.len()
                );
            }
        }
    }
}
