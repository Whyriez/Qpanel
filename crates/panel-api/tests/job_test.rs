#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use panel_api::agent_client::AgentClient;
use panel_api::auth::{LockoutManager, SESSION_COOKIE_NAME};
use panel_api::config::AuthConfig;
use panel_api::create_router;
use panel_api::jobs::{JobItem, JobListResponse, JobRunner, process_one_job};
use panel_api::routes::AppState;
use panel_core::auth::hash_password;
use panel_db::models::{CreateJob, CreateUser};
use panel_db::pool::{DbPool, init_pool, run_migrations};
use panel_db::repositories::{SqliteJobRepository, SqliteUserRepository};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, DbPool, Arc<JobRunner>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("job_test.db");
    let db_url = format!("sqlite://{}", db_path.to_str().expect("to_str"));

    let pool = init_pool(&db_url).await.expect("init pool");
    run_migrations(&pool).await.expect("run migrations");

    let user_repo = SqliteUserRepository::new(pool.clone());

    // 1. Create admin user
    let admin_hash = hash_password("AdminPass123!").expect("hash admin");
    user_repo
        .create(&CreateUser {
            username: "admin".to_string(),
            password_hash: admin_hash,
            role: "admin".to_string(),
            email: Some("admin@example.com".to_string()),
            linux_username: None,
            linux_uid: None,
            plan_id: None,
        })
        .await
        .expect("create admin");

    // 2. Create regular user: alice
    let alice_hash = hash_password("AlicePass123!").expect("hash alice");
    user_repo
        .create(&CreateUser {
            username: "alice".to_string(),
            password_hash: alice_hash,
            role: "user".to_string(),
            email: Some("alice@example.com".to_string()),
            linux_username: None,
            linux_uid: None,
            plan_id: None,
        })
        .await
        .expect("create alice");

    // 3. Create regular user: bob
    let bob_hash = hash_password("BobPass123!").expect("hash bob");
    user_repo
        .create(&CreateUser {
            username: "bob".to_string(),
            password_hash: bob_hash,
            role: "user".to_string(),
            email: Some("bob@example.com".to_string()),
            linux_username: None,
            linux_uid: None,
            plan_id: None,
        })
        .await
        .expect("create bob");

    let sock_path = tmp.path().join("agent.sock");
    let auth_config = AuthConfig {
        session_duration_secs: 3600,
        max_failed_attempts: 5,
        lockout_duration_secs: 300,
        cookie_secure: Some(false),
    };

    let lockout_manager = Arc::new(LockoutManager::new(5, 300));
    let agent_client = Arc::new(AgentClient::new(sock_path));
    let state = AppState::new(
        agent_client,
        pool.clone(),
        "development".to_string(),
        auth_config,
        lockout_manager,
    );

    let runner = Arc::clone(&state.job_runner);
    (create_router(state), pool, runner)
}

fn extract_cookie_value(cookie_header: &str, cookie_name: &str) -> Option<String> {
    for pair in cookie_header.split(';') {
        let (key, val) = pair.trim().split_once('=')?;
        if key == cookie_name {
            return Some(val.to_string());
        }
    }
    None
}

async fn login(app: &axum::Router, username: &str, password: &str) -> (String, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "username": username,
                "password": password
            }))
            .expect("json"),
        ))
        .expect("build");

    let res = app.clone().oneshot(req).await.expect("res");
    assert_eq!(res.status(), StatusCode::OK);

    let session_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|c| c.to_str().ok())
        .and_then(|h| extract_cookie_value(h, SESSION_COOKIE_NAME))
        .expect("session cookie");

    let body_bytes = res.into_body().collect().await.expect("body").to_bytes();
    let body: Value = serde_json::from_slice(&body_bytes).expect("parse json");
    let csrf_token = body["csrf_token"].as_str().expect("csrf").to_string();

    (session_cookie, csrf_token)
}

#[tokio::test]
async fn test_job_enqueue_and_idempotency() {
    let (app, _pool, _runner) = setup_test_app().await;
    let (alice_cookie, alice_csrf) = login(&app, "alice", "AlicePass123!").await;

    // 1. Enqueue job without explicit ID
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .header("X-CSRF-Token", &alice_csrf)
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "name": "system.echo",
                "payload": { "msg": "hello auto id" }
            }))
            .expect("json"),
        ))
        .expect("req");

    let res = app.clone().oneshot(req).await.expect("res");
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = res.into_body().collect().await.expect("body").to_bytes();
    let job: JobItem = serde_json::from_slice(&body_bytes).expect("parse job");
    assert_eq!(job.name, "system.echo");
    assert_eq!(job.status, "queued");
    assert_eq!(job.progress, 0);

    // 2. Enqueue job with idempotency ID
    let custom_id = "test-idempotency-key-001";
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .header("X-CSRF-Token", &alice_csrf)
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "id": custom_id,
                "name": "system.echo",
                "payload": { "attempt": 1 }
            }))
            .expect("json"),
        ))
        .expect("req");

    let res2 = app.clone().oneshot(req2).await.expect("res");
    assert_eq!(res2.status(), StatusCode::CREATED);
    let job2: JobItem =
        serde_json::from_slice(&res2.into_body().collect().await.expect("b").to_bytes())
            .expect("parse job");
    assert_eq!(job2.id, custom_id);

    // 3. Retry same enqueue with identical ID -> should return 200 OK (idempotent duplicate)
    let req3 = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .header("X-CSRF-Token", &alice_csrf)
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "id": custom_id,
                "name": "system.echo",
                "payload": { "attempt": 2 }
            }))
            .expect("json"),
        ))
        .expect("req");

    let res3 = app.clone().oneshot(req3).await.expect("res");
    assert_eq!(res3.status(), StatusCode::OK);
    let job3: JobItem =
        serde_json::from_slice(&res3.into_body().collect().await.expect("b").to_bytes())
            .expect("parse job");
    assert_eq!(job3.id, custom_id);
    assert_eq!(job3.status, "queued");
}

#[tokio::test]
async fn test_job_worker_execution_success() {
    let (_app, _pool, runner) = setup_test_app().await;

    // Enqueue an echo job
    let (job, _) = runner
        .enqueue(
            Some(1),
            None,
            "system.echo".to_string(),
            r#"{"output":"test payload"}"#.to_string(),
            Some(3),
        )
        .await
        .expect("enqueue");

    assert_eq!(job.status, "queued");

    // Process the job
    let processed = process_one_job(&runner).await.expect("process");
    assert!(processed);

    // Verify finished job in DB
    let finished = runner
        .job_repo
        .find_by_id(&job.id)
        .await
        .expect("find")
        .expect("exists");

    assert_eq!(finished.status, "succeeded");
    assert_eq!(finished.progress, 100);
    assert_eq!(
        finished.result_json.as_deref(),
        Some(r#"{"output":"test payload"}"#)
    );
    assert!(finished.error_message.is_none());
}

#[tokio::test]
async fn test_job_retry_backoff_and_eventual_failure() {
    let (_app, _pool, runner) = setup_test_app().await;

    // Enqueue a job with test.fail and max_retries = 2
    let (job, _) = runner
        .enqueue(
            Some(1),
            None,
            "test.fail".to_string(),
            r#"{"error":"simulated database timeout"}"#.to_string(),
            Some(2),
        )
        .await
        .expect("enqueue");

    // Attempt 1 (retry_count: 0 -> 1, max_retries: 2) => Requeued to "queued"
    let processed = process_one_job(&runner).await.expect("process 1");
    assert!(processed);

    let state1 = runner
        .job_repo
        .find_by_id(&job.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(state1.status, "queued");
    assert_eq!(state1.retry_count, 1);
    assert_eq!(
        state1.error_message.as_deref(),
        Some("simulated database timeout")
    );

    // Attempt 2 (retry_count: 1 -> 2 >= max_retries 2) => Transitions to "failed"
    let processed2 = process_one_job(&runner).await.expect("process 2");
    assert!(processed2);

    let state2 = runner
        .job_repo
        .find_by_id(&job.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(state2.status, "failed");
    assert_eq!(state2.retry_count, 2);
    assert_eq!(
        state2.error_message.as_deref(),
        Some("simulated database timeout")
    );
}

#[tokio::test]
async fn test_job_cancellation_queued_and_running() {
    let (app, _pool, runner) = setup_test_app().await;
    let (alice_cookie, alice_csrf) = login(&app, "alice", "AlicePass123!").await;

    // 1. Cancel while in queued state
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .header("X-CSRF-Token", &alice_csrf)
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "name": "system.echo",
                "payload": { "task": "cancel me" }
            }))
            .expect("json"),
        ))
        .expect("req");

    let res = app.clone().oneshot(req).await.expect("res");
    let job: JobItem =
        serde_json::from_slice(&res.into_body().collect().await.expect("b").to_bytes())
            .expect("parse job");

    // Cancel the queued job via API
    let cancel_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/jobs/{}/cancel", job.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .header("X-CSRF-Token", &alice_csrf)
        .body(Body::empty())
        .expect("req");

    let cancel_res = app.clone().oneshot(cancel_req).await.expect("res");
    assert_eq!(cancel_res.status(), StatusCode::OK);
    let cancelled_job: JobItem = serde_json::from_slice(
        &cancel_res
            .into_body()
            .collect()
            .await
            .expect("b")
            .to_bytes(),
    )
    .expect("parse");
    assert_eq!(cancelled_job.status, "cancelled");

    // Processing queued jobs should not claim this cancelled job
    let claim_res = runner.job_repo.claim_next_queued().await.expect("claim");
    assert!(claim_res.is_none());

    // 2. Cancel while running (cancellation token check)
    let (sleep_job, _) = runner
        .enqueue(
            Some(2),
            None,
            "test.sleep".to_string(),
            r#"{"duration_ms": 300, "steps": 6}"#.to_string(),
            Some(1),
        )
        .await
        .expect("enqueue sleep");

    // Spawn a worker to process it
    let r = Arc::clone(&runner);
    let worker_task = tokio::spawn(async move {
        process_one_job(&r).await.expect("process sleep");
    });

    // Wait briefly for job to start running
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Trigger cancellation
    let cancelled = runner.cancel(&sleep_job.id).await.expect("cancel");
    assert!(cancelled.is_some());

    worker_task.await.expect("worker task finish");

    let final_job = runner
        .job_repo
        .find_by_id(&sleep_job.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(final_job.status, "cancelled");
}

#[tokio::test]
async fn test_startup_recovery_interrupted_jobs() {
    let (_app, pool, runner) = setup_test_app().await;
    let job_repo = SqliteJobRepository::new(pool);

    // Manually insert an interrupted running job
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_secs() as i64;

    job_repo
        .create(&CreateJob {
            id: "interrupted-001".to_string(),
            user_id: Some(1),
            name: "test.sleep".to_string(),
            payload_json: "{}".to_string(),
            max_retries: 3,
        })
        .await
        .expect("create");

    // Force its status to 'running'
    job_repo
        .update_status("interrupted-001", "running", None, None)
        .await
        .expect("update status to running");

    let before = job_repo
        .find_by_id("interrupted-001")
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(before.status, "running");

    // Simulate server startup recovery
    let recovered_count = runner.recover_interrupted().await.expect("recover");
    assert_eq!(recovered_count, 1);

    let after = job_repo
        .find_by_id("interrupted-001")
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(after.status, "failed");
    assert_eq!(
        after.error_message.as_deref(),
        Some("Interrupted by server restart")
    );
    assert!(after.updated_at >= now);
}

#[tokio::test]
async fn test_job_rbac_isolation() {
    let (app, _pool, runner) = setup_test_app().await;
    let (alice_cookie, alice_csrf) = login(&app, "alice", "AlicePass123!").await;
    let (bob_cookie, bob_csrf) = login(&app, "bob", "BobPass123!").await;
    let (admin_cookie, admin_csrf) = login(&app, "admin", "AdminPass123!").await;

    // Alice creates a job
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .header("X-CSRF-Token", &alice_csrf)
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "name": "system.echo",
                "payload": { "owner": "alice" }
            }))
            .expect("json"),
        ))
        .expect("req");

    let res = app.clone().oneshot(req).await.expect("res");
    assert_eq!(res.status(), StatusCode::CREATED);
    let alice_job: JobItem =
        serde_json::from_slice(&res.into_body().collect().await.expect("b").to_bytes())
            .expect("parse job");

    // 1. Bob tries to inspect Alice's job -> 403 Forbidden
    let bob_get = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/jobs/{}", alice_job.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, bob_cookie),
        )
        .body(Body::empty())
        .expect("req");

    let bob_res = app.clone().oneshot(bob_get).await.expect("res");
    assert_eq!(bob_res.status(), StatusCode::FORBIDDEN);

    // 2. Bob tries to cancel Alice's job -> 403 Forbidden
    let bob_cancel = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/jobs/{}/cancel", alice_job.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, bob_cookie),
        )
        .header("X-CSRF-Token", &bob_csrf)
        .body(Body::empty())
        .expect("req");

    let bob_cancel_res = app.clone().oneshot(bob_cancel).await.expect("res");
    assert_eq!(bob_cancel_res.status(), StatusCode::FORBIDDEN);

    // 3. Bob tries to stream Alice's job events -> 403 Forbidden
    let bob_stream = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/jobs/{}/events", alice_job.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, bob_cookie),
        )
        .body(Body::empty())
        .expect("req");

    let bob_stream_res = app.clone().oneshot(bob_stream).await.expect("res");
    assert_eq!(bob_stream_res.status(), StatusCode::FORBIDDEN);

    // 4. Bob lists jobs -> Alice's job is NOT included
    let bob_list = Request::builder()
        .method("GET")
        .uri("/api/v1/jobs")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, bob_cookie),
        )
        .body(Body::empty())
        .expect("req");

    let bob_list_res = app.clone().oneshot(bob_list).await.expect("res");
    assert_eq!(bob_list_res.status(), StatusCode::OK);
    let bob_list_body: JobListResponse = serde_json::from_slice(
        &bob_list_res
            .into_body()
            .collect()
            .await
            .expect("b")
            .to_bytes(),
    )
    .expect("parse list");
    assert_eq!(bob_list_body.total, 0);

    // 5. Alice lists jobs -> sees her own job
    let alice_list = Request::builder()
        .method("GET")
        .uri("/api/v1/jobs")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .body(Body::empty())
        .expect("req");

    let alice_list_res = app.clone().oneshot(alice_list).await.expect("res");
    assert_eq!(alice_list_res.status(), StatusCode::OK);
    let alice_list_body: JobListResponse = serde_json::from_slice(
        &alice_list_res
            .into_body()
            .collect()
            .await
            .expect("b")
            .to_bytes(),
    )
    .expect("parse list");
    assert_eq!(alice_list_body.total, 1);
    assert_eq!(alice_list_body.items[0].id, alice_job.id);

    // 6. Admin can inspect Alice's job -> 200 OK
    let admin_get = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/jobs/{}", alice_job.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, admin_cookie),
        )
        .body(Body::empty())
        .expect("req");

    let admin_res = app.clone().oneshot(admin_get).await.expect("res");
    assert_eq!(admin_res.status(), StatusCode::OK);

    // 7. Admin can cancel Alice's job -> 200 OK
    let admin_cancel = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/jobs/{}/cancel", alice_job.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, admin_cookie),
        )
        .header("X-CSRF-Token", &admin_csrf)
        .body(Body::empty())
        .expect("req");

    let admin_cancel_res = app.clone().oneshot(admin_cancel).await.expect("res");
    assert_eq!(admin_cancel_res.status(), StatusCode::OK);

    let updated_job = runner
        .job_repo
        .find_by_id(&alice_job.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(updated_job.status, "cancelled");
}

#[tokio::test]
async fn test_job_sse_events_stream() {
    let (app, _pool, runner) = setup_test_app().await;
    let (alice_cookie, _alice_csrf) = login(&app, "alice", "AlicePass123!").await;

    // Enqueue and complete a job
    let (job, _) = runner
        .enqueue(
            Some(2),
            None,
            "system.echo".to_string(),
            r#"{"status":"done"}"#.to_string(),
            Some(1),
        )
        .await
        .expect("enqueue");

    process_one_job(&runner).await.expect("process");

    // Connect to SSE stream for this already-succeeded job
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/jobs/{}/events", job.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .body(Body::empty())
        .expect("req");

    let res = app.clone().oneshot(req).await.expect("res");
    assert_eq!(res.status(), StatusCode::OK);
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    assert_eq!(content_type, Some("text/event-stream"));

    let body_bytes = res.into_body().collect().await.expect("body").to_bytes();
    let sse_text = String::from_utf8_lossy(&body_bytes);
    assert!(sse_text.contains("event: progress"));
    assert!(sse_text.contains(&job.id));
    assert!(sse_text.contains("succeeded"));
}

#[tokio::test]
async fn test_job_csrf_enforcement() {
    let (app, _pool, _runner) = setup_test_app().await;
    let (alice_cookie, _alice_csrf) = login(&app, "alice", "AlicePass123!").await;

    // POST /api/v1/jobs without X-CSRF-Token header -> 403 Forbidden
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, alice_cookie),
        )
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "name": "system.echo",
                "payload": {}
            }))
            .expect("json"),
        ))
        .expect("req");

    let res = app.clone().oneshot(req).await.expect("res");
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
