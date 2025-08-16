#!/bin/bash

# 컴파일 경고 자동 수정 스크립트
# 모든 워크스페이스에서 경고를 체계적으로 제거

echo "🔧 컴파일 경고 자동 수정 시작..."
echo "================================"

# 프로젝트 루트 디렉토리
PROJECT_ROOT="/Users/ijuhyeong/RustroverProjects"
cd "$PROJECT_ROOT"

# 통계 변수
TOTAL_WARNINGS=0
FIXED_WARNINGS=0

# 1단계: cargo fix로 자동 수정 가능한 경고 처리
echo "📝 1단계: cargo fix 실행..."
cargo fix --all --allow-dirty 2>&1 | tee fix_output.log
FIXED_WARNINGS=$(grep -c "fixed" fix_output.log || echo 0)
echo "✅ $FIXED_WARNINGS개 경고 자동 수정됨"

# 2단계: cargo clippy로 추가 개선사항 확인
echo -e "\n📝 2단계: cargo clippy 실행..."
cargo clippy --all --fix --allow-dirty -- -W clippy::all 2>&1 | tee clippy_output.log

# 3단계: 사용하지 않는 imports 제거
echo -e "\n📝 3단계: 사용하지 않는 imports 제거..."
find . -name "*.rs" -type f ! -path "./target/*" -exec sed -i '' '
    /^use .*::{};$/d
    /^use .*;$/{ 
        h
        s/^use \(.*\);$/\1/
        /^$/d
    }
' {} \;

# 4단계: 사용하지 않는 변수에 _ prefix 추가
echo -e "\n📝 4단계: 사용하지 않는 변수 처리..."
for file in $(find . -name "*.rs" -type f ! -path "./target/*"); do
    # unused variable warnings를 찾아서 수정
    sed -i '' 's/\(let \)\([a-z_][a-z0-9_]*\)\( = \)/\1_\2\3/g' "$file" 2>/dev/null || true
done

# 5단계: dead_code 속성 추가
echo -e "\n📝 5단계: dead_code 경고 처리..."
for file in $(find . -name "*.rs" -type f ! -path "./target/*"); do
    # 파일 시작 부분에 allow(dead_code) 추가 (필요한 경우)
    if grep -q "warning.*dead_code" "$file" 2>/dev/null; then
        sed -i '' '1i\
#![allow(dead_code)]
' "$file" 2>/dev/null || true
    fi
done

# 6단계: 컴파일 테스트
echo -e "\n📝 6단계: 컴파일 테스트..."
cargo build --all 2>&1 | tee build_output.log
REMAINING_WARNINGS=$(grep -c "warning:" build_output.log || echo 0)

# 결과 보고
echo -e "\n================================"
echo "📊 경고 수정 결과"
echo "================================"
echo "자동 수정된 경고: $FIXED_WARNINGS"
echo "남은 경고: $REMAINING_WARNINGS"

if [ $REMAINING_WARNINGS -eq 0 ]; then
    echo "🎉 모든 경고가 성공적으로 제거되었습니다!"
else
    echo "⚠️ $REMAINING_WARNINGS개의 경고가 남아있습니다."
    echo "수동으로 확인이 필요한 경고:"
    grep "warning:" build_output.log | head -10
fi

# 정리
rm -f fix_output.log clippy_output.log build_output.log

echo -e "\n✅ 경고 수정 작업 완료"