@echo off
setlocal enabledelayedexpansion

REM Police Thief Game Server Management Script for Windows
REM 통합 서버 실행 및 관리 도구 (Windows 버전)

set "PROJECT_NAME=Police Thief Game Server"
set "VERSION=1.0.0"
set "AUTHOR=SuperClaude Framework"

REM 경로 설정
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR%"
set "DOCKER_DIR=%PROJECT_ROOT%gamecenter\docker"

REM 색상 설정 (Windows 10+)
set "RED=[91m"
set "GREEN=[92m"
set "YELLOW=[93m"
set "BLUE=[94m"
set "PURPLE=[95m"
set "CYAN=[96m"
set "WHITE=[97m"
set "NC=[0m"

REM 로고 출력
:show_logo
echo %CYAN%
echo     ____        ___              _______ __    _      ____
echo    / __ \____  / ^(_^)_______     /_  __/ / /_  ^(_^)__  / __/
echo   / /_/ / __ \/ / / ___/ _ \     / / / / __ \/ / _ \/ /_  
echo  / ____/ /_/ / / / /__/  __/    / / / / / / / /  __/ __/  
echo /_/    \____/_/_/\___/\___/    /_/ /_/_/ /_/_/\___/_/     
echo.
echo     Game Server Management Console v1.0.0%NC%
echo.
goto :eof

REM 도움말 출력
:show_help
echo %WHITE%%PROJECT_NAME% Management Tool%NC%
echo %CYAN%=================================================%NC%
echo.
echo %YELLOW%USAGE:%NC%
echo   %0 [COMMAND] [OPTIONS]
echo.
echo %YELLOW%COMMANDS:%NC%
echo.
echo %GREEN%🚀 Server Management:%NC%
echo   start                Start unified server (recommended)
echo   start-docker         Start with Docker (unified container)
echo   start-micro          Start microservices with Docker
echo   start-native         Start native binary directly
echo   stop                 Stop all running servers
echo   restart              Restart current servers
echo   status               Show server status
echo.
echo %GREEN%🔧 Individual Services:%NC%
echo   grpc                 Start only gRPC server (port 50051)
echo   tcp                  Start only TCP server (port 4000)
echo   rudp                 Start only RUDP server (port 5000)
echo.
echo %GREEN%📊 Monitoring ^& Debug:%NC%
echo   logs                 Show real-time logs
echo   health               Check service health
echo   test                 Run connectivity tests
echo.
echo %GREEN%🛠️ Development:%NC%
echo   build                Build all components
echo   build-docker         Build Docker images
echo   clean                Clean build artifacts
echo.
echo %GREEN%⚙️ Utilities:%NC%
echo   setup                Initial environment setup
echo   shell                Access server container
echo   version              Show version information
echo.
echo %YELLOW%EXAMPLES:%NC%
echo   %0 start                    # Quick start (Docker unified)
echo   %0 start-native             # Start without Docker
echo   %0 grpc                     # Start only gRPC server
echo   %0 health                   # Check system health
echo.
goto :eof

REM 버전 정보
:show_version
echo %WHITE%%PROJECT_NAME%%NC%
echo Version: %GREEN%%VERSION%%NC%
echo Author: %BLUE%%AUTHOR%%NC%
echo Build: %date% %time%
echo.
echo %YELLOW%Components:%NC%
echo   - gRPC Server (Authentication ^& Room Management)
echo   - TCP Server (High-Performance Game Communication)
echo   - RUDP Server (Experimental Real-time Protocol)  
echo   - Redis (Session ^& State Management)
echo.
echo %YELLOW%Performance Targets:%NC%
echo   - TCP: 12,991+ msg/sec
echo   - RUDP: 20,000+ msg/sec (target)
echo   - Latency: ^<1ms p99
echo   - Concurrent Players: 500+
goto :eof

REM 환경 체크
:check_environment
echo %BLUE%🔍 Checking environment...%NC%

set "errors=0"

REM Rust 체크
where cargo >nul 2>&1
if errorlevel 1 (
    echo %RED%❌ Cargo not found. Please install Rust.%NC%
    set /a errors+=1
) else (
    echo %GREEN%✅ Cargo found%NC%
)

REM Docker 체크
where docker >nul 2>&1
if errorlevel 1 (
    echo %YELLOW%⚠️ Docker not found. Native mode only.%NC%
) else (
    echo %GREEN%✅ Docker found%NC%
    
    REM Docker Compose 체크
    where docker-compose >nul 2>&1
    if errorlevel 1 (
        echo %YELLOW%⚠️ Docker Compose not found.%NC%
    ) else (
        echo %GREEN%✅ Docker Compose found%NC%
    )
)

REM .env 파일 체크
if not exist "%PROJECT_ROOT%.env" (
    echo %YELLOW%⚠️ .env file not found. Run 'setup' command first.%NC%
) else (
    echo %GREEN%✅ Environment file found%NC%
)

if %errors% gtr 0 (
    echo %RED%❌ Environment check failed. Please fix the issues above.%NC%
    exit /b 1
)

echo %GREEN%✅ Environment check passed!%NC%
echo.
goto :eof

REM 환경 설정
:setup_environment
echo %BLUE%🔧 Setting up environment...%NC%

if not exist "%PROJECT_ROOT%.env" (
    if exist "%PROJECT_ROOT%.env.example" (
        copy "%PROJECT_ROOT%.env.example" "%PROJECT_ROOT%.env" >nul
        echo %GREEN%✅ Created .env from .env.example%NC%
    ) else (
        echo %RED%❌ .env.example not found%NC%
        exit /b 1
    )
) else (
    echo %YELLOW%⚠️ .env file already exists%NC%
)

if exist "%DOCKER_DIR%\.env.example" (
    copy "%DOCKER_DIR%\.env.example" "%DOCKER_DIR%\.env" >nul 2>&1
    echo %GREEN%✅ Docker environment configured%NC%
)

echo %GREEN%✅ Environment setup complete!%NC%
echo %YELLOW%📝 Please edit .env file with your configuration.%NC%
echo.
goto :eof

REM 빌드 함수들
:build_all
echo %BLUE%🔨 Building all components...%NC%
cd /d "%PROJECT_ROOT%"
cargo build --release
if errorlevel 1 (
    echo %RED%❌ Build failed%NC%
    exit /b 1
)
echo %GREEN%✅ Build completed!%NC%
goto :eof

:build_docker
echo %BLUE%🐳 Building Docker images...%NC%
cd /d "%DOCKER_DIR%"

if exist "Makefile" (
    make build
) else (
    docker-compose -f docker-compose.unified.yml build
    docker-compose -f docker-compose.microservices.yml build
)

echo %GREEN%✅ Docker images built!%NC%
goto :eof

:clean_build
echo %BLUE%🧹 Cleaning build artifacts...%NC%
cd /d "%PROJECT_ROOT%"
cargo clean
echo %GREEN%✅ Clean completed!%NC%
goto :eof

REM 서버 시작 함수들
:start_unified_docker
echo %BLUE%🚀 Starting unified server with Docker...%NC%
cd /d "%DOCKER_DIR%"

if exist "Makefile" (
    make unified
) else (
    docker-compose -f docker-compose.unified.yml up -d
)

echo %GREEN%✅ Unified server started!%NC%
echo %CYAN%📡 gRPC Server: http://localhost:50051%NC%
echo %CYAN%🔌 TCP Server: localhost:4000%NC%
echo %CYAN%📶 RUDP Server: localhost:5000%NC%
echo.
echo %YELLOW%Use '%0 logs' to watch logs%NC%
echo %YELLOW%Use '%0 health' to check status%NC%
goto :eof

:start_microservices
echo %BLUE%🚀 Starting microservices with Docker...%NC%
cd /d "%DOCKER_DIR%"

if exist "Makefile" (
    make micro
) else (
    docker-compose -f docker-compose.microservices.yml up -d
)

echo %GREEN%✅ Microservices started!%NC%
call :show_service_status
goto :eof

:start_native
echo %BLUE%🚀 Starting native server...%NC%
cd /d "%PROJECT_ROOT%"

echo %BLUE%Starting gamecenter...%NC%
cargo run -p gamecenter --release -- start
goto :eof

:start_individual_service
set "service=%1"
echo %BLUE%🚀 Starting %service% server...%NC%
cd /d "%PROJECT_ROOT%"

if "%service%"=="grpc" (
    cargo run -p gamecenter --release -- grpc
) else if "%service%"=="tcp" (
    cargo run -p gamecenter --release -- tcp
) else if "%service%"=="rudp" (
    cargo run -p gamecenter --release -- rudp
) else (
    echo %RED%❌ Unknown service: %service%%NC%
    exit /b 1
)
goto :eof

REM 서버 중지
:stop_servers
echo %BLUE%🛑 Stopping all servers...%NC%
cd /d "%DOCKER_DIR%"

REM Docker 서비스 중지
if exist "Makefile" (
    make down 2>nul
) else (
    docker-compose -f docker-compose.unified.yml down 2>nul
    docker-compose -f docker-compose.microservices.yml down 2>nul
)

REM 네이티브 서비스 중지
taskkill /f /im gamecenter.exe 2>nul
taskkill /f /im grpcserver.exe 2>nul
taskkill /f /im tcpserver.exe 2>nul
taskkill /f /im rudpserver.exe 2>nul

echo %GREEN%✅ All servers stopped!%NC%
goto :eof

REM 상태 확인
:check_status
echo %BLUE%📊 Checking server status...%NC%
echo.

REM Docker 컨테이너 상태
echo %YELLOW%Docker Containers:%NC%
where docker >nul 2>&1
if not errorlevel 1 (
    docker ps --filter "name=police" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" 2>nul
    if errorlevel 1 echo No Docker containers running
) else (
    echo Docker not available
)
echo.

REM 포트 상태 확인
echo %YELLOW%Port Status:%NC%
call :check_port "50051" "gRPC"
call :check_port "4000" "TCP"
call :check_port "5000" "RUDP" 
call :check_port "6379" "Redis"
echo.

REM 프로세스 상태
echo %YELLOW%Process Status:%NC%
tasklist /fi "imagename eq gamecenter.exe" 2>nul | find /i "gamecenter.exe" >nul
if not errorlevel 1 (
    echo %GREEN%✅ GameCenter process running%NC%
) else (
    echo %RED%❌ GameCenter process not found%NC%
)
goto :eof

:check_port
set "port=%~1"
set "service=%~2"

netstat -an | find ":%port% " >nul 2>&1
if not errorlevel 1 (
    echo %GREEN%✅ %service% (%port%)%NC%
) else (
    echo %RED%❌ %service% (%port%)%NC%
)
goto :eof

REM 헬스체크
:health_check
echo %BLUE%🏥 Running health checks...%NC%
echo.

set "health_score=0"
set "total_checks=3"

REM gRPC 체크
where curl >nul 2>&1
if not errorlevel 1 (
    curl -s -f http://localhost:50051/health >nul 2>&1
    if not errorlevel 1 (
        echo %GREEN%✅ gRPC Server: Healthy%NC%
        set /a health_score+=1
    ) else (
        echo %RED%❌ gRPC Server: Unhealthy%NC%
    )
) else (
    echo %YELLOW%⚠️ gRPC Server: Cannot check (curl not available)%NC%
    set /a total_checks-=1
)

REM TCP 체크
netstat -an | find ":4000 " >nul 2>&1
if not errorlevel 1 (
    echo %GREEN%✅ TCP Server: Healthy%NC%
    set /a health_score+=1
) else (
    echo %RED%❌ TCP Server: Unhealthy%NC%
)

REM RUDP 체크
netstat -an | find ":5000 " >nul 2>&1  
if not errorlevel 1 (
    echo %GREEN%✅ RUDP Server: Healthy%NC%
    set /a health_score+=1
) else (
    echo %RED%❌ RUDP Server: Unhealthy%NC%
)

echo.
set /a health_percentage=health_score*100/total_checks

if %health_percentage% geq 75 (
    echo %GREEN%🎯 Overall Health: %health_percentage%%% (%health_score%/%total_checks%) - HEALTHY%NC%
) else if %health_percentage% geq 50 (
    echo %YELLOW%⚠️ Overall Health: %health_percentage%%% (%health_score%/%total_checks%) - DEGRADED%NC%
) else (
    echo %RED%❌ Overall Health: %health_percentage%%% (%health_score%/%total_checks%) - UNHEALTHY%NC%
)
goto :eof

REM 로그 보기
:show_logs
echo %BLUE%📋 Showing server logs...%NC%
cd /d "%DOCKER_DIR%"

if exist "Makefile" (
    make logs
) else (
    docker-compose -f docker-compose.unified.yml ps -q | find /v "" >nul 2>&1
    if not errorlevel 1 (
        docker-compose -f docker-compose.unified.yml logs -f
    ) else (
        docker-compose -f docker-compose.microservices.yml ps -q | find /v "" >nul 2>&1
        if not errorlevel 1 (
            docker-compose -f docker-compose.microservices.yml logs -f
        ) else (
            echo %YELLOW%⚠️ No Docker containers running. Check native logs in project directory.%NC%
        )
    )
)
goto :eof

REM 서비스 상태 표시
:show_service_status
echo.
echo %CYAN%Service Endpoints:%NC%
echo 📡 gRPC API: %WHITE%http://localhost:50051%NC%
echo 🔌 TCP Game: %WHITE%localhost:4000%NC%
echo 📶 RUDP Game: %WHITE%localhost:5000%NC%
echo 🔴 Redis DB: %WHITE%localhost:6379%NC%
echo 📊 Monitoring: %WHITE%http://localhost:9090%NC%
echo.
goto :eof

REM 메인 함수
:main
set "command=%1"
if "%command%"=="" set "command=help"

if "%command%"=="start" (
    call :show_logo
    call :check_environment
    call :start_unified_docker
) else if "%command%"=="start-docker" (
    call :show_logo
    call :check_environment
    call :start_unified_docker
) else if "%command%"=="start-micro" (
    call :show_logo
    call :check_environment
    call :start_microservices
) else if "%command%"=="start-native" (
    call :show_logo
    call :check_environment
    call :start_native
) else if "%command%"=="grpc" (
    call :show_logo
    call :check_environment
    call :start_individual_service grpc
) else if "%command%"=="tcp" (
    call :show_logo
    call :check_environment
    call :start_individual_service tcp
) else if "%command%"=="rudp" (
    call :show_logo
    call :check_environment
    call :start_individual_service rudp
) else if "%command%"=="stop" (
    call :stop_servers
) else if "%command%"=="restart" (
    call :stop_servers
    timeout /t 2 >nul
    call :start_unified_docker
) else if "%command%"=="status" (
    call :check_status
) else if "%command%"=="health" (
    call :health_check
) else if "%command%"=="logs" (
    call :show_logs
) else if "%command%"=="build" (
    call :build_all
) else if "%command%"=="build-docker" (
    call :build_docker
) else if "%command%"=="clean" (
    call :clean_build
) else if "%command%"=="setup" (
    call :setup_environment
) else if "%command%"=="shell" (
    echo %BLUE%🐚 Accessing server shell...%NC%
    docker exec -it police-gamecenter cmd 2>nul
    if errorlevel 1 echo %RED%❌ Container not running%NC%
) else if "%command%"=="version" (
    call :show_version
) else if "%command%"=="-v" (
    call :show_version
) else if "%command%"=="--version" (
    call :show_version
) else if "%command%"=="help" (
    call :show_help
) else if "%command%"=="-h" (
    call :show_help
) else if "%command%"=="--help" (
    call :show_help
) else (
    echo %RED%❌ Unknown command: %command%%NC%
    echo.
    call :show_help
    exit /b 1
)

goto :eof

REM 스크립트 실행
call :main %*