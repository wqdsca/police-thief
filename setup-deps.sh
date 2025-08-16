#!/bin/bash
#
# Police Thief Game Server - Cross-Platform Dependency Setup
# Supports Ubuntu, macOS, and Windows (via WSL/Cygwin)
#
# Usage: ./setup-deps.sh [platform]
# Platform: ubuntu, macos, windows, auto (default: auto)

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}ℹ️  INFO:${NC} $1"
}

log_success() {
    echo -e "${GREEN}✅ SUCCESS:${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}⚠️  WARNING:${NC} $1"
}

log_error() {
    echo -e "${RED}❌ ERROR:${NC} $1"
}

# Detect platform automatically
detect_platform() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if command -v apt-get >/dev/null 2>&1; then
            echo "ubuntu"
        elif command -v yum >/dev/null 2>&1; then
            echo "centos"
        elif command -v pacman >/dev/null 2>&1; then
            echo "arch"
        else
            echo "linux"
        fi
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    elif [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
        echo "windows"
    else
        echo "unknown"
    fi
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Install dependencies for Ubuntu/Debian
install_ubuntu_deps() {
    log_info "Installing dependencies for Ubuntu/Debian..."
    
    # Update package list
    sudo apt-get update
    
    # Install essential build tools
    sudo apt-get install -y \
        build-essential \
        pkg-config \
        libssl-dev \
        cmake \
        nasm \
        protobuf-compiler \
        libprotobuf-dev \
        redis-server \
        mysql-server \
        curl \
        git
    
    # Install Rust if not present
    if ! command_exists rustc; then
        log_info "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
    fi
    
    # Install cargo-audit for security scanning
    if ! command_exists cargo-audit; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit
    fi
    
    # Start and enable Redis
    sudo systemctl start redis-server
    sudo systemctl enable redis-server
    
    # Start and enable MySQL
    sudo systemctl start mysql
    sudo systemctl enable mysql
    
    log_success "Ubuntu dependencies installed successfully!"
}

# Install dependencies for macOS
install_macos_deps() {
    log_info "Installing dependencies for macOS..."
    
    # Install Homebrew if not present
    if ! command_exists brew; then
        log_info "Installing Homebrew..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    fi
    
    # Install dependencies via Homebrew
    brew install \
        cmake \
        nasm \
        protobuf \
        redis \
        mysql \
        pkg-config \
        openssl
    
    # Install Rust if not present
    if ! command_exists rustc; then
        log_info "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
    fi
    
    # Install cargo-audit for security scanning
    if ! command_exists cargo-audit; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit
    fi
    
    # Start services
    brew services start redis
    brew services start mysql
    
    log_success "macOS dependencies installed successfully!"
}

# Install dependencies for Windows
install_windows_deps() {
    log_info "Installing dependencies for Windows..."
    
    # Check if we're in WSL
    if grep -qi microsoft /proc/version 2>/dev/null; then
        log_info "Detected WSL - using Ubuntu installation method"
        install_ubuntu_deps
        return
    fi
    
    # Check for Chocolatey
    if ! command_exists choco; then
        log_error "Chocolatey not found. Please install Chocolatey first:"
        log_info "Visit: https://chocolatey.org/install"
        log_info "Or run as Administrator:"
        log_info 'Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString("https://community.chocolatey.org/install.ps1"))'
        exit 1
    fi
    
    # Install dependencies via Chocolatey
    choco install -y \
        cmake \
        nasm \
        protoc \
        redis-64 \
        mysql \
        git \
        microsoft-visual-cpp-build-tools
    
    # Install Rust if not present
    if ! command_exists rustc; then
        log_info "Installing Rust..."
        choco install -y rust
    fi
    
    # Install cargo-audit for security scanning
    if ! command_exists cargo-audit; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit
    fi
    
    log_success "Windows dependencies installed successfully!"
    log_warning "Please restart your terminal to ensure all tools are available"
}

# Verify installation
verify_installation() {
    log_info "Verifying installation..."
    
    local missing_deps=()
    
    # Check essential tools
    local tools=("rustc" "cargo" "cmake" "nasm" "protoc" "redis-server" "mysql")
    
    for tool in "${tools[@]}"; do
        if ! command_exists "$tool"; then
            missing_deps+=("$tool")
        fi
    done
    
    if [ ${#missing_deps[@]} -eq 0 ]; then
        log_success "All dependencies are installed correctly!"
        return 0
    else
        log_error "Missing dependencies: ${missing_deps[*]}"
        return 1
    fi
}

# Test build
test_build() {
    log_info "Testing build..."
    
    # Test shared library build
    if cargo check -p shared --lib; then
        log_success "Shared library builds successfully!"
    else
        log_error "Shared library build failed!"
        return 1
    fi
    
    # Test if Redis is accessible
    if redis-cli ping >/dev/null 2>&1; then
        log_success "Redis is accessible!"
    else
        log_warning "Redis is not accessible - make sure it's running"
    fi
    
    # Test if MySQL is accessible
    if mysql -e "SELECT 1" >/dev/null 2>&1; then
        log_success "MySQL is accessible!"
    else
        log_warning "MySQL is not accessible - you may need to configure it"
    fi
}

# Main function
main() {
    local platform="${1:-auto}"
    
    if [ "$platform" = "auto" ]; then
        platform=$(detect_platform)
    fi
    
    log_info "Setting up dependencies for platform: $platform"
    
    case "$platform" in
        ubuntu|debian)
            install_ubuntu_deps
            ;;
        macos|darwin)
            install_macos_deps
            ;;
        windows|win32|cygwin|msys)
            install_windows_deps
            ;;
        *)
            log_error "Unsupported platform: $platform"
            log_info "Supported platforms: ubuntu, macos, windows"
            exit 1
            ;;
    esac
    
    # Verify installation
    if verify_installation; then
        log_success "Setup completed successfully!"
        
        # Test build
        test_build
        
        log_info "🚀 You can now build the project with:"
        log_info "   cargo build --release"
        log_info "   cargo run -p gamecenter start"
    else
        log_error "Setup completed with errors. Please check the missing dependencies."
        exit 1
    fi
}

# Handle script arguments
if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
    echo "Police Thief Game Server - Dependency Setup"
    echo ""
    echo "Usage: $0 [platform]"
    echo ""
    echo "Platforms:"
    echo "  ubuntu    - Ubuntu/Debian systems"
    echo "  macos     - macOS systems"
    echo "  windows   - Windows systems (requires Chocolatey)"
    echo "  auto      - Auto-detect platform (default)"
    echo ""
    echo "Options:"
    echo "  -h, --help    Show this help message"
    echo ""
    exit 0
fi

# Run main function
main "$@"