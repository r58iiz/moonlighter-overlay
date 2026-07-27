use std::sync::Arc;
use std::time::Duration;
use xcap::{Window, image::DynamicImage};

use crate::config::SharedConfig;
use crate::matchers::{MatchResult, MatchTemplateParams, Rect, match_template};
use crate::overlay::draw_list::{DebugRect, ItemCard, OverlayMode, SharedDrawList, color};

use super::{
    assets::ShopAssets,
    types::{DetectedItem, ShopCoords},
};

pub fn start_detection_loop(
    draw_list: SharedDrawList,
    config: SharedConfig,
    assets: Arc<ShopAssets>,
) {
    println!("[detect] starting detection loop for Moonlighter...");

    let mut cached_shop_coords: Option<ShopCoords> = None;
    let mut cached_window: Option<Window> = None;

    loop {
        let (delay_ms, is_paused, redetect, target_title) = {
            let cfg = config.lock().unwrap();
            let mut dl = draw_list.lock().unwrap();
            let redetect = dl.redetect_requested;
            if redetect {
                dl.redetect_requested = false;
                dl.item_cards.clear();
                dl.debug_rects.clear();
            }
            let is_paused = dl.mode == OverlayMode::Marking
                || dl.mode == OverlayMode::Search
                || dl.mode == OverlayMode::Paused;
            if dl.mode == OverlayMode::Paused {
                dl.item_cards.clear();
                dl.debug_rects.clear();
            }
            (
                cfg.detection_delay_ms.max(50),
                is_paused,
                redetect,
                cfg.target_window_title.clone(),
            )
        };

        if redetect {
            cached_shop_coords = None;
            cached_window = None;
            std::thread::sleep(Duration::from_millis(200));
        }

        if is_paused {
            std::thread::sleep(Duration::from_millis(delay_ms));
            continue;
        }

        if cached_window.is_none() {
            let matcher = crate::config::TitleMatcher::new(&target_title);
            let found = match Window::all() {
                Ok(wins) => wins
                    .into_iter()
                    .find(|w| w.title().map(|t| matcher.is_match(&t)).unwrap_or(false)),

                Err(e) => {
                    eprintln!("[detect] xcap window enumeration error: {e}");
                    None
                }
            };

            match found {
                Some(w) => {
                    println!("[detect] connected to target window '{target_title}'");
                    cached_window = Some(w);
                }
                None => {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    continue;
                }
            }
        }

        let Some(ref window) = cached_window else {
            continue;
        };

        let dpi_scale = window
            .current_monitor()
            .map(|m| m.scale_factor().unwrap_or(1.0))
            .unwrap_or(1.0);

        let custom_slots = {
            let cfg = config.lock().unwrap();
            cfg.marked_slots.clone()
        };

        let shop = if !custom_slots.is_empty() {
            let rects: Vec<Rect> = custom_slots.iter().map(|s| s.to_rect()).collect();
            let min_x = rects.iter().map(|r| r.x).min().unwrap_or(0);
            let min_y = rects.iter().map(|r| r.y).min().unwrap_or(0);
            let max_r = rects.iter().map(|r| r.right()).max().unwrap_or(100);
            let max_b = rects.iter().map(|r| r.bottom()).max().unwrap_or(100);
            let bbox = Rect::new(
                min_x,
                min_y,
                max_r.saturating_sub(min_x),
                max_b.saturating_sub(min_y),
            );
            Some(ShopCoords::new(rects, bbox, dpi_scale))
        } else if let Some(ref cached) = cached_shop_coords {
            Some(cached.clone())
        } else {
            match try_find_shop(window, dpi_scale, &assets, &config) {
                Ok(s) => {
                    if let Some(ref found) = s {
                        println!("[detect] locked shop grid anchors successfully.");
                        cached_shop_coords = Some(found.clone());
                    }
                    s
                }
                Err(e) => {
                    eprintln!("[detect] error searching for shop anchors: {e}");
                    None
                }
            }
        };

        if let Some(shop_coords) = shop {
            match detect_items(window, &shop_coords, &assets, &config, dpi_scale) {
                Ok(items) => {
                    let (is_debug, is_ng_plus) = {
                        let cfg = config.lock().unwrap();
                        (cfg.debug_mode, cfg.ng_plus_mode)
                    };

                    let mut dl = draw_list.lock().unwrap();
                    dl.item_cards.clear();
                    dl.debug_rects.clear();

                    for item in &items {
                        let cell = &shop_coords.cells[item.slot_index];
                        dl.item_cards.push(ItemCard {
                            x: cell.item.x,
                            y: cell.item.y,
                            width: cell.item.width,
                            height: cell.item.height,
                            name: item.name.clone(),
                            popularity: item.popularity.to_string(),
                            price: item.detected_price,
                            match_score: item.match_score,
                            is_ng_plus,
                        });
                    }

                    if is_debug {
                        dl.debug_rects.push(DebugRect {
                            rect: shop_coords.bbox,
                            color: color::ORANGE,
                            label: "SHOP BBOX".to_string(),
                            thickness: 2,
                        });

                        for (idx, cell) in shop_coords.cells.iter().enumerate() {
                            dl.debug_rects.push(DebugRect {
                                rect: cell.slot,
                                color: color::WHITE,
                                label: format!("Slot {idx}"),
                                thickness: 1,
                            });
                            dl.debug_rects.push(DebugRect {
                                rect: cell.item,
                                color: color::BLUE,
                                label: String::new(),
                                thickness: 1,
                            });
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[detect] item detection error: {e}");
                }
            }
        } else {
            let mut dl = draw_list.lock().unwrap();
            dl.item_cards.clear();
            dl.debug_rects.clear();
        }

        std::thread::sleep(Duration::from_millis(delay_ms));
    }
}

fn try_find_shop(
    window: &Window,
    dpi_scale: f32,
    assets: &ShopAssets,
    config: &SharedConfig,
) -> anyhow::Result<Option<ShopCoords>> {
    let gray = capture_gray_window(window)?;
    let anchors = &assets.anchors;

    let (anchor_thresh, slot_thresh) = {
        let cfg = config.lock().unwrap();
        match cfg.match_algorithm {
            crate::matchers::MatcherAlgorithm::SAD => (0.85, 0.70),
            crate::matchers::MatcherAlgorithm::ZNCC => (0.75, 0.60),
            crate::matchers::MatcherAlgorithm::Chamfer => (0.65, 0.55),
        }
    };

    let tr = match match_template_cfg(
        &gray,
        &anchors.top_right,
        Some(&anchors.top_right_mask),
        config,
    ) {
        Some(m) if m.score >= anchor_thresh => m.rect,
        _ => return Ok(None),
    };

    let bl = match match_template_cfg(
        &gray,
        &anchors.bottom_left,
        Some(&anchors.bottom_left_mask),
        config,
    ) {
        Some(m) if m.score >= anchor_thresh => m.rect,
        _ => return Ok(None),
    };

    let left = bl.x;
    let top = tr.y;
    let right = tr.right();
    let bottom = bl.bottom();

    if right <= left || bottom <= top {
        return Ok(None);
    }

    let bbox = Rect::new(left, top, right - left, bottom - top);
    let hw = bbox.width / 2;
    let top_h = bbox.height / 2;

    let mut slots = Vec::new();
    let top_quads = [
        Rect::new(bbox.x, bbox.y, hw, top_h),
        Rect::new(bbox.x + hw, bbox.y, hw, top_h),
    ];

    for quad in &top_quads {
        let crop_img = crop(&gray, *quad);
        if let Some(m) = match_template_cfg(
            &crop_img,
            &anchors.item_grid,
            Some(&anchors.item_grid_mask),
            config,
        ) && m.score >= slot_thresh
        {
            slots.push(Rect::new(
                m.rect.x + quad.x,
                m.rect.y + quad.y,
                m.rect.width,
                m.rect.height,
            ));
        }
    }

    let start_offset = (top_h as f32 * 0.75).round() as u32;
    let lower_start = (bbox.y + start_offset).min(bbox.y + bbox.height.saturating_sub(1));
    let bottom_h = (bbox.y + bbox.height).saturating_sub(lower_start);

    let bot_quads = [
        Rect::new(bbox.x, lower_start, hw, bottom_h),
        Rect::new(bbox.x + hw, lower_start, hw, bottom_h),
    ];

    for quad in &bot_quads {
        let crop_img = crop(&gray, *quad);
        if let Some(m) = match_template_cfg(
            &crop_img,
            &anchors.item_grid,
            Some(&anchors.item_grid_mask),
            config,
        ) && m.score >= slot_thresh
        {
            slots.push(Rect::new(
                m.rect.x + quad.x,
                m.rect.y + quad.y,
                m.rect.width,
                m.rect.height,
            ));
        }
    }

    if slots.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ShopCoords::new(slots, bbox, dpi_scale)))
    }
}

fn detect_items(
    window: &Window,
    shop: &ShopCoords,
    assets: &ShopAssets,
    config: &SharedConfig,
    _dpi_scale: f32,
) -> anyhow::Result<Vec<DetectedItem>> {
    let gray = capture_gray_window(window)?;
    let mut detected = Vec::new();

    let item_threshold = {
        let cfg = config.lock().unwrap();
        match cfg.match_algorithm {
            crate::matchers::MatcherAlgorithm::SAD => 0.70,
            crate::matchers::MatcherAlgorithm::ZNCC => 0.60,
            crate::matchers::MatcherAlgorithm::Chamfer => 0.55,
        }
    };

    for (slot_index, cell) in shop.cells.iter().enumerate() {
        let pop_crop = crop(&gray, cell.popularity);
        let popularity = best_match_above(&pop_crop, &assets.pop_templates, 0.65, |t, img| {
            let (tmpl, mask) = t.scaled(1.0);
            match_template_cfg(img, &tmpl, Some(&mask), config)
        })
        .map(|(t, _)| t.popularity)
        .unwrap_or_default();

        let item_crop = crop(&gray, cell.item);

        let var = image_variance(&item_crop);
        if var < 500.0 {
            continue;
        }

        let best_item = best_match_above(
            &item_crop,
            &assets.item_templates,
            item_threshold,
            |t, img| {
                let (tmpl, mask) = t.scaled(1.0);
                let mask_pixel_count = mask.as_raw().iter().filter(|&&v| v > 0).count();
                match_template_cfg(img, &tmpl, Some(&mask), config).map(|mut m| {
                    let min_pixels: usize = 200;
                    if mask_pixel_count < min_pixels {
                        m.score *= mask_pixel_count as f32 / min_pixels as f32;
                    }
                    m
                })
            },
        );

        let Some((template, best)) = best_item else {
            continue;
        };

        let item_rect = Rect::new(
            best.rect.x + cell.item.x,
            best.rect.y + cell.item.y,
            best.rect.width,
            best.rect.height,
        );

        let ng_plus = {
            let cfg = config.lock().unwrap();
            cfg.ng_plus_mode
        };
        let prices = assets
            .price_table
            .lookup(&template.name)
            .cloned()
            .unwrap_or_default();
        let detected_price = assets
            .price_table
            .lookup_price(&template.name, popularity, ng_plus);

        detected.push(DetectedItem {
            slot_index,
            name: template.name.clone(),
            match_score: best.score,
            popularity,
            detected_price,
            prices,
            item_rect,
            popularity_rect: cell.popularity,
            price_rect: cell.price,
        });
    }

    Ok(detected)
}

fn capture_gray_window(window: &Window) -> anyhow::Result<image::GrayImage> {
    Ok(DynamicImage::ImageRgba8(window.capture_image()?).into_luma8())
}

fn crop(gray: &image::GrayImage, r: Rect) -> image::GrayImage {
    image::imageops::crop_imm(gray, r.x, r.y, r.width, r.height).to_image()
}

fn match_template_cfg(
    gray: &image::GrayImage,
    template: &image::GrayImage,
    mask: Option<&image::GrayImage>,
    config: &SharedConfig,
) -> Option<MatchResult> {
    let (algorithm, mode, sample_step) = {
        let cfg = config.lock().unwrap();
        (cfg.match_algorithm, cfg.execution_mode(), cfg.sample_step)
    };

    let (tw, th) = template.dimensions();
    let (gw, gh) = gray.dimensions();

    if gw == 0 || gh == 0 || tw == 0 || th == 0 {
        return None;
    }

    let (scaled_tmpl, scaled_mask_storage) = if tw > gw || th > gh {
        let scale_w = gw as f32 / tw as f32;
        let scale_h = gh as f32 / th as f32;
        let fit_scale = scale_w.min(scale_h);

        let nw = ((tw as f32 * fit_scale).round() as u32).min(gw).max(1);
        let nh = ((th as f32 * fit_scale).round() as u32).min(gh).max(1);

        let st = image::imageops::resize(template, nw, nh, image::imageops::FilterType::Triangle);
        let sm =
            mask.map(|m| image::imageops::resize(m, nw, nh, image::imageops::FilterType::Nearest));
        (st, sm)
    } else {
        (template.clone(), mask.cloned())
    };

    let params = MatchTemplateParams {
        algorithm,
        mode,
        mask: scaled_mask_storage.as_ref(),
        sample_step,
    };

    match_template(gray, &scaled_tmpl, params)
}

fn best_match_above<'a, T>(
    image: &image::GrayImage,
    templates: &'a [T],
    threshold: f32,
    match_fn: impl Fn(&T, &image::GrayImage) -> Option<MatchResult>,
) -> Option<(&'a T, MatchResult)> {
    templates
        .iter()
        .filter_map(|t| match_fn(t, image).map(|m| (t, m)))
        .filter(|(_, m)| m.score >= threshold)
        .max_by(|(_, a), (_, b)| a.score.partial_cmp(&b.score).unwrap())
}

/// Empty slots will have very low variance (uniform background color).
fn image_variance(img: &image::GrayImage) -> f64 {
    let pixels = img.as_raw();
    let n = pixels.len() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let sum: f64 = pixels.iter().map(|&p| p as f64).sum();
    let mean = sum / n;
    let var: f64 = pixels
        .iter()
        .map(|&p| {
            let d = p as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    var
}
