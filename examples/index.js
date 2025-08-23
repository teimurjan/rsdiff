const fs = require("fs");
const path = require("path");
const { compare: compareRsdiff } = require("@rsdiff/bin");

async function runRsdiffComparison(img1Path, img2Path) {
  try {
    // Use the @rsdiff/bin package
    const result = await compareRsdiff(img1Path, img2Path, null, {
      threshold: 0.1,
      includeAA: false,
      alpha: 0.1,
    });

    return {
      diffCount: result.diffCount,
      totalPixels: result.totalPixels,
      diffPercentage: result.diffPercentage,
      duration: result.durationMs,
      width: result.width,
      height: result.height,
    };
  } catch (error) {
    console.error("Error comparing images:", error);
    throw error;
  }
}

async function runBenchmark() {
  console.log("🚀 rsdiff Benchmark\n");

  const images = fs.readdirSync(path.resolve(__dirname, "../images")).sort();

  const testCases = images.reduce((acc, image, index) => {
    if (index % 2 === 0) {
      acc.push({
        name: image,
        image1: path.resolve(__dirname, "../images", image),
        image2: path.resolve(__dirname, "../images", images[index + 1]),
      });
    }

    return acc;
  }, []);

  console.log(`📊 Running ${testCases.length} test cases...\n`);

  // Run all rsdiff tests
  console.log("🦀 Running rsdiff tests...");
  console.log("=".repeat(50));

  const rsdiffResults = [];
  for (const testCase of testCases) {
    console.log(`\n📊 Testing: ${testCase.name}`);
    console.log(`   Image 1: ${testCase.image1}`);
    console.log(`   Image 2: ${testCase.image2}`);

    try {
      const rsdiffResult = await runRsdiffComparison(
        testCase.image1,
        testCase.image2
      );
      console.log(`   Time: ${rsdiffResult.duration.toFixed(2)}ms`);
      console.log(`   Different pixels: ${rsdiffResult.diffCount}`);
      console.log(`   Difference: ${rsdiffResult.diffPercentage.toFixed(2)}%`);
      console.log(
        `   Image dimensions: ${rsdiffResult.width}x${rsdiffResult.height}`
      );

      rsdiffResults.push({
        name: testCase.name,
        ...rsdiffResult,
      });
    } catch (error) {
      console.log(`   Error: ${error.message}`);
    }
  }

  // Calculate and display performance summary
  console.log("\n\n📈 Performance Summary:");
  console.log("=".repeat(50));

  if (rsdiffResults.length > 0) {
    const rsdiffAvgTime =
      rsdiffResults.reduce((sum, r) => sum + r.duration, 0) /
      rsdiffResults.length;

    const totalPixels = rsdiffResults.reduce(
      (sum, r) => sum + r.totalPixels,
      0
    );
    const avgDiffPercentage =
      rsdiffResults.reduce((sum, r) => sum + r.diffPercentage, 0) /
      rsdiffResults.length;

    console.log(`\n⏱️  Performance:`);
    console.log(`   Average time: ${rsdiffAvgTime.toFixed(2)}ms`);
    console.log(`   Total pixels processed: ${totalPixels.toLocaleString()}`);
    console.log(`   Average difference: ${avgDiffPercentage.toFixed(2)}%`);

    console.log(`\n📊 Test Results by Case:`);
    console.log(
      "   Name".padEnd(20) +
        "Time (ms)".padEnd(12) +
        "Diff Pixels".padEnd(15) +
        "Diff %".padEnd(10) +
        "Dimensions".padEnd(15)
    );
    console.log("   " + "-".repeat(80));

    for (const result of rsdiffResults) {
      console.log(
        `   ${result.name.padEnd(20)}${result.duration
          .toFixed(1)
          .padEnd(12)}${result.diffCount
          .toString()
          .padEnd(15)}${result.diffPercentage.toFixed(2).padEnd(10)}${
          result.width
        }x${result.height}`
      );
    }
  }

  console.log("\n📈 Benchmark Summary:");
  console.log("=".repeat(50));
  console.log("• rsdiff: Pure Rust implementation via @rsdiff/bin package");
  console.log("• Uses perceptually accurate color difference algorithms");
  console.log("• Optimized for performance and memory efficiency");
  console.log("• Supports various image formats and color spaces");
  console.log("• Performance may vary based on image size and complexity");

  // Clean up output files
  try {
    if (fs.existsSync("rsdiff_output.png")) fs.unlinkSync("rsdiff_output.png");
  } catch (e) {
    // Ignore cleanup errors
  }
}

runBenchmark().catch(console.error);
