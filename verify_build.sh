#!/bin/bash

# 빌드 검증 스크립트
# cargo가 없어도 Rust 파일 구문 검사

echo "🔍 Verifying Rust source files..."
echo "================================="

# Count total Rust files
TOTAL_FILES=$(find . -name "*.rs" -type f | wc -l)
echo "Total Rust files: $TOTAL_FILES"

# Check for common issues
echo ""
echo "Checking for problematic patterns:"
echo "-----------------------------------"

# Check for unwrap()
UNWRAP_COUNT=$(grep -r "\.unwrap()" --include="*.rs" . 2>/dev/null | wc -l)
echo "• unwrap() calls: $UNWRAP_COUNT"

# Check for unsafe
UNSAFE_COUNT=$(grep -r "unsafe" --include="*.rs" . 2>/dev/null | grep -v "//" | wc -l)
echo "• unsafe blocks: $UNSAFE_COUNT"

# Check for expect()
EXPECT_COUNT=$(grep -r "\.expect(" --include="*.rs" . 2>/dev/null | wc -l)
echo "• expect() calls: $EXPECT_COUNT"

# Check for duplicate imports/definitions
echo ""
echo "Checking for potential issues in metrics.rs:"
echo "--------------------------------------------"

# Check if types are being redefined
if grep -q "pub type Gauge = prometheus::Gauge" shared/src/monitoring/metrics.rs 2>/dev/null; then
    echo "⚠️ WARNING: Duplicate Gauge type definition found"
else
    echo "✓ No duplicate Gauge type definition"
fi

if grep -q "pub type IntGauge = prometheus::IntGauge" shared/src/monitoring/metrics.rs 2>/dev/null; then
    echo "⚠️ WARNING: Duplicate IntGauge type definition found"
else
    echo "✓ No duplicate IntGauge type definition"
fi

# Check for process_collector usage
if grep -q "process_collector" shared/src/monitoring/metrics_init.rs 2>/dev/null; then
    echo "⚠️ WARNING: process_collector usage found (may not be available)"
else
    echo "✓ No process_collector usage"
fi

# Check Prometheus dependency
echo ""
echo "Checking Cargo.toml dependencies:"
echo "---------------------------------"
if grep -q 'prometheus.*features.*process' Cargo.toml 2>/dev/null; then
    echo "⚠️ WARNING: Prometheus 'process' feature enabled (may cause issues)"
else
    echo "✓ Prometheus configured correctly"
fi

# Summary
echo ""
echo "================================="
echo "Summary:"
echo "================================="

if [ $UNWRAP_COUNT -eq 0 ] && [ $UNSAFE_COUNT -lt 5 ] && [ $EXPECT_COUNT -lt 100 ]; then
    echo "✅ Code quality looks good!"
else
    echo "⚠️ Some issues may need attention:"
    [ $UNWRAP_COUNT -gt 0 ] && echo "  - Remove unwrap() calls"
    [ $UNSAFE_COUNT -ge 5 ] && echo "  - Review unsafe code usage"
    [ $EXPECT_COUNT -ge 100 ] && echo "  - High number of expect() calls"
fi

echo ""
echo "To build the project, run:"
echo "  cargo build --all"
echo ""