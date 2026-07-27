use super::draw_list::{DrawListState, color};
use super::renderer::{Renderer, format_item_name};

pub struct SearchOverlay;

impl SearchOverlay {
    pub fn render_egui(ctx: &egui::Context, state: &DrawListState) {
        let title = if state.is_ng_plus {
            "ITEM SEARCH [NG+]"
        } else {
            "ITEM SEARCH"
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 100.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Query: ");
                    ui.label(&state.search_query);
                });
                ui.label("Type to search | ESC to close");
                ui.separator();

                if state.search_results.is_empty() && !state.search_query.is_empty() {
                    ui.label("No items found.");
                } else if !state.search_results.is_empty() {
                    egui::Grid::new("search_results_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.colored_label(egui::Color32::LIGHT_GRAY, "ITEM");
                            ui.colored_label(egui::Color32::LIGHT_RED, "LOW");
                            ui.colored_label(egui::Color32::YELLOW, "NORMAL");
                            ui.colored_label(egui::Color32::LIGHT_GREEN, "HIGH");
                            ui.end_row();

                            for result in state.search_results.iter().take(10) {
                                ui.label(format_item_name(&result.name));
                                let prices = if state.is_ng_plus {
                                    result.ng_plus_prices.as_ref().unwrap_or(&result.prices)
                                } else {
                                    &result.prices
                                };
                                let low = prices
                                    .get("low")
                                    .map(|p| format!("{p}g"))
                                    .unwrap_or_else(|| "---".into());
                                let normal = prices
                                    .get("normal")
                                    .map(|p| format!("{p}g"))
                                    .unwrap_or_else(|| "---".into());
                                let high = prices
                                    .get("high")
                                    .map(|p| format!("{p}g"))
                                    .unwrap_or_else(|| "---".into());

                                ui.label(low);
                                ui.label(normal);
                                ui.label(high);
                                ui.end_row();
                            }
                        });
                }
            });
    }

    pub fn render(renderer: &mut Renderer<'_>, state: &DrawListState) {
        // Semi-transparent dark overlay
        let dim_bg = 0x55_00_00_00;
        for y in 0..renderer.height {
            for x in 0..renderer.width {
                renderer.set_pixel(x, y, dim_bg);
            }
        }

        let panel_w = 500usize;
        let header_h = 70usize;
        let row_h = 28usize;
        let max_visible = 10usize;
        let result_count = state.search_results.len().min(max_visible);
        let results_h = if result_count > 0 {
            // Column header + rows
            24 + result_count * row_h + 16
        } else if !state.search_query.is_empty() {
            // "No results" message
            44
        } else {
            0
        };
        let panel_h = header_h + results_h;
        let panel_x = (renderer.width.saturating_sub(panel_w)) / 2;
        let panel_y = renderer.height / 5;

        // Accent color changes when NG+ is active
        let accent = if state.is_ng_plus {
            color::PURPLE
        } else {
            color::TEAL
        };

        // Main card
        renderer.draw_rounded_rect(
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            12,
            color::DARK_BG,
            Some(accent),
        );

        // Title
        let title = if state.is_ng_plus {
            "ITEM SEARCH [NG+]"
        } else {
            "ITEM SEARCH"
        };
        renderer.draw_text_smooth(panel_x + 20, panel_y + 10, title, accent, 14.0);

        // Hint text
        renderer.draw_text_smooth(
            panel_x + 200,
            panel_y + 12,
            "Type to search | ESC to close",
            0xAA_AA_AA_AA,
            11.0,
        );

        // Search input field background
        let input_x = panel_x + 16;
        let input_y = panel_y + 34;
        let input_w = panel_w - 32;
        let input_h = 28;
        renderer.draw_rounded_rect(
            input_x,
            input_y,
            input_w,
            input_h,
            6,
            0xCC_2C_2C_2E,
            Some(0xFF_58_58_5C),
        );

        // Query text with cursor
        let display_text = format!("{}|", &state.search_query);
        renderer.draw_text_smooth(input_x + 10, input_y + 6, &display_text, color::WHITE, 13.0);

        if state.search_query.is_empty() {
            // Placeholder
            renderer.draw_text_smooth(
                input_x + 20,
                input_y + 6,
                "Search item name...",
                0x77_88_88_88,
                13.0,
            );
        }

        // Results area
        let results_y = panel_y + header_h;

        if state.search_results.is_empty() && !state.search_query.is_empty() {
            renderer.draw_text_smooth(
                panel_x + 20,
                results_y + 10,
                "No items found.",
                0xCC_AA_AA_AA,
                12.0,
            );
            return;
        }

        if state.search_results.is_empty() {
            return;
        }

        // Column headers
        let col_name_x = panel_x + 20;
        let col_low_x = panel_x + 260;
        let col_norm_x = panel_x + 330;
        let col_high_x = panel_x + 410;
        let header_color = 0xCC_88_88_8C;

        renderer.draw_text_smooth(col_name_x, results_y, "ITEM", header_color, 11.0);
        renderer.draw_text_smooth(col_low_x, results_y, "LOW", color::RED, 11.0);
        renderer.draw_text_smooth(col_norm_x, results_y, "NORMAL", color::ORANGE, 11.0);
        renderer.draw_text_smooth(col_high_x, results_y, "HIGH", color::GREEN, 11.0);

        // Separator line
        let sep_y = results_y + 18;
        for x in (panel_x + 16)..(panel_x + panel_w - 16) {
            renderer.set_pixel(x, sep_y, 0x66_58_58_5C);
        }

        // Result rows
        for (i, result) in state.search_results.iter().take(max_visible).enumerate() {
            let row_y = sep_y + 6 + i * row_h;

            // Alternate row shading
            if i % 2 == 0 {
                for ry in row_y..(row_y + row_h).min(renderer.height) {
                    for rx in (panel_x + 4)..(panel_x + panel_w - 4) {
                        renderer.set_pixel(rx, ry, 0x22_FF_FF_FF);
                    }
                }
            }

            // Item name (formatted)
            let name = format_item_name(&result.name);
            let display_name = if name.len() > 28 {
                format!("{}...", &name[..25])
            } else {
                name
            };
            renderer.draw_text_smooth(col_name_x, row_y + 5, &display_name, color::WHITE, 12.0);

            // Pick the correct price map based on NG+ mode
            let prices = if state.is_ng_plus {
                result.ng_plus_prices.as_ref().unwrap_or(&result.prices)
            } else {
                &result.prices
            };

            let low = prices
                .get("low")
                .map(|p| format!("{p}g"))
                .unwrap_or_else(|| "---".into());
            let normal = prices
                .get("normal")
                .map(|p| format!("{p}g"))
                .unwrap_or_else(|| "---".into());
            let high = prices
                .get("high")
                .map(|p| format!("{p}g"))
                .unwrap_or_else(|| "---".into());

            let gold = 0xFF_FF_D7_00;
            renderer.draw_text_smooth(col_low_x, row_y + 5, &low, gold, 12.0);
            renderer.draw_text_smooth(col_norm_x, row_y + 5, &normal, gold, 12.0);
            renderer.draw_text_smooth(col_high_x, row_y + 5, &high, gold, 12.0);
        }

        if state.search_results.len() > max_visible {
            let more = state.search_results.len() - max_visible;
            let more_text = format!("...and {more} more");
            let more_y = sep_y + 6 + max_visible * row_h + 2;
            renderer.draw_text_smooth(col_name_x, more_y, &more_text, 0xAA_88_88_8C, 11.0);
        }
    }
}
