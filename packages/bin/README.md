# @rsdiff/bin

Fast image diffing tool with cross-platform binaries. Compare images pixel-by-pixel with perceptual color difference algorithms.

## Features

- ⚡ **Blazing Fast** - Written in Rust, faster than most alternatives
- 🎯 **Perceptually Accurate** - Uses YIQ color space for better visual difference detection
- 🔍 **Anti-aliasing Detection** - Optional detection and highlighting of anti-aliased pixels
- 📦 **Cross-platform** - Works on macOS, Linux, and Windows
- 🚀 **Zero Dependencies** - Self-contained native binaries
- 📊 **Detailed Results** - Returns comprehensive comparison statistics

## Installation

```bash
npm install @rsdiff/bin
```

## Usage

### JavaScript API

```javascript
const { compare } = require('@rsdiff/bin');

async function compareImages() {
  const result = await compare(
    'path/to/image1.png',
    'path/to/image2.png',
    'path/to/output.png', // optional - if provided, diff image will be saved here
    {
      threshold: 0.1,      // optional, default: 0.1
      includeAA: false,    // optional, default: false
      alpha: 0.1           // optional, default: 0.1
    }
  );

  console.log(result);
  // {
  //   success: true,
  //   diffCount: 1234,
  //   totalPixels: 100000,
  //   diffPercentage: 1.23,
  //   width: 400,
  //   height: 250,
  //   durationMs: 45.67,
  //   outputPath: 'path/to/output.png' // actual path where diff was saved
  // }
}
```

### CLI Usage

```bash
# Via npx
npx @rsdiff/bin image1.png image2.png --json --output=diff.png

# If installed globally
npm install -g @rsdiff/bin
rsdiff image1.png image2.png --json --threshold=0.1 --output=diff.png
```

### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `outputPath` | string | null | Path to save the diff image (optional) |
| `threshold` | number | 0.1 | Difference threshold (0-1). Lower values are more sensitive |
| `includeAA` | boolean | false | Include anti-aliasing detection |
| `alpha` | number | 0.1 | Alpha value for output image generation (0-1) |

## API Reference

### `compare(image1Path, image2Path, outputPath?, options?)`

Compare two images and return detailed results.

**Parameters:**
- `image1Path` (string) - Path to the first image
- `image2Path` (string) - Path to the second image  
- `outputPath` (string, optional) - Path to save diff output image
- `options` (object, optional) - Comparison options

**Returns:** Promise<CompareResult>

```typescript
interface CompareResult {
  success: boolean;
  diffCount: number;
  totalPixels: number;
  diffPercentage: number;
  width: number;
  height: number;
  durationMs: number;
  outputPath: string | null;
}
```

### `getPlatformInfo()`

Get information about the current platform and binary.

**Returns:** PlatformInfo

```typescript
interface PlatformInfo {
  platform: string;
  arch: string;
  binaryName: string;
  binaryPath: string;
}
```

### `isBinaryAvailable()`

Check if the binary is available for the current platform.

**Returns:** boolean

## Supported Formats

- PNG
- JPEG
- TIFF
- And other formats supported by the Rust `image` crate

## Platform Support

| Platform | Architecture | Binary |
|----------|--------------|--------|
| macOS | x64 | ✅ |
| macOS | ARM64 (Apple Silicon) | ✅ |
| Linux | x64 | ✅ |
| Windows | x64 | ✅ |

## Performance

rsdiff is built in Rust and is typically faster than equivalent tools:

- **~1.8x faster** than odiff (C++)
- Efficient memory usage
- Multi-threaded where possible

## Error Handling

The library provides detailed error messages for common issues:

```javascript
try {
  const result = await compare('img1.png', 'img2.png');
} catch (error) {
  console.error('Comparison failed:', error.message);
  // Possible errors:
  // - "Image 1 does not exist: img1.png"
  // - "Images must have equal dimensions"
  // - "Unsupported image format"
}
```

## Development

To build the package locally:

```bash
# Clone the repository
git clone https://github.com/your-username/rsdiff
cd rsdiff/packages/bin

# Build the binary for your platform
npm run build

# Run tests
npm test
```

## License

MIT

## Contributing

Contributions are welcome! Please see the main repository for guidelines.
