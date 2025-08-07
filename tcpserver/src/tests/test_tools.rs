//! 도구(Tool) 레이어 테스트
//! 
//! 공통 유틸리티 함수들의 기능 테스트

use crate::tool::SimpleUtils;

/// 타임스탬프 생성 테스트
#[tokio::test]
async fn test_current_timestamp() {
    let timestamp1 = SimpleUtils::current_timestamp();
    
    // 기본 검증
    assert!(timestamp1 > 0, "타임스탬프는 양수여야 함");
    assert!(timestamp1 > 1_600_000_000, "타임스탬프는 2020년 이후여야 함");
    
    // 시간 간격 테스트
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let timestamp2 = SimpleUtils::current_timestamp();
    assert!(timestamp2 >= timestamp1, "시간은 증가해야 함");
    
    println!("✅ 타임스탬프 생성 테스트 통과: {}", timestamp1);
}

/// 바이트 → 16진수 변환 테스트
#[tokio::test]
async fn test_bytes_to_hex() {
    let test_cases = vec![
        (vec![0x00], "00"),
        (vec![0xFF], "ff"),
        (vec![0x12, 0x34], "1234"),
        (vec![0xAB, 0xCD, 0xEF], "abcdef"),
        (vec![], ""),
    ];
    
    for (bytes, expected) in test_cases {
        let result = SimpleUtils::bytes_to_hex(&bytes);
        assert_eq!(result, expected, "변환 결과가 예상과 다름: {:?} → {}", bytes, result);
    }
    
    println!("✅ 바이트 → 16진수 변환 테스트 통과");
}

/// 16진수 → 바이트 변환 테스트  
#[tokio::test]
async fn test_hex_to_bytes() {
    let test_cases = vec![
        ("00", vec![0x00]),
        ("ff", vec![0xFF]),
        ("1234", vec![0x12, 0x34]),
        ("ABCDEF", vec![0xAB, 0xCD, 0xEF]),
        ("0x1234", vec![0x12, 0x34]), // 0x 접두사
        ("", vec![]),
    ];
    
    for (hex, expected) in test_cases {
        let result = SimpleUtils::hex_to_bytes(hex).unwrap();
        assert_eq!(result, expected, "변환 결과가 예상과 다름: {} → {:?}", hex, result);
    }
    
    println!("✅ 16진수 → 바이트 변환 테스트 통과");
}

/// 16진수 변환 에러 케이스 테스트
#[tokio::test]
async fn test_hex_conversion_errors() {
    let error_cases = vec![
        "123",      // 홀수 길이
        "GG",       // 잘못된 16진수 문자
        "1Z34",     // 잘못된 문자 포함
    ];
    
    for hex in error_cases {
        let result = SimpleUtils::hex_to_bytes(hex);
        assert!(result.is_err(), "잘못된 16진수는 에러를 반환해야 함: {}", hex);
    }
    
    println!("✅ 16진수 변환 에러 케이스 테스트 통과");
}

/// 바이트 ↔ 16진수 라운드트립 테스트
#[tokio::test]
async fn test_hex_roundtrip() {
    let original_data = vec![
        vec![0x00, 0x01, 0x02, 0x03],
        vec![0xFF, 0xFE, 0xFD, 0xFC],
        vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
        vec![],
    ];
    
    for data in original_data {
        // 바이트 → 16진수 → 바이트
        let hex = SimpleUtils::bytes_to_hex(&data);
        let converted_back = SimpleUtils::hex_to_bytes(&hex).unwrap();
        
        assert_eq!(data, converted_back, "라운드트립 변환에서 데이터가 보존되어야 함");
    }
    
    println!("✅ 16진수 라운드트립 테스트 통과");
}