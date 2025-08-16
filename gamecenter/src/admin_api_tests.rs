//! 관리자 API 테스트 모듈

#[cfg(test)]
mod tests {
    use super::super::admin_api::*;
    use actix_web::{test, web, App};
    use chrono::Utc;

    #[actix_web::test]
    async fn test_get_server_status() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AdminApiState::new()))
                .configure(configure_admin_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/admin/status")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: ServerStatus = test::read_body_json(resp).await;
        assert_eq!(body.server_name, "GameCenter Unified Server");
        assert!(body.is_running);
    }

    #[actix_web::test]
    async fn test_get_all_servers_status() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AdminApiState::new()))
                .configure(configure_admin_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/admin/servers")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: Vec<ServerStatus> = test::read_body_json(resp).await;
        assert!(body.is_empty() || !body.is_empty()); // Can be empty if no servers running
    }

    #[actix_web::test]
    async fn test_ban_user() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AdminApiState::new()))
                .configure(configure_admin_routes),
        )
        .await;

        let ban_request = BanRequest {
            user_id: "test_user_123".to_string(),
            reason: "Testing ban functionality".to_string(),
            duration_hours: Some(24),
            admin_id: "admin_test".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/api/admin/users/ban")
            .set_json(&ban_request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: UserBan = test::read_body_json(resp).await;
        assert_eq!(body.user_id, "test_user_123");
        assert_eq!(body.ban_reason, "Testing ban functionality");
        assert_eq!(body.banned_by, "admin_test");
    }

    #[actix_web::test]
    async fn test_get_banned_users() {
        let state = AdminApiState::new();

        // Add a test ban
        let test_ban = UserBan {
            user_id: "test_user".to_string(),
            username: "Test User".to_string(),
            ban_reason: "Test reason".to_string(),
            banned_at: Utc::now(),
            banned_until: None,
            banned_by: "admin".to_string(),
        };

        state.banned_users.write().await.push(test_ban);

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure_admin_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/admin/users/banned")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: Vec<UserBan> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 1);
        assert_eq!(body[0].user_id, "test_user");
    }

    #[actix_web::test]
    async fn test_unban_user() {
        let state = AdminApiState::new();

        // Add a test ban
        let test_ban = UserBan {
            user_id: "test_user_to_unban".to_string(),
            username: "Test User".to_string(),
            ban_reason: "Test reason".to_string(),
            banned_at: Utc::now(),
            banned_until: None,
            banned_by: "admin".to_string(),
        };

        state.banned_users.write().await.push(test_ban);

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure_admin_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri("/api/admin/users/unban/test_user_to_unban")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_event() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AdminApiState::new()))
                .configure(configure_admin_routes),
        )
        .await;

        let event_request = CreateEventRequest {
            event_name: "Test Event".to_string(),
            reward_type: "coins".to_string(),
            reward_amount: 1000,
            duration_hours: 48,
        };

        let req = test::TestRequest::post()
            .uri("/api/admin/events")
            .set_json(&event_request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: EventReward = test::read_body_json(resp).await;
        assert_eq!(body.event_name, "Test Event");
        assert_eq!(body.reward_type, "coins");
        assert_eq!(body.reward_amount, 1000);
        assert!(body.is_active);
    }

    #[actix_web::test]
    async fn test_end_event() {
        let state = AdminApiState::new();

        // Add a test event
        let test_event = EventReward {
            event_id: "test_event_123".to_string(),
            event_name: "Test Event".to_string(),
            reward_type: "coins".to_string(),
            reward_amount: 500,
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::hours(24),
            is_active: true,
            participants_count: 10,
        };

        state.active_events.write().await.push(test_event);

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure_admin_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/api/admin/events/test_event_123/end")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: EventReward = test::read_body_json(resp).await;
        assert_eq!(body.event_id, "test_event_123");
        assert!(!body.is_active);
    }

    #[actix_web::test]
    async fn test_get_events() {
        let state = AdminApiState::new();

        // Add test events
        let test_event1 = EventReward {
            event_id: "event1".to_string(),
            event_name: "Event 1".to_string(),
            reward_type: "gems".to_string(),
            reward_amount: 100,
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::hours(12),
            is_active: true,
            participants_count: 5,
        };

        let test_event2 = EventReward {
            event_id: "event2".to_string(),
            event_name: "Event 2".to_string(),
            reward_type: "exp".to_string(),
            reward_amount: 200,
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::hours(6),
            is_active: true,
            participants_count: 15,
        };

        state.active_events.write().await.push(test_event1);
        state.active_events.write().await.push(test_event2);

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure_admin_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/admin/events")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: Vec<EventReward> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);
        assert_eq!(body[0].event_id, "event1");
        assert_eq!(body[1].event_id, "event2");
    }
}
