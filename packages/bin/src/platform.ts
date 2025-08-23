import * as os from 'os';
import * as path from 'path';
import * as fs from 'fs';

export interface PlatformInfo {
  platform: string;
  arch: string;
  binaryName: string;
  binaryPath: string;
}

/**
 * Detect the current platform and return the appropriate binary name
 */
export function getBinaryName(): string {
  const platform = os.platform();
  const arch = os.arch();
  
  switch (platform) {
    case 'win32':
      // Windows x64 (using MSVC target for better compatibility)
      return 'rsdiff-windows-x64.exe';
    
    case 'darwin':
      // macOS
      if (arch === 'arm64') {
        return 'rsdiff-macos-arm64';
      } else {
        return 'rsdiff-macos-x64';
      }
    
    case 'linux':
      // Linux supports both x64 and ARM64
      if (arch === 'arm64') {
        return 'rsdiff-linux-arm64';
      } else {
        return 'rsdiff-linux-x64';
      }
    
    default:
      throw new Error(`Unsupported platform: ${platform} ${arch}`);
  }
}

/**
 * Get the full path to the rsdiff binary for the current platform
 */
export function getBinaryPath(): string {
  const binaryName = getBinaryName();
  // When built, __dirname will be in dist/, so we need to go up two levels to reach binaries/
  const binariesDir = path.join(__dirname, '..', 'binaries');
  const binaryPath = path.join(binariesDir, binaryName);
  
  if (!fs.existsSync(binaryPath)) {
    throw new Error(
      `Binary not found for platform ${os.platform()} ${os.arch()}. ` +
      `Expected: ${binaryPath}`
    );
  }
  
  return binaryPath;
}

/**
 * Get platform info for debugging
 */
export function getPlatformInfo(): PlatformInfo {
  return {
    platform: os.platform(),
    arch: os.arch(),
    binaryName: getBinaryName(),
    binaryPath: getBinaryPath()
  };
}
