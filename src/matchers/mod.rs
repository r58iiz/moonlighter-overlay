pub mod chamfer;
pub mod sad;
pub mod traits;
pub mod zncc;

use image::GrayImage;
use rayon::prelude::*;

pub use traits::{ExecutionMode, MatchResult, MatchTemplateParams, MatcherAlgorithm, Rect};

pub struct PreparedTemplate {
    pub width: usize,
    pub data: Vec<u8>,
    pub mask: Option<Vec<u8>>,
    pub sampled_cols: usize,
    pub sampled_rows: usize,
}

impl PreparedTemplate {
    pub fn new(template: &GrayImage, mask: Option<&GrayImage>, step: usize) -> Self {
        let (w, h) = template.dimensions();
        let (w, h) = (w as usize, h as usize);

        let tpl_raw = template.as_raw();
        let mask_raw = mask.map(|m| m.as_raw());

        let sampled_cols = w.div_ceil(step);
        let sampled_rows = h.div_ceil(step);

        let capacity = sampled_rows * sampled_cols;
        let mut data = Vec::with_capacity(capacity);
        let mut mask_data = mask_raw.map(|_| Vec::<u8>::with_capacity(capacity));

        let mut ty = 0usize;
        while ty < h {
            let row_start = ty * w;
            let mut tx = 0usize;
            while tx < w {
                data.push(tpl_raw[row_start + tx]);
                if let (Some(m), Some(md)) = (mask_raw, &mut mask_data) {
                    md.push(m[row_start + tx]);
                }
                tx += step;
            }
            ty += step;
        }

        Self {
            width: w,
            data,
            mask: mask_data,
            sampled_cols,
            sampled_rows,
        }
    }

    pub fn max_error(&self) -> u64 {
        match &self.mask {
            None => (self.sampled_rows * self.sampled_cols) as u64 * 255,
            Some(m) => m.iter().filter(|&&v| v > 0).count() as u64 * 255,
        }
    }
}

fn validate(
    image: &GrayImage,
    template: &GrayImage,
    params: &MatchTemplateParams<'_>,
) -> Option<(u32, u32, u32)> {
    let (tw, th) = template.dimensions();
    let (iw, ih) = image.dimensions();

    if tw == 0 || th == 0 || tw > iw || th > ih {
        return None;
    }
    if let Some(mask) = params.mask
        && mask.dimensions() != (tw, th)
    {
        return None;
    }

    let step = params.sample_step.max(1);
    Some((iw - tw + 1, ih - th + 1, step))
}

pub fn match_template(
    image: &GrayImage,
    template: &GrayImage,
    params: MatchTemplateParams<'_>,
) -> Option<MatchResult> {
    let (search_cols, search_rows, step) = validate(image, template, &params)?;
    let step = step as usize;

    let tpl = PreparedTemplate::new(template, params.mask, step);
    let max_error = tpl.max_error();

    let img = image.as_raw();
    let img_stride = image.width() as usize;

    let tpl_ref = &tpl;
    let img_ref: &[u8] = img;

    let (best_score, best_x, best_y) = (0..search_rows as usize)
        .into_par_iter()
        .map(|oy| {
            let mut row_best_score = -1.0f32;
            let mut row_best_err = u64::MAX;
            let mut row_best_x = 0usize;

            for ox in 0..search_cols as usize {
                let score = match (params.algorithm, params.mode) {
                    (MatcherAlgorithm::SAD, ExecutionMode::Normal) => {
                        let (err, sc) = sad::sad_normal(
                            img_ref,
                            img_stride,
                            tpl_ref,
                            ox,
                            oy,
                            step,
                            max_error,
                            row_best_err,
                        );
                        if err < row_best_err {
                            row_best_err = err;
                        }
                        sc
                    }
                    (MatcherAlgorithm::SAD, ExecutionMode::Simd) => {
                        let (err, sc) = sad::sad_simd(
                            img_ref,
                            img_stride,
                            tpl_ref,
                            ox,
                            oy,
                            step,
                            max_error,
                            row_best_err,
                        );
                        if err < row_best_err {
                            row_best_err = err;
                        }
                        sc
                    }
                    (MatcherAlgorithm::ZNCC, ExecutionMode::Normal) => {
                        zncc::zncc_normal(img_ref, img_stride, tpl_ref, ox, oy, step)
                    }
                    (MatcherAlgorithm::ZNCC, ExecutionMode::Simd) => {
                        zncc::zncc_simd(img_ref, img_stride, tpl_ref, ox, oy, step)
                    }
                    (MatcherAlgorithm::Chamfer, ExecutionMode::Normal) => {
                        let (err, sc) = chamfer::chamfer_normal(
                            img_ref,
                            img_stride,
                            tpl_ref,
                            ox,
                            oy,
                            step,
                            max_error,
                            row_best_err,
                        );
                        if err < row_best_err {
                            row_best_err = err;
                        }
                        sc
                    }
                    (MatcherAlgorithm::Chamfer, ExecutionMode::Simd) => {
                        let (err, sc) = chamfer::chamfer_simd(
                            img_ref,
                            img_stride,
                            tpl_ref,
                            ox,
                            oy,
                            step,
                            max_error,
                            row_best_err,
                        );
                        if err < row_best_err {
                            row_best_err = err;
                        }
                        sc
                    }
                };

                if score > row_best_score {
                    row_best_score = score;
                    row_best_x = ox;
                }
            }

            (row_best_score, row_best_x, oy)
        })
        .reduce(|| (-1.0f32, 0, 0), |a, b| if a.0 >= b.0 { a } else { b });

    if best_score < 0.0 {
        return None;
    }

    Some(MatchResult {
        rect: Rect {
            x: best_x as u32,
            y: best_y as u32,
            width: template.width(),
            height: template.height(),
        },
        score: best_score,
    })
}
