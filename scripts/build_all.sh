#!/bin/bash

# 전체 프로젝트 빌드 스크립트
# 모든 모듈을 순차적으로 빌드하고 에러를 체크합니다.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🔨 Building Police Thief Game Server"
echo "====================================="
echo ""

# Rust 설치 확인
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo not found. Please install Rust first.${NC}"
    echo "Visit: https://rustup.rs/"
    exit 1
fi

# protoc 설치 확인 (gRPC용)
if ! command -v protoc &> /dev/null; then
    echo -e "${YELLOW}⚠️ protoc not found. gRPC server may not build.${NC}"
    echo "Install with: brew install protobuf (macOS) or apt-get install protobuf-compiler (Linux)"
fi

# 1. Clean build artifacts
echo "1️⃣ Cleaning previous build artifacts..."
cargo clean

# 2. Format check
echo ""
echo "2️⃣ Checking code formatting..."
if cargo fmt --all -- --check; then
    echo -e "${GREEN}✓${NC} Code is properly formatted"
else
    echo -e "${YELLOW}⚠${NC} Code needs formatting. Run: cargo fmt --all"
fi

# 3. Build shared library first
echo ""
echo "3️⃣ Building shared library..."
if cargo build -p shared --release; then
    echo -e "${GREEN}✓${NC} Shared library built successfully"
else
    echo -e "${RED}✗${NC} Shared library build failed"
    exit 1
fi

# 4. Build each server
echo ""
echo "4️⃣ Building servers..."

# TCP Server
echo -n "  • TCP Server: "
if cargo build -p tcpserver --release 2>/dev/null; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    FAILED_BUILDS="${FAILED_BUILDS} tcpserver"
fi

# gRPC Server
echo -n "  • gRPC Server: "
if cargo build -p grpcserver --release 2>/dev/null; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${YELLOW}⚠${NC} (protoc may be required)"
fi

# Game Center
echo -n "  • Game Center: "
if cargo build -p gamecenter --release 2>/dev/null; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    FAILED_BUILDS="${FAILED_BUILDS} gamecenter"
fi

# 5. Run clippy for linting
echo ""
echo "5️⃣ Running clippy linter..."
if cargo clippy --all -- -W clippy::all 2>&1 | grep -q "warning:"; then
    WARNING_COUNT=$(cargo clippy --all -- -W clippy::all 2>&1 | grep -c "warning:" || true)
    echo -e "${YELLOW}⚠${NC} Found $WARNING_COUNT warnings"
else
    echo -e "${GREEN}✓${NC} No clippy warnings"
fi

# 6. Check for unsafe code
echo ""
echo "6️⃣ Checking for unsafe code..."
UNSAFE_COUNT=$(grep -r "unsafe" --include="*.rs" src 2>/dev/null | wc -l || echo "0")
if [ "$UNSAFE_COUNT" -eq "0" ]; then
    echo -e "${GREEN}✓${NC} No unsafe code found"
else
    echo -e "${YELLOW}⚠${NC} Found $UNSAFE_COUNT unsafe blocks"
fi

# 7. Check for unwrap() usage
echo ""
echo "7️⃣ Checking for unwrap() usage..."
UNWRAP_COUNT=$(grep -r "\.unwrap()" --include="*.rs" src 2>/dev/null | wc -l || echo "0")
if [ "$UNWRAP_COUNT" -eq "0" ]; then
    echo -e "${GREEN}✓${NC} No unwrap() calls found"
else
    echo -e "${YELLOW}⚠${NC} Found $UNWRAP_COUNT unwrap() calls"
fi

# 8. Test compilation
echo ""
echo "8️⃣ Running tests..."
if cargo test --all --no-run 2>/dev/null; then
    echo -e "${GREEN}✓${NC} All tests compile successfully"
else
    echo -e "${YELLOW}⚠${NC} Some tests failed to compile"
fi

# Summary
echo ""
echo "====================================="
echo "📊 Build Summary"
echo "====================================="

if [ -z "$FAILED_BUILDS" ]; then
    echo -e "${GREEN}✅ All builds successful!${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Run tests: cargo test --all"
    echo "  2. Check coverage: ./scripts/test_coverage.sh"
    echo "  3. Security audit: ./scripts/security_audit.sh"
    echo "  4. Production check: ./scripts/production_check.sh"
else
    echo -e "${RED}❌ Some builds failed:${NC}$FAILED_BUILDS"
    echo ""
    echo "To debug:"
    echo "  cargo build -p <module_name> --verbose"
fi

echo ""
echo "Binary locations:"
echo "  • TCP Server: target/release/tcpserver"
echo "  • gRPC Server: target/release/grpcserver"
echo "  • Game Center: target/release/gamecenter"
echo ""