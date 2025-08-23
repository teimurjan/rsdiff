import * as fs from 'fs';
import { execSync } from 'child_process';
import { getBinaryPath } from './platform';

export interface ComparisonOptions {
  /** Difference threshold (0-1) */
  threshold?: number;
  /** Include anti-aliasing detection */
  includeAA?: boolean;
  /** Alpha value for output (0-1) */
  alpha?: number;
}

export interface ComparisonResult {
  success: boolean;
  diffCount: number;
  totalPixels: number;
  diffPercentage: number;
  width: number;
  height: number;
  durationMs: number;
  outputPath: string | null;
}

interface RustBinaryResult {
  success: boolean;
  error?: string;
  diff_count: number;
  total_pixels: number;
  diff_percentage: number;
  width: number;
  height: number;
  duration_ms: number;
  output_path?: string;
}

/**
 * Compare two images and generate a diff
 */
export async function compare(
  image1Path: string,
  image2Path: string,
  outputPath?: string,
  options: ComparisonOptions = {}
): Promise<ComparisonResult> {
  if (!fs.existsSync(image1Path)) {
    throw new Error(`Image 1 does not exist: ${image1Path}`);
  }
  if (!fs.existsSync(image2Path)) {
    throw new Error(`Image 2 does not exist: ${image2Path}`);
  }

  // Set default options
  const {
    threshold = 0.1,
    includeAA = false,
    alpha = 0.1
  } = options;

  try {
    // Get the appropriate binary for this platform
    const binaryPath = getBinaryPath();
    
    // Build command arguments
    const args = [
      `"${image1Path}"`,
      `"${image2Path}"`,
      '--json',
      `--threshold=${threshold}`,
      `--alpha=${alpha}`
    ];
    
    if (includeAA) {
      args.push('--include-aa');
    }
    
    if (outputPath) {
      args.push(`--output=${outputPath}`);
    }
    
    const command = `"${binaryPath}" ${args.join(' ')}`;
    
    // Execute the binary
    const result = execSync(command, {
      encoding: 'utf8',
      timeout: 30000
    });
    
    // Parse JSON result
    const jsonResult: RustBinaryResult = JSON.parse(result.trim());
    
    if (!jsonResult.success) {
      throw new Error(jsonResult.error || 'Unknown error from rsdiff');
    }
    
    return {
      success: true,
      diffCount: jsonResult.diff_count,
      totalPixels: jsonResult.total_pixels,
      diffPercentage: jsonResult.diff_percentage,
      width: jsonResult.width,
      height: jsonResult.height,
      durationMs: jsonResult.duration_ms,
      outputPath: jsonResult.output_path || null
    };
    
  } catch (error: any) {
    if (error.stdout) {
      // Try to parse error from stdout
      try {
        const errorResult = JSON.parse(error.stdout);
        if (errorResult.error) {
          throw new Error(errorResult.error);
        }
      } catch (parseError) {
        // Fall through to original error
      }
    }
    
    throw new Error(`Failed to compare images: ${error.message}`);
  }
}

/**
 * Get information about the current platform and binary
 */
export { getPlatformInfo } from './platform';

/**
 * Check if the binary exists for the current platform
 */
export function isBinaryAvailable(): boolean {
  try {
    getBinaryPath();
    return true;
  } catch (error) {
    return false;
  }
}

// Re-export platform utilities
export { getBinaryPath, getBinaryName, type PlatformInfo } from './platform';
