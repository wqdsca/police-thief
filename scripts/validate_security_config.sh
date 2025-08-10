#!/bin/bash

# Police Thief Game Server - 보안 설정 검증 스크립트
# 프로덕션 배포 전 필수 보안 검증을 수행합니다.

set -e  # 에러 발생 시 스크립트 중단

echo "🛡️  Police Thief 서버 보안 설정 검증 시작"
echo "=================================================="

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 검증 결과 추적
VALIDATION_FAILED=0

# 1. JWT_SECRET_KEY 검증
echo -e "\n${BLUE}1. JWT 시크릿 키 검증${NC}"
echo "----------------------------------------"

if [ -z "$JWT_SECRET_KEY" ]; then
    echo -e "${RED}❌ CRITICAL: JWT_SECRET_KEY environment variable is not set${NC}"
    echo "   Please generate a secure random key:"
    echo "   openssl rand -hex 32"
    VALIDATION_FAILED=1
else
    # 길이 검증 (최소 32자)
    JWT_KEY_LENGTH=${#JWT_SECRET_KEY}
    if [ $JWT_KEY_LENGTH -lt 32 ]; then
        echo -e "${RED}❌ CRITICAL: JWT_SECRET_KEY is too short ($JWT_KEY_LENGTH characters)${NC}"
        echo "   Minimum required: 32 characters"
        VALIDATION_FAILED=1
    else
        echo -e "${GREEN}✅ JWT_SECRET_KEY length: $JWT_KEY_LENGTH characters${NC}"
    fi
    
    # 약한 키 패턴 검증
    LOWER_SECRET=$(echo "$JWT_SECRET_KEY" | tr '[:upper:]' '[:lower:]')
    if [[ $LOWER_SECRET == *"default"* ]] || \
       [[ $LOWER_SECRET == *"secret"* ]] || \
       [[ $LOWER_SECRET == *"change"* ]] || \
       [[ $LOWER_SECRET == *"your_"* ]] || \
       [[ $LOWER_SECRET == *"please"* ]] || \
       [[ $LOWER_SECRET == *"example"* ]] || \
       [[ $LOWER_SECRET == *"insecure"* ]] || \
       [[ $LOWER_SECRET == *"replace"* ]]; then
        echo -e "${RED}❌ CRITICAL: JWT_SECRET_KEY contains weak/default patterns${NC}"
        echo "   Please generate a cryptographically secure random key"
        VALIDATION_FAILED=1
    else
        echo -e "${GREEN}✅ JWT_SECRET_KEY appears to be secure${NC}"
    fi
fi

# 2. 데이터베이스 보안 검증
echo -e "\n${BLUE}2. 데이터베이스 보안 설정 검증${NC}"
echo "----------------------------------------"

if [ -z "$db_password" ]; then
    echo -e "${RED}❌ CRITICAL: db_password is not set${NC}"
    VALIDATION_FAILED=1
else
    DB_PASSWORD_LENGTH=${#db_password}
    if [ $DB_PASSWORD_LENGTH -lt 16 ]; then
        echo -e "${YELLOW}⚠️  WARNING: Database password is short ($DB_PASSWORD_LENGTH characters)${NC}"
        echo "   Recommended: 16+ characters with mixed case, numbers, and symbols"
    else
        echo -e "${GREEN}✅ Database password length: $DB_PASSWORD_LENGTH characters${NC}"
    fi
fi

# SSL 모드 검증
if [ "$db_ssl_mode" = "required" ]; then
    echo -e "${GREEN}✅ Database SSL mode: required${NC}"
elif [ "$db_ssl_mode" = "preferred" ]; then
    echo -e "${YELLOW}⚠️  WARNING: Database SSL mode is 'preferred', consider 'required'${NC}"
else
    echo -e "${RED}❌ CRITICAL: Database SSL mode not secure: ${db_ssl_mode}${NC}"
    echo "   Set db_ssl_mode=required for production"
    VALIDATION_FAILED=1
fi

# 3. 보안 매개변수 검증
echo -e "\n${BLUE}3. 보안 매개변수 검증${NC}"
echo "----------------------------------------"

# JWT 만료시간 검증
JWT_EXPIRATION=${JWT_EXPIRATION_HOURS:-24}
if [ $JWT_EXPIRATION -gt 24 ]; then
    echo -e "${YELLOW}⚠️  WARNING: JWT expiration time is long ($JWT_EXPIRATION hours)${NC}"
    echo "   Consider shorter expiration for better security (1-8 hours)"
else
    echo -e "${GREEN}✅ JWT expiration time: $JWT_EXPIRATION hours${NC}"
fi

# BCrypt 라운드 검증
BCRYPT_ROUNDS=${BCRYPT_ROUNDS:-12}
if [ $BCRYPT_ROUNDS -lt 10 ]; then
    echo -e "${RED}❌ CRITICAL: BCrypt rounds too low ($BCRYPT_ROUNDS)${NC}"
    echo "   Minimum recommended: 10 rounds"
    VALIDATION_FAILED=1
elif [ $BCRYPT_ROUNDS -gt 15 ]; then
    echo -e "${YELLOW}⚠️  WARNING: BCrypt rounds very high ($BCRYPT_ROUNDS)${NC}"
    echo "   May impact performance. Consider 10-12 for balance"
else
    echo -e "${GREEN}✅ BCrypt rounds: $BCRYPT_ROUNDS${NC}"
fi

# Rate limiting 검증
RATE_LIMIT=${RATE_LIMIT_RPM:-100}
if [ $RATE_LIMIT -gt 200 ]; then
    echo -e "${YELLOW}⚠️  WARNING: Rate limit is high ($RATE_LIMIT RPM)${NC}"
    echo "   Consider lower limits to prevent abuse"
else
    echo -e "${GREEN}✅ Rate limit: $RATE_LIMIT requests per minute${NC}"
fi

# 4. 네트워크 보안 검증
echo -e "\n${BLUE}4. 네트워크 보안 설정 검증${NC}"
echo "----------------------------------------"

# CORS 설정 검증
if [ -n "$CORS_ALLOWED_ORIGINS" ]; then
    if [[ $CORS_ALLOWED_ORIGINS == *"*"* ]]; then
        echo -e "${RED}❌ CRITICAL: CORS allows all origins (*)${NC}"
        echo "   Specify exact allowed origins for production"
        VALIDATION_FAILED=1
    elif [[ $CORS_ALLOWED_ORIGINS == *"localhost"* ]]; then
        echo -e "${YELLOW}⚠️  WARNING: CORS allows localhost${NC}"
        echo "   Remove localhost origins for production"
    else
        echo -e "${GREEN}✅ CORS origins: $CORS_ALLOWED_ORIGINS${NC}"
    fi
fi

# 5. 환경 검증
echo -e "\n${BLUE}5. 환경 및 배포 검증${NC}"
echo "----------------------------------------"

# 프로덕션 환경 감지
if [ "$NODE_ENV" = "production" ] || [ "$RUST_ENV" = "production" ] || [ "$ENV" = "production" ]; then
    echo -e "${GREEN}✅ Production environment detected${NC}"
    
    # 프로덕션에서는 더 엄격한 검증
    if [ "$LOG_LEVEL" = "debug" ] || [ "$LOG_LEVEL" = "trace" ]; then
        echo -e "${YELLOW}⚠️  WARNING: Debug logging enabled in production${NC}"
        echo "   Consider setting LOG_LEVEL=info or LOG_LEVEL=warn"
    fi
else
    echo -e "${YELLOW}⚠️  Environment not set to production${NC}"
    echo "   Current environment: ${NODE_ENV:-${RUST_ENV:-${ENV:-development}}}"
fi

# 6. 종합 결과
echo -e "\n${BLUE}보안 검증 완료${NC}"
echo "=================================================="

if [ $VALIDATION_FAILED -eq 1 ]; then
    echo -e "${RED}❌ 보안 검증 실패: 위의 CRITICAL 이슈들을 해결해주세요${NC}"
    echo -e "${RED}   프로덕션 배포를 중단합니다.${NC}"
    exit 1
else
    echo -e "${GREEN}✅ 모든 보안 검증을 통과했습니다${NC}"
    echo -e "${GREEN}   프로덕션 배포 준비가 완료되었습니다.${NC}"
    
    # 보안 체크리스트 출력
    echo -e "\n${BLUE}보안 체크리스트:${NC}"
    echo "✅ JWT 시크릿 키 보안"
    echo "✅ 데이터베이스 연결 보안"
    echo "✅ 암호화 매개변수 적절"
    echo "✅ 네트워크 보안 설정"
    echo "✅ 환경 설정 검증"
fi

echo -e "\n${BLUE}추가 권장사항:${NC}"
echo "• 정기적인 보안 키 순환 (JWT, DB 비밀번호)"
echo "• 보안 업데이트 모니터링"
echo "• 접근 로그 및 보안 이벤트 모니터링"
echo "• 정기적인 보안 감사 수행"

exit 0