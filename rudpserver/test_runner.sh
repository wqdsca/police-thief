#!/bin/bash

# RUDP Server Test Runner for Linux/Mac
# Optimized for 1vCPU, 1GB RAM environment

echo "====================================="
echo "RUDP Server Test Suite"
echo "Target: 300 concurrent players"
echo "Environment: 1vCPU, 1GB RAM"
echo "====================================="
echo ""

export RUST_BACKTRACE=1
export RUST_LOG=rudpserver=debug

show_menu() {
    echo -e "\033[33mSelect test to run:\033[0m"
    echo "1. Quick Smoke Test (< 1 minute)"
    echo "2. Unit Tests Only"
    echo "3. Integration Tests Only"
    echo "4. Load Test - 300 Players (2 minutes)"
    echo "5. Stress Tests (5 minutes)"
    echo "6. Performance Benchmarks (10 minutes)"
    echo "7. Full Test Suite (20 minutes)"
    echo "8. Continuous Load Test (until stopped)"
    echo "9. Resource Monitor"
    echo "0. Exit"
    echo ""
}

run_test() {
    local test_name="$1"
    local command="$2"
    
    echo -e "\n\033[32mRunning: $test_name\033[0m"
    echo -e "\033[90mCommand: $command\033[0m"
    echo "-------------------------------------"
    
    start_time=$(date +%s)
    eval "$command"
    end_time=$(date +%s)
    
    duration=$((end_time - start_time))
    echo -e "\n\033[32mTest completed in: ${duration} seconds\033[0m"
    echo "-------------------------------------"
}

monitor_resources() {
    echo -e "\033[33mMonitoring RUDP Server Resources (Press Ctrl+C to stop)...\033[0m"
    
    while true; do
        clear
        echo -e "\033[36mRUDP Server Resource Monitor\033[0m"
        echo "============================="
        
        if pgrep rudpserver > /dev/null; then
            pid=$(pgrep rudpserver | head -1)
            
            # Get CPU usage
            cpu=$(ps -p $pid -o %cpu= | tr -d ' ')
            
            # Get memory usage in MB
            if [[ "$OSTYPE" == "darwin"* ]]; then
                mem_kb=$(ps -p $pid -o rss= | tr -d ' ')
                mem_mb=$((mem_kb / 1024))
            else
                mem_mb=$(ps -p $pid -o rss= | awk '{print int($1/1024)}')
            fi
            
            # Get thread count
            if [[ "$OSTYPE" == "darwin"* ]]; then
                threads=$(ps -M $pid | wc -l)
            else
                threads=$(ps -T -p $pid | wc -l)
            fi
            
            echo "PID: $pid"
            echo "CPU: ${cpu}%"
            echo "Memory: ${mem_mb} MB"
            echo "Threads: $threads"
            echo ""
            echo -e "\033[33mTarget: 1vCPU, 1GB RAM\033[0m"
            
            mem_percent=$((mem_mb * 100 / 1024))
            if [ $mem_mb -gt 900 ]; then
                echo -e "\033[31mMemory Usage: ${mem_percent}% of 1GB limit\033[0m"
            elif [ $mem_mb -gt 700 ]; then
                echo -e "\033[33mMemory Usage: ${mem_percent}% of 1GB limit\033[0m"
            else
                echo -e "\033[32mMemory Usage: ${mem_percent}% of 1GB limit\033[0m"
            fi
        else
            echo -e "\033[31mRUDP Server not running\033[0m"
        fi
        
        sleep 1
    done
}

# Main loop
while true; do
    show_menu
    read -p "Enter your choice: " choice
    
    case $choice in
        1)
            run_test "Quick Smoke Test" "cargo test -p rudpserver quick_smoke_test"
            ;;
        2)
            run_test "Unit Tests" "cargo test -p rudpserver --test protocol_tests"
            ;;
        3)
            run_test "Integration Tests" "cargo test -p rudpserver --test server_integration_tests"
            ;;
        4)
            run_test "Load Test - 300 Players" "cargo test -p rudpserver test_load_300_players -- --nocapture"
            ;;
        5)
            run_test "Stress Tests" "cargo test -p rudpserver run_all_stress_tests -- --nocapture --test-threads=1"
            ;;
        6)
            run_test "Performance Benchmarks" "cargo bench -p rudpserver"
            ;;
        7)
            run_test "Full Test Suite" "cargo test -p rudpserver full_test_suite -- --ignored --nocapture --test-threads=1"
            ;;
        8)
            echo -e "\n\033[33mStarting continuous load test (Press Ctrl+C to stop)...\033[0m"
            cargo run --release --bin rudp_load_test
            ;;
        9)
            monitor_resources
            ;;
        0)
            echo -e "\033[36mExiting test runner...\033[0m"
            exit 0
            ;;
        *)
            echo -e "\033[31mInvalid choice. Please try again.\033[0m"
            ;;
    esac
    
    if [ "$choice" != "0" ] && [ "$choice" != "9" ]; then
        echo ""
        read -p "Press Enter to continue..."
        clear
    fi
done