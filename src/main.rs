use image::{ImageBuffer, RgbaImage};
use rsdiff::{DiffOptions, diff_images};
use serde_json;
use std::env;
use std::path::Path;

#[derive(serde::Serialize)]
struct CliResult {
    success: bool,
    diff_count: u32,
    total_pixels: u32,
    diff_percentage: f64,
    output_path: Option<String>,
    error: Option<String>,
}

fn save_output_image(
    output_data: &[u8],
    width: u32,
    height: u32,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let img_buffer: RgbaImage = ImageBuffer::from_raw(width, height, output_data.to_vec())
        .ok_or("Failed to create image buffer from diff output")?;
    img_buffer.save(path)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse command line arguments
    if args.len() < 3 {
        let error_result = CliResult {
            success: false,
            diff_count: 0,
            total_pixels: 0,
            diff_percentage: 0.0,
            output_path: None,
            error: Some("Usage: rsdiff <image1> <image2> [options]".to_string()),
        };

        if args.contains(&"--json".to_string()) {
            println!("{}", serde_json::to_string(&error_result).unwrap());
        } else {
            eprintln!("Usage: {} <image1> <image2> [options]", args[0]);
            eprintln!("");
            eprintln!("Options:");
            eprintln!("  --output=<path>     Save diff output to specified path");
            eprintln!("  --=<path> Save diff output to specified path (alias)");
            eprintln!("  --json              Output results in JSON format");
            eprintln!("  --threshold=<value> Difference threshold (default: 0.1)");
            eprintln!("  --include-aa        Include anti-aliasing detection");
            eprintln!("  --alpha=<value>     Alpha value for output (default: 0.1)");
        }
        std::process::exit(1);
    }

    let img1_path = &args[1];
    let img2_path = &args[2];
    let json_output = args.contains(&"--json".to_string());

    // Parse options
    let mut threshold = 0.1;
    let mut include_aa = false;
    let mut alpha = 0.1;
    let mut output_path: Option<String> = None;

    for arg in &args[3..] {
        if arg.starts_with("--threshold=") {
            if let Ok(val) = arg.split('=').nth(1).unwrap_or("0.1").parse::<f32>() {
                threshold = val;
            }
        } else if arg.starts_with("--alpha=") {
            if let Ok(val) = arg.split('=').nth(1).unwrap_or("0.1").parse::<f32>() {
                alpha = val;
            }
        } else if arg.starts_with("--output=") {
            output_path = arg.split('=').nth(1).map(|s| s.to_string());
        } else if arg == "--include-aa" {
            include_aa = true;
        }
    }

    // Check if input files exist
    if !Path::new(img1_path).exists() {
        let error_result = CliResult {
            success: false,
            diff_count: 0,
            total_pixels: 0,
            diff_percentage: 0.0,
            output_path: None,
            error: Some(format!("Image 1 does not exist: {}", img1_path)),
        };

        if json_output {
            println!("{}", serde_json::to_string(&error_result).unwrap());
        } else {
            eprintln!("Error: Image 1 does not exist: {}", img1_path);
        }
        std::process::exit(1);
    }

    if !Path::new(img2_path).exists() {
        let error_result = CliResult {
            success: false,
            diff_count: 0,
            total_pixels: 0,
            diff_percentage: 0.0,
            output_path: None,
            error: Some(format!("Image 2 does not exist: {}", img2_path)),
        };

        if json_output {
            println!("{}", serde_json::to_string(&error_result).unwrap());
        } else {
            eprintln!("Error: Image 2 does not exist: {}", img2_path);
        }
        std::process::exit(1);
    }

    // Configure diff options
    let opts = DiffOptions {
        threshold,
        include_aa,
        alpha,
        aa_color: [255, 255, 0],   // Yellow for anti-aliased pixels
        diff_color: [255, 0, 255], // Magenta for different pixels
        diff_color_alt: None,
    };

    // Start timing

    // Compare the images
    match diff_images(img1_path, img2_path, Some(opts)) {
        Ok(result) => {
            let total_pixels = result.width * result.height;
            let diff_percentage = (result.diff_count as f64 / total_pixels as f64) * 100.0;

            // Save output if path is provided
            let final_output_path = if let Some(ref path) = output_path {
                match save_output_image(&result.output, result.width, result.height, path) {
                    Ok(_) => Some(path.clone()),
                    Err(e) => {
                        let error_result = CliResult {
                            success: false,
                            diff_count: 0,
                            total_pixels: 0,
                            diff_percentage: 0.0,
                            output_path: None,
                            error: Some(format!("Failed to save output: {}", e)),
                        };

                        if json_output {
                            println!("{}", serde_json::to_string(&error_result).unwrap());
                        } else {
                            eprintln!("Error: Failed to save output: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };

            let cli_result = CliResult {
                success: true,
                diff_count: result.diff_count,
                total_pixels,
                diff_percentage,
                output_path: final_output_path.clone(),
                error: None,
            };

            if json_output {
                println!("{}", serde_json::to_string(&cli_result).unwrap());
            } else {
                println!("Diff completed successfully!");
                println!("Image dimensions: {}x{}", result.width, result.height);
                println!("Different pixels: {}", result.diff_count);
                println!("Total pixels: {}", total_pixels);
                println!("Difference percentage: {:.2}%", diff_percentage);
                if let Some(path) = final_output_path {
                    println!("Output saved to: {}", path);
                }
            }
        }
        Err(e) => {
            let error_result = CliResult {
                success: false,
                diff_count: 0,
                total_pixels: 0,
                diff_percentage: 0.0,
                output_path: None,
                error: Some(e.to_string()),
            };

            if json_output {
                println!("{}", serde_json::to_string(&error_result).unwrap());
            } else {
                eprintln!("Error: {}", e);
            }
            std::process::exit(1);
        }
    }
}
