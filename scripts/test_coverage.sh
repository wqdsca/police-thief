#!/bin/bash

# 테스트 커버리지 측정 스크립트
# cargo-tarpaulin을 사용하여 코드 커버리지를 측정합니다.

set -e

echo "🧪 Installing cargo-tarpaulin if not already installed..."
if ! command -v cargo-tarpaulin &> /dev/null; then
    cargo install cargo-tarpaulin
fi

echo "📊 Running test coverage analysis..."

# 전체 워크스페이스 커버리지 측정
echo "Running coverage for entire workspace..."
cargo tarpaulin \
    --workspace \
    --out Html \
    --output-dir target/coverage \
    --skip-clean \
    --ignore-tests \
    --ignore-panics \
    --timeout 300 \
    --exclude-files "*/tests/*" \
    --exclude-files "*/examples/*" \
    --exclude-files "*/build.rs" \
    --exclude-files "*/proto/*"

# 커버리지 보고서 생성
echo "✅ Coverage report generated at: target/coverage/tarpaulin-report.html"

# 커버리지 요약 표시
cargo tarpaulin \
    --workspace \
    --print-summary \
    --skip-clean

# 최소 커버리지 체크 (선택사항)
MIN_COVERAGE=60
ACTUAL_COVERAGE=$(cargo tarpaulin --workspace --print-summary --skip-clean 2>/dev/null | grep "Coverage" | awk '{print int($2)}')

if [ "$ACTUAL_COVERAGE" -lt "$MIN_COVERAGE" ]; then
    echo "⚠️ Warning: Coverage ($ACTUAL_COVERAGE%) is below minimum threshold ($MIN_COVERAGE%)"
    exit 1
else
    echo "✅ Coverage ($ACTUAL_COVERAGE%) meets minimum threshold ($MIN_COVERAGE%)"
fi

echo "📈 To view detailed report, open: target/coverage/tarpaulin-report.html"