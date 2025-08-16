# Police Thief Game Server - Windows Dependency Setup
# Requires PowerShell 5.0+ and Administrator privileges
#
# Usage: .\setup-deps.ps1

param(
    [string]$Mode = "auto",
    [switch]$Help
)

# Colors for output
$Colors = @{
    Red = 'Red'
    Green = 'Green'
    Yellow = 'Yellow'
    Blue = 'Blue'
    White = 'White'
}

function Write-Info {
    param([string]$Message)
    Write-Host "ℹ️  INFO: $Message" -ForegroundColor $Colors.Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "✅ SUCCESS: $Message" -ForegroundColor $Colors.Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "⚠️  WARNING: $Message" -ForegroundColor $Colors.Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "❌ ERROR: $Message" -ForegroundColor $Colors.Red
}

function Test-Administrator {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Test-CommandExists {
    param([string]$Command)
    try {
        Get-Command $Command -ErrorAction Stop | Out-Null
        return $true
    }
    catch {
        return $false
    }
}

function Install-Chocolatey {
    Write-Info "Installing Chocolatey package manager..."
    
    try {
        Set-ExecutionPolicy Bypass -Scope Process -Force
        [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
        Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
        
        # Refresh environment variables
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
        
        Write-Success "Chocolatey installed successfully!"
        return $true
    }
    catch {
        Write-Error "Failed to install Chocolatey: $($_.Exception.Message)"
        return $false
    }
}

function Install-Dependencies {
    Write-Info "Installing development dependencies..."
    
    # Check if Chocolatey is installed
    if (-not (Test-CommandExists "choco")) {
        if (-not (Install-Chocolatey)) {
            return $false
        }
    }
    
    # Install essential development tools
    $packages = @(
        "cmake",
        "nasm", 
        "protoc",
        "git",
        "microsoft-visual-cpp-build-tools",
        "redis-64",
        "mysql"
    )
    
    foreach ($package in $packages) {
        Write-Info "Installing $package..."
        try {
            choco install $package -y --no-progress
            Write-Success "$package installed successfully!"
        }
        catch {
            Write-Warning "Failed to install $package, continuing..."
        }
    }
    
    # Install Rust
    if (-not (Test-CommandExists "rustc")) {
        Write-Info "Installing Rust..."
        try {
            choco install rust -y --no-progress
            Write-Success "Rust installed successfully!"
        }
        catch {
            Write-Warning "Failed to install Rust via Chocolatey, trying rustup..."
            
            # Download and install rustup
            $rustupInit = "$env:TEMP\rustup-init.exe"
            Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -OutFile $rustupInit
            
            Start-Process -FilePath $rustupInit -ArgumentList "-y" -Wait
            Remove-Item $rustupInit
        }
    }
    
    # Refresh environment variables
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
    
    # Install cargo-audit
    if (Test-CommandExists "cargo") {
        Write-Info "Installing cargo-audit..."
        try {
            cargo install cargo-audit
            Write-Success "cargo-audit installed successfully!"
        }
        catch {
            Write-Warning "Failed to install cargo-audit"
        }
    }
    
    return $true
}

function Start-Services {
    Write-Info "Starting required services..."
    
    # Start Redis service if installed
    try {
        $redisService = Get-Service -Name "Redis" -ErrorAction SilentlyContinue
        if ($redisService) {
            if ($redisService.Status -ne "Running") {
                Start-Service -Name "Redis"
                Write-Success "Redis service started!"
            } else {
                Write-Success "Redis service is already running!"
            }
        } else {
            Write-Warning "Redis service not found - you may need to configure it manually"
        }
    }
    catch {
        Write-Warning "Could not start Redis service: $($_.Exception.Message)"
    }
    
    # Start MySQL service if installed
    try {
        $mysqlService = Get-Service -Name "MySQL*" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($mysqlService) {
            if ($mysqlService.Status -ne "Running") {
                Start-Service -Name $mysqlService.Name
                Write-Success "MySQL service started!"
            } else {
                Write-Success "MySQL service is already running!"
            }
        } else {
            Write-Warning "MySQL service not found - you may need to configure it manually"
        }
    }
    catch {
        Write-Warning "Could not start MySQL service: $($_.Exception.Message)"
    }
}

function Test-Installation {
    Write-Info "Verifying installation..."
    
    $tools = @("rustc", "cargo", "cmake", "nasm", "protoc", "git")
    $missing = @()
    
    foreach ($tool in $tools) {
        if (-not (Test-CommandExists $tool)) {
            $missing += $tool
        }
    }
    
    if ($missing.Count -eq 0) {
        Write-Success "All essential tools are installed!"
        return $true
    } else {
        Write-Error "Missing tools: $($missing -join ', ')"
        Write-Info "You may need to restart your PowerShell session or reboot your computer"
        return $false
    }
}

function Test-Build {
    Write-Info "Testing build..."
    
    try {
        # Test shared library build
        $result = cargo check -p shared --lib 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Success "Shared library builds successfully!"
        } else {
            Write-Error "Shared library build failed!"
            Write-Info "Output: $result"
            return $false
        }
    }
    catch {
        Write-Error "Build test failed: $($_.Exception.Message)"
        return $false
    }
    
    return $true
}

function Show-Help {
    Write-Host @"
Police Thief Game Server - Windows Dependency Setup

USAGE:
    .\setup-deps.ps1 [OPTIONS]

OPTIONS:
    -Mode <mode>     Setup mode (auto, install, verify, test)
    -Help           Show this help message

REQUIREMENTS:
    - PowerShell 5.0 or later
    - Administrator privileges
    - Internet connection

DESCRIPTION:
    This script installs all necessary dependencies for building and running
    the Police Thief game server on Windows systems.

    Dependencies installed:
    - Chocolatey package manager
    - CMake build system
    - NASM assembler
    - Protocol Buffers compiler
    - Rust programming language
    - Git version control
    - Visual C++ Build Tools
    - Redis database
    - MySQL database

EXAMPLES:
    .\setup-deps.ps1                 # Full setup
    .\setup-deps.ps1 -Mode verify    # Verify installation only
    .\setup-deps.ps1 -Help           # Show this help

"@
}

function Main {
    if ($Help) {
        Show-Help
        return
    }
    
    Write-Info "Police Thief Game Server - Windows Setup"
    Write-Info "=========================================="
    
    # Check if running as administrator
    if (-not (Test-Administrator)) {
        Write-Error "This script must be run as Administrator!"
        Write-Info "Right-click PowerShell and select 'Run as Administrator'"
        return
    }
    
    switch ($Mode.ToLower()) {
        "auto" {
            if (Install-Dependencies) {
                Start-Services
                if (Test-Installation) {
                    Write-Success "Setup completed successfully!"
                    
                    if (Test-Build) {
                        Write-Info "🚀 You can now build the project with:"
                        Write-Info "   cargo build --release"
                        Write-Info "   cargo run -p gamecenter start"
                    }
                } else {
                    Write-Error "Setup completed with errors"
                }
            }
        }
        "install" {
            Install-Dependencies
        }
        "verify" {
            Test-Installation
        }
        "test" {
            Test-Build
        }
        default {
            Write-Error "Unknown mode: $Mode"
            Show-Help
        }
    }
}

# Run main function
Main