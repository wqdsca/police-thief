//! 리소스 제한 환경 테스트 도구
//! 
//! 1vCPU, 0.5GB RAM 제한 환경에서의 성능 테스트를 위한 도구입니다.
//! 시스템 리소스 모니터링 및 제한 시뮬레이션을 제공합니다.

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::interval;
use sysinfo::{System, SystemExt, ProcessExt, CpuExt, PidExt};
use anyhow::Result;
use tracing::{info, warn, error};

/// 리소스 제약 설정
#[derive(Debug, Clone)]
pub struct ResourceConstraints {
    /// 최대 메모리 사용량 (MB)
    pub max_memory_mb: u64,
    /// 최대 CPU 사용률 (%)
    pub max_cpu_percent: f32,
    /// 모니터링 간격 (초)
    pub monitoring_interval_secs: u64,
    /// 경고 임계값
    pub warning_thresholds: WarningThresholds,
}

impl Default for ResourceConstraints {
    fn default() -> Self {
        Self {
            max_memory_mb: 512, // 0.5GB
            max_cpu_percent: 100.0, // 1vCPU = 100%
            monitoring_interval_secs: 2,
            warning_thresholds: WarningThresholds::default(),
        }
    }
}

/// 경고 임계값 설정
#[derive(Debug, Clone)]
pub struct WarningThresholds {
    /// 메모리 경고 임계값 (%)
    pub memory_warning_percent: f32,
    /// CPU 경고 임계값 (%)
    pub cpu_warning_percent: f32,
    /// 위험 임계값 (%)
    pub critical_percent: f32,
}

impl Default for WarningThresholds {
    fn default() -> Self {
        Self {
            memory_warning_percent: 70.0,
            cpu_warning_percent: 80.0,
            critical_percent: 95.0,
        }
    }
}

/// 리소스 모니터링 상태
#[derive(Debug, Clone)]
pub struct ResourceStatus {
    pub current_memory_mb: u64,
    pub current_cpu_percent: f32,
    pub memory_usage_percent: f32,
    pub cpu_usage_percent: f32,
    pub is_memory_warning: bool,
    pub is_cpu_warning: bool,
    pub is_critical: bool,
    pub uptime_seconds: u64,
}

/// 리소스 통계
#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub avg_memory_mb: f64,
    pub max_memory_mb: u64,
    pub avg_cpu_percent: f64,
    pub max_cpu_percent: f32,
    pub warning_count: u32,
    pub critical_count: u32,
    pub total_samples: u32,
}

/// 제약 환경 시뮬레이터
pub struct ResourceConstraintSimulator {
    constraints: ResourceConstraints,
    system: Arc<Mutex<System>>,
    monitoring_active: Arc<AtomicBool>,
    stats: Arc<Mutex<Vec<ResourceStatus>>>,
    start_time: Instant,
}

impl ResourceConstraintSimulator {
    pub fn new(constraints: ResourceConstraints) -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        Self {
            constraints,
            system: Arc::new(Mutex::new(system)),
            monitoring_active: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
        }
    }
    
    /// 리소스 모니터링 시작
    pub async fn start_monitoring(&self) -> tokio::task::JoinHandle<()> {
        self.monitoring_active.store(true, Ordering::Relaxed);
        
        let constraints = self.constraints.clone();
        let system = self.system.clone();
        let monitoring_active = self.monitoring_active.clone();
        let stats = self.stats.clone();
        let start_time = self.start_time;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(constraints.monitoring_interval_secs));
            let mut warning_count = 0u32;
            let mut critical_count = 0u32;
            
            while monitoring_active.load(Ordering::Relaxed) {
                interval.tick().await;
                
                let mut sys = system.lock().await;
                sys.refresh_all();
                
                let status = Self::collect_resource_status(&sys, &constraints, start_time);
                
                // 경고 및 위험 상태 체크
                if status.is_critical {
                    critical_count += 1;
                    error!("🚨 리소스 위험: 메모리 {:.1}%, CPU {:.1}%", 
                          status.memory_usage_percent, status.cpu_usage_percent);
                } else if status.is_memory_warning || status.is_cpu_warning {
                    warning_count += 1;
                    warn!("⚠️ 리소스 경고: 메모리 {:.1}%, CPU {:.1}%", 
                         status.memory_usage_percent, status.cpu_usage_percent);
                }
                
                // 상태 기록
                stats.lock().await.push(status.clone());
                
                // 주기적 상태 출력
                if stats.lock().await.len() % 15 == 0 { // 30초마다 (2초 간격 * 15)
                    info!("📊 리소스 상태: 메모리 {}MB ({:.1}%), CPU {:.1}%, 가동시간 {}초", 
                         status.current_memory_mb, 
                         status.memory_usage_percent, 
                         status.current_cpu_percent,
                         status.uptime_seconds);
                }
            }
            
            info!("리소스 모니터링 종료: 경고 {} 회, 위험 {} 회", warning_count, critical_count);
        })
    }
    
    /// 현재 리소스 상태 수집
    fn collect_resource_status(
        system: &System, 
        constraints: &ResourceConstraints, 
        start_time: Instant
    ) -> ResourceStatus {
        // 시스템 전체 메모리 사용량
        let total_memory_kb = system.total_memory();
        let used_memory_kb = system.used_memory();
        let current_memory_mb = used_memory_kb / 1024;
        
        // CPU 사용률
        let current_cpu_percent = system.global_cpu_info().cpu_usage();
        
        // 사용률 계산
        let memory_usage_percent = (current_memory_mb as f32 / constraints.max_memory_mb as f32) * 100.0;
        let cpu_usage_percent = current_cpu_percent / constraints.max_cpu_percent * 100.0;
        
        // 경고 및 위험 상태 판단
        let is_memory_warning = memory_usage_percent > constraints.warning_thresholds.memory_warning_percent;
        let is_cpu_warning = cpu_usage_percent > constraints.warning_thresholds.cpu_warning_percent;
        let is_critical = memory_usage_percent > constraints.warning_thresholds.critical_percent ||
                         cpu_usage_percent > constraints.warning_thresholds.critical_percent;
        
        let uptime_seconds = start_time.elapsed().as_secs();
        
        ResourceStatus {
            current_memory_mb,
            current_cpu_percent,
            memory_usage_percent,
            cpu_usage_percent,
            is_memory_warning,
            is_cpu_warning,
            is_critical,
            uptime_seconds,
        }
    }
    
    /// 모니터링 중지
    pub async fn stop_monitoring(&self) {
        self.monitoring_active.store(false, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    /// 통계 수집
    pub async fn get_stats(&self) -> ResourceStats {
        let stats_vec = self.stats.lock().await;
        
        if stats_vec.is_empty() {
            return ResourceStats {
                avg_memory_mb: 0.0,
                max_memory_mb: 0,
                avg_cpu_percent: 0.0,
                max_cpu_percent: 0.0,
                warning_count: 0,
                critical_count: 0,
                total_samples: 0,
            };
        }
        
        let total_samples = stats_vec.len() as u32;
        
        let total_memory: u64 = stats_vec.iter().map(|s| s.current_memory_mb).sum();
        let avg_memory_mb = total_memory as f64 / total_samples as f64;
        let max_memory_mb = stats_vec.iter().map(|s| s.current_memory_mb).max().unwrap_or(0);
        
        let total_cpu: f32 = stats_vec.iter().map(|s| s.current_cpu_percent).sum();
        let avg_cpu_percent = (total_cpu / total_samples as f32) as f64;
        let max_cpu_percent = stats_vec.iter()
            .map(|s| s.current_cpu_percent)
            .fold(0.0f32, |a, b| a.max(b));
        
        let warning_count = stats_vec.iter()
            .filter(|s| s.is_memory_warning || s.is_cpu_warning)
            .count() as u32;
        
        let critical_count = stats_vec.iter()
            .filter(|s| s.is_critical)
            .count() as u32;
        
        ResourceStats {
            avg_memory_mb,
            max_memory_mb,
            avg_cpu_percent,
            max_cpu_percent,
            warning_count,
            critical_count,
            total_samples,
        }
    }
    
    /// 제약 환경 준수 여부 평가
    pub async fn evaluate_compliance(&self) -> ComplianceReport {
        let stats = self.get_stats().await;
        
        let memory_compliance = stats.max_memory_mb <= self.constraints.max_memory_mb;
        let cpu_compliance = stats.max_cpu_percent <= self.constraints.max_cpu_percent;
        
        let overall_compliance = memory_compliance && cpu_compliance;
        
        let memory_efficiency = if self.constraints.max_memory_mb > 0 {
            (stats.avg_memory_mb / self.constraints.max_memory_mb as f64) * 100.0
        } else {
            0.0
        };
        
        let cpu_efficiency = (stats.avg_cpu_percent / self.constraints.max_cpu_percent as f64) * 100.0;
        
        // 성능 점수 계산 (0-100)
        let performance_score = if overall_compliance {
            let stability_score = if stats.critical_count == 0 { 30.0 } else { 0.0 };
            let efficiency_score = (100.0 - memory_efficiency.min(100.0)) / 100.0 * 35.0;
            let cpu_score = (100.0 - cpu_efficiency.min(100.0)) / 100.0 * 35.0;
            stability_score + efficiency_score + cpu_score
        } else {
            0.0
        };
        
        ComplianceReport {
            overall_compliance,
            memory_compliance,
            cpu_compliance,
            memory_efficiency,
            cpu_efficiency,
            performance_score,
            warning_percentage: (stats.warning_count as f64 / stats.total_samples as f64) * 100.0,
            critical_percentage: (stats.critical_count as f64 / stats.total_samples as f64) * 100.0,
            stats,
        }
    }
    
    /// 상세 보고서 출력
    pub async fn print_detailed_report(&self) {
        let report = self.evaluate_compliance().await;
        
        println!("📊 리소스 제약 환경 테스트 결과");
        println!("═══════════════════════════════════════════════════════════════════");
        
        // 제약 설정 출력
        println!("🎯 제약 설정:");
        println!("   최대 메모리: {} MB", self.constraints.max_memory_mb);
        println!("   최대 CPU: {:.1}%", self.constraints.max_cpu_percent);
        
        // 사용량 통계
        println!("📈 리소스 사용량:");
        println!("   평균 메모리: {:.1} MB ({:.1}%)", report.stats.avg_memory_mb, report.memory_efficiency);
        println!("   최대 메모리: {} MB", report.stats.max_memory_mb);
        println!("   평균 CPU: {:.1}%", report.stats.avg_cpu_percent);
        println!("   최대 CPU: {:.1}%", report.stats.max_cpu_percent);
        
        // 준수 여부
        println!("✅ 준수 여부:");
        println!("   전체 준수: {}", if report.overall_compliance { "✅" } else { "❌" });
        println!("   메모리 준수: {}", if report.memory_compliance { "✅" } else { "❌" });
        println!("   CPU 준수: {}", if report.cpu_compliance { "✅" } else { "❌" });
        
        // 안정성
        println!("⚠️ 안정성:");
        println!("   경고 발생률: {:.1}%", report.warning_percentage);
        println!("   위험 발생률: {:.1}%", report.critical_percentage);
        println!("   총 샘플 수: {}", report.stats.total_samples);
        
        // 성능 점수
        println!("🏆 종합 성능 점수: {:.1}/100", report.performance_score);
        
        let grade = match report.performance_score as u32 {
            90..=100 => "A+ (최적)",
            80..=89 => "A (우수)",
            70..=79 => "B (양호)",
            60..=69 => "C (보통)",
            _ => "D (개선 필요)",
        };
        
        println!("🎖️ 성능 등급: {}", grade);
        println!("═══════════════════════════════════════════════════════════════════");
    }
}

/// 준수 보고서
#[derive(Debug)]
pub struct ComplianceReport {
    pub overall_compliance: bool,
    pub memory_compliance: bool,
    pub cpu_compliance: bool,
    pub memory_efficiency: f64,
    pub cpu_efficiency: f64,
    pub performance_score: f64,
    pub warning_percentage: f64,
    pub critical_percentage: f64,
    pub stats: ResourceStats,
}

/// 통합 리소스 제약 테스트
pub async fn run_resource_constraint_test<F, Fut>(
    test_name: &str,
    constraints: ResourceConstraints,
    test_function: F,
) -> Result<ComplianceReport>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    info!("🎯 리소스 제약 테스트 시작: {}", test_name);
    
    let simulator = ResourceConstraintSimulator::new(constraints);
    let monitor_handle = simulator.start_monitoring().await;
    
    // 테스트 실행
    let test_result = test_function().await;
    
    // 모니터링 중지
    simulator.stop_monitoring().await;
    monitor_handle.abort();
    
    // 결과 분석
    let report = simulator.evaluate_compliance().await;
    
    info!("테스트 '{}' 완료: 준수={}, 점수={:.1}", 
         test_name, report.overall_compliance, report.performance_score);
    
    if let Err(e) = test_result {
        warn!("테스트 실행 중 오류: {}", e);
    }
    
    Ok(report)
}

#[tokio::test]
async fn test_resource_constraint_compliance() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    
    let constraints = ResourceConstraints::default();
    
    let report = run_resource_constraint_test(
        "기본 리소스 제약 테스트",
        constraints,
        || async {
            // 가벼운 워크로드 시뮬레이션
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 메모리 할당 테스트
            let _data: Vec<u8> = vec![0; 10 * 1024 * 1024]; // 10MB
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            Ok(())
        },
    ).await?;
    
    // 기본적인 어설션
    assert!(report.stats.total_samples > 0);
    assert!(report.memory_efficiency >= 0.0);
    assert!(report.cpu_efficiency >= 0.0);
    
    Ok(())
}

#[tokio::test]
#[ignore = "대규모 리소스 테스트"]
async fn test_heavy_workload_constraint() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    
    let constraints = ResourceConstraints::default();
    
    let report = run_resource_constraint_test(
        "고부하 워크로드 제약 테스트",
        constraints,
        || async {
            // CPU 집약적 작업
            let cpu_task = tokio::spawn(async {
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(10) {
                    // CPU 사용률 증가를 위한 계산
                    let _: f64 = (0..1000).map(|i| (i as f64).sqrt()).sum();
                    tokio::task::yield_now().await;
                }
            });
            
            // 메모리 집약적 작업
            let memory_task = tokio::spawn(async {
                let mut data = Vec::new();
                for _ in 0..50 {
                    data.push(vec![0u8; 1024 * 1024]); // 1MB씩 할당
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            });
            
            // 두 작업 완료 대기
            let (cpu_result, memory_result) = tokio::join!(cpu_task, memory_task);
            cpu_result.unwrap();
            memory_result.unwrap();
            
            Ok(())
        },
    ).await?;
    
    // 고부하 테스트 결과 검증
    assert!(report.stats.max_memory_mb > 50); // 최소 50MB는 사용했어야 함
    assert!(report.stats.max_cpu_percent > 10.0); // CPU도 어느 정도 사용
    
    println!("고부하 테스트 완료: 메모리 최대 {}MB, CPU 최대 {:.1}%", 
             report.stats.max_memory_mb, report.stats.max_cpu_percent);
    
    Ok(())
}