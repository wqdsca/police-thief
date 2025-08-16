#!/bin/bash

# 보안 감사 스크립트
# cargo-audit와 cargo-deny를 사용하여 의존성 취약점을 검사합니다.

set -e

echo "🔒 Security Audit for Police Thief Game Server"
echo "=============================================="

# cargo-audit 설치 확인
echo "📦 Installing cargo-audit if not already installed..."
if ! command -v cargo-audit &> /dev/null; then
    cargo install cargo-audit
fi

# cargo-deny 설치 확인
echo "📦 Installing cargo-deny if not already installed..."
if ! command -v cargo-deny &> /dev/null; then
    cargo install cargo-deny
fi

# 1. 의존성 취약점 검사
echo ""
echo "1️⃣ Checking for known vulnerabilities with cargo-audit..."
echo "-----------------------------------------------------------"
cargo audit || {
    echo "⚠️ Vulnerabilities found! Please review and update dependencies."
    AUDIT_FAILED=true
}

# 2. 의존성 라이센스 및 보안 검사
echo ""
echo "2️⃣ Checking dependencies with cargo-deny..."
echo "-------------------------------------------"

# deny.toml이 없으면 기본 설정 생성
if [ ! -f "deny.toml" ]; then
    echo "Creating default deny.toml configuration..."
    cat > deny.toml << 'EOF'
[bans]
multiple-versions = "warn"
wildcards = "allow"
deny = [
    # 보안상 위험한 크레이트들
    { name = "openssl", version = "<0.10.38" },
]

[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
]
copyleft = "warn"

[sources]
unknown-registry = "warn"
unknown-git = "warn"
EOF
fi

cargo deny check licenses || echo "⚠️ License issues found"
cargo deny check bans || echo "⚠️ Banned dependencies found"

# 3. OWASP 체크리스트 검증
echo ""
echo "3️⃣ OWASP Security Checklist..."
echo "--------------------------------"

# JWT Secret 검증
echo -n "✓ JWT Secret Key: "
if [ -f ".env" ] && grep -q "JWT_SECRET_KEY" .env; then
    JWT_SECRET=$(grep "JWT_SECRET_KEY" .env | cut -d'=' -f2)
    if [[ ${#JWT_SECRET} -ge 32 ]]; then
        echo "Configured (${#JWT_SECRET} chars)"
    else
        echo "⚠️ Too short (${#JWT_SECRET} chars, minimum 32)"
    fi
else
    echo "❌ Not configured"
fi

# TLS 설정 확인
echo -n "✓ TLS Configuration: "
if [ -f ".env" ] && grep -q "ENABLE_TLS=true" .env; then
    echo "Enabled"
else
    echo "⚠️ Disabled (recommended for production)"
fi

# Rate Limiting 확인
echo -n "✓ Rate Limiting: "
if [ -f ".env" ] && grep -q "RATE_LIMIT_PER_MINUTE" .env; then
    RATE_LIMIT=$(grep "RATE_LIMIT_PER_MINUTE" .env | cut -d'=' -f2)
    echo "Configured ($RATE_LIMIT req/min)"
else
    echo "Using default (100 req/min)"
fi

# 4. 코드 내 보안 패턴 검사
echo ""
echo "4️⃣ Checking for insecure code patterns..."
echo "------------------------------------------"

# unwrap() 사용 검사
UNWRAP_COUNT=$(grep -r "\.unwrap()" --include="*.rs" src 2>/dev/null | wc -l || echo "0")
echo "• unwrap() calls: $UNWRAP_COUNT $([ $UNWRAP_COUNT -gt 0 ] && echo "⚠️ Consider using proper error handling" || echo "✓")"

# expect() 사용 검사
EXPECT_COUNT=$(grep -r "\.expect(" --include="*.rs" src 2>/dev/null | wc -l || echo "0")
echo "• expect() calls: $EXPECT_COUNT $([ $EXPECT_COUNT -gt 50 ] && echo "⚠️ High number of expect() calls" || echo "✓")"

# unsafe 사용 검사
UNSAFE_COUNT=$(grep -r "unsafe" --include="*.rs" src 2>/dev/null | wc -l || echo "0")
echo "• unsafe blocks: $UNSAFE_COUNT $([ $UNSAFE_COUNT -gt 0 ] && echo "⚠️ Review unsafe code carefully" || echo "✓")"

# 하드코딩된 비밀 검사
echo -n "• Hardcoded secrets: "
if grep -r "password\|secret\|api_key" --include="*.rs" src 2>/dev/null | grep -v "// " | grep -q "="; then
    echo "⚠️ Potential hardcoded secrets found"
else
    echo "✓ None detected"
fi

# 5. 보안 권장사항
echo ""
echo "5️⃣ Security Recommendations"
echo "----------------------------"

if [ "$AUDIT_FAILED" = true ]; then
    echo "❗ Critical: Fix vulnerability issues found by cargo-audit"
fi

if [ $UNWRAP_COUNT -gt 100 ]; then
    echo "❗ High Priority: Replace unwrap() with proper error handling"
fi

if [ $EXPECT_COUNT -gt 100 ]; then
    echo "❗ Medium Priority: Review and minimize expect() usage"
fi

echo ""
echo "📊 Security Audit Complete"
echo ""
echo "For production deployment, ensure:"
echo "  1. All vulnerabilities are patched"
echo "  2. TLS is enabled"
echo "  3. Strong JWT secret key is configured"
echo "  4. Rate limiting is properly configured"
echo "  5. All expect()/unwrap() calls are reviewed"
echo ""