#!/bin/bash

# Police Thief Game Server Management Script
# 통합 서버 실행 및 관리 도구

set -e

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m' # No Color

# 프로젝트 정보
PROJECT_NAME="Police Thief Game Server"
VERSION="1.0.0"
AUTHOR="SuperClaude Framework"

# 경로 설정
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
DOCKER_DIR="$PROJECT_ROOT/gamecenter/docker"

# 로고 출력
show_logo() {
    echo -e "${CYAN}"
    cat << "EOF"
    ____        ___              _______ __    _      ____
   / __ \____  / (_)_______     /_  __/ / /_  (_)__  / __/
  / /_/ / __ \/ / / ___/ _ \     / / / / __ \/ / _ \/ /_  
 / ____/ /_/ / / / /__/  __/    / / / / / / / /  __/ __/  
/_/    \____/_/_/\___/\___/    /_/ /_/_/ /_/_/\___/_/     
                                                          
    Game Server Management Console v1.0.0
EOF
    echo -e "${NC}"
}

# 도움말 출력
show_help() {
    echo -e "${WHITE}$PROJECT_NAME Management Tool${NC}"
    echo -e "${CYAN}=================================================${NC}"
    echo ""
    echo -e "${YELLOW}USAGE:${NC}"
    echo "  $0 [COMMAND] [OPTIONS]"
    echo ""
    echo -e "${YELLOW}COMMANDS:${NC}"
    echo ""
    echo -e "${GREEN}🚀 Server Management:${NC}"
    echo "  start                Start unified server (recommended)"
    echo "  start-docker         Start with Docker (unified container)"
    echo "  start-micro          Start microservices with Docker"
    echo "  start-native         Start native binary directly"
    echo "  stop                 Stop all running servers"
    echo "  restart              Restart current servers"
    echo "  status               Show server status"
    echo ""
    echo -e "${GREEN}🔧 Individual Services:${NC}"
    echo "  grpc                 Start only gRPC server (port 50051)"
    echo "  tcp                  Start only TCP server (port 4000)"
    echo "  rudp                 Start only RUDP server (port 5000)"
    echo ""
    echo -e "${GREEN}📊 Monitoring & Debug:${NC}"
    echo "  logs                 Show real-time logs"
    echo "  health               Check service health"
    echo "  monitor              Open monitoring dashboard"
    echo "  test                 Run connectivity tests"
    echo ""
    echo -e "${GREEN}🛠️ Development:${NC}"
    echo "  build                Build all components"
    echo "  build-docker         Build Docker images"
    echo "  clean                Clean build artifacts"
    echo "  dev                  Start in development mode"
    echo ""
    echo -e "${GREEN}⚙️ Utilities:${NC}"
    echo "  setup                Initial environment setup"
    echo "  backup               Backup Redis data"
    echo "  shell                Access server container"
    echo "  version              Show version information"
    echo ""
    echo -e "${YELLOW}EXAMPLES:${NC}"
    echo "  $0 start                    # Quick start (Docker unified)"
    echo "  $0 start-native             # Start without Docker"
    echo "  $0 start-micro              # Start microservices"
    echo "  $0 grpc                     # Start only gRPC server"
    echo "  $0 logs                     # Watch logs"
    echo "  $0 health                   # Check system health"
    echo ""
    echo -e "${CYAN}For more information, visit: https://github.com/your-repo${NC}"
}

# 버전 정보
show_version() {
    echo -e "${WHITE}$PROJECT_NAME${NC}"
    echo -e "Version: ${GREEN}$VERSION${NC}"
    echo -e "Author: ${BLUE}$AUTHOR${NC}"
    echo -e "Build: $(date '+%Y-%m-%d %H:%M:%S')"
    echo ""
    echo -e "${YELLOW}Components:${NC}"
    echo "  - gRPC Server (Authentication & Room Management)"
    echo "  - TCP Server (High-Performance Game Communication)"  
    echo "  - RUDP Server (Experimental Real-time Protocol)"
    echo "  - Redis (Session & State Management)"
    echo ""
    echo -e "${YELLOW}Performance Targets:${NC}"
    echo "  - TCP: 12,991+ msg/sec"
    echo "  - RUDP: 20,000+ msg/sec (target)"
    echo "  - Latency: <1ms p99"
    echo "  - Concurrent Players: 500+"
}

# 환경 체크
check_environment() {
    echo -e "${BLUE}🔍 Checking environment...${NC}"
    
    local errors=0
    
    # Rust 체크
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}❌ Cargo not found. Please install Rust.${NC}"
        errors=$((errors + 1))
    else
        echo -e "${GREEN}✅ Cargo found: $(cargo --version)${NC}"
    fi
    
    # Docker 체크
    if ! command -v docker &> /dev/null; then
        echo -e "${YELLOW}⚠️ Docker not found. Native mode only.${NC}"
    else
        echo -e "${GREEN}✅ Docker found: $(docker --version)${NC}"
        
        # Docker Compose 체크
        if ! command -v docker-compose &> /dev/null; then
            echo -e "${YELLOW}⚠️ Docker Compose not found.${NC}"
        else
            echo -e "${GREEN}✅ Docker Compose found: $(docker-compose --version)${NC}"
        fi
    fi
    
    # Redis 체크
    if ! command -v redis-cli &> /dev/null; then
        echo -e "${YELLOW}⚠️ Redis CLI not found. Install for advanced features.${NC}"
    else
        echo -e "${GREEN}✅ Redis CLI found${NC}"
    fi
    
    # .env 파일 체크
    if [ ! -f "$PROJECT_ROOT/.env" ]; then
        echo -e "${YELLOW}⚠️ .env file not found. Run 'setup' command first.${NC}"
    else
        echo -e "${GREEN}✅ Environment file found${NC}"
    fi
    
    if [ $errors -gt 0 ]; then
        echo -e "${RED}❌ Environment check failed. Please fix the issues above.${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ Environment check passed!${NC}"
    echo ""
}

# 환경 설정
setup_environment() {
    echo -e "${BLUE}🔧 Setting up environment...${NC}"
    
    # .env 파일 생성
    if [ ! -f "$PROJECT_ROOT/.env" ]; then
        if [ -f "$PROJECT_ROOT/.env.example" ]; then
            cp "$PROJECT_ROOT/.env.example" "$PROJECT_ROOT/.env"
            echo -e "${GREEN}✅ Created .env from .env.example${NC}"
        else
            echo -e "${RED}❌ .env.example not found${NC}"
            exit 1
        fi
    else
        echo -e "${YELLOW}⚠️ .env file already exists${NC}"
    fi
    
    # Docker .env 파일 생성
    if [ -f "$DOCKER_DIR/.env.example" ]; then
        cp "$DOCKER_DIR/.env.example" "$DOCKER_DIR/.env" 2>/dev/null || true
        echo -e "${GREEN}✅ Docker environment configured${NC}"
    fi
    
    echo -e "${GREEN}✅ Environment setup complete!${NC}"
    echo -e "${YELLOW}📝 Please edit .env file with your configuration.${NC}"
    echo ""
}

# 빌드 함수들
build_all() {
    echo -e "${BLUE}🔨 Building all components...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --release
    echo -e "${GREEN}✅ Build completed!${NC}"
}

build_docker() {
    echo -e "${BLUE}🐳 Building Docker images...${NC}"
    cd "$DOCKER_DIR"
    
    if [ -f "Makefile" ]; then
        make build
    else
        docker-compose -f docker-compose.unified.yml build
        docker-compose -f docker-compose.microservices.yml build
    fi
    
    echo -e "${GREEN}✅ Docker images built!${NC}"
}

clean_build() {
    echo -e "${BLUE}🧹 Cleaning build artifacts...${NC}"
    cd "$PROJECT_ROOT"
    cargo clean
    echo -e "${GREEN}✅ Clean completed!${NC}"
}

# 서버 시작 함수들
start_unified_docker() {
    echo -e "${BLUE}🚀 Starting unified server with Docker...${NC}"
    cd "$DOCKER_DIR"
    
    if [ -f "Makefile" ]; then
        make unified
    else
        docker-compose -f docker-compose.unified.yml up -d
    fi
    
    echo -e "${GREEN}✅ Unified server started!${NC}"
    echo -e "${CYAN}📡 gRPC Server: http://localhost:50051${NC}"
    echo -e "${CYAN}🔌 TCP Server: localhost:4000${NC}"
    echo -e "${CYAN}📶 RUDP Server: localhost:5000${NC}"
    echo ""
    echo -e "${YELLOW}Use '$0 logs' to watch logs${NC}"
    echo -e "${YELLOW}Use '$0 health' to check status${NC}"
}

start_microservices() {
    echo -e "${BLUE}🚀 Starting microservices with Docker...${NC}"
    cd "$DOCKER_DIR"
    
    if [ -f "Makefile" ]; then
        make micro
    else
        docker-compose -f docker-compose.microservices.yml up -d
    fi
    
    echo -e "${GREEN}✅ Microservices started!${NC}"
    show_service_status
}

start_native() {
    echo -e "${BLUE}🚀 Starting native server...${NC}"
    cd "$PROJECT_ROOT"
    
    # Redis 시작 (백그라운드)
    if command -v redis-server &> /dev/null; then
        echo -e "${BLUE}Starting Redis server...${NC}"
        redis-server --daemonize yes --logfile redis.log || true
    fi
    
    # 서버 시작
    echo -e "${BLUE}Starting gamecenter...${NC}"
    cargo run -p gamecenter --release -- start
}

start_individual_service() {
    local service=$1
    echo -e "${BLUE}🚀 Starting $service server...${NC}"
    cd "$PROJECT_ROOT"
    
    case $service in
        "grpc")
            cargo run -p gamecenter --release -- grpc
            ;;
        "tcp")
            cargo run -p gamecenter --release -- tcp
            ;;
        "rudp")
            cargo run -p gamecenter --release -- rudp
            ;;
        *)
            echo -e "${RED}❌ Unknown service: $service${NC}"
            exit 1
            ;;
    esac
}

# 서버 중지
stop_servers() {
    echo -e "${BLUE}🛑 Stopping all servers...${NC}"
    cd "$DOCKER_DIR"
    
    # Docker 서비스 중지
    if [ -f "Makefile" ]; then
        make down 2>/dev/null || true
    else
        docker-compose -f docker-compose.unified.yml down 2>/dev/null || true
        docker-compose -f docker-compose.microservices.yml down 2>/dev/null || true
    fi
    
    # 네이티브 서비스 중지
    pkill -f "gamecenter" 2>/dev/null || true
    pkill -f "grpcserver" 2>/dev/null || true
    pkill -f "tcpserver" 2>/dev/null || true
    pkill -f "rudpserver" 2>/dev/null || true
    
    echo -e "${GREEN}✅ All servers stopped!${NC}"
}

# 상태 확인
check_status() {
    echo -e "${BLUE}📊 Checking server status...${NC}"
    echo ""
    
    # Docker 컨테이너 상태
    echo -e "${YELLOW}Docker Containers:${NC}"
    if command -v docker &> /dev/null; then
        docker ps --filter "name=police" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" 2>/dev/null || echo "No Docker containers running"
    else
        echo "Docker not available"
    fi
    echo ""
    
    # 포트 상태 확인
    echo -e "${YELLOW}Port Status:${NC}"
    check_port "50051" "gRPC"
    check_port "4000" "TCP"
    check_port "5000" "RUDP"
    check_port "6379" "Redis"
    echo ""
    
    # 프로세스 상태
    echo -e "${YELLOW}Process Status:${NC}"
    if pgrep -f "gamecenter" > /dev/null; then
        echo -e "${GREEN}✅ GameCenter process running${NC}"
    else
        echo -e "${RED}❌ GameCenter process not found${NC}"
    fi
}

check_port() {
    local port=$1
    local service=$2
    
    if command -v netstat &> /dev/null; then
        if netstat -tuln 2>/dev/null | grep ":$port " > /dev/null; then
            echo -e "${GREEN}✅ $service ($port)${NC}"
        else
            echo -e "${RED}❌ $service ($port)${NC}"
        fi
    elif command -v ss &> /dev/null; then
        if ss -tuln 2>/dev/null | grep ":$port " > /dev/null; then
            echo -e "${GREEN}✅ $service ($port)${NC}"
        else
            echo -e "${RED}❌ $service ($port)${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️ $service ($port) - Cannot check${NC}"
    fi
}

# 헬스체크
health_check() {
    echo -e "${BLUE}🏥 Running health checks...${NC}"
    echo ""
    
    local health_score=0
    local total_checks=4
    
    # Redis 체크
    if command -v redis-cli &> /dev/null; then
        if redis-cli ping 2>/dev/null | grep -q "PONG"; then
            echo -e "${GREEN}✅ Redis: Healthy${NC}"
            health_score=$((health_score + 1))
        else
            echo -e "${RED}❌ Redis: Unhealthy${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️ Redis: Cannot check (redis-cli not available)${NC}"
        total_checks=$((total_checks - 1))
    fi
    
    # gRPC 체크
    if command -v curl &> /dev/null; then
        if curl -s -f http://localhost:50051/health >/dev/null 2>&1; then
            echo -e "${GREEN}✅ gRPC Server: Healthy${NC}"
            health_score=$((health_score + 1))
        else
            echo -e "${RED}❌ gRPC Server: Unhealthy${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️ gRPC Server: Cannot check (curl not available)${NC}"
        total_checks=$((total_checks - 1))
    fi
    
    # TCP 체크
    if command -v nc &> /dev/null; then
        if echo "PING" | nc -w 1 localhost 4000 >/dev/null 2>&1; then
            echo -e "${GREEN}✅ TCP Server: Healthy${NC}"
            health_score=$((health_score + 1))
        else
            echo -e "${RED}❌ TCP Server: Unhealthy${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️ TCP Server: Cannot check (nc not available)${NC}"
        total_checks=$((total_checks - 1))
    fi
    
    # RUDP 체크 (포트 리스닝 확인)
    if command -v ss &> /dev/null; then
        if ss -uln 2>/dev/null | grep ":5000 " >/dev/null; then
            echo -e "${GREEN}✅ RUDP Server: Healthy${NC}"
            health_score=$((health_score + 1))
        else
            echo -e "${RED}❌ RUDP Server: Unhealthy${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️ RUDP Server: Cannot check (ss not available)${NC}"
        total_checks=$((total_checks - 1))
    fi
    
    echo ""
    local health_percentage=$((health_score * 100 / total_checks))
    
    if [ $health_percentage -ge 75 ]; then
        echo -e "${GREEN}🎯 Overall Health: ${health_percentage}% (${health_score}/${total_checks}) - HEALTHY${NC}"
    elif [ $health_percentage -ge 50 ]; then
        echo -e "${YELLOW}⚠️ Overall Health: ${health_percentage}% (${health_score}/${total_checks}) - DEGRADED${NC}"
    else
        echo -e "${RED}❌ Overall Health: ${health_percentage}% (${health_score}/${total_checks}) - UNHEALTHY${NC}"
    fi
}

# 로그 보기
show_logs() {
    echo -e "${BLUE}📋 Showing server logs...${NC}"
    cd "$DOCKER_DIR"
    
    if [ -f "Makefile" ]; then
        make logs
    elif docker-compose -f docker-compose.unified.yml ps -q | grep -q .; then
        docker-compose -f docker-compose.unified.yml logs -f
    elif docker-compose -f docker-compose.microservices.yml ps -q | grep -q .; then
        docker-compose -f docker-compose.microservices.yml logs -f
    else
        echo -e "${YELLOW}⚠️ No Docker containers running. Check native logs in project directory.${NC}"
    fi
}

# 서비스 상태 표시
show_service_status() {
    echo ""
    echo -e "${CYAN}Service Endpoints:${NC}"
    echo -e "📡 gRPC API: ${WHITE}http://localhost:50051${NC}"
    echo -e "🔌 TCP Game: ${WHITE}localhost:4000${NC}"
    echo -e "📶 RUDP Game: ${WHITE}localhost:5000${NC}"
    echo -e "🔴 Redis DB: ${WHITE}localhost:6379${NC}"
    echo -e "📊 Monitoring: ${WHITE}http://localhost:9090${NC}"
    echo ""
}

# 메인 함수
main() {
    local command=${1:-"help"}
    
    case $command in
        "start")
            show_logo
            check_environment
            start_unified_docker
            ;;
        "start-docker")
            show_logo
            check_environment
            start_unified_docker
            ;;
        "start-micro")
            show_logo
            check_environment
            start_microservices
            ;;
        "start-native")
            show_logo
            check_environment
            start_native
            ;;
        "grpc"|"tcp"|"rudp")
            show_logo
            check_environment
            start_individual_service $command
            ;;
        "stop")
            stop_servers
            ;;
        "restart")
            stop_servers
            sleep 2
            start_unified_docker
            ;;
        "status")
            check_status
            ;;
        "health")
            health_check
            ;;
        "logs")
            show_logs
            ;;
        "build")
            build_all
            ;;
        "build-docker")
            build_docker
            ;;
        "clean")
            clean_build
            ;;
        "setup")
            setup_environment
            ;;
        "dev")
            echo -e "${BLUE}🔧 Starting development mode...${NC}"
            cd "$DOCKER_DIR"
            if [ -f "Makefile" ]; then
                make dev
            else
                echo -e "${YELLOW}⚠️ Development mode not configured${NC}"
            fi
            ;;
        "shell")
            echo -e "${BLUE}🐚 Accessing server shell...${NC}"
            docker exec -it police-gamecenter bash 2>/dev/null || echo -e "${RED}❌ Container not running${NC}"
            ;;
        "backup")
            echo -e "${BLUE}💾 Creating Redis backup...${NC}"
            docker exec police-redis redis-cli BGSAVE 2>/dev/null || echo -e "${RED}❌ Backup failed${NC}"
            ;;
        "monitor")
            echo -e "${BLUE}📊 Opening monitoring dashboard...${NC}"
            if command -v xdg-open &> /dev/null; then
                xdg-open http://localhost:9090
            elif command -v open &> /dev/null; then
                open http://localhost:9090
            else
                echo -e "${CYAN}Please open http://localhost:9090 in your browser${NC}"
            fi
            ;;
        "test")
            echo -e "${BLUE}🧪 Running connectivity tests...${NC}"
            health_check
            ;;
        "version"|"-v"|"--version")
            show_version
            ;;
        "help"|"-h"|"--help")
            show_help
            ;;
        *)
            echo -e "${RED}❌ Unknown command: $command${NC}"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# 스크립트 실행
main "$@"