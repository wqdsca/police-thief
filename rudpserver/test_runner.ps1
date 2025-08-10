# RUDP Server Test Runner for Windows
# Optimized for 1vCPU, 1GB RAM environment

Write-Host "=====================================
RUDP Server Test Suite
Target: 300 concurrent players
Environment: 1vCPU, 1GB RAM
=====================================`n" -ForegroundColor Cyan

$env:RUST_BACKTRACE = "1"
$env:RUST_LOG = "rudpserver=debug"

function Show-Menu {
    Write-Host "Select test to run:" -ForegroundColor Yellow
    Write-Host "1. Quick Smoke Test (< 1 minute)"
    Write-Host "2. Unit Tests Only"
    Write-Host "3. Integration Tests Only"
    Write-Host "4. Load Test - 300 Players (2 minutes)"
    Write-Host "5. Stress Tests (5 minutes)"
    Write-Host "6. Performance Benchmarks (10 minutes)"
    Write-Host "7. Full Test Suite (20 minutes)"
    Write-Host "8. Continuous Load Test (until stopped)"
    Write-Host "9. Resource Monitor"
    Write-Host "0. Exit"
    Write-Host ""
}

function Run-Test {
    param($TestName, $Command)
    
    Write-Host "`nRunning: $TestName" -ForegroundColor Green
    Write-Host "Command: $Command" -ForegroundColor Gray
    Write-Host "-------------------------------------" -ForegroundColor Gray
    
    $startTime = Get-Date
    Invoke-Expression $Command
    $endTime = Get-Date
    
    $duration = $endTime - $startTime
    Write-Host "`nTest completed in: $($duration.ToString())" -ForegroundColor Green
    Write-Host "-------------------------------------`n" -ForegroundColor Gray
}

function Monitor-Resources {
    Write-Host "Monitoring RUDP Server Resources (Press Ctrl+C to stop)..." -ForegroundColor Yellow
    
    while ($true) {
        $process = Get-Process -Name "rudpserver" -ErrorAction SilentlyContinue
        
        if ($process) {
            $cpu = [math]::Round($process.CPU, 2)
            $memMB = [math]::Round($process.WorkingSet64 / 1MB, 2)
            $threads = $process.Threads.Count
            $handles = $process.HandleCount
            
            Clear-Host
            Write-Host "RUDP Server Resource Monitor" -ForegroundColor Cyan
            Write-Host "=============================" -ForegroundColor Cyan
            Write-Host "CPU Time: $cpu seconds"
            Write-Host "Memory: $memMB MB"
            Write-Host "Threads: $threads"
            Write-Host "Handles: $handles"
            Write-Host ""
            Write-Host "Target: 1vCPU, 1GB RAM" -ForegroundColor Yellow
            Write-Host "Memory Usage: $([math]::Round($memMB / 1024 * 100, 1))% of 1GB limit" -ForegroundColor $(if ($memMB -gt 900) {"Red"} elseif ($memMB -gt 700) {"Yellow"} else {"Green"})
        } else {
            Write-Host "RUDP Server not running" -ForegroundColor Red
        }
        
        Start-Sleep -Seconds 1
    }
}

# Main loop
do {
    Show-Menu
    $choice = Read-Host "Enter your choice"
    
    switch ($choice) {
        "1" {
            Run-Test "Quick Smoke Test" "cargo test -p rudpserver quick_smoke_test"
        }
        "2" {
            Run-Test "Unit Tests" "cargo test -p rudpserver --test protocol_tests"
        }
        "3" {
            Run-Test "Integration Tests" "cargo test -p rudpserver --test server_integration_tests"
        }
        "4" {
            Run-Test "Load Test - 300 Players" "cargo test -p rudpserver test_load_300_players -- --nocapture"
        }
        "5" {
            Run-Test "Stress Tests" "cargo test -p rudpserver run_all_stress_tests -- --nocapture --test-threads=1"
        }
        "6" {
            Run-Test "Performance Benchmarks" "cargo bench -p rudpserver"
        }
        "7" {
            Run-Test "Full Test Suite" "cargo test -p rudpserver full_test_suite -- --ignored --nocapture --test-threads=1"
        }
        "8" {
            Write-Host "`nStarting continuous load test (Press Ctrl+C to stop)..." -ForegroundColor Yellow
            cargo run --release --bin rudp_load_test
        }
        "9" {
            Monitor-Resources
        }
        "0" {
            Write-Host "Exiting test runner..." -ForegroundColor Cyan
        }
        default {
            Write-Host "Invalid choice. Please try again." -ForegroundColor Red
        }
    }
    
    if ($choice -ne "0" -and $choice -ne "9") {
        Write-Host "`nPress Enter to continue..."
        Read-Host
        Clear-Host
    }
    
} while ($choice -ne "0")

Write-Host "`nTest runner terminated." -ForegroundColor Cyan