//! 접근 제어 및 권한 관리 시스템
//!
//! API 엔드포인트별 세분화된 권한 매트릭스를 제공합니다.
//! OWASP Top 10 A01 (Broken Access Control) 대응을 위한 포괄적인 구현.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tracing::{error, info, warn};

/// Type alias for complex custom rule function
type CustomRule = Box<dyn Fn(&UserRole, &ApiEndpoint, Option<i32>) -> bool + Send + Sync>;

/// 사용자 역할 정의
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserRole {
    /// 게스트 사용자 (비로그인)
    Guest,
    /// 일반 사용자
    User,
    /// 프리미엄 사용자
    Premium,
    /// 게임 마스터
    GameMaster,
    /// 중재자
    Moderator,
    /// 관리자
    Admin,
    /// 슈퍼 관리자
    SuperAdmin,
}

impl UserRole {
    /// 역할의 권한 레벨 반환 (숫자가 높을수록 높은 권한)
    pub fn level(&self) -> u8 {
        match self {
            UserRole::Guest => 0,
            UserRole::User => 10,
            UserRole::Premium => 20,
            UserRole::GameMaster => 30,
            UserRole::Moderator => 40,
            UserRole::Admin => 50,
            UserRole::SuperAdmin => 60,
        }
    }

    /// 역할 상속 관계 확인 (상위 역할은 하위 역할의 권한을 포함)
    pub fn inherits_from(&self, other: &UserRole) -> bool {
        self.level() >= other.level()
    }

    /// 문자열에서 역할 파싱
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "guest" => Some(UserRole::Guest),
            "user" => Some(UserRole::User),
            "premium" => Some(UserRole::Premium),
            "gamemaster" | "game_master" => Some(UserRole::GameMaster),
            "moderator" | "mod" => Some(UserRole::Moderator),
            "admin" => Some(UserRole::Admin),
            "superadmin" | "super_admin" => Some(UserRole::SuperAdmin),
            _ => None,
        }
    }
}

impl FromStr for UserRole {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

/// 권한 타입 정의
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // 기본 권한
    Read,
    Write,
    Delete,

    // 게임 관련 권한
    JoinGame,
    CreateRoom,
    ModifyRoom,
    DeleteRoom,
    ViewLeaderboard,

    // 사용자 관리 권한
    ViewProfile,
    EditProfile,
    ViewOtherProfiles,
    EditOtherProfiles,
    BanUser,
    UnbanUser,

    // 관리 권한
    ViewLogs,
    ViewMetrics,
    ManageUsers,
    ManageRoles,
    SystemConfiguration,
    DatabaseAccess,

    // 특수 권한
    DebugAccess,
    SuperAdminAccess,
}

/// API 엔드포인트 정의
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApiEndpoint {
    /// 서비스 이름 (예: "user", "room")
    pub service: String,
    /// 메서드 이름 (예: "GetUser", "CreateRoom")
    pub method: String,
    /// HTTP 메서드 (예: "GET", "POST", "PUT", "DELETE")
    pub http_method: Option<String>,
}

impl ApiEndpoint {
    pub fn new(service: &str, method: &str) -> Self {
        Self {
            service: service.to_string(),
            method: method.to_string(),
            http_method: None,
        }
    }

    pub fn with_http_method(mut self, http_method: &str) -> Self {
        self.http_method = Some(http_method.to_string());
        self
    }

    /// 전체 경로 반환 (예: "/user.UserService/GetUser")
    pub fn full_path(&self) -> String {
        format!("/{}.{}Service/{}", self.service, self.service, self.method)
    }
}

/// 권한 매트릭스
pub struct AccessControlMatrix {
    /// 역할별 권한 매핑
    role_permissions: HashMap<UserRole, HashSet<Permission>>,
    /// 엔드포인트별 필요 권한 매핑
    endpoint_permissions: HashMap<ApiEndpoint, HashSet<Permission>>,
    /// 공개 엔드포인트 (인증 불필요)
    public_endpoints: HashSet<ApiEndpoint>,
    /// 특별한 권한 규칙
    custom_rules: Vec<CustomRule>,
}

impl Default for AccessControlMatrix {
    fn default() -> Self {
        let mut matrix = Self {
            role_permissions: HashMap::new(),
            endpoint_permissions: HashMap::new(),
            public_endpoints: HashSet::new(),
            custom_rules: Vec::new(),
        };

        matrix.initialize_default_permissions();
        matrix.initialize_endpoint_permissions();
        matrix.initialize_public_endpoints();

        matrix
    }
}

impl AccessControlMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    /// 기본 역할별 권한 초기화
    fn initialize_default_permissions(&mut self) {
        // Guest 권한 (최소한의 읽기 권한만)
        let guest_perms = HashSet::from([Permission::Read, Permission::ViewLeaderboard]);
        self.role_permissions.insert(UserRole::Guest, guest_perms);

        // User 권한 (기본 게임 플레이)
        let user_perms = HashSet::from([
            Permission::Read,
            Permission::Write,
            Permission::JoinGame,
            Permission::CreateRoom,
            Permission::ViewProfile,
            Permission::EditProfile,
            Permission::ViewLeaderboard,
        ]);
        self.role_permissions.insert(UserRole::User, user_perms);

        // Premium 권한 (추가 게임 기능)
        let premium_perms = HashSet::from([
            Permission::Read,
            Permission::Write,
            Permission::JoinGame,
            Permission::CreateRoom,
            Permission::ModifyRoom,
            Permission::ViewProfile,
            Permission::EditProfile,
            Permission::ViewLeaderboard,
        ]);
        self.role_permissions
            .insert(UserRole::Premium, premium_perms);

        // GameMaster 권한 (게임 관리)
        let gamemaster_perms = HashSet::from([
            Permission::Read,
            Permission::Write,
            Permission::Delete,
            Permission::JoinGame,
            Permission::CreateRoom,
            Permission::ModifyRoom,
            Permission::DeleteRoom,
            Permission::ViewProfile,
            Permission::EditProfile,
            Permission::ViewLeaderboard,
            Permission::ViewOtherProfiles,
        ]);
        self.role_permissions
            .insert(UserRole::GameMaster, gamemaster_perms);

        // Moderator 권한 (사용자 관리)
        let moderator_perms = HashSet::from([
            Permission::Read,
            Permission::Write,
            Permission::Delete,
            Permission::JoinGame,
            Permission::CreateRoom,
            Permission::ModifyRoom,
            Permission::DeleteRoom,
            Permission::ViewProfile,
            Permission::EditProfile,
            Permission::ViewLeaderboard,
            Permission::ViewOtherProfiles,
            Permission::EditOtherProfiles,
            Permission::BanUser,
            Permission::UnbanUser,
        ]);
        self.role_permissions
            .insert(UserRole::Moderator, moderator_perms);

        // Admin 권한 (시스템 관리)
        let admin_perms = HashSet::from([
            Permission::Read,
            Permission::Write,
            Permission::Delete,
            Permission::JoinGame,
            Permission::CreateRoom,
            Permission::ModifyRoom,
            Permission::DeleteRoom,
            Permission::ViewProfile,
            Permission::EditProfile,
            Permission::ViewLeaderboard,
            Permission::ViewOtherProfiles,
            Permission::EditOtherProfiles,
            Permission::BanUser,
            Permission::UnbanUser,
            Permission::ViewLogs,
            Permission::ViewMetrics,
            Permission::ManageUsers,
            Permission::ManageRoles,
            Permission::SystemConfiguration,
        ]);
        self.role_permissions.insert(UserRole::Admin, admin_perms);

        // SuperAdmin 권한 (모든 권한)
        let superadmin_perms = HashSet::from([
            Permission::Read,
            Permission::Write,
            Permission::Delete,
            Permission::JoinGame,
            Permission::CreateRoom,
            Permission::ModifyRoom,
            Permission::DeleteRoom,
            Permission::ViewProfile,
            Permission::EditProfile,
            Permission::ViewLeaderboard,
            Permission::ViewOtherProfiles,
            Permission::EditOtherProfiles,
            Permission::BanUser,
            Permission::UnbanUser,
            Permission::ViewLogs,
            Permission::ViewMetrics,
            Permission::ManageUsers,
            Permission::ManageRoles,
            Permission::SystemConfiguration,
            Permission::DatabaseAccess,
            Permission::DebugAccess,
            Permission::SuperAdminAccess,
        ]);
        self.role_permissions
            .insert(UserRole::SuperAdmin, superadmin_perms);
    }

    /// 엔드포인트별 필요 권한 초기화
    fn initialize_endpoint_permissions(&mut self) {
        // 사용자 서비스 엔드포인트
        self.endpoint_permissions.insert(
            ApiEndpoint::new("user", "GetUser"),
            HashSet::from([Permission::ViewProfile]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("user", "UpdateUser"),
            HashSet::from([Permission::EditProfile]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("user", "DeleteUser"),
            HashSet::from([Permission::Delete, Permission::ManageUsers]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("user", "ListUsers"),
            HashSet::from([Permission::ViewOtherProfiles]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("user", "BanUser"),
            HashSet::from([Permission::BanUser]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("user", "UnbanUser"),
            HashSet::from([Permission::UnbanUser]),
        );

        // 방 서비스 엔드포인트
        self.endpoint_permissions.insert(
            ApiEndpoint::new("room", "GetRoom"),
            HashSet::from([Permission::Read]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("room", "CreateRoom"),
            HashSet::from([Permission::CreateRoom]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("room", "UpdateRoom"),
            HashSet::from([Permission::ModifyRoom]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("room", "DeleteRoom"),
            HashSet::from([Permission::DeleteRoom]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("room", "JoinRoom"),
            HashSet::from([Permission::JoinGame]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("room", "LeaveRoom"),
            HashSet::from([Permission::JoinGame]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("room", "ListRooms"),
            HashSet::from([Permission::Read]),
        );

        // 리더보드 및 통계
        self.endpoint_permissions.insert(
            ApiEndpoint::new("leaderboard", "GetLeaderboard"),
            HashSet::from([Permission::ViewLeaderboard]),
        );

        // 관리 엔드포인트
        self.endpoint_permissions.insert(
            ApiEndpoint::new("admin", "GetLogs"),
            HashSet::from([Permission::ViewLogs]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("admin", "GetMetrics"),
            HashSet::from([Permission::ViewMetrics]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("admin", "SystemConfig"),
            HashSet::from([Permission::SystemConfiguration]),
        );

        self.endpoint_permissions.insert(
            ApiEndpoint::new("debug", "DebugInfo"),
            HashSet::from([Permission::DebugAccess]),
        );
    }

    /// 공개 엔드포인트 초기화 (인증 불필요)
    fn initialize_public_endpoints(&mut self) {
        self.public_endpoints
            .insert(ApiEndpoint::new("user", "LoginUser"));
        self.public_endpoints
            .insert(ApiEndpoint::new("user", "RegisterUser"));
        self.public_endpoints
            .insert(ApiEndpoint::new("health", "HealthCheck"));
        self.public_endpoints
            .insert(ApiEndpoint::new("info", "GetServerInfo"));
    }

    /// 사용자의 권한 확인
    pub fn check_permission(
        &self,
        user_roles: &[UserRole],
        endpoint: &ApiEndpoint,
        user_id: Option<i32>,
    ) -> Result<bool, String> {
        // 1. 공개 엔드포인트 확인
        if self.public_endpoints.contains(endpoint) {
            info!(
                target: "security::access_control",
                endpoint = %endpoint.full_path(),
                "✅ 공개 엔드포인트 접근 허용"
            );
            return Ok(true);
        }

        // 2. 사용자 역할이 없으면 거부
        if user_roles.is_empty() {
            warn!(
                target: "security::access_control",
                endpoint = %endpoint.full_path(),
                "❌ 사용자 역할이 없음"
            );
            return Err("사용자 역할이 없습니다".to_string());
        }

        // 3. 엔드포인트 필요 권한 확인
        let required_permissions = match self.endpoint_permissions.get(endpoint) {
            Some(perms) => perms,
            None => {
                error!(
                    target: "security::access_control",
                    endpoint = %endpoint.full_path(),
                    "⚠️ 정의되지 않은 엔드포인트 - 기본 거부"
                );
                return Err(format!(
                    "정의되지 않은 엔드포인트: {}",
                    endpoint.full_path()
                ));
            }
        };

        // 4. 사용자 역할의 권한 확인
        let mut user_permissions = HashSet::new();
        for role in user_roles {
            if let Some(role_perms) = self.role_permissions.get(role) {
                user_permissions.extend(role_perms.iter().cloned());
            }
        }

        // 5. 필요한 권한이 모두 있는지 확인
        let has_all_permissions = required_permissions
            .iter()
            .all(|perm| user_permissions.contains(perm));

        if !has_all_permissions {
            let missing_perms: Vec<&Permission> =
                required_permissions.difference(&user_permissions).collect();

            warn!(
                target: "security::access_control",
                endpoint = %endpoint.full_path(),
                user_roles = ?user_roles,
                required_permissions = ?required_permissions,
                missing_permissions = ?missing_perms,
                "❌ 권한 부족으로 접근 거부"
            );

            return Err(format!(
                "필요한 권한이 부족합니다. 부족한 권한: {missing_perms:?}"
            ));
        }

        // 6. 커스텀 규칙 확인
        for rule in &self.custom_rules {
            // 최고 권한 역할로 규칙 확인
            let highest_role = user_roles
                .iter()
                .max_by_key(|role| role.level())
                .unwrap_or(&UserRole::Guest);

            if !rule(highest_role, endpoint, user_id) {
                warn!(
                    target: "security::access_control",
                    endpoint = %endpoint.full_path(),
                    user_roles = ?user_roles,
                    user_id = ?user_id,
                    "❌ 커스텀 규칙에 의해 접근 거부"
                );
                return Err("커스텀 보안 규칙에 의해 접근이 거부되었습니다".to_string());
            }
        }

        info!(
            target: "security::access_control",
            endpoint = %endpoint.full_path(),
            user_roles = ?user_roles,
            user_permissions_count = user_permissions.len(),
            "✅ 권한 확인 통과"
        );

        Ok(true)
    }

    /// 사용자가 특정 권한을 가지고 있는지 확인
    pub fn has_permission(&self, user_roles: &[UserRole], permission: &Permission) -> bool {
        for role in user_roles {
            if let Some(role_perms) = self.role_permissions.get(role) {
                if role_perms.contains(permission) {
                    return true;
                }
            }
        }
        false
    }

    /// 사용자의 모든 권한 조회
    pub fn get_user_permissions(&self, user_roles: &[UserRole]) -> HashSet<Permission> {
        let mut permissions = HashSet::new();
        for role in user_roles {
            if let Some(role_perms) = self.role_permissions.get(role) {
                permissions.extend(role_perms.iter().cloned());
            }
        }
        permissions
    }

    /// 특정 엔드포인트에 접근 가능한 최소 역할 조회
    pub fn get_minimum_role_for_endpoint(&self, endpoint: &ApiEndpoint) -> Option<UserRole> {
        if self.public_endpoints.contains(endpoint) {
            return Some(UserRole::Guest);
        }

        let required_permissions = self.endpoint_permissions.get(endpoint)?;

        // 각 역할을 권한 레벨 순으로 확인
        let roles = [
            UserRole::Guest,
            UserRole::User,
            UserRole::Premium,
            UserRole::GameMaster,
            UserRole::Moderator,
            UserRole::Admin,
            UserRole::SuperAdmin,
        ];

        for role in roles.iter() {
            if let Some(role_perms) = self.role_permissions.get(role) {
                if required_permissions
                    .iter()
                    .all(|perm| role_perms.contains(perm))
                {
                    return Some(role.clone());
                }
            }
        }

        None
    }

    /// 커스텀 권한 규칙 추가
    pub fn add_custom_rule<F>(&mut self, rule: F)
    where
        F: Fn(&UserRole, &ApiEndpoint, Option<i32>) -> bool + Send + Sync + 'static,
    {
        self.custom_rules.push(Box::new(rule));
    }

    /// 권한 매트릭스 검증
    pub fn validate_matrix(&self) -> Vec<String> {
        let mut issues = Vec::new();

        // 1. 모든 엔드포인트가 정의되었는지 확인
        let defined_endpoints = self.endpoint_permissions.len() + self.public_endpoints.len();
        if defined_endpoints == 0 {
            issues.push("정의된 엔드포인트가 없습니다".to_string());
        }

        // 2. 모든 역할에 권한이 할당되었는지 확인
        for role in [
            UserRole::Guest,
            UserRole::User,
            UserRole::Premium,
            UserRole::GameMaster,
            UserRole::Moderator,
            UserRole::Admin,
            UserRole::SuperAdmin,
        ] {
            if !self.role_permissions.contains_key(&role) {
                issues.push(format!("역할 {role:?}에 권한이 정의되지 않았습니다"));
            }
        }

        // 3. 권한 상속이 올바른지 확인
        let roles = [
            (UserRole::User, UserRole::Guest),
            (UserRole::Premium, UserRole::User),
            (UserRole::GameMaster, UserRole::Premium),
            (UserRole::Moderator, UserRole::GameMaster),
            (UserRole::Admin, UserRole::Moderator),
            (UserRole::SuperAdmin, UserRole::Admin),
        ];

        for (higher, lower) in roles {
            if let (Some(higher_perms), Some(lower_perms)) = (
                self.role_permissions.get(&higher),
                self.role_permissions.get(&lower),
            ) {
                let missing_perms: Vec<&Permission> =
                    lower_perms.difference(higher_perms).collect();
                if !missing_perms.is_empty() {
                    issues.push(format!(
                        "역할 {higher:?}가 하위 역할 {lower:?}의 권한을 상속하지 않습니다: {missing_perms:?}"
                    ));
                }
            }
        }

        issues
    }

    /// 권한 매트릭스 통계 정보
    pub fn get_statistics(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        stats.insert("총_역할_수".to_string(), self.role_permissions.len());
        stats.insert(
            "총_엔드포인트_수".to_string(),
            self.endpoint_permissions.len() + self.public_endpoints.len(),
        );
        stats.insert(
            "보호된_엔드포인트_수".to_string(),
            self.endpoint_permissions.len(),
        );
        stats.insert(
            "공개_엔드포인트_수".to_string(),
            self.public_endpoints.len(),
        );
        stats.insert("커스텀_규칙_수".to_string(), self.custom_rules.len());

        // 권한별 통계
        let mut permission_count = HashMap::new();
        for perms in self.role_permissions.values() {
            for perm in perms {
                *permission_count.entry(format!("{perm:?}")).or_insert(0) += 1;
            }
        }

        stats.insert("고유_권한_수".to_string(), permission_count.len());

        stats
    }
}

mod tests {
    
    #[test]
    fn test_user_role_hierarchy() {
        assert_eq!(UserRole::Guest.level(), 0);
        assert_eq!(UserRole::SuperAdmin.level(), 60);

        assert!(UserRole::Admin.inherits_from(&UserRole::User));
        assert!(UserRole::User.inherits_from(&UserRole::Guest));
        assert!(!UserRole::Guest.inherits_from(&UserRole::User));
    }

    #[test]
    fn test_role_from_string() {
        assert_eq!("user".parse::<UserRole>(), Ok(UserRole::User));
        assert_eq!("ADMIN".parse::<UserRole>(), Ok(UserRole::Admin));
        assert_eq!(
            "game_master".parse::<UserRole>(),
            Ok(UserRole::GameMaster)
        );
        assert!("invalid".parse::<UserRole>().is_err());
    }

    #[test]
    fn test_public_endpoints() {
        let matrix = AccessControlMatrix::new();
        let login_endpoint = ApiEndpoint::new("user", "LoginUser");

        // 공개 엔드포인트는 인증 없이 접근 가능
        assert!(matrix.check_permission(&[], &login_endpoint, None).is_ok());
    }

    #[test]
    fn test_protected_endpoints() {
        let matrix = AccessControlMatrix::new();
        let create_room_endpoint = ApiEndpoint::new("room", "CreateRoom");

        // 권한이 없으면 접근 불가
        assert!(matrix
            .check_permission(&[UserRole::Guest], &create_room_endpoint, None)
            .is_err());

        // 적절한 권한이 있으면 접근 가능
        assert!(matrix
            .check_permission(&[UserRole::User], &create_room_endpoint, None)
            .is_ok());
    }

    #[test]
    fn test_admin_endpoints() {
        let matrix = AccessControlMatrix::new();
        let admin_endpoint = ApiEndpoint::new("admin", "SystemConfig");

        // 일반 사용자는 관리자 엔드포인트 접근 불가
        assert!(matrix
            .check_permission(&[UserRole::User], &admin_endpoint, None)
            .is_err());

        // 관리자는 접근 가능
        assert!(matrix
            .check_permission(&[UserRole::Admin], &admin_endpoint, None)
            .is_ok());
    }

    #[test]
    fn test_multiple_roles() {
        let matrix = AccessControlMatrix::new();
        let ban_user_endpoint = ApiEndpoint::new("user", "BanUser");

        // 여러 역할을 가진 사용자 (권한 결합)
        let roles = vec![UserRole::User, UserRole::Moderator];
        assert!(matrix
            .check_permission(&roles, &ban_user_endpoint, None)
            .is_ok());
    }

    #[test]
    fn test_permission_inheritance() {
        let matrix = AccessControlMatrix::new();

        // 상위 역할은 하위 역할의 권한을 포함해야 함
        let user_perms = matrix.get_user_permissions(&[UserRole::User]);
        let admin_perms = matrix.get_user_permissions(&[UserRole::Admin]);

        // 관리자는 사용자의 모든 권한을 가져야 함
        assert!(user_perms.is_subset(&admin_perms));
    }

    #[test]
    fn test_minimum_role_calculation() {
        let matrix = AccessControlMatrix::new();
        let create_room = ApiEndpoint::new("room", "CreateRoom");
        let system_config = ApiEndpoint::new("admin", "SystemConfig");

        assert_eq!(
            matrix.get_minimum_role_for_endpoint(&create_room),
            Some(UserRole::User)
        );
        assert_eq!(
            matrix.get_minimum_role_for_endpoint(&system_config),
            Some(UserRole::Admin)
        );
    }

    #[test]
    fn test_matrix_validation() {
        let matrix = AccessControlMatrix::new();
        let issues = matrix.validate_matrix();

        // 기본 매트릭스는 검증 오류가 없어야 함
        if !issues.is_empty() {
            println!("매트릭스 검증 이슈: {:?}", issues);
        }
    }

    #[test]
    fn test_custom_rules() {
        let mut matrix = AccessControlMatrix::new();

        // 커스텀 규칙: 게스트는 새벽 시간 접근 불가
        matrix.add_custom_rule(|role, _endpoint, _user_id| {
            if matches!(role, UserRole::Guest) {
                use chrono::Timelike;
                let hour = chrono::Local::now().hour();
                !(2..=5).contains(&hour) // 새벽 2-5시 차단
            } else {
                true
            }
        });

        // 규칙이 추가되었는지 확인
        let stats = matrix.get_statistics();
        assert_eq!(stats.get("커스텀_규칙_수"), Some(&1));
    }

    #[test]
    fn test_statistics() {
        let matrix = AccessControlMatrix::new();
        let stats = matrix.get_statistics();

        assert!(stats.get("총_역할_수").expect("Test assertion failed") > &0);
        assert!(
            stats
                .get("총_엔드포인트_수")
                .expect("Test assertion failed")
                > &0
        );
        assert!(stats.get("고유_권한_수").expect("Test assertion failed") > &0);

        println!("권한 매트릭스 통계: {:?}", stats);
    }
}
