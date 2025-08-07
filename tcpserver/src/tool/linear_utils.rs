//! 선형 연산 및 게임 로직 유틸리티
//! 
//! 2D 좌표, 거리 계산, 충돌 감지 등 게임에 필요한 수학적 연산을 제공합니다.

use serde::{Serialize, Deserialize};
use std::f64::consts::PI;

/// 2D 좌표점
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    /// 새로운 점 생성
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    
    /// 원점 (0, 0)
    pub fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
    
    /// 두 점 사이의 거리
    pub fn distance_to(&self, other: &Point2D) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
    
    /// 벡터 덧셈
    pub fn add(&self, other: &Point2D) -> Point2D {
        Point2D::new(self.x + other.x, self.y + other.y)
    }
    
    /// 벡터 뺄셈  
    pub fn subtract(&self, other: &Point2D) -> Point2D {
        Point2D::new(self.x - other.x, self.y - other.y)
    }
    
    /// 스칼라 곱셈
    pub fn multiply(&self, scalar: f64) -> Point2D {
        Point2D::new(self.x * scalar, self.y * scalar)
    }
    
    /// 벡터 크기
    pub fn magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    
    /// 정규화 (단위 벡터)
    pub fn normalize(&self) -> Point2D {
        let mag = self.magnitude();
        if mag == 0.0 {
            Point2D::origin()
        } else {
            Point2D::new(self.x / mag, self.y / mag)
        }
    }
    
    /// 각도 계산 (라디안)
    pub fn angle_to(&self, other: &Point2D) -> f64 {
        let diff = other.subtract(self);
        diff.y.atan2(diff.x)
    }
    
    /// 각도를 도 단위로 변환
    pub fn radians_to_degrees(radians: f64) -> f64 {
        radians * 180.0 / PI
    }
    
    /// 도를 라디안으로 변환
    pub fn degrees_to_radians(degrees: f64) -> f64 {
        degrees * PI / 180.0
    }
}

/// 게임 영역 (사각형)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    /// 새로운 사각형 생성
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }
    
    /// 점이 사각형 내부에 있는지 확인
    pub fn contains_point(&self, point: &Point2D) -> bool {
        point.x >= self.x && 
        point.x <= self.x + self.width &&
        point.y >= self.y && 
        point.y <= self.y + self.height
    }
    
    /// 두 사각형이 겹치는지 확인
    pub fn intersects(&self, other: &Rectangle) -> bool {
        !(self.x + self.width < other.x ||
          other.x + other.width < self.x ||
          self.y + self.height < other.y ||
          other.y + other.height < self.y)
    }
    
    /// 사각형 중심점
    pub fn center(&self) -> Point2D {
        Point2D::new(
            self.x + self.width / 2.0,
            self.y + self.height / 2.0
        )
    }
    
    /// 사각형 면적
    pub fn area(&self) -> f64 {
        self.width * self.height
    }
}

/// 원 (충돌 감지용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Circle {
    pub center: Point2D,
    pub radius: f64,
}

impl Circle {
    /// 새로운 원 생성
    pub fn new(center: Point2D, radius: f64) -> Self {
        Self { center, radius }
    }
    
    /// 점이 원 내부에 있는지 확인
    pub fn contains_point(&self, point: &Point2D) -> bool {
        self.center.distance_to(point) <= self.radius
    }
    
    /// 두 원이 겹치는지 확인
    pub fn intersects(&self, other: &Circle) -> bool {
        self.center.distance_to(&other.center) <= self.radius + other.radius
    }
    
    /// 원과 사각형이 겹치는지 확인
    pub fn intersects_rectangle(&self, rect: &Rectangle) -> bool {
        // 원의 중심에서 가장 가까운 사각형 상의 점 찾기
        let closest_x = self.center.x.max(rect.x).min(rect.x + rect.width);
        let closest_y = self.center.y.max(rect.y).min(rect.y + rect.height);
        let closest_point = Point2D::new(closest_x, closest_y);
        
        self.contains_point(&closest_point)
    }
}

/// 선형 보간 유틸리티
pub struct LinearUtils;

impl LinearUtils {
    /// 선형 보간 (lerp)
    pub fn lerp(start: f64, end: f64, t: f64) -> f64 {
        start + (end - start) * t.clamp(0.0, 1.0)
    }
    
    /// 2D 점 선형 보간
    pub fn lerp_point(start: &Point2D, end: &Point2D, t: f64) -> Point2D {
        Point2D::new(
            Self::lerp(start.x, end.x, t),
            Self::lerp(start.y, end.y, t)
        )
    }
    
    /// 값을 지정된 범위로 제한
    pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
        value.max(min).min(max)
    }
    
    /// 값을 범위로 정규화 (0.0 ~ 1.0)
    pub fn normalize_range(value: f64, min: f64, max: f64) -> f64 {
        if max == min {
            0.0
        } else {
            Self::clamp((value - min) / (max - min), 0.0, 1.0)
        }
    }
    
    /// 값을 다른 범위로 매핑
    pub fn map_range(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
        let normalized = Self::normalize_range(value, from_min, from_max);
        Self::lerp(to_min, to_max, normalized)
    }
    
    /// 부드러운 보간 (smooth step)
    pub fn smooth_step(start: f64, end: f64, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let t2 = t * t;
        let t3 = t2 * t;
        let smooth_t = 3.0 * t2 - 2.0 * t3;
        Self::lerp(start, end, smooth_t)
    }
    
    /// 거리 기반 감쇠 계산 (1/distance²)
    pub fn distance_falloff(distance: f64, max_distance: f64) -> f64 {
        if distance >= max_distance || distance == 0.0 {
            0.0
        } else {
            let normalized = 1.0 - (distance / max_distance);
            normalized * normalized
        }
    }
    
    /// 각도 정규화 (-PI ~ PI)
    pub fn normalize_angle(angle: f64) -> f64 {
        let mut normalized = angle % (2.0 * PI);
        if normalized > PI {
            normalized -= 2.0 * PI;
        } else if normalized <= -PI {
            normalized += 2.0 * PI;
        }
        normalized
    }
    
    /// 두 각도 사이의 최단 각도 차이
    pub fn angle_difference(angle1: f64, angle2: f64) -> f64 {
        let diff = Self::normalize_angle(angle2 - angle1);
        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_point_operations() {
        let p1 = Point2D::new(1.0, 2.0);
        let p2 = Point2D::new(4.0, 6.0);
        
        assert_eq!(p1.distance_to(&p2), 5.0); // 3-4-5 삼각형
        
        let sum = p1.add(&p2);
        assert_eq!(sum, Point2D::new(5.0, 8.0));
        
        let diff = p2.subtract(&p1);
        assert_eq!(diff, Point2D::new(3.0, 4.0));
    }
    
    #[test]
    fn test_rectangle_collision() {
        let rect1 = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        let rect2 = Rectangle::new(5.0, 5.0, 10.0, 10.0);
        let rect3 = Rectangle::new(15.0, 15.0, 5.0, 5.0);
        
        assert!(rect1.intersects(&rect2));
        assert!(!rect1.intersects(&rect3));
        
        let point_inside = Point2D::new(5.0, 5.0);
        let point_outside = Point2D::new(15.0, 15.0);
        
        assert!(rect1.contains_point(&point_inside));
        assert!(!rect1.contains_point(&point_outside));
    }
    
    #[test]
    fn test_circle_collision() {
        let circle1 = Circle::new(Point2D::new(0.0, 0.0), 5.0);
        let circle2 = Circle::new(Point2D::new(8.0, 0.0), 5.0);
        let circle3 = Circle::new(Point2D::new(15.0, 0.0), 5.0);
        
        assert!(circle1.intersects(&circle2)); // 거리 8, 반지름 합 10
        assert!(!circle1.intersects(&circle3)); // 거리 15, 반지름 합 10
    }
    
    #[test]
    fn test_linear_interpolation() {
        assert_eq!(LinearUtils::lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(LinearUtils::lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(LinearUtils::lerp(0.0, 10.0, 1.0), 10.0);
        
        // 범위 초과 테스트
        assert_eq!(LinearUtils::lerp(0.0, 10.0, 1.5), 10.0);
        assert_eq!(LinearUtils::lerp(0.0, 10.0, -0.5), 0.0);
    }
    
    #[test]
    fn test_range_mapping() {
        let mapped = LinearUtils::map_range(50.0, 0.0, 100.0, 0.0, 1.0);
        assert_eq!(mapped, 0.5);
        
        let mapped2 = LinearUtils::map_range(25.0, 0.0, 100.0, -1.0, 1.0);
        assert_eq!(mapped2, -0.5);
    }
    
    #[test]
    fn test_angle_operations() {
        let angle = PI + 0.5;
        let normalized = LinearUtils::normalize_angle(angle);
        assert!(normalized < 0.0); // PI + 0.5 -> -PI + 0.5
        
        let diff = LinearUtils::angle_difference(0.0, PI + 0.1);
        assert!(diff.abs() < PI); // 최단 경로
    }
}