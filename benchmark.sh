#!/bin/bash

# CLI Benchmark for Image Diffing Tools
# Compares rsdiff, odiff, and pixelmatch using hyperfine

set -e

echo "🚀 CLI Benchmark: rsdiff vs odiff vs pixelmatch"
echo "================================================"

# Check if required tools are available
check_tool() {
    if ! command -v "$1" &> /dev/null; then
        echo "❌ Error: $1 is not installed or not in PATH"
        exit 1
    fi
    echo "✅ $1 found: $(command -v $1)"
}

echo "🔍 Checking required tools..."
check_tool "rsdiff"
check_tool "odiff"
check_tool "hyperfine"

BENCHMARK_OUTPUTS_DIR="$(pwd)/benchmark-outputs"
mkdir -p "$BENCHMARK_OUTPUTS_DIR"

IMAGES_DIR="$(pwd)/images"

# Find image pairs
echo "🖼️  Finding image pairs for comparison..."
IMAGE_PAIRS=()
for img1 in "$IMAGES_DIR"/*.png "$IMAGES_DIR"/*.jpg; do
    if [[ -f "$img1" ]]; then
        base_name=$(basename "$img1" | sed 's/\.[^.]*$//')
        # Look for corresponding image with -1 suffix
        img2="$IMAGES_DIR/${base_name%-1}.png"
        if [[ -f "$img2" && "$img1" != "$img2" ]]; then
            IMAGE_PAIRS+=("$img1:$img2")
        fi
    fi
done

if [ ${#IMAGE_PAIRS[@]} -eq 0 ]; then
    echo "❌ No image pairs found. Looking for any two images..."
    # Fallback: use first two images
    IMAGES=($(ls "$IMAGES_DIR"/*.png "$IMAGES_DIR"/*.jpg 2>/dev/null | head -2))
    if [ ${#IMAGES[@]} -ge 2 ]; then
        IMAGE_PAIRS+=("${IMAGES[0]}:${IMAGES[1]}")
    else
        echo "❌ No images found in $IMAGES_DIR"
        exit 1
    fi
fi

echo "📊 Found ${#IMAGE_PAIRS[@]} image pair(s):"
for pair in "${IMAGE_PAIRS[@]}"; do
    echo "   $(basename "${pair%:*}") ↔ $(basename "${pair#*:}")"
done

# Run benchmarks for each image pair
for pair in "${IMAGE_PAIRS[@]}"; do
    img1="${pair%:*}"
    img2="${pair#*:}"
    pair_name="$(basename "${img1%.*}")_vs_$(basename "${img2%.*}")"
    
    echo ""
    echo "🏃 Running benchmark for: $pair_name"
    echo "======================================"
    
    # Benchmark rsdiff
    echo "🦀 Benchmarking rsdiff..."
    hyperfine -i \
        --warmup 3 \
        --min-runs 50 \
        --max-runs 100 \
        --export-json "${BENCHMARK_OUTPUTS_DIR}/rsdiff_${pair_name}.json" \
        --export-markdown "${BENCHMARK_OUTPUTS_DIR}/rsdiff_${pair_name}.md" \
        "rsdiff \"$img1\" \"$img2\" --output rsdiff_${pair_name}_diff.png"
    
    # Benchmark odiff
    echo "🐌 Benchmarking odiff..."
    hyperfine -i \
        --warmup 3 \
        --min-runs 50 \
        --max-runs 100 \
        --export-json "${BENCHMARK_OUTPUTS_DIR}/odiff_${pair_name}.json" \
        --export-markdown "${BENCHMARK_OUTPUTS_DIR}/odiff_${pair_name}.md" \
        "node_modules/.bin/odiff --fail-on-layout=false \"$img1\" \"$img2\" odiff_${pair_name}_diff.png"
    
    # Benchmark pixelmatch
    echo "🔍 Benchmarking pixelmatch..."
    hyperfine -i \
        --warmup 3 \
        --min-runs 50 \
        --max-runs 100 \
        --export-json "${BENCHMARK_OUTPUTS_DIR}/pixelmatch_${pair_name}.json" \
        --export-markdown "${BENCHMARK_OUTPUTS_DIR}/pixelmatch_${pair_name}.md" \
        "node_modules/.bin/pixelmatch \"$img1\" \"$img2\" --output pixelmatch_${pair_name}_diff.png"
done
