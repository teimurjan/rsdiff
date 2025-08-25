#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};
use wide::{CmpEq, CmpGt, f32x4, f32x8, u32x4, u32x8};

/// Options for the diff algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOptions {
    pub threshold: f32,
    pub include_aa: bool,
    pub alpha: f32,
    pub aa_color: [u8; 3],
    pub diff_color: [u8; 3],
    pub diff_color_alt: Option<[u8; 3]>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            threshold: 0.1,
            include_aa: false,
            alpha: 0.1,
            aa_color: [255, 255, 0],   // yellow
            diff_color: [255, 0, 255], // magenta
            diff_color_alt: None,
        }
    }
}

/// Result of the diff operation
#[derive(Debug, Serialize, Deserialize)]
pub struct DiffResult {
    pub diff_count: u32,
    pub output: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// Pre-computed YIQ coefficients as constants
const Y_R: f32 = 0.29889531;
const Y_G: f32 = 0.58662247;
const Y_B: f32 = 0.11448223;

const I_R: f32 = 0.59597799;
const I_G: f32 = 0.27417610;
const I_B: f32 = 0.32180189;

const Q_R: f32 = 0.21147017;
const Q_G: f32 = 0.52261711;
const Q_B: f32 = 0.31114694;

const YIQ_Y_WEIGHT: f32 = 0.5053;
const YIQ_I_WEIGHT: f32 = 0.299;
const YIQ_Q_WEIGHT: f32 = 0.1957;

// SIMD constants - these will be initialized at runtime
#[inline(always)]
fn get_simd_constants() -> (
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
    f32x8,
) {
    (
        f32x8::splat(Y_R),          // SIMD_Y_R
        f32x8::splat(Y_G),          // SIMD_Y_G
        f32x8::splat(Y_B),          // SIMD_Y_B
        f32x8::splat(I_R),          // SIMD_I_R
        f32x8::splat(I_G),          // SIMD_I_G
        f32x8::splat(I_B),          // SIMD_I_B
        f32x8::splat(Q_R),          // SIMD_Q_R
        f32x8::splat(Q_G),          // SIMD_Q_G
        f32x8::splat(Q_B),          // SIMD_Q_B
        f32x8::splat(YIQ_Y_WEIGHT), // SIMD_YIQ_Y_WEIGHT
        f32x8::splat(YIQ_I_WEIGHT), // SIMD_YIQ_I_WEIGHT
        f32x8::splat(YIQ_Q_WEIGHT), // SIMD_YIQ_Q_WEIGHT
    )
}

/// Main diff function for RGBA images with SIMD optimization
pub fn diff_rgba(
    img1: &[u8],
    img2: &[u8],
    width: u32,
    height: u32,
    opts: Option<DiffOptions>,
) -> DiffResult {
    let opts = opts.unwrap_or_default();
    let w = width as usize;
    let h = height as usize;
    let total_pixels = w * h;

    // Pre-allocate output buffer
    let mut output = vec![0u8; total_pixels * 4];
    let mut diff_count = 0u32;

    // Pre-compute threshold
    let max_delta = 35215.0 * (opts.threshold * opts.threshold);
    let simd_max_delta = f32x8::splat(max_delta);

    // Pre-compute alpha blend factor
    let alpha_blend = opts.alpha;
    let simd_alpha_blend = f32x8::splat(alpha_blend);

    // Get SIMD constants
    let (
        simd_y_r,
        simd_y_g,
        simd_y_b,
        simd_i_r,
        simd_i_g,
        simd_i_b,
        simd_q_r,
        simd_q_g,
        simd_q_b,
        simd_yiq_y_weight,
        simd_yiq_i_weight,
        simd_yiq_q_weight,
    ) = get_simd_constants();

    // Process in SIMD-friendly chunks
    const SIMD_WIDTH: usize = 8; // Process 8 pixels at once

    for y in 0..h {
        let row_offset = y * w * 4;
        let mut x = 0;

        // Process 8 pixels at once with SIMD
        while x + SIMD_WIDTH <= w {
            let base_pos = row_offset + x * 4;

            // Load 8 pixels worth of data (32 bytes each image)
            let pixels1 = load_8_pixels_u32(img1, base_pos);
            let pixels2 = load_8_pixels_u32(img2, base_pos);

            // Check for exact matches first
            let exact_matches = pixels1.cmp_eq(pixels2);

            if exact_matches.all() {
                // All pixels match exactly, draw gray pixels
                draw_8_gray_pixels_fast(img1, base_pos, simd_alpha_blend, &mut output);
            } else {
                // Calculate color deltas for all 8 pixels
                let deltas = calculate_8_pixel_color_deltas_fast(
                    pixels1,
                    pixels2,
                    &simd_y_r,
                    &simd_y_g,
                    &simd_y_b,
                    &simd_i_r,
                    &simd_i_g,
                    &simd_i_b,
                    &simd_q_r,
                    &simd_q_g,
                    &simd_q_b,
                    &simd_yiq_y_weight,
                    &simd_yiq_i_weight,
                    &simd_yiq_q_weight,
                );

                // Compare with threshold
                let diff_mask = deltas.cmp_gt(simd_max_delta);

                // Process each pixel in the group
                for i in 0..SIMD_WIDTH {
                    let pixel_pos = base_pos + i * 4;
                    let is_exact_match = exact_matches.as_array_ref()[i] != 0;
                    let is_diff = diff_mask.as_array_ref()[i] != 0.0;

                    if is_exact_match {
                        draw_gray_pixel_fast(img1, pixel_pos, alpha_blend, &mut output);
                    } else if is_diff {
                        // Check if this is anti-aliasing
                        if opts.include_aa
                            && is_pixel_antialiased_optimized(
                                img1,
                                img2,
                                (x + i) as i32,
                                y as i32,
                                w as i32,
                                h as i32,
                            )
                        {
                            write_color(&mut output, pixel_pos, &opts.aa_color);
                        } else {
                            write_color(&mut output, pixel_pos, &opts.diff_color);
                            diff_count += 1;
                        }
                    } else {
                        draw_gray_pixel_fast(img1, pixel_pos, alpha_blend, &mut output);
                    }
                }
            }

            x += SIMD_WIDTH;
        }

        // Handle remaining pixels that don't fit in SIMD width
        while x < w {
            let pos = row_offset + x * 4;

            // Load pixels once
            let pixel1 = load_pixel_u32(img1, pos);
            let pixel2 = load_pixel_u32(img2, pos);

            if pixel1 == pixel2 {
                draw_gray_pixel_fast(img1, pos, alpha_blend, &mut output);
            } else {
                let delta = calculate_pixel_color_delta_fast(pixel1, pixel2);

                if delta > max_delta {
                    // Check if this is anti-aliasing
                    if opts.include_aa
                        && is_pixel_antialiased_optimized(
                            img1, img2, x as i32, y as i32, w as i32, h as i32,
                        )
                    {
                        write_color(&mut output, pos, &opts.aa_color);
                    } else {
                        write_color(&mut output, pos, &opts.diff_color);
                        diff_count += 1;
                    }
                } else {
                    draw_gray_pixel_fast(img1, pos, alpha_blend, &mut output);
                }
            }
            x += 1;
        }
    }

    DiffResult {
        diff_count,
        output,
        width,
        height,
    }
}

/// Load 8 consecutive pixels as u32x8
#[inline(always)]
fn load_8_pixels_u32(img: &[u8], base_pos: usize) -> u32x8 {
    let mut pixels = [0u32; 8];
    for i in 0..8 {
        let pos = base_pos + i * 4;
        pixels[i] = u32::from_ne_bytes([img[pos], img[pos + 1], img[pos + 2], img[pos + 3]]);
    }
    u32x8::from(pixels)
}

/// Calculate color deltas for 8 pixels using SIMD
#[inline]
fn calculate_8_pixel_color_deltas_fast(
    pixels_a: u32x8,
    pixels_b: u32x8,
    simd_y_r: &f32x8,
    simd_y_g: &f32x8,
    simd_y_b: &f32x8,
    simd_i_r: &f32x8,
    simd_i_g: &f32x8,
    simd_i_b: &f32x8,
    simd_q_r: &f32x8,
    simd_q_g: &f32x8,
    simd_q_b: &f32x8,
    simd_yiq_y_weight: &f32x8,
    simd_yiq_i_weight: &f32x8,
    simd_yiq_q_weight: &f32x8,
) -> f32x8 {
    let simd_255 = f32x8::splat(255.0);
    let simd_zero = f32x8::splat(0.0);

    // Extract RGBA components for all 8 pixels
    let mask_r = u32x8::splat(0xFF);
    let mask_g = u32x8::splat(0xFF00);
    let mask_b = u32x8::splat(0xFF0000);
    let mask_a = u32x8::splat(0xFF000000);

    let a_r_u32 = pixels_a & mask_r;
    let a_r = f32x8::new(a_r_u32.as_array_ref().map(|x| x as f32));
    let a_g_u32: u32x8 = (pixels_a & mask_g) >> 8;
    let a_g = f32x8::new(a_g_u32.as_array_ref().map(|x| x as f32));
    let a_b_u32: u32x8 = (pixels_a & mask_b) >> 16;
    let a_b = f32x8::new(a_b_u32.as_array_ref().map(|x| x as f32));
    let a_a_u32: u32x8 = (pixels_a & mask_a) >> 24;
    let a_a = f32x8::new(a_a_u32.as_array_ref().map(|x| x as f32));

    let b_r_u32 = pixels_b & mask_r;
    let b_r = f32x8::new(b_r_u32.as_array_ref().map(|x| x as f32));
    let b_g_u32: u32x8 = (pixels_b & mask_g) >> 8;
    let b_g = f32x8::new(b_g_u32.as_array_ref().map(|x| x as f32));
    let b_b_u32: u32x8 = (pixels_b & mask_b) >> 16;
    let b_b = f32x8::new(b_b_u32.as_array_ref().map(|x| x as f32));
    let b_a_u32: u32x8 = (pixels_b & mask_a) >> 24;
    let b_a = f32x8::new(b_a_u32.as_array_ref().map(|x| x as f32));

    // Alpha blending with white background for all pixels
    let alpha_a = a_a / simd_255;
    let alpha_b = b_a / simd_255;

    // Check for transparent pixels (alpha == 0)
    let transparent_a = a_a.cmp_eq(simd_zero);
    let opaque_a = a_a.cmp_eq(simd_255);
    let transparent_b = b_a.cmp_eq(simd_zero);
    let opaque_b = b_a.cmp_eq(simd_255);

    // Blend colors
    let r1 = transparent_a.blend(
        simd_255,
        opaque_a.blend(a_r, simd_255 + (a_r - simd_255) * alpha_a),
    );
    let g1 = transparent_a.blend(
        simd_255,
        opaque_a.blend(a_g, simd_255 + (a_g - simd_255) * alpha_a),
    );
    let b1 = transparent_a.blend(
        simd_255,
        opaque_a.blend(a_b, simd_255 + (a_b - simd_255) * alpha_a),
    );

    let r2 = transparent_b.blend(
        simd_255,
        opaque_b.blend(b_r, simd_255 + (b_r - simd_255) * alpha_b),
    );
    let g2 = transparent_b.blend(
        simd_255,
        opaque_b.blend(b_g, simd_255 + (b_g - simd_255) * alpha_b),
    );
    let b2 = transparent_b.blend(
        simd_255,
        opaque_b.blend(b_b, simd_255 + (b_b - simd_255) * alpha_b),
    );

    // Calculate YIQ differences
    let y_diff = (r1 * *simd_y_r + g1 * *simd_y_g + b1 * *simd_y_b)
        - (r2 * *simd_y_r + g2 * *simd_y_g + b2 * *simd_y_b);
    let i_diff = (r1 * *simd_i_r - g1 * *simd_i_g - b1 * *simd_i_b)
        - (r2 * *simd_i_r - g2 * *simd_i_g - b2 * *simd_i_b);
    let q_diff = (r1 * *simd_q_r - g1 * *simd_q_g + b1 * *simd_q_b)
        - (r2 * *simd_q_r - g2 * *simd_q_g + b2 * *simd_q_b);

    // Final weighted sum
    *simd_yiq_y_weight * y_diff * y_diff
        + *simd_yiq_i_weight * i_diff * i_diff
        + *simd_yiq_q_weight * q_diff * q_diff
}

/// Draw 8 gray pixels using SIMD
#[inline]
fn draw_8_gray_pixels_fast(img: &[u8], base_pos: usize, alpha_blend: f32x8, out: &mut [u8]) {
    let simd_255 = f32x8::splat(255.0);
    let simd_zero = f32x8::splat(0.0);
    let simd_y_r = f32x8::splat(Y_R);
    let simd_y_g = f32x8::splat(Y_G);
    let simd_y_b = f32x8::splat(Y_B);

    // Load 8 pixels worth of RGB data
    let mut r_vals = [0f32; 8];
    let mut g_vals = [0f32; 8];
    let mut b_vals = [0f32; 8];
    let mut a_vals = [0f32; 8];

    for i in 0..8 {
        let pos = base_pos + i * 4;
        r_vals[i] = img[pos] as f32;
        g_vals[i] = img[pos + 1] as f32;
        b_vals[i] = img[pos + 2] as f32;
        a_vals[i] = img[pos + 3] as f32;
    }

    let r_simd = f32x8::from(r_vals);
    let g_simd = f32x8::from(g_vals);
    let b_simd = f32x8::from(b_vals);
    let a_simd = f32x8::from(a_vals);

    // Calculate luma for all pixels
    let y_simd = r_simd * simd_y_r + g_simd * simd_y_g + b_simd * simd_y_b;

    // Apply alpha blending
    let alpha_norm = a_simd / simd_255;
    let val_simd = (simd_255 + (y_simd - simd_255) * alpha_blend * alpha_norm)
        .max(simd_zero)
        .min(simd_255);

    // Store results
    let results: [f32; 8] = val_simd.into();
    for i in 0..8 {
        let pos = base_pos + i * 4;
        let val = results[i] as u8;
        out[pos] = val;
        out[pos + 1] = val;
        out[pos + 2] = val;
        out[pos + 3] = 255;
    }
}

/// Load pixel as u32 directly from byte array (unchanged)
#[inline(always)]
fn load_pixel_u32(img: &[u8], pos: usize) -> u32 {
    u32::from_ne_bytes([img[pos], img[pos + 1], img[pos + 2], img[pos + 3]])
}

/// Optimized pixel color delta calculation (unchanged for fallback)
#[inline(always)]
fn calculate_pixel_color_delta_fast(pixel_a: u32, pixel_b: u32) -> f32 {
    // Extract components directly
    let a_a = ((pixel_a >> 24) & 0xFF) as f32;
    let a_b = ((pixel_a >> 16) & 0xFF) as f32;
    let a_g = ((pixel_a >> 8) & 0xFF) as f32;
    let a_r = (pixel_a & 0xFF) as f32;

    let b_a = ((pixel_b >> 24) & 0xFF) as f32;
    let b_b = ((pixel_b >> 16) & 0xFF) as f32;
    let b_g = ((pixel_b >> 8) & 0xFF) as f32;
    let b_r = (pixel_b & 0xFF) as f32;

    // Blend with white background inline
    let (r1, g1, b1) = if a_a == 0.0 {
        (255.0, 255.0, 255.0)
    } else if a_a == 255.0 {
        (a_r, a_g, a_b)
    } else {
        let alpha = a_a / 255.0;
        (
            255.0 + (a_r - 255.0) * alpha,
            255.0 + (a_g - 255.0) * alpha,
            255.0 + (a_b - 255.0) * alpha,
        )
    };

    let (r2, g2, b2) = if b_a == 0.0 {
        (255.0, 255.0, 255.0)
    } else if b_a == 255.0 {
        (b_r, b_g, b_b)
    } else {
        let alpha = b_a / 255.0;
        (
            255.0 + (b_r - 255.0) * alpha,
            255.0 + (b_g - 255.0) * alpha,
            255.0 + (b_b - 255.0) * alpha,
        )
    };

    // Calculate YIQ differences inline
    let y_diff = (r1 * Y_R + g1 * Y_G + b1 * Y_B) - (r2 * Y_R + g2 * Y_G + b2 * Y_B);
    let i_diff = (r1 * I_R - g1 * I_G - b1 * I_B) - (r2 * I_R - g2 * I_G - b2 * I_B);
    let q_diff = (r1 * Q_R - g1 * Q_G + b1 * Q_B) - (r2 * Q_R - g2 * Q_G + b2 * Q_B);

    YIQ_Y_WEIGHT * y_diff * y_diff + YIQ_I_WEIGHT * i_diff * i_diff + YIQ_Q_WEIGHT * q_diff * q_diff
}

/// Calculate brightness delta for antialiasing detection with SIMD optimization
#[inline(always)]
fn calculate_brightness_delta_fast(pixel_a: u32, pixel_b: u32) -> f32 {
    // Use SIMD for single pixel calculations too
    let pixels_a = u32x4::from([pixel_a, 0, 0, 0]);
    let pixels_b = u32x4::from([pixel_b, 0, 0, 0]);

    // Extract components
    let mask_r = u32x4::splat(0xFF);
    let mask_g = u32x4::splat(0xFF00);
    let mask_b = u32x4::splat(0xFF0000);
    let mask_a = u32x4::splat(0xFF000000);

    let a_r_u32 = pixels_a & mask_r;
    let a_r = f32x4::new(a_r_u32.as_array_ref().map(|x| x as f32));
    let a_g_u32: u32x4 = (pixels_a & mask_g) >> 8;
    let a_g = f32x4::new(a_g_u32.as_array_ref().map(|x| x as f32));
    let a_b_u32: u32x4 = (pixels_a & mask_b) >> 16;
    let a_b = f32x4::new(a_b_u32.as_array_ref().map(|x| x as f32));
    let a_a_u32: u32x4 = (pixels_a & mask_a) >> 24;
    let a_a = f32x4::new(a_a_u32.as_array_ref().map(|x| x as f32));

    let b_r_u32 = pixels_b & mask_r;
    let b_r = f32x4::new(b_r_u32.as_array_ref().map(|x| x as f32));
    let b_g_u32: u32x4 = (pixels_b & mask_g) >> 8;
    let b_g = f32x4::new(b_g_u32.as_array_ref().map(|x| x as f32));
    let b_b_u32: u32x4 = (pixels_b & mask_b) >> 16;
    let b_b = f32x4::new(b_b_u32.as_array_ref().map(|x| x as f32));
    let b_a_u32: u32x4 = (pixels_b & mask_a) >> 24;
    let b_a = f32x4::new(b_a_u32.as_array_ref().map(|x| x as f32));

    let simd_255 = f32x4::splat(255.0);
    let simd_zero = f32x4::splat(0.0);
    let simd_y_r = f32x4::splat(Y_R);
    let simd_y_g = f32x4::splat(Y_G);
    let simd_y_b = f32x4::splat(Y_B);

    // Alpha blending
    let alpha_a = a_a / simd_255;
    let alpha_b = b_a / simd_255;

    let transparent_a = a_a.cmp_eq(simd_zero);
    let opaque_a = a_a.cmp_eq(simd_255);
    let transparent_b = b_a.cmp_eq(simd_zero);
    let opaque_b = b_a.cmp_eq(simd_255);

    let white_luma = simd_255 * (simd_y_r + simd_y_g + simd_y_b);

    let y1 = transparent_a.blend(
        white_luma,
        opaque_a.blend(
            a_r * simd_y_r + a_g * simd_y_g + a_b * simd_y_b,
            (simd_255 + (a_r - simd_255) * alpha_a) * simd_y_r
                + (simd_255 + (a_g - simd_255) * alpha_a) * simd_y_g
                + (simd_255 + (a_b - simd_255) * alpha_a) * simd_y_b,
        ),
    );

    let y2 = transparent_b.blend(
        white_luma,
        opaque_b.blend(
            b_r * simd_y_r + b_g * simd_y_g + b_b * simd_y_b,
            (simd_255 + (b_r - simd_255) * alpha_b) * simd_y_r
                + (simd_255 + (b_g - simd_255) * alpha_b) * simd_y_g
                + (simd_255 + (b_b - simd_255) * alpha_b) * simd_y_b,
        ),
    );

    let result: [f32; 4] = (y1 - y2).into();
    result[0]
}

/// Optimized antialiasing detection (unchanged)
#[inline]
fn is_pixel_antialiased_optimized(
    base_img: &[u8],
    comp_img: &[u8],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> bool {
    // Early boundary check
    let is_edge = x == 0 || x == width - 1 || y == 0 || y == height - 1;

    let x0 = (x - 1).max(0);
    let y0 = (y - 1).max(0);
    let x1 = (x + 1).min(width - 1);
    let y1 = (y + 1).min(height - 1);

    let mut min_delta = f32::MAX;
    let mut max_delta = f32::MIN;
    let mut min_coord = (0, 0);
    let mut max_coord = (0, 0);
    let mut zeroes = if is_edge { 1 } else { 0 };

    let base_pos = ((y * width + x) * 4) as usize;
    let base_color = load_pixel_u32(base_img, base_pos);

    // Unroll the 3x3 kernel loop for better performance
    for adj_y in y0..=y1 {
        let row_offset = (adj_y * width * 4) as usize;
        for adj_x in x0..=x1 {
            if zeroes >= 3 || (x == adj_x && y == adj_y) {
                continue;
            }

            let adj_pos = row_offset + (adj_x * 4) as usize;
            let adjacent_color = load_pixel_u32(base_img, adj_pos);

            if base_color == adjacent_color {
                zeroes += 1;
                if zeroes >= 3 {
                    return false;
                }
            } else {
                let delta = calculate_brightness_delta_fast(base_color, adjacent_color);
                if delta < min_delta {
                    min_delta = delta;
                    min_coord = (adj_x, adj_y);
                }
                if delta > max_delta {
                    max_delta = delta;
                    max_coord = (adj_x, adj_y);
                }
            }
        }
    }

    if zeroes >= 3 || min_delta == f32::MAX || max_delta == f32::MIN {
        return false;
    }

    // Check sibling colors
    let (min_x, min_y) = min_coord;
    let (max_x, max_y) = max_coord;

    (has_many_siblings_optimized(base_img, min_x, min_y, width, height)
        || has_many_siblings_optimized(base_img, max_x, max_y, width, height))
        && (has_many_siblings_optimized(comp_img, min_x, min_y, width, height)
            || has_many_siblings_optimized(comp_img, max_x, max_y, width, height))
}

#[inline]
fn has_many_siblings_optimized(img: &[u8], x: i32, y: i32, width: i32, height: i32) -> bool {
    if x > width - 1 || y > height - 1 {
        return false;
    }

    let is_edge = x == 0 || x == width - 1 || y == 0 || y == height - 1;
    let mut zeroes = if is_edge { 1 } else { 0 };

    if zeroes >= 3 {
        return true;
    }

    let x0 = (x - 1).max(0);
    let y0 = (y - 1).max(0);
    let x1 = (x + 1).min(width - 1);
    let y1 = (y + 1).min(height - 1);

    let base_pos = ((y * width + x) * 4) as usize;
    let base_color = load_pixel_u32(img, base_pos);

    for adj_y in y0..=y1 {
        let row_offset = (adj_y * width * 4) as usize;
        for adj_x in x0..=x1 {
            if zeroes >= 3 || (x == adj_x && y == adj_y) {
                continue;
            }

            let adj_pos = row_offset + (adj_x * 4) as usize;
            if load_pixel_u32(img, adj_pos) == base_color {
                zeroes += 1;
                if zeroes >= 3 {
                    return true;
                }
            }
        }
    }

    false
}

#[inline(always)]
fn write_color(out: &mut [u8], pos: usize, color: &[u8; 3]) {
    out[pos] = color[0];
    out[pos + 1] = color[1];
    out[pos + 2] = color[2];
    out[pos + 3] = 255;
}

#[inline(always)]
fn draw_gray_pixel_fast(img: &[u8], i: usize, alpha: f32, out: &mut [u8]) {
    // Pre-compute luma using integer math where possible
    let y = (img[i] as f32 * Y_R + img[i + 1] as f32 * Y_G + img[i + 2] as f32 * Y_B) as u32;

    let a = img[i + 3] as f32 * (1.0 / 255.0); // Multiply by reciprocal
    let val = ((255.0 + (y as f32 - 255.0) * alpha * a).max(0.0).min(255.0)) as u8;

    out[i] = val;
    out[i + 1] = val;
    out[i + 2] = val;
    out[i + 3] = 255;
}

// === Image decoding and main diff functions ============================================================

/// Compare two images from file paths
pub fn diff_images<P: AsRef<std::path::Path>>(
    img1_path: P,
    img2_path: P,
    opts: Option<DiffOptions>,
) -> Result<DiffResult, Box<dyn std::error::Error>> {
    use image::io::Reader as ImageReader;

    // Load images in parallel if possible
    let img1 = ImageReader::open(img1_path)?.decode()?;
    let img2 = ImageReader::open(img2_path)?.decode()?;

    // Check dimensions before conversion
    if img1.width() != img2.width() || img1.height() != img2.height() {
        return Err(format!(
            "Images must have equal dimensions. Image 1: {:?}x{:?}, Image 2: {:?}x{:?}",
            img1.width(),
            img2.width(),
            img1.height(),
            img2.height()
        )
        .into());
    }

    // Convert to RGBA8
    let img1 = img1.to_rgba8();
    let img2 = img2.to_rgba8();

    let (w, h) = img1.dimensions();
    Ok(diff_rgba(img1.as_raw(), img2.as_raw(), w, h, opts))
}

/// Compare two images from byte data
pub fn diff_bytes(
    img1_bytes: &[u8],
    img2_bytes: &[u8],
    opts: Option<DiffOptions>,
) -> Result<DiffResult, Box<dyn std::error::Error>> {
    use image::io::Reader as ImageReader;

    let img1 = ImageReader::new(std::io::Cursor::new(img1_bytes))
        .with_guessed_format()?
        .decode()?;
    let img2 = ImageReader::new(std::io::Cursor::new(img2_bytes))
        .with_guessed_format()?
        .decode()?;

    // Check dimensions before conversion
    if img1.width() != img2.width() || img1.height() != img2.height() {
        return Err(format!(
            "Images must have equal dimensions. Image 1: {:?}x{:?}, Image 2: {:?}x{:?}",
            img1.width(),
            img2.width(),
            img1.height(),
            img2.height()
        )
        .into());
    }

    // Convert to RGBA8
    let img1 = img1.to_rgba8();
    let img2 = img2.to_rgba8();

    let (w, h) = img1.dimensions();
    Ok(diff_rgba(img1.as_raw(), img2.as_raw(), w, h, opts))
}
