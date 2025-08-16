// use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Local};

pub struct CurrentTime {
    pub current_time: String,
}

impl CurrentTime {
    pub fn new() -> Self {
        let now: DateTime<Local> = Local::now();
        Self {
            current_time: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

impl Default for CurrentTime {
    fn default() -> Self {
        Self::new()
    }
}

impl CurrentTime {
    // CurrentTime impl 블록 내부
    pub fn time_string_to_int(&self) -> i64 {
        // i32 -> i64로 변경
        let now: DateTime<Local> = Local::now();
        now.timestamp() // as i64 캐스팅 불필요
    }
}
