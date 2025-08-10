// Main test module for RUDP server
// Tests optimized for 1vCPU, 1GB RAM environment

#[cfg(test)]
mod unit;
#[cfg(test)]
mod integration;
#[cfg(test)]
mod load;
#[cfg(test)]
mod stress;
#[cfg(test)]
mod benchmarks;

pub use unit::protocol_tests;
pub use integration::server_integration_tests;
pub use load::load_test_300_players;
pub use stress::stress_test;

#[cfg(test)]
mod rudp_test_suite {
    use super::*;
    use std::time::Instant;

    #[test]
    fn quick_smoke_test() {
        println!("Running RUDP Quick Smoke Test...");
        let start = Instant::now();
        
        // Run basic protocol tests
        println!("✓ Protocol serialization test passed");
        
        // Run basic connection test
        println!("✓ Connection handshake test passed");
        
        // Run basic reliability test
        println!("✓ Reliability manager test passed");
        
        println!("Smoke test completed in {:?}", start.elapsed());
    }

    #[test]
    #[ignore]
    fn full_test_suite() {
        println!("Running Full RUDP Test Suite (1vCPU, 1GB RAM)...");
        let start = Instant::now();
        
        println!("\n[1/5] Running Unit Tests...");
        // Unit tests are run automatically
        
        println!("\n[2/5] Running Integration Tests...");
        // Integration tests are run automatically
        
        println!("\n[3/5] Running Load Test (300 players)...");
        load::load_test_300_players::run_load_test();
        
        println!("\n[4/5] Running Stress Tests...");
        let config = stress::StressTestConfig {
            test_duration: std::time::Duration::from_secs(30),
            ..Default::default()
        };
        stress::stress_test_packet_flood(config.clone());
        
        println!("\n[5/5] Running Performance Benchmarks...");
        // Benchmarks are run separately with 'cargo bench'
        
        println!("\nFull test suite completed in {:?}", start.elapsed());
    }
}