#!/bin/bash

# 프로덕션 준비 상태 종합 검사 스크립트
# 모든 프로덕션 요구사항을 체크합니다.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🚀 Production Readiness Check for Police Thief Game Server"
echo "=========================================================="
echo ""

SCORE=0
MAX_SCORE=100

# 1. 빌드 체크
echo "1️⃣ Build Check"
echo "---------------"
if cargo build --release --all 2>/dev/null; then
    echo -e "${GREEN}✓${NC} All modules build successfully"
    SCORE=$((SCORE + 15))
else
    echo -e "${RED}✗${NC} Build failed"
fi

# 2. 테스트 실행
echo ""
echo "2️⃣ Test Suite"
echo "--------------"
if cargo test --all 2>/dev/null; then
    echo -e "${GREEN}✓${NC} All tests pass"
    SCORE=$((SCORE + 15))
else
    echo -e "${YELLOW}⚠${NC} Some tests failed"
    SCORE=$((SCORE + 5))
fi

# 3. 보안 검사
echo ""
echo "3️⃣ Security Check"
echo "------------------"

# JWT Secret
if [ -f ".env" ] && grep -q "JWT_SECRET_KEY" .env; then
    JWT_SECRET=$(grep "JWT_SECRET_KEY" .env | cut -d'=' -f2)
    if [[ ${#JWT_SECRET} -ge 32 ]] && [[ ! "$JWT_SECRET" =~ "REPLACE" ]]; then
        echo -e "${GREEN}✓${NC} JWT Secret configured securely"
        SCORE=$((SCORE + 10))
    else
        echo -e "${YELLOW}⚠${NC} JWT Secret needs improvement"
        SCORE=$((SCORE + 3))
    fi
else
    echo -e "${RED}✗${NC} JWT Secret not configured"
fi

# TLS
if [ -f ".env" ] && grep -q "ENABLE_TLS=true" .env; then
    echo -e "${GREEN}✓${NC} TLS enabled"
    SCORE=$((SCORE + 5))
else
    echo -e "${YELLOW}⚠${NC} TLS not enabled"
fi

# 4. 모니터링
echo ""
echo "4️⃣ Monitoring"
echo "--------------"

# Prometheus 체크
if grep -q "prometheus" Cargo.toml; then
    echo -e "${GREEN}✓${NC} Prometheus metrics configured"
    SCORE=$((SCORE + 10))
else
    echo -e "${RED}✗${NC} Prometheus not configured"
fi

# 로깅
if [ -f ".env" ] && grep -q "RUST_LOG" .env; then
    echo -e "${GREEN}✓${NC} Logging configured"
    SCORE=$((SCORE + 5))
else
    echo -e "${YELLOW}⚠${NC} Logging not configured"
fi

# 5. 성능 체크
echo ""
echo "5️⃣ Performance"
echo "---------------"

# Release 빌드 최적화
if grep -q "opt-level = 3" Cargo.toml 2>/dev/null || grep -q "lto = true" Cargo.toml 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Release optimizations enabled"
    SCORE=$((SCORE + 5))
else
    echo -e "${YELLOW}⚠${NC} Consider enabling release optimizations"
fi

# 6. Docker
echo ""
echo "6️⃣ Deployment"
echo "--------------"

if [ -f "Dockerfile" ]; then
    echo -e "${GREEN}✓${NC} Dockerfile exists"
    SCORE=$((SCORE + 5))
    
    # Multi-stage 빌드 체크
    if grep -q "FROM.*as builder" Dockerfile; then
        echo -e "${GREEN}✓${NC} Multi-stage build configured"
        SCORE=$((SCORE + 5))
    fi
    
    # Non-root user 체크
    if grep -q "USER" Dockerfile; then
        echo -e "${GREEN}✓${NC} Non-root user configured"
        SCORE=$((SCORE + 5))
    fi
else
    echo -e "${RED}✗${NC} Dockerfile not found"
fi

if [ -f "docker-compose.yml" ]; then
    echo -e "${GREEN}✓${NC} Docker Compose configured"
    SCORE=$((SCORE + 5))
else
    echo -e "${YELLOW}⚠${NC} Docker Compose not configured"
fi

# 7. 문서화
echo ""
echo "7️⃣ Documentation"
echo "-----------------"

if [ -f "README.md" ]; then
    echo -e "${GREEN}✓${NC} README.md exists"
    SCORE=$((SCORE + 3))
else
    echo -e "${YELLOW}⚠${NC} README.md not found"
fi

if [ -f "CLAUDE.md" ]; then
    echo -e "${GREEN}✓${NC} CLAUDE.md exists"
    SCORE=$((SCORE + 2))
else
    echo -e "${YELLOW}⚠${NC} CLAUDE.md not found"
fi

# 8. CI/CD
echo ""
echo "8️⃣ CI/CD"
echo "---------"

if [ -d ".github/workflows" ]; then
    echo -e "${GREEN}✓${NC} GitHub Actions configured"
    SCORE=$((SCORE + 5))
else
    echo -e "${YELLOW}⚠${NC} CI/CD not configured"
fi

# 9. 코드 품질
echo ""
echo "9️⃣ Code Quality"
echo "----------------"

# Clippy 실행
if cargo clippy --all -- -D warnings 2>/dev/null; then
    echo -e "${GREEN}✓${NC} No clippy warnings"
    SCORE=$((SCORE + 5))
else
    echo -e "${YELLOW}⚠${NC} Clippy warnings found"
    SCORE=$((SCORE + 2))
fi

# Format 체크
if cargo fmt --all -- --check 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Code properly formatted"
    SCORE=$((SCORE + 3))
else
    echo -e "${YELLOW}⚠${NC} Code needs formatting"
fi

# 결과 요약
echo ""
echo "==========================================="
echo "📊 Production Readiness Score: ${SCORE}/${MAX_SCORE}"
echo "==========================================="

if [ $SCORE -ge 90 ]; then
    echo -e "${GREEN}🎉 Excellent! Ready for production deployment${NC}"
elif [ $SCORE -ge 75 ]; then
    echo -e "${GREEN}✅ Good! Minor improvements recommended${NC}"
elif [ $SCORE -ge 60 ]; then
    echo -e "${YELLOW}⚠️ Fair. Several improvements needed before production${NC}"
else
    echo -e "${RED}❌ Not ready. Critical improvements required${NC}"
fi

echo ""
echo "Recommendations:"
if [ $SCORE -lt 15 ]; then
    echo "  • Fix build errors"
fi
if [ $SCORE -lt 30 ]; then
    echo "  • Ensure all tests pass"
fi
if [ ! -f ".env" ] || ! grep -q "JWT_SECRET_KEY" .env ]; then
    echo "  • Configure JWT secret key"
fi
if [ ! -f ".env" ] || ! grep -q "ENABLE_TLS=true" .env ]; then
    echo "  • Enable TLS for production"
fi
if ! grep -q "prometheus" Cargo.toml; then
    echo "  • Implement Prometheus monitoring"
fi

echo ""
echo "Run specific checks:"
echo "  ./scripts/security_audit.sh - Security audit"
echo "  ./scripts/test_coverage.sh - Test coverage"
echo ""