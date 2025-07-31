use anyhow::Result;
use std::time::Duration;

use super::workload::{WorkloadConfig, WorkloadPattern, KeyPattern};
use super::transport_tests::{Transport, run_transport_benchmark};

pub async fn run_store_comparison(base_workload: WorkloadConfig) -> Result<()> {
    println!("\n=== Store Type Performance Comparison ===\n");
    
    let store_types = ["periodic", "probabilistic", "adaptive"];
    
    // Test 1: Steady load comparison
    println!("Test 1: Steady Load Performance");
    println!("--------------------------------");
    
    for store in &store_types {
        run_transport_benchmark(Transport::Native, store, base_workload.clone()).await?;
    }
    
    // Test 2: Memory efficiency test (high key cardinality)
    println!("\nTest 2: Memory Efficiency (1M unique keys)");
    println!("------------------------------------------");
    
    let high_cardinality_workload = WorkloadConfig {
        pattern: WorkloadPattern::Steady { rps: 5000 },
        duration: Duration::from_secs(30),
        key_space_size: 1_000_000,
        key_pattern: KeyPattern::Sequential,
    };
    
    for store in &store_types {
        run_transport_benchmark(Transport::Native, store, high_cardinality_workload.clone()).await?;
    }
    
    // Test 3: Cleanup efficiency test (keys with short TTL)
    println!("\nTest 3: Cleanup Efficiency (Short TTL keys)");
    println!("--------------------------------------------");
    
    let short_ttl_workload = WorkloadConfig {
        pattern: WorkloadPattern::Steady { rps: 10000 },
        duration: Duration::from_secs(60),
        key_space_size: 100_000,
        key_pattern: KeyPattern::Random { pool_size: 100_000 },
    };
    
    for store in &store_types {
        println!("\nTesting {} store with short TTL keys", store);
        run_transport_benchmark(Transport::Native, store, short_ttl_workload.clone()).await?;
    }
    
    // Test 4: Burst handling comparison
    println!("\nTest 4: Burst Handling");
    println!("----------------------");
    
    let burst_workload = WorkloadConfig {
        pattern: WorkloadPattern::Burst {
            high_rps: 50_000,
            low_rps: 1_000,
            burst_duration: Duration::from_secs(5),
            quiet_duration: Duration::from_secs(10),
        },
        duration: Duration::from_secs(45),
        key_space_size: 10_000,
        key_pattern: KeyPattern::Random { pool_size: 10_000 },
    };
    
    for store in &store_types {
        run_transport_benchmark(Transport::Native, store, burst_workload.clone()).await?;
    }
    
    // Test 5: Hotspot handling (Zipfian distribution)
    println!("\nTest 5: Hotspot Handling (Zipfian distribution)");
    println!("------------------------------------------------");
    
    let hotspot_workload = WorkloadConfig {
        pattern: WorkloadPattern::Steady { rps: 20_000 },
        duration: Duration::from_secs(30),
        key_space_size: 10_000,
        key_pattern: KeyPattern::Zipfian { alpha: 1.5 },
    };
    
    for store in &store_types {
        run_transport_benchmark(Transport::Native, store, hotspot_workload.clone()).await?;
    }
    
    println!("\n=== Store Comparison Summary ===");
    println!("\nRecommendations:");
    println!("- Periodic: Best for predictable workloads with consistent key distribution");
    println!("- Probabilistic: Good for high-throughput scenarios with random access patterns");
    println!("- Adaptive: Optimal for variable workloads that need to balance cleanup overhead");
    
    Ok(())
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_store_comparison_quick() -> Result<()> {
        let workload = WorkloadConfig {
            pattern: WorkloadPattern::Steady { rps: 1000 },
            duration: Duration::from_secs(5),
            key_space_size: 1000,
            key_pattern: KeyPattern::Random { pool_size: 1000 },
        };
        
        run_store_comparison(workload).await?;
        Ok(())
    }
}