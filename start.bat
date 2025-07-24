@echo off
chcp 65001 >nul
title PoliceThief NodeServer - Redis + UDP Server

echo.
echo ========================================
echo    PoliceThief NodeServer 시작
echo ========================================
echo.

:: 현재 디렉토리 확인
echo 📁 현재 디렉토리: %CD%
echo.

:: 환경변수 파일 확인
if not exist ".env" (
    echo ❌ .env 파일이 없습니다!
    echo 📝 .env 파일을 생성해주세요:
    echo.
    echo REDIS_HOST=localhost
    echo REDIS_PORT=6379
    echo REDIS_PASSWORD=
    echo REDIS_DB=0
    echo UDP_IP=0.0.0.0
    echo UDP_PORT=8080
    echo.
    echo .env 파일을 생성한 후 다시 실행해주세요.
    pause
    exit /b 1
)

echo ✅ .env 파일 확인 완료

:: Node.js 설치 확인
echo 🔍 Node.js 설치 확인 중...
node --version >nul 2>&1
if errorlevel 1 (
    echo ❌ Node.js가 설치되지 않았습니다!
    echo 📥 https://nodejs.org 에서 Node.js를 설치해주세요.
    pause
    exit /b 1
)

for /f "tokens=*" %%i in ('node --version') do set NODE_VERSION=%%i
echo ✅ Node.js 버전: %NODE_VERSION%

:: npm 패키지 설치 확인
if not exist "node_modules" (
    echo 📦 npm 패키지를 설치합니다...
    npm install
    if errorlevel 1 (
        echo ❌ npm 패키지 설치 실패!
        echo npm install을 수동으로 실행해주세요.
        pause
        exit /b 1
    )
    echo ✅ npm 패키지 설치 완료
) else (
    echo ✅ node_modules 폴더 확인 완료
)

:: Redis 서버 상태 확인
echo 🔍 Redis 서버 상태를 확인합니다...
"C:\Program Files\Redis\redis-cli.exe" ping >nul 2>&1
if errorlevel 1 (
    echo ⚠️  Redis 서버가 실행되지 않았습니다.
    echo 🚀 Redis 서버를 시작합니다...
    
    :: Redis 서버 시작 (Windows용)
    start "Redis Server" /min "C:\Program Files\Redis\redis-server.exe"
    
    :: Redis 서버 시작 대기
    echo ⏳ Redis 서버 시작 대기 중... (3초)
    timeout /t 3 /nobreak >nul
    
    :: Redis 연결 재확인
    "C:\Program Files\Redis\redis-cli.exe" ping >nul 2>&1
    if errorlevel 1 (
        echo ❌ Redis 서버 시작 실패!
        echo 📥 Redis를 설치하거나 수동으로 시작해주세요.
        echo Redis 설치: https://redis.io/download
        pause
        exit /b 1
    )
    echo ✅ Redis 서버가 시작되었습니다.
) else (
    echo ✅ Redis 서버가 이미 실행 중입니다.
)

:: 환경변수 로드
echo 📋 환경변수를 로드합니다...
for /f "tokens=1,2 delims==" %%a in (.env) do (
    set %%a=%%b
)

:: 환경변수 확인
echo 📡 UDP_IP: %UDP_IP%
echo 📡 UDP_PORT: %UDP_PORT%
echo 🔗 REDIS_HOST: %REDIS_HOST%
echo 🔗 REDIS_PORT: %REDIS_PORT%

:: 서버 시작
echo.
echo 🚀 UDP 서버를 시작합니다...

:: UDP 서버 시작 (콘솔 창에서 실행)
start "UDP Server" cmd /k "node Server/Udp-server.js"

:: 서버 시작 대기
echo ⏳ UDP 서버 시작 대기 중... (2초)
timeout /t 2 /nobreak >nul

echo.
echo ========================================
echo    🎉 서버가 성공적으로 시작되었습니다!
echo ========================================
echo.
echo 📊 실행 중인 프로세스:
echo    - Redis Server
echo    - UDP Server (Node.js)
echo.
echo 🛑 서버를 종료하려면 각 창을 닫거나 이 창에서 아무 키나 누르세요.
echo.

:: 사용자 입력 대기
pause

:: 프로세스 종료
echo.
echo 🛑 서버 종료 중...
taskkill /f /im node.exe >nul 2>&1
taskkill /f /im redis-server.exe >nul 2>&1
echo ✅ 모든 서버가 종료되었습니다.
pause 