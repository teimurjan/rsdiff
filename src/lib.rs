#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};

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

/// Main diff function for RGBA images
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

    // Pre-compute alpha blend factor
    let alpha_blend = opts.alpha;

    // Process in chunks for better cache locality
    const CHUNK_SIZE: usize = 64; // Process 64 pixels at a time

    for chunk_y in (0..h).step_by(CHUNK_SIZE) {
        let chunk_h = (CHUNK_SIZE).min(h - chunk_y);

        for y in chunk_y..(chunk_y + chunk_h) {
            let row_offset = y * w * 4;

            // Process row with better memory access pattern
            for x in 0..w {
                let pos = row_offset + x * 4;

                // Load pixels once
                let pixel1 = load_pixel_u32(img1, pos);
                let pixel2 = load_pixel_u32(img2, pos);

                if pixel1 != pixel2 {
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
                } else {
                    draw_gray_pixel_fast(img1, pos, alpha_blend, &mut output);
                }
            }
        }
    }

    DiffResult {
        diff_count,
        output,
        width,
        height,
    }
}

/// Load pixel as u32 directly from byte array
#[inline(always)]
fn load_pixel_u32(img: &[u8], pos: usize) -> u32 {
    // Use native endianness for consistency
    u32::from_ne_bytes([img[pos], img[pos + 1], img[pos + 2], img[pos + 3]])
}

/// Optimized pixel color delta calculation
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

/// Calculate brightness delta for antialiasing detection
#[inline(always)]
fn calculate_brightness_delta_fast(pixel_a: u32, pixel_b: u32) -> f32 {
    // Extract and blend in one pass
    let a_a = ((pixel_a >> 24) & 0xFF) as f32;
    let a_b = ((pixel_a >> 16) & 0xFF) as f32;
    let a_g = ((pixel_a >> 8) & 0xFF) as f32;
    let a_r = (pixel_a & 0xFF) as f32;

    let b_a = ((pixel_b >> 24) & 0xFF) as f32;
    let b_b = ((pixel_b >> 16) & 0xFF) as f32;
    let b_g = ((pixel_b >> 8) & 0xFF) as f32;
    let b_r = (pixel_b & 0xFF) as f32;

    let y1 = if a_a == 0.0 {
        255.0 * (Y_R + Y_G + Y_B) // White pixel
    } else if a_a == 255.0 {
        a_r * Y_R + a_g * Y_G + a_b * Y_B
    } else {
        let alpha = a_a / 255.0;
        (255.0 + (a_r - 255.0) * alpha) * Y_R
            + (255.0 + (a_g - 255.0) * alpha) * Y_G
            + (255.0 + (a_b - 255.0) * alpha) * Y_B
    };

    let y2 = if b_a == 0.0 {
        255.0 * (Y_R + Y_G + Y_B) // White pixel
    } else if b_a == 255.0 {
        b_r * Y_R + b_g * Y_G + b_b * Y_B
    } else {
        let alpha = b_a / 255.0;
        (255.0 + (b_r - 255.0) * alpha) * Y_R
            + (255.0 + (b_g - 255.0) * alpha) * Y_G
            + (255.0 + (b_b - 255.0) * alpha) * Y_B
    };

    y1 - y2
}

/// Optimized antialiasing detection
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