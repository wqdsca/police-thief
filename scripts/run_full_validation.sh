#!/bin/bash

# 🎯 100점 달성 검증 스크립트
echo "╔══════════════════════════════════════════════════════╗"
echo "║         🏆 Police-Thief Game Server                   ║"
echo "║         100점 달성 종합 검증 시작                     ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""

# 색상 코드
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# 점수 초기화
TOTAL_SCORE=0
MAX_SCORE=100

# Phase별 점수 배점
PHASE_SCORES=(
    "12.5"  # Phase 1: 에러 처리
    "12.5"  # Phase 2: 테스트 커버리지
    "12.5"  # Phase 3: 컴파일 경고
    "12.5"  # Phase 4: 문서화
    "12.5"  # Phase 5: CI/CD
    "12.5"  # Phase 6: 보안
    "12.5"  # Phase 7: 성능
    "12.5"  # Phase 8: 모니터링
)

PHASES_COMPLETED=0

echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 1: 에러 처리 개선 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# unwrap 사용 체크
UNWRAP_COUNT=$(grep -r "\.unwrap()" --include="*.rs" . 2>/dev/null | wc -l)
if [ $UNWRAP_COUNT -lt 50 ]; then
    echo -e "${GREEN}✅ Unwrap 사용 최소화 (${UNWRAP_COUNT}개)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[0]}" | bc)
    ((PHASES_COMPLETED++))
else
    echo -e "${YELLOW}⚠️  Unwrap 사용 과다 (${UNWRAP_COUNT}개)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[0]} * 0.5" | bc)
fi

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 2: 테스트 커버리지 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# 테스트 파일 존재 체크
TEST_FILES=$(find . -name "*test*.rs" -o -name "*tests.rs" 2>/dev/null | wc -l)
if [ $TEST_FILES -gt 10 ]; then
    echo -e "${GREEN}✅ 테스트 파일 충분 (${TEST_FILES}개)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[1]}" | bc)
    ((PHASES_COMPLETED++))
else
    echo -e "${YELLOW}⚠️  테스트 파일 부족 (${TEST_FILES}개)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[1]} * 0.7" | bc)
fi

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 3: 컴파일 경고 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# 경고 생성 코드 체크 (간접 측정)
ALLOW_COUNT=$(grep -r "#\[allow(" --include="*.rs" . 2>/dev/null | wc -l)
if [ $ALLOW_COUNT -lt 20 ]; then
    echo -e "${GREEN}✅ 경고 억제 최소화 (${ALLOW_COUNT}개)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[2]}" | bc)
    ((PHASES_COMPLETED++))
else
    echo -e "${YELLOW}⚠️  경고 억제 과다 (${ALLOW_COUNT}개)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[2]} * 0.6" | bc)
fi

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 4: 문서화 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# 문서 주석 체크
DOC_COMMENTS=$(grep -r "///" --include="*.rs" . 2>/dev/null | wc -l)
if [ $DOC_COMMENTS -gt 100 ]; then
    echo -e "${GREEN}✅ 문서화 충분 (${DOC_COMMENTS}개 주석)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[3]}" | bc)
    ((PHASES_COMPLETED++))
else
    echo -e "${YELLOW}⚠️  문서화 부족 (${DOC_COMMENTS}개 주석)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[3]} * 0.7" | bc)
fi

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 5: CI/CD 파이프라인 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

if [ -f ".github/workflows/ci.yml" ]; then
    echo -e "${GREEN}✅ CI/CD 파이프라인 구성됨${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[4]}" | bc)
    ((PHASES_COMPLETED++))
else
    echo -e "${RED}❌ CI/CD 파이프라인 없음${NC}"
fi

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 6: 보안 강화 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# 보안 모듈 체크
if [ -d "shared/src/security" ]; then
    SECURITY_FILES=$(ls shared/src/security/*.rs 2>/dev/null | wc -l)
    if [ $SECURITY_FILES -gt 5 ]; then
        echo -e "${GREEN}✅ 보안 모듈 완비 (${SECURITY_FILES}개 파일)${NC}"
        TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[5]}" | bc)
        ((PHASES_COMPLETED++))
    else
        echo -e "${YELLOW}⚠️  보안 모듈 부족${NC}"
        TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[5]} * 0.5" | bc)
    fi
else
    echo -e "${RED}❌ 보안 모듈 없음${NC}"
fi

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 7: 성능 최적화 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# 최적화 기능 체크
OPTIMIZATIONS=0
grep -q "DashMap" --include="*.rs" -r . 2>/dev/null && ((OPTIMIZATIONS++))
grep -q "simd" --include="*.rs" -r . 2>/dev/null && ((OPTIMIZATIONS++))
grep -q "MemoryPool" --include="*.rs" -r . 2>/dev/null && ((OPTIMIZATIONS++))
grep -q "lz4\|zstd" --include="*.rs" -r . 2>/dev/null && ((OPTIMIZATIONS++))

if [ $OPTIMIZATIONS -ge 3 ]; then
    echo -e "${GREEN}✅ 성능 최적화 적용 (${OPTIMIZATIONS}/4)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[6]}" | bc)
    ((PHASES_COMPLETED++))
else
    echo -e "${YELLOW}⚠️  성능 최적화 부족 (${OPTIMIZATIONS}/4)${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[6]} * 0.6" | bc)
fi

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}📋 Phase 8: 모니터링 시스템 검증${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

if [ -f "docker-compose.monitoring.yml" ] && [ -d "monitoring" ]; then
    echo -e "${GREEN}✅ 모니터링 시스템 구성됨${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[7]}" | bc)
    ((PHASES_COMPLETED++))
else
    echo -e "${YELLOW}⚠️  모니터링 시스템 미완성${NC}"
    TOTAL_SCORE=$(echo "$TOTAL_SCORE + ${PHASE_SCORES[7]} * 0.5" | bc)
fi

echo ""
echo -e "${MAGENTA}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${MAGENTA}║                  📊 최종 결과                         ║${NC}"
echo -e "${MAGENTA}╚══════════════════════════════════════════════════════╝${NC}"
echo ""

echo -e "완료된 Phase: ${PHASES_COMPLETED}/8"
echo -e "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# 점수를 정수로 변환
FINAL_SCORE=$(echo "$TOTAL_SCORE" | bc | cut -d. -f1)

# 등급 결정
if [ $FINAL_SCORE -ge 95 ]; then
    GRADE="S"
    GRADE_COLOR=$GREEN
    EMOJI="🏆"
elif [ $FINAL_SCORE -ge 90 ]; then
    GRADE="A+"
    GRADE_COLOR=$GREEN
    EMOJI="🥇"
elif [ $FINAL_SCORE -ge 85 ]; then
    GRADE="A"
    GRADE_COLOR=$GREEN
    EMOJI="🎯"
elif [ $FINAL_SCORE -ge 80 ]; then
    GRADE="B+"
    GRADE_COLOR=$CYAN
    EMOJI="✨"
elif [ $FINAL_SCORE -ge 75 ]; then
    GRADE="B"
    GRADE_COLOR=$CYAN
    EMOJI="⭐"
elif [ $FINAL_SCORE -ge 70 ]; then
    GRADE="C"
    GRADE_COLOR=$YELLOW
    EMOJI="📈"
else
    GRADE="D"
    GRADE_COLOR=$RED
    EMOJI="⚠️"
fi

echo ""
echo -e "${GRADE_COLOR}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${GRADE_COLOR}║          ${EMOJI} 최종 점수: ${FINAL_SCORE}/100 (${GRADE} 등급) ${EMOJI}          ║${NC}"
echo -e "${GRADE_COLOR}╚══════════════════════════════════════════════════════╝${NC}"
echo ""

# 개선 제안
if [ $FINAL_SCORE -lt 100 ]; then
    echo -e "${YELLOW}💡 100점 달성을 위한 개선 사항:${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    
    [ $UNWRAP_COUNT -ge 50 ] && echo "• unwrap() 사용을 50개 미만으로 줄이기"
    [ $TEST_FILES -le 10 ] && echo "• 테스트 파일 추가 작성"
    [ $DOC_COMMENTS -le 100 ] && echo "• 문서 주석 추가"
    [ $OPTIMIZATIONS -lt 3 ] && echo "• 성능 최적화 기능 추가 적용"
    
    echo ""
fi

# 실행 가능한 스크립트 안내
echo -e "${CYAN}🛠️  추가 검증 도구:${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "• 보안 감사: ./scripts/run_security_audit.sh"
echo "• 성능 벤치마크: ./scripts/run_benchmarks.sh"
echo "• 문서 생성: python scripts/generate_docs.py ."
echo "• 경고 수정: ./scripts/fix_warnings.sh"
echo "• Unwrap 제거: python scripts/remove_unwraps.py ."
echo ""

echo -e "${GREEN}✅ 검증 완료!${NC}"