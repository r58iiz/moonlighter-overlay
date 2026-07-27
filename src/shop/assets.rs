use super::types::Popularity;
use ab_glyph::FontArc;
use anyhow::Context;
use font_loader::system_fonts;
use image::GrayImage;
use rust_embed::Embed;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Embed)]
#[folder = "assets/"]
#[exclude = "*.js"]
pub struct Asset;

#[derive(Debug, Deserialize, Default)]
pub struct ItemPriceTable {
    pub items: Vec<ItemPriceEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ItemPriceEntry {
    pub name: String,
    pub prices: HashMap<String, u32>,
    #[serde(default)]
    pub ng_plus_prices: Option<HashMap<String, u32>>,
}

fn normalize_name(s: &str) -> String {
    s.to_lowercase().replace([' ', '-'], "_")
}

impl ItemPriceTable {
    fn load() -> anyhow::Result<Self> {
        let mut table = ItemPriceTable::default();

        for path in Asset::iter() {
            if path.starts_with("items/")
                && path.ends_with("prices.json")
                && let Some(file) = Asset::get(&path)
            {
                match serde_json::from_slice::<ItemPriceTable>(&file.data) {
                    Ok(sub_table) => {
                        println!(
                            "[assets] loaded price table from '{path}' ({} items)",
                            sub_table.items.len()
                        );
                        table.items.extend(sub_table.items);
                    }
                    Err(e) => eprintln!("[assets] error parsing price file '{path}': {e}"),
                }
            }
        }

        Ok(table)
    }

    pub fn lookup_entry(&self, name: &str) -> Option<&ItemPriceEntry> {
        let norm_target = normalize_name(name);
        if norm_target.is_empty() {
            return None;
        }
        self.items
            .iter()
            .find(|e| !normalize_name(&e.name).is_empty() && normalize_name(&e.name) == norm_target)
    }

    pub fn lookup(&self, name: &str) -> Option<&HashMap<String, u32>> {
        self.lookup_entry(name).map(|e| &e.prices)
    }

    pub fn lookup_price(&self, name: &str, popularity: Popularity, ng_plus: bool) -> Option<u32> {
        let entry = self.lookup_entry(name)?;
        let key = popularity.as_key();

        if ng_plus
            && let Some(ng_prices) = &entry.ng_plus_prices
            && let Some(&p) = ng_prices.get(key)
        {
            return Some(p);
        }

        entry.prices.get(key).copied()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<&ItemPriceEntry> {
        let norm_query = normalize_name(query);
        if norm_query.is_empty() {
            return Vec::new();
        }
        self.items
            .iter()
            .filter(|e| normalize_name(&e.name).contains(&norm_query))
            .take(limit)
            .collect()
    }
}

pub struct ItemTemplate {
    pub name: String,
    pub template: GrayImage,
    pub mask: GrayImage,
}

impl ItemTemplate {
    pub fn scaled(&self, scale: f32) -> (GrayImage, GrayImage) {
        if (scale - 1.0).abs() < 1e-3 {
            return (self.template.clone(), self.mask.clone());
        }
        let nw = ((self.template.width() as f32 * scale).round() as u32).max(1);
        let nh = ((self.template.height() as f32 * scale).round() as u32).max(1);
        let st = image::imageops::resize(
            &self.template,
            nw,
            nh,
            image::imageops::FilterType::Triangle,
        );
        let sm = image::imageops::resize(&self.mask, nw, nh, image::imageops::FilterType::Nearest);
        (st, sm)
    }
}

pub struct PopularityTemplate {
    pub popularity: Popularity,
    pub template: GrayImage,
    pub mask: GrayImage,
}

impl PopularityTemplate {
    pub fn scaled(&self, scale: f32) -> (GrayImage, GrayImage) {
        if (scale - 1.0).abs() < 1e-3 {
            return (self.template.clone(), self.mask.clone());
        }
        let nw = ((self.template.width() as f32 * scale).round() as u32).max(1);
        let nh = ((self.template.height() as f32 * scale).round() as u32).max(1);
        let st = image::imageops::resize(
            &self.template,
            nw,
            nh,
            image::imageops::FilterType::Triangle,
        );
        let sm = image::imageops::resize(&self.mask, nw, nh, image::imageops::FilterType::Nearest);
        (st, sm)
    }
}

pub struct AnchorSet {
    pub item_grid: GrayImage,
    pub item_grid_mask: GrayImage,
    pub top_right: GrayImage,
    pub top_right_mask: GrayImage,
    pub bottom_left: GrayImage,
    pub bottom_left_mask: GrayImage,
}

pub struct ShopAssets {
    pub font: FontArc,
    pub item_templates: Vec<ItemTemplate>,
    pub pop_templates: Vec<PopularityTemplate>,
    pub anchors: AnchorSet,
    pub price_table: ItemPriceTable,
}

impl ShopAssets {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            font: load_font()?,
            item_templates: load_item_templates()?,
            pop_templates: load_popularity_templates()?,
            anchors: load_anchors()?,
            price_table: ItemPriceTable::load()?,
        })
    }

    pub fn dump_templates(&self) -> anyhow::Result<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let folder_name = format!("dump-{timestamp}");
        std::fs::create_dir_all(&folder_name)?;

        let mut count = 0;
        for item in &self.item_templates {
            let tmpl_path = format!("{folder_name}/{}_template.png", item.name);
            let mask_path = format!("{folder_name}/{}_mask.png", item.name);
            let _ = item.template.save(&tmpl_path);
            let _ = item.mask.save(&mask_path);
            count += 1;
        }

        for pop in &self.pop_templates {
            let tmpl_path = format!("{folder_name}/pop_{}_template.png", pop.popularity);
            let mask_path = format!("{folder_name}/pop_{}_mask.png", pop.popularity);
            let _ = pop.template.save(&tmpl_path);
            let _ = pop.mask.save(&mask_path);
        }

        let _ = self
            .anchors
            .item_grid
            .save(format!("{folder_name}/anchor_item_grid.png"));
        let _ = self
            .anchors
            .item_grid_mask
            .save(format!("{folder_name}/anchor_item_grid_mask.png"));
        let _ = self
            .anchors
            .top_right
            .save(format!("{folder_name}/anchor_top_right.png"));
        let _ = self
            .anchors
            .top_right_mask
            .save(format!("{folder_name}/anchor_top_right_mask.png"));
        let _ = self
            .anchors
            .bottom_left
            .save(format!("{folder_name}/anchor_bottom_left.png"));
        let _ = self
            .anchors
            .bottom_left_mask
            .save(format!("{folder_name}/anchor_bottom_left_mask.png"));

        println!("[dump] dumped {count} item templates and masks into directory '{folder_name}'");
        Ok(folder_name)
    }
}

fn load_font() -> anyhow::Result<FontArc> {
    let property = system_fonts::FontPropertyBuilder::new().monospace().build();
    let (font_data, _) = system_fonts::get(&property).context("no monospace system font found")?;
    FontArc::try_from_vec(font_data).context("system monospace font is not a valid font")
}

pub fn load_gray(relative_path: &str) -> anyhow::Result<GrayImage> {
    let file = Asset::get(relative_path)
        .ok_or_else(|| anyhow::anyhow!("embedded asset not found: {}", relative_path))?;
    image::load_from_memory(&file.data)
        .with_context(|| format!("failed to decode embedded image: {}", relative_path))
        .map(|i| i.into_luma8())
}

pub fn load_image_and_automask(relative_path: &str) -> anyhow::Result<(GrayImage, GrayImage)> {
    let file = Asset::get(relative_path)
        .ok_or_else(|| anyhow::anyhow!("embedded asset not found: {}", relative_path))?;
    let dyn_img = image::load_from_memory(&file.data)
        .with_context(|| format!("failed to decode embedded image: {}", relative_path))?;

    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut gray = GrayImage::new(w, h);
    let mut mask = GrayImage::new(w, h);

    let margin = 4u32;
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;
    let mut count = 0u64;

    for y in 0..h {
        for x in 0..w {
            let is_border_zone = x < margin || x + margin >= w || y < margin || y + margin >= h;
            if is_border_zone {
                let p = rgba.get_pixel(x, y);
                if p[3] == 255 {
                    sum_r += p[0] as u64;
                    sum_g += p[1] as u64;
                    sum_b += p[2] as u64;
                    count += 1;
                }
            }
        }
    }

    let bg_color = if count > 0 {
        Some((
            (sum_r / count) as i32,
            (sum_g / count) as i32,
            (sum_b / count) as i32,
        ))
    } else {
        None
    };

    let bg_tolerance_sq = 45 * 45;

    for (x, y, p) in rgba.enumerate_pixels() {
        let r = p[0] as i32;
        let g = p[1] as i32;
        let b = p[2] as i32;
        let a = p[3];

        let luma = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
        gray.put_pixel(x, y, image::Luma([luma]));

        let mask_val = if a <= 30 {
            0
        } else if let Some((bg_r, bg_g, bg_b)) = bg_color {
            let dist_sq = (r - bg_r).pow(2) + (g - bg_g).pow(2) + (b - bg_b).pow(2);
            if dist_sq <= bg_tolerance_sq { 0 } else { 255 }
        } else {
            255
        };

        mask.put_pixel(x, y, image::Luma([mask_val]));
    }

    let mask = erode_mask(&mask, 2);
    let (gray, mask) = autocrop_to_mask(gray, mask);
    Ok((gray, mask))
}

pub fn erode_mask(mask: &GrayImage, radius: u32) -> GrayImage {
    if radius == 0 {
        return mask.clone();
    }
    let (w, h) = mask.dimensions();
    let mut eroded = GrayImage::new(w, h);
    let r = radius as i32;

    for y in 0..h {
        for x in 0..w {
            if mask.get_pixel(x, y)[0] == 0 {
                continue;
            }
            let mut keep = true;
            'outer: for dy in -r..=r {
                for dx in -r..=r {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                        keep = false;
                        break 'outer;
                    }
                    if mask.get_pixel(nx as u32, ny as u32)[0] == 0 {
                        keep = false;
                        break 'outer;
                    }
                }
            }
            if keep {
                eroded.put_pixel(x, y, image::Luma([255]));
            }
        }
    }
    eroded
}

pub fn autocrop_to_mask(gray: GrayImage, mask: GrayImage) -> (GrayImage, GrayImage) {
    let (w, h) = mask.dimensions();
    let mut min_x = w;
    let mut max_x = 0;
    let mut min_y = h;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..h {
        for x in 0..w {
            if mask.get_pixel(x, y)[0] > 0 {
                found = true;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    if !found || min_x > max_x || min_y > max_y {
        return (gray, mask);
    }

    let crop_w = max_x - min_x + 1;
    let crop_h = max_y - min_y + 1;

    let cropped_gray = image::imageops::crop_imm(&gray, min_x, min_y, crop_w, crop_h).to_image();
    let cropped_mask = image::imageops::crop_imm(&mask, min_x, min_y, crop_w, crop_h).to_image();

    (cropped_gray, cropped_mask)
}

fn load_anchors() -> anyhow::Result<AnchorSet> {
    Ok(AnchorSet {
        item_grid: load_gray("anchors/item_grid/item_grid.png")?,
        item_grid_mask: load_gray("anchors/item_grid/item_grid_mask.png")?,
        top_right: load_gray("anchors/shop/top_right_corner.png")?,
        top_right_mask: load_gray("anchors/shop/top_right_corner_mask.png")?,
        bottom_left: load_gray("anchors/shop/bottom_left_corner.png")?,
        bottom_left_mask: load_gray("anchors/shop/bottom_left_corner_mask.png")?,
    })
}

fn collect_mask_paths(prefix: &str) -> Vec<String> {
    Asset::iter()
        .filter(|p| p.starts_with(prefix) && p.ends_with("_mask.png"))
        .map(|p| p.into_owned())
        .collect()
}

fn load_item_templates() -> anyhow::Result<Vec<ItemTemplate>> {
    let mut templates = Vec::new();
    let mut loaded_names = std::collections::HashSet::new();

    for mask_path in collect_mask_paths("items/") {
        let name = stem_without_mask(&mask_path)?;
        let tmpl_path = mask_path.replace("_mask.png", ".png");
        let gray = load_gray(&tmpl_path)?;
        let mask = load_gray(&mask_path)?;
        let mask = erode_mask(&mask, 2);
        let (gray, mask) = autocrop_to_mask(gray, mask);

        templates.push(ItemTemplate {
            name: name.to_owned(),
            template: gray,
            mask,
        });

        loaded_names.insert(name.to_string());
    }

    for path in Asset::iter().filter(|p| p.starts_with("items/")) {
        if path.ends_with("_mask.png") || path.ends_with(".json") {
            continue;
        }

        let filename = path.rsplit('/').next().unwrap_or(&path);
        let stem = match filename.rsplit_once('.') {
            Some((s, _)) => s,
            None => filename,
        };

        if loaded_names.contains(stem) {
            continue;
        }

        match load_image_and_automask(&path) {
            Ok((template, mask)) => {
                println!(
                    "[assets] loaded item template '{stem}' ({}x{}) from '{path}'",
                    template.width(),
                    template.height()
                );
                templates.push(ItemTemplate {
                    name: stem.to_string(),
                    template,
                    mask,
                });
                loaded_names.insert(stem.to_string());
            }
            Err(e) => eprintln!("[assets] failed to auto-load image '{path}': {e}"),
        }
    }

    println!(
        "[assets] total item templates loaded into memory: {}",
        templates.len()
    );
    Ok(templates)
}

fn load_popularity_templates() -> anyhow::Result<Vec<PopularityTemplate>> {
    let mut templates = Vec::new();
    for mask_path in collect_mask_paths("identifier/popularity/") {
        let name = stem_without_mask(&mask_path)?;
        let popularity = Popularity::from_template_name(name).ok_or_else(|| {
            anyhow::anyhow!(
                "unrecognised popularity template name {:?} — expected low / normal / high",
                name
            )
        })?;
        let tmpl_path = mask_path.replace("_mask.png", ".png");
        templates.push(PopularityTemplate {
            popularity,
            template: load_gray(&tmpl_path)?,
            mask: load_gray(&mask_path)?,
        });
    }
    Ok(templates)
}

fn stem_without_mask(path: &str) -> anyhow::Result<&str> {
    path.rsplit('/')
        .next()
        .and_then(|f| f.strip_suffix("_mask.png"))
        .ok_or_else(|| anyhow::anyhow!("unexpected mask path format: {}", path))
}
