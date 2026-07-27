use super::PreparedTemplate;

pub fn chamfer_normal(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
    max_error: u64,
    best_error: u64,
) -> (u64, f32) {
    let err = if let Some(mask) = tpl.mask.as_deref() {
        chamfer_normal_masked(img, img_stride, tpl, mask, ox, oy, step, best_error)
    } else {
        chamfer_normal_unmasked(img, img_stride, tpl, ox, oy, step, best_error)
    };

    let score = if max_error == 0 {
        1.0
    } else {
        1.0 - (err as f32 / max_error as f32).clamp(0.0, 1.0)
    };
    (err, score)
}

pub fn chamfer_simd(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
    max_error: u64,
    best_error: u64,
) -> (u64, f32) {
    let err = unsafe {
        if let Some(mask) = tpl.mask.as_deref() {
            chamfer_simd_masked(img, img_stride, tpl, mask, ox, oy, step, best_error)
        } else {
            chamfer_simd_unmasked(img, img_stride, tpl, ox, oy, step, best_error)
        }
    };

    let score = if max_error == 0 {
        1.0
    } else {
        1.0 - (err as f32 / max_error as f32).clamp(0.0, 1.0)
    };
    (err, score)
}

fn chamfer_normal_unmasked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
    best_error: u64,
) -> u64 {
    let mut total_dist: u64 = 0;

    for row in 0..tpl.sampled_rows {
        let img_row_start = (oy + row * step) * img_stride + ox;
        let tpl_row_start = row * tpl.sampled_cols;

        let img_row = &img[img_row_start..img_row_start + tpl.width];
        let tpl_row = &tpl.data[tpl_row_start..tpl_row_start + tpl.sampled_cols];

        for col in 0..tpl.sampled_cols {
            let t = tpl_row[col] as i32;
            let i = img_row[col * step] as i32;
            let diff = (i - t).abs();
            total_dist += (diff * diff / 255) as u64;
        }

        if total_dist >= best_error {
            return total_dist;
        }
    }

    total_dist
}

fn chamfer_normal_masked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    mask: &[u8],
    ox: usize,
    oy: usize,
    step: usize,
    best_error: u64,
) -> u64 {
    let mut total_dist: u64 = 0;

    for row in 0..tpl.sampled_rows {
        let img_row_start = (oy + row * step) * img_stride + ox;
        let tpl_row_start = row * tpl.sampled_cols;

        let img_row = &img[img_row_start..img_row_start + tpl.width];
        let tpl_row = &tpl.data[tpl_row_start..tpl_row_start + tpl.sampled_cols];
        let mask_row = &mask[tpl_row_start..tpl_row_start + tpl.sampled_cols];

        for col in 0..tpl.sampled_cols {
            if mask_row[col] > 0 {
                let t = tpl_row[col] as i32;
                let i = img_row[col * step] as i32;
                let diff = (i - t).abs();
                total_dist += (diff * diff / 255) as u64;
            }
        }

        if total_dist >= best_error {
            return total_dist;
        }
    }

    total_dist
}

#[cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature(enable = "avx2")
)]
unsafe fn chamfer_simd_unmasked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
    best_error: u64,
) -> u64 {
    chamfer_normal_unmasked(img, img_stride, tpl, ox, oy, step, best_error)
}

#[cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature(enable = "avx2")
)]
unsafe fn chamfer_simd_masked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    mask: &[u8],
    ox: usize,
    oy: usize,
    step: usize,
    best_error: u64,
) -> u64 {
    chamfer_normal_masked(img, img_stride, tpl, mask, ox, oy, step, best_error)
}
