use super::PreparedTemplate;

pub fn zncc_normal(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
) -> f32 {
    let raw_score = if let Some(mask) = tpl.mask.as_deref() {
        zncc_normal_masked(img, img_stride, tpl, mask, ox, oy, step)
    } else {
        zncc_normal_unmasked(img, img_stride, tpl, ox, oy, step)
    };

    // Map correlation [-1.0, 1.0] to similarity score [0.0, 1.0]
    ((raw_score + 1.0) / 2.0).clamp(0.0, 1.0)
}

pub fn zncc_simd(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
) -> f32 {
    let raw_score = unsafe {
        if let Some(mask) = tpl.mask.as_deref() {
            zncc_simd_masked(img, img_stride, tpl, mask, ox, oy, step)
        } else {
            zncc_simd_unmasked(img, img_stride, tpl, ox, oy, step)
        }
    };

    ((raw_score + 1.0) / 2.0).clamp(0.0, 1.0)
}

fn zncc_normal_unmasked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
) -> f32 {
    let count = tpl.sampled_rows * tpl.sampled_cols;
    if count == 0 {
        return -1.0;
    }

    let mut sum_i = 0.0f64;
    let mut sum_t = 0.0f64;

    for row in 0..tpl.sampled_rows {
        let img_row_start = (oy + row * step) * img_stride + ox;
        let tpl_row_start = row * tpl.sampled_cols;

        let img_row = &img[img_row_start..img_row_start + tpl.width];
        let tpl_row = &tpl.data[tpl_row_start..tpl_row_start + tpl.sampled_cols];

        for (col, &t) in tpl_row.iter().enumerate() {
            sum_i += img_row[col * step] as f64;
            sum_t += t as f64;
        }
    }

    let mean_i = sum_i / count as f64;
    let mean_t = sum_t / count as f64;

    let mut numerator = 0.0f64;
    let mut denom_i = 0.0f64;
    let mut denom_t = 0.0f64;

    for row in 0..tpl.sampled_rows {
        let img_row_start = (oy + row * step) * img_stride + ox;
        let tpl_row_start = row * tpl.sampled_cols;

        let img_row = &img[img_row_start..img_row_start + tpl.width];
        let tpl_row = &tpl.data[tpl_row_start..tpl_row_start + tpl.sampled_cols];

        for (col, &t) in tpl_row.iter().enumerate() {
            let i = img_row[col * step] as f64 - mean_i;
            let t = t as f64 - mean_t;

            numerator += i * t;
            denom_i += i * i;
            denom_t += t * t;
        }
    }

    let denom = (denom_i * denom_t).sqrt();
    if denom == 0.0 {
        -1.0
    } else {
        (numerator / denom) as f32
    }
}

fn zncc_normal_masked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    mask: &[u8],
    ox: usize,
    oy: usize,
    step: usize,
) -> f32 {
    let mut sum_i = 0.0f64;
    let mut sum_t = 0.0f64;
    let mut count = 0usize;

    for row in 0..tpl.sampled_rows {
        let img_row_start = (oy + row * step) * img_stride + ox;
        let tpl_row_start = row * tpl.sampled_cols;

        let img_row = &img[img_row_start..img_row_start + tpl.width];
        let tpl_row = &tpl.data[tpl_row_start..tpl_row_start + tpl.sampled_cols];
        let mask_row = &mask[tpl_row_start..tpl_row_start + tpl.sampled_cols];

        for col in 0..tpl.sampled_cols {
            if mask_row[col] > 0 {
                sum_i += img_row[col * step] as f64;
                sum_t += tpl_row[col] as f64;
                count += 1;
            }
        }
    }

    if count == 0 {
        return -1.0;
    }

    let mean_i = sum_i / count as f64;
    let mean_t = sum_t / count as f64;

    let mut numerator = 0.0f64;
    let mut denom_i = 0.0f64;
    let mut denom_t = 0.0f64;

    for row in 0..tpl.sampled_rows {
        let img_row_start = (oy + row * step) * img_stride + ox;
        let tpl_row_start = row * tpl.sampled_cols;

        let img_row = &img[img_row_start..img_row_start + tpl.width];
        let tpl_row = &tpl.data[tpl_row_start..tpl_row_start + tpl.sampled_cols];
        let mask_row = &mask[tpl_row_start..tpl_row_start + tpl.sampled_cols];

        for col in 0..tpl.sampled_cols {
            if mask_row[col] > 0 {
                let i = img_row[col * step] as f64 - mean_i;
                let t = tpl_row[col] as f64 - mean_t;

                numerator += i * t;
                denom_i += i * i;
                denom_t += t * t;
            }
        }
    }

    let denom = (denom_i * denom_t).sqrt();
    if denom == 0.0 {
        -1.0
    } else {
        (numerator / denom) as f32
    }
}

#[cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature(enable = "avx2")
)]
unsafe fn zncc_simd_unmasked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    ox: usize,
    oy: usize,
    step: usize,
) -> f32 {
    zncc_normal_unmasked(img, img_stride, tpl, ox, oy, step)
}

#[cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature(enable = "avx2")
)]
unsafe fn zncc_simd_masked(
    img: &[u8],
    img_stride: usize,
    tpl: &PreparedTemplate,
    mask: &[u8],
    ox: usize,
    oy: usize,
    step: usize,
) -> f32 {
    zncc_normal_masked(img, img_stride, tpl, mask, ox, oy, step)
}
