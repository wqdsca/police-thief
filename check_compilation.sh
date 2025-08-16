#!/bin/bash

# 컴파일 체크 스크립트
# Rust 코드의 일반적인 컴파일 문제를 검사합니다.

echo "🔍 Compilation Check for Police Thief Game Server"
echo "=================================================="
echo ""

# 1. Prometheus 관련 체크
echo "1️⃣ Checking Prometheus configuration..."
echo "-----------------------------------------"

# Check for duplicate type definitions
if grep -q "pub type Gauge = prometheus::Gauge" shared/src/monitoring/metrics.rs 2>/dev/null; then
    echo "❌ ERROR: Duplicate Gauge type definition found"
    exit 1
else
    echo "✅ No duplicate type definitions"
fi

# Check for process_collector usage
if grep -q "process_collector" shared/src/monitoring/metrics_init.rs 2>/dev/null; then
    echo "❌ ERROR: process_collector usage found (not available in prometheus 0.13)"
    exit 1
else
    echo "✅ No process_collector usage"
fi

# Check for PrometheusError in metrics_init.rs
if grep -q "PrometheusError" shared/src/monitoring/metrics_init.rs 2>/dev/null; then
    echo "⚠️  WARNING: PrometheusError reference found (might cause clone issues)"
else
    echo "✅ No PrometheusError reference"
fi

# 2. Check for common Rust compilation issues
echo ""
echo "2️⃣ Checking for common issues..."
echo "---------------------------------"

# Check for Result clone issues
if grep -q "result\.clone()" shared/src/monitoring/metrics_init.rs 2>/dev/null; then
    echo "⚠️  WARNING: Attempting to clone Result (might not implement Clone)"
else
    echo "✅ No Result clone attempts"
fi

# Check for unwrap() in source code
UNWRAP_COUNT=$(find . -name "*.rs" -path "*/src/*" -exec grep -l "\.unwrap()" {} \; 2>/dev/null | wc -l)
if [ "$UNWRAP_COUNT" -eq "0" ]; then
    echo "✅ No unwrap() calls in source code"
else
    echo "⚠️  WARNING: Found unwrap() in $UNWRAP_COUNT source files"
fi

# Check for unsafe blocks
UNSAFE_COUNT=$(find . -name "*.rs" -path "*/src/*" -exec grep -l "unsafe" {} \; 2>/dev/null | wc -l)
echo "ℹ️  Found unsafe in $UNSAFE_COUNT source files (some may be necessary)"

# 3. Check dependencies
echo ""
echo "3️⃣ Checking dependencies..."
echo "----------------------------"

# Check Prometheus version
if grep -q 'prometheus = "0.13"' Cargo.toml 2>/dev/null; then
    echo "✅ Prometheus 0.13 configured"
else
    echo "⚠️  WARNING: Check Prometheus version in Cargo.toml"
fi

# Check for crossbeam-queue (for safe_primitives)
if grep -q 'crossbeam' Cargo.toml 2>/dev/null; then
    echo "✅ Crossbeam dependency found"
else
    echo "⚠️  WARNING: Crossbeam not found (needed for safe_primitives)"
fi

# 4. Check module structure
echo ""
echo "4️⃣ Checking module structure..."
echo "--------------------------------"

# Check if safe_primitives module is included
if grep -q "pub mod safe_primitives" shared/src/tool/high_performance/mod.rs 2>/dev/null; then
    echo "✅ safe_primitives module included"
else
    echo "⚠️  WARNING: safe_primitives module not found in mod.rs"
fi

# Check if metrics_init module is included
if grep -q "pub mod metrics_init" shared/src/monitoring/mod.rs 2>/dev/null; then
    echo "✅ metrics_init module included"
else
    echo "⚠️  WARNING: metrics_init module not found in mod.rs"
fi

# 5. Summary
echo ""
echo "=================================================="
echo "📊 Summary"
echo "=================================================="

ERROR_COUNT=0
WARNING_COUNT=0

# Count errors and warnings from above checks
if grep -q "pub type Gauge = prometheus::Gauge" shared/src/monitoring/metrics.rs 2>/dev/null; then
    ERROR_COUNT=$((ERROR_COUNT + 1))
fi

if grep -q "process_collector" shared/src/monitoring/metrics_init.rs 2>/dev/null; then
    ERROR_COUNT=$((ERROR_COUNT + 1))
fi

if [ "$ERROR_COUNT" -eq "0" ]; then
    echo "✅ No critical errors found"
    echo ""
    echo "The code should compile successfully."
    echo "Run 'cargo build --all' to build the project."
else
    echo "❌ Found $ERROR_COUNT critical errors that will prevent compilation"
    echo ""
    echo "Please fix the errors above before attempting to build."
fi

echo ""
echo "Next steps:"
echo "  1. cargo check --all     # Quick syntax check"
echo "  2. cargo build --all     # Full build"
echo "  3. cargo test --all      # Run tests"
echo ""