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

// Fixed-point scaling for integer luma calculations (16-bit precision)
const LUMA_SCALE: u32 = 65536;
const LUMA_Y_R: u32 = (Y_R * LUMA_SCALE as f32) as u32;
const LUMA_Y_G: u32 = (Y_G * LUMA_SCALE as f32) as u32;
const LUMA_Y_B: u32 = (Y_B * LUMA_SCALE as f32) as u32;

// Pre-computed threshold for luma gating (scaled to match our fixed point)
const LUMA_GATE_SCALE: f32 = 0.7; // Conservative gate to avoid missing diffs

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

    // Pre-compute luma gate threshold (scaled for integer comparison)
    let luma_gate_threshold = (opts.threshold * LUMA_GATE_SCALE * 255.0 * 255.0) as u32;

    // Pre-compute alpha blend factor
    let alpha_blend = opts.alpha;

    // Initialize luma ring buffers for both images
    let mut luma1_prev = vec![0u32; w];
    let mut luma1_curr = vec![0u32; w];
    let mut luma1_next = vec![0u32; w];
    let mut luma2_prev = vec![0u32; w];
    let mut luma2_curr = vec![0u32; w];
    let mut luma2_next = vec![0u32; w];

    // Pre-compute luma for first row
    if h > 0 {
        compute_luma_row(img1, 0, w, &mut luma1_curr);
        compute_luma_row(img2, 0, w, &mut luma2_curr);
    }

    // Process rows sequentially for better cache locality
    for y in 0..h {
        let row_offset = y * w * 4;

        // Pre-compute luma for next row if it exists
        if y + 1 < h {
            compute_luma_row(img1, y + 1, w, &mut luma1_next);
            compute_luma_row(img2, y + 1, w, &mut luma2_next);
        }

        // Process row with safe byte-level operations
        for x in 0..w {
            let pos = row_offset + x * 4;
            let pixel1 = load_pixel_u32(img1, pos);
            let pixel2 = load_pixel_u32(img2, pos);

            if pixel1 == pixel2 {
                // Fast path: equal pixels - write output in one u32 store
                let gray_pixel = create_gray_pixel_u32(img1, pos, alpha_blend);
                write_pixel_u32(&mut output, pos, gray_pixel);
                continue;
            }

            // Luma gate: check if we need full YIQ calculation
            let luma1 = luma1_curr[x];
            let luma2 = luma2_curr[x];
            let luma_diff = if luma1 > luma2 {
                luma1 - luma2
            } else {
                luma2 - luma1
            };

            if luma_diff > luma_gate_threshold {
                // Full YIQ calculation needed
                let delta = calculate_pixel_color_delta_fast(pixel1, pixel2);

                if delta > max_delta {
                    // Check if this is anti-aliasing
                    if opts.include_aa
                        && is_pixel_antialiased_with_luma(
                            x as i32,
                            y as i32,
                            w as i32,
                            h as i32,
                            &luma1_prev,
                            &luma1_curr,
                            &luma1_next,
                            &luma2_prev,
                            &luma2_curr,
                            &luma2_next,
                        )
                    {
                        write_color(&mut output, pos, &opts.aa_color);
                    } else {
                        write_color(&mut output, pos, &opts.diff_color);
                        diff_count += 1;
                    }
                } else {
                    let gray_pixel = create_gray_pixel_u32(img1, pos, alpha_blend);
                    write_pixel_u32(&mut output, pos, gray_pixel);
                }
            } else {
                // Luma gate passed - pixel is similar enough
                let gray_pixel = create_gray_pixel_u32(img1, pos, alpha_blend);
                write_pixel_u32(&mut output, pos, gray_pixel);
            }
        }

        // Rotate luma buffers for next iteration
        if y + 1 < h {
            std::mem::swap(&mut luma1_prev, &mut luma1_curr);
            std::mem::swap(&mut luma1_curr, &mut luma1_next);
            std::mem::swap(&mut luma2_prev, &mut luma2_curr);
            std::mem::swap(&mut luma2_curr, &mut luma2_next);
        }
    }

    DiffResult {
        diff_count,
        output,
        width,
        height,
    }
}

/// Compute luma for a single row using integer math
#[inline(always)]
fn compute_luma_row(img: &[u8], y: usize, width: usize, luma_buffer: &mut [u32]) {
    let row_offset = y * width * 4;
    for x in 0..width {
        let pos = row_offset + x * 4;
        let r = img[pos] as u32;
        let g = img[pos + 1] as u32;
        let b = img[pos + 2] as u32;
        let a = img[pos + 3] as u32;

        // Integer blend-to-white and luma calculation
        let luma = if a == 0 {
            // Transparent pixel -> white
            LUMA_Y_R + LUMA_Y_G + LUMA_Y_B
        } else if a == 255 {
            // Opaque pixel -> direct luma
            r * LUMA_Y_R + g * LUMA_Y_G + b * LUMA_Y_B
        } else {
            // Semi-transparent pixel -> blend with white
            let alpha = a as f32 / 255.0;
            let blend_r = (255.0 + (r as f32 - 255.0) * alpha) as u32;
            let blend_g = (255.0 + (g as f32 - 255.0) * alpha) as u32;
            let blend_b = (255.0 + (b as f32 - 255.0) * alpha) as u32;
            blend_r * LUMA_Y_R + blend_g * LUMA_Y_G + blend_b * LUMA_Y_B
        };

        luma_buffer[x] = luma;
    }
}

/// Create a gray pixel as u32 for fast output writing
#[inline(always)]
fn create_gray_pixel_u32(img: &[u8], pos: usize, alpha: f32) -> u32 {
    let r = img[pos] as f32;
    let g = img[pos + 1] as f32;
    let b = img[pos + 2] as f32;
    let a = img[pos + 3] as f32;

    // Calculate luma and apply alpha blend
    let y = r * Y_R + g * Y_G + b * Y_B;
    let alpha_factor = a / 255.0;
    let val = (255.0 + (y - 255.0) * alpha * alpha_factor)
        .max(0.0)
        .min(255.0) as u8;

    // Pack as RGBA u32
    (255u32 << 24) | ((val as u32) << 16) | ((val as u32) << 8) | (val as u32)
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

/// Antialiasing detection using pre-computed luma buffers
#[inline]
fn is_pixel_antialiased_with_luma(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    luma1_prev: &[u32],
    luma1_curr: &[u32],
    luma1_next: &[u32],
    luma2_prev: &[u32],
    luma2_curr: &[u32],
    luma2_next: &[u32],
) -> bool {
    // Early boundary check
    let is_edge = x == 0 || x == width - 1 || y == 0 || y == height - 1;

    let x0 = (x - 1).max(0);
    let y0 = (y - 1).max(0);
    let x1 = (x + 1).min(width - 1);
    let y1 = (y + 1).min(height - 1);

    let mut min_delta = u32::MAX;
    let mut max_delta = u32::MIN;
    let mut min_coord = (0, 0);
    let mut max_coord = (0, 0);
    let mut zeroes = if is_edge { 1 } else { 0 };

    let base_luma = luma1_curr[x as usize];

    // Unroll the 3x3 kernel loop for better performance
    for adj_y in y0..=y1 {
        let luma_buffer = if adj_y == y - 1 {
            luma1_prev
        } else if adj_y == y {
            luma1_curr
        } else {
            luma1_next
        };

        for adj_x in x0..=x1 {
            if zeroes >= 3 || (x == adj_x && y == adj_y) {
                continue;
            }

            let adj_luma = luma_buffer[adj_x as usize];

            if base_luma == adj_luma {
                zeroes += 1;
                if zeroes >= 3 {
                    return false;
                }
            } else {
                let delta = if base_luma > adj_luma {
                    base_luma - adj_luma
                } else {
                    adj_luma - base_luma
                };

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

    if zeroes >= 3 || min_delta == u32::MAX || max_delta == u32::MIN {
        return false;
    }

    // Check sibling colors using luma buffers
    let (min_x, min_y) = min_coord;
    let (max_x, max_y) = max_coord;

    (has_many_siblings_with_luma(
        min_x, min_y, width, height, luma1_prev, luma1_curr, luma1_next,
    ) || has_many_siblings_with_luma(
        max_x, max_y, width, height, luma1_prev, luma1_curr, luma1_next,
    )) && (has_many_siblings_with_luma(
        min_x, min_y, width, height, luma2_prev, luma2_curr, luma2_next,
    ) || has_many_siblings_with_luma(
        max_x, max_y, width, height, luma2_prev, luma2_curr, luma2_next,
    ))
}

#[inline]
fn has_many_siblings_with_luma(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    luma_prev: &[u32],
    luma_curr: &[u32],
    luma_next: &[u32],
) -> bool {
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

    let base_luma = if y == 0 {
        luma_curr[x as usize]
    } else if y == height - 1 {
        luma_prev[x as usize]
    } else {
        luma_curr[x as usize]
    };

    for adj_y in y0..=y1 {
        let luma_buffer = if adj_y == y - 1 {
            luma_prev
        } else if adj_y == y {
            luma_curr
        } else {
            luma_next
        };

        for adj_x in x0..=x1 {
            if zeroes >= 3 || (x == adj_x && y == adj_y) {
                continue;
            }

            if luma_buffer[adj_x as usize] == base_luma {
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
fn write_pixel_u32(out: &mut [u8], pos: usize, pixel: u32) {
    out[pos..pos + 4].copy_from_slice(&pixel.to_ne_bytes());
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
            img1.height(),
            img2.width(),
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
            img1.height(),
            img2.width(),
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
