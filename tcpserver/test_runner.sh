#!/bin/bash

# TCP 서버 테스트 실행 스크립트
# 각 모듈별로 테스트를 실행할 수 있습니다.

cd "$(dirname "$0")/.."

echo "🧪 TCP 서버 테스트 실행기"
echo "=========================="

# 컬러 출력 함수
print_success() {
    echo -e "\033[32m✅ $1\033[0m"
}

print_info() {
    echo -e "\033[34mℹ️ $1\033[0m"
}

print_error() {
    echo -e "\033[31m❌ $1\033[0m"
}

# 테스트 실행 함수
run_test() {
    local test_name=$1
    local description=$2
    
    print_info "실행 중: $description"
    if cargo test -p tcpserver --lib "$test_name" -- --nocapture; then
        print_success "$description 통과"
        return 0
    else
        print_error "$description 실패"
        return 1
    fi
}

# 메뉴 표시
show_menu() {
    echo ""
    echo "테스트 메뉴:"
    echo "1) 프로토콜 테스트"
    echo "2) 연결 관리 테스트"
    echo "3) 하트비트 테스트"
    echo "4) 서비스 테스트"
    echo "5) 핸들러 테스트"
    echo "6) 도구 테스트"
    echo "7) 통합 테스트"
    echo "8) 전체 테스트"
    echo "9) 성능 테스트만"
    echo "0) 종료"
    echo ""
}

# 개별 테스트 실행
run_protocol_tests() {
    echo "📨 프로토콜 테스트 실행"
    run_test "tests::test_protocol" "프로토콜 테스트"
}

run_connection_tests() {
    echo "🔗 연결 관리 테스트 실행"
    run_test "tests::test_connection" "연결 관리 테스트"
}

run_heartbeat_tests() {
    echo "💓 하트비트 테스트 실행"
    run_test "tests::test_heartbeat" "하트비트 테스트"
}

run_service_tests() {
    echo "⚙️ 서비스 테스트 실행"
    run_test "tests::test_service" "서비스 테스트"
}

run_handler_tests() {
    echo "🎮 핸들러 테스트 실행"
    run_test "tests::test_handler" "핸들러 테스트"
}

run_tool_tests() {
    echo "🔧 도구 테스트 실행"
    run_test "tests::test_tools" "도구 테스트"
}

run_integration_tests() {
    echo "🚀 통합 테스트 실행"
    run_test "tests::all_test" "통합 테스트"
}

run_all_tests() {
    echo "🎯 전체 테스트 실행"
    run_test "tests" "전체 테스트 스위트"
}

run_performance_tests() {
    echo "⚡ 성능 테스트 실행"
    run_test "tests::all_test::test_basic_performance" "성능 테스트"
}

# 메인 루프
main() {
    while true; do
        show_menu
        read -p "선택하세요 (0-9): " choice
        
        case $choice in
            1) run_protocol_tests ;;
            2) run_connection_tests ;;
            3) run_heartbeat_tests ;;
            4) run_service_tests ;;
            5) run_handler_tests ;;
            6) run_tool_tests ;;
            7) run_integration_tests ;;
            8) run_all_tests ;;
            9) run_performance_tests ;;
            0) echo "테스트 실행기를 종료합니다."; exit 0 ;;
            *) echo "잘못된 선택입니다. 0-9 사이의 숫자를 입력하세요." ;;
        esac
        
        echo ""
        read -p "계속하려면 Enter를 누르세요..."
    done
}

# 인수가 있으면 직접 실행
if [ $# -gt 0 ]; then
    case $1 in
        "protocol") run_protocol_tests ;;
        "connection") run_connection_tests ;;
        "heartbeat") run_heartbeat_tests ;;
        "service") run_service_tests ;;
        "handler") run_handler_tests ;;
        "tools") run_tool_tests ;;
        "integration") run_integration_tests ;;
        "all") run_all_tests ;;
        "performance") run_performance_tests ;;
        *) echo "사용법: $0 [protocol|connection|heartbeat|service|handler|tools|integration|all|performance]" ;;
    esac
else
    main
fi