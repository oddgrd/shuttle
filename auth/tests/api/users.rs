mod needs_docker {
    use std::time::Duration;

    use crate::{
        helpers::{self, app},
        stripe::{MOCKED_CHECKOUT_SESSIONS, MOCKED_SUBSCRIPTIONS},
    };
    use axum::body::Body;
    use hyper::http::{header::AUTHORIZATION, Request, StatusCode};
    use serde_json::{self, Value};

    #[tokio::test]
    async fn post_user() {
        let app = app().await;

        // POST user without bearer token.
        let request = Request::builder()
            .uri("/users/test-user/basic")
            .method("POST")
            .body(Body::empty())
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // POST user with invalid bearer token.
        let request = Request::builder()
            .uri("/users/test-user/basic")
            .method("POST")
            .header(AUTHORIZATION, "Bearer notadmin")
            .body(Body::empty())
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // POST user with valid bearer token and basic tier.
        let response = app.post_user("test-user", "basic").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(user["name"], "test-user");
        assert_eq!(user["account_tier"], "basic");
        assert!(user["key"].to_string().is_ascii());

        // POST user with valid bearer token and pro tier.
        let response = app.post_user("pro-user", "pro").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(user["name"], "pro-user");
        assert_eq!(user["account_tier"], "pro");
        assert!(user["key"].to_string().is_ascii());
    }

    #[tokio::test]
    async fn get_user() {
        let app = app().await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();

        // GET user without bearer token.
        let request = Request::builder()
            .uri("/users/test-user")
            .body(Body::empty())
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // GET user with invalid bearer token.
        let request = Request::builder()
            .uri("/users/test-user")
            .header(AUTHORIZATION, "Bearer notadmin")
            .body(Body::empty())
            .unwrap();

        let response = app.send_request(request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // GET user that doesn't exist with valid bearer token.
        let response = app.get_user("not-test-user").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // GET user with valid bearer token.
        let response = app.get_user("test-user").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let persisted_user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(user, persisted_user);
    }

    #[tokio::test]
    async fn successful_upgrade_to_pro() {
        let app = app().await;

        // Wait for the mocked Stripe server to start.
        tokio::task::spawn(app.mocked_stripe_server.clone().serve());
        tokio::time::sleep(Duration::from_secs(1)).await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let expected_user: Value = serde_json::from_slice(&body).unwrap();

        let response = app
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[0])
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        let response = app.get_user("test-user").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let actual_user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            expected_user.as_object().unwrap().get("name").unwrap(),
            actual_user.as_object().unwrap().get("name").unwrap()
        );

        assert_eq!(
            expected_user.as_object().unwrap().get("key").unwrap(),
            actual_user.as_object().unwrap().get("key").unwrap()
        );

        assert_eq!(
            actual_user
                .as_object()
                .unwrap()
                .get("account_tier")
                .unwrap(),
            "pro"
        );

        let mocked_subscription_obj: Value = serde_json::from_str(MOCKED_SUBSCRIPTIONS[0]).unwrap();
        assert_eq!(
            actual_user
                .as_object()
                .unwrap()
                .get("subscription_id")
                .unwrap(),
            mocked_subscription_obj
                .as_object()
                .unwrap()
                .get("id")
                .unwrap()
        );
    }

    #[tokio::test]
    async fn local_stripe_successful_upgrade_to_pro() {
        let app = app().await;

        let sub = app.scaffold_stripe().await;

        println!("stripe sub: {:?}", sub);

        // // POST user first so one exists in the database.
        // let response = app.post_user("test-user", "basic").await;

        // assert_eq!(response.status(), StatusCode::OK);

        // let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        // let expected_user: Value = serde_json::from_slice(&body).unwrap();

        // let response = app
        //     .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[0])
        //     .await;
        // assert_eq!(response.status(), StatusCode::OK);

        // let response = app.get_user("test-user").await;
        // assert_eq!(response.status(), StatusCode::OK);
        // let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        // let actual_user: Value = serde_json::from_slice(&body).unwrap();

        // assert_eq!(
        //     expected_user.as_object().unwrap().get("name").unwrap(),
        //     actual_user.as_object().unwrap().get("name").unwrap()
        // );

        // assert_eq!(
        //     expected_user.as_object().unwrap().get("key").unwrap(),
        //     actual_user.as_object().unwrap().get("key").unwrap()
        // );

        // assert_eq!(
        //     actual_user
        //         .as_object()
        //         .unwrap()
        //         .get("account_tier")
        //         .unwrap(),
        //     "pro"
        // );

        // let mocked_subscription_obj: Value = serde_json::from_str(MOCKED_SUBSCRIPTIONS[0]).unwrap();
        // assert_eq!(
        //     actual_user
        //         .as_object()
        //         .unwrap()
        //         .get("subscription_id")
        //         .unwrap(),
        //     mocked_subscription_obj
        //         .as_object()
        //         .unwrap()
        //         .get("id")
        //         .unwrap()
        // );
    }

    #[tokio::test]
    async fn unsuccessful_upgrade_to_pro() {
        let app = app().await;

        // Wait for the mocked Stripe server to start.
        tokio::task::spawn(app.mocked_stripe_server.clone().serve());
        tokio::time::sleep(Duration::from_secs(1)).await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Test upgrading to pro without a checkout session object.
        let response = app.put_user("test-user", "pro", "").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Test upgrading to pro with an incomplete checkout session object.
        let response = app
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[1])
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn downgrade_in_case_subscription_due_payment() {
        let app = app().await;

        // Wait for the mocked Stripe server to start.
        tokio::task::spawn(app.mocked_stripe_server.clone().serve());
        tokio::time::sleep(Duration::from_secs(1)).await;

        // POST user first so one exists in the database.
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Test upgrading to pro with a checkout session that points to a due session.
        let response = app
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[2])
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // This get_user request should check the subscription status and return an accurate tier.
        let response = app.get_user("test-user").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let actual_user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            actual_user
                .as_object()
                .unwrap()
                .get("account_tier")
                .unwrap(),
            "pendingpaymentpro"
        );
    }

    #[tokio::test]
    async fn test_reset_key() {
        let app = app().await;

        // Reset API key without API key.
        let request = Request::builder()
            .uri("/users/reset-api-key")
            .method("PUT")
            .body(Body::empty())
            .unwrap();
        let response = app.send_request(request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Reset API key with API key.
        let request = Request::builder()
            .uri("/users/reset-api-key")
            .method("PUT")
            .header(AUTHORIZATION, format!("Bearer {}", helpers::ADMIN_KEY))
            .body(Body::empty())
            .unwrap();
        let response = app.send_request(request).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn downgrade_from_cancelledpro() {
        let app = app().await;

        // Wait for the mocked Stripe server to start.
        tokio::task::spawn(app.mocked_stripe_server.clone().serve());
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Create user with basic tier
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Upgrade user to pro
        let response = app
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[3])
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Cancel subscription
        let response = app.put_user("test-user", "cancelledpro", "").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Trigger status change to canceled. This call has a side effect because the user has a
        // subscription that is handled in a specific way by the MockedStripeServer, which changes
        // the subscription state to cancelled.
        let response = app.get_user("test-user").await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            user.as_object().unwrap().get("account_tier").unwrap(),
            "cancelledpro"
        );

        // Check if user is downgraded to basic
        let response = app.get_user("test-user").await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            user.as_object().unwrap().get("account_tier").unwrap(),
            "basic"
        );
    }

    #[tokio::test]
    async fn retain_cancelledpro_status() {
        let app = app().await;

        // Wait for the mocked Stripe server to start.
        tokio::task::spawn(app.mocked_stripe_server.clone().serve());
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Create user with basic tier
        let response = app.post_user("test-user", "basic").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Upgrade user to pro
        let response = app
            .put_user("test-user", "pro", MOCKED_CHECKOUT_SESSIONS[3])
            .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Cancel subscription
        let response = app.put_user("test-user", "cancelledpro", "").await;
        assert_eq!(response.status(), StatusCode::OK);

        // Check if user has cancelledpro status
        let response = app.get_user("test-user").await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            user.as_object().unwrap().get("account_tier").unwrap(),
            "cancelledpro"
        );
    }
}
