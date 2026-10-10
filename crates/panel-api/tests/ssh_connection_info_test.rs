#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use panel_agent::ssh::SshManager;
use panel_agent::system::MockSystemRunner;
use panel_agent::{AgentConfig, AgentServer};
use panel_api::agent_client::AgentClient;
use panel_api::auth::{LockoutManager, SESSION_COOKIE_NAME};
use panel_api::config::AuthConfig;
use panel_api::create_router;
use panel_api::routes::AppState;
use panel_core::auth::hash_password;
use panel_db::models::{CreateDomain, CreatePlan, CreateSite, CreateUser};
use panel_db::pool::{DbPool, init_pool, run_migrations};
use panel_db::repositories::{
    SqliteDomainRepository, SqlitePlanRepository, SqliteSiteRepository, SqliteUserRepository,
};
use serde_json::Value;
use tower::ServiceExt;

struct TestContext {
    app: axum::Router,
    pool: DbPool,
    alice_id: i64,
    alice_session: String,
    bob_id: i64,
    bob_session: String,
    admin_session: String,
    _temp: tempfile::TempDir,
    _agent_shutdown_tx: tokio::sync::broadcast::Sender<()>,
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

async fn setup_test_context() -> TestContext {
    let tmp = tempfile::tempdir().expect("tempdir");
    let sock_path = tmp.path().join("agent.sock");
    let db_path = tmp.path().join("panel_test.db");
    let ssh_keys_dir = tmp.path().join("etc_panel_ssh_authorized_keys");
    let sshd_dropin_path = tmp.path().join("50-panel.conf");
    std::fs::create_dir_all(&ssh_keys_dir).expect("create ssh keys dir");

    let runner = Arc::new(MockSystemRunner::new());
    let ssh_manager = Arc::new(SshManager::with_paths(
        runner,
        ssh_keys_dir.clone(),
        sshd_dropin_path,
    ));

    let agent_cfg = AgentConfig {
        socket_path: sock_path.clone(),
        allowed_uid: None,
    };
    let agent_server = Arc::new(AgentServer::new(agent_cfg).with_ssh_manager(ssh_manager));
    let (agent_shutdown_tx, agent_shutdown_rx) = tokio::sync::broadcast::channel(1);

    tokio::spawn(async move {
        let _ = agent_server.run(agent_shutdown_rx).await;
    });

    let start = std::time::Instant::now();
    while !sock_path.exists() && start.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let pool = init_pool(&format!(
        "sqlite://{}",
        db_path.to_str().expect("db_path").replace('\\', "/")
    ))
    .await
    .expect("init db pool");
    run_migrations(&pool).await.expect("run db migrations");

    let user_repo = SqliteUserRepository::new(pool.clone());
    let plan_repo = SqlitePlanRepository::new(pool.clone());

    let full_plan = plan_repo
        .create(&CreatePlan {
            name: "Standard Plan".to_string(),
            description: None,
            cpu_quota_percent: Some(100),
            memory_limit_bytes: Some(1024 * 1024 * 1024),
            memory_high_bytes: None,
            tasks_max: Some(50),
            io_weight: Some(100),
            disk_limit_bytes: Some(5 * 1024 * 1024 * 1024),
            inode_limit: Some(500_000),
            max_sites: Some(5),
            max_domains: Some(5),
            max_databases: Some(2),
            max_cron_jobs: Some(5),
            max_terminal_sessions: Some(2),
            max_upload_bytes: Some(50 * 1024 * 1024),
            shell_policy: Some("full".to_string()),
            allow_external_upstream: Some(false),
        })
        .await
        .expect("create plan");

    let pw_hash = hash_password("Password123!").expect("hash password");

    let admin = user_repo
        .create(&CreateUser {
            username: "admin".to_string(),
            password_hash: pw_hash.clone(),
            role: "admin".to_string(),
            email: Some("admin@example.com".to_string()),
            plan_id: None,
            linux_uid: None,
            linux_username: None,
        })
        .await
        .expect("create admin");

    let alice = user_repo
        .create(&CreateUser {
            username: "alice".to_string(),
            password_hash: pw_hash.clone(),
            role: "user".to_string(),
            email: Some("alice@example.com".to_string()),
            plan_id: Some(full_plan.id),
            linux_uid: Some(10001),
            linux_username: Some("p_alice".to_string()),
        })
        .await
        .expect("create alice");

    let bob = user_repo
        .create(&CreateUser {
            username: "bob".to_string(),
            password_hash: pw_hash.clone(),
            role: "user".to_string(),
            email: Some("bob@example.com".to_string()),
            plan_id: Some(full_plan.id),
            linux_uid: Some(10002),
            linux_username: Some("p_bob".to_string()),
        })
        .await
        .expect("create bob");

    // Set Bob's shell policy to SFTP
    user_repo
        .update_shell_policy(bob.id, Some("sftp"))
        .await
        .expect("set bob policy");

    let agent_client = Arc::new(AgentClient::new(sock_path));
    let auth_config = AuthConfig {
        session_duration_secs: 3600,
        max_failed_attempts: 5,
        lockout_duration_secs: 300,
        cookie_secure: Some(false),
    };
    let lockout_manager = Arc::new(LockoutManager::new(5, 300));
    let state = AppState::new(
        agent_client,
        pool.clone(),
        "testing".to_string(),
        auth_config,
        lockout_manager,
    );
    let app = create_router(state);

    let login_as = |username: &str| {
        let app = app.clone();
        let u = username.to_string();
        async move {
            let req = Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": u,
                        "password": "Password123!"
                    })
                    .to_string(),
                ))
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            let cookie_hdr = res
                .headers()
                .get(header::SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap();
            extract_cookie_value(cookie_hdr, SESSION_COOKIE_NAME).unwrap()
        }
    };

    let admin_session = login_as(&admin.username).await;
    let alice_session = login_as(&alice.username).await;
    let bob_session = login_as(&bob.username).await;

    TestContext {
        app,
        pool,
        alice_id: alice.id,
        alice_session,
        bob_id: bob.id,
        bob_session,
        admin_session,
        _temp: tmp,
        _agent_shutdown_tx: agent_shutdown_tx,
    }
}

#[tokio::test]
async fn test_ssh_connection_info_default_alice() {
    let ctx = setup_test_context().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/ssh/connection-info")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.alice_session),
        )
        .header(header::HOST, "panel.example.com")
        .body(Body::empty())
        .unwrap();

    let res = ctx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["username"], "p_alice");
    assert_eq!(json["user_id"], ctx.alice_id);
    assert_eq!(json["home_dir"], "/home/p_alice");
    assert_eq!(json["active_path"], "/home/p_alice");
    assert_eq!(json["relative_path"], "");
    assert_eq!(json["shell_policy"], "full");
    assert_eq!(json["can_shell"], true);
    assert_eq!(json["can_sftp"], true);
    assert_eq!(json["port"], 22);
    assert_eq!(json["host"], "panel.example.com");

    let ssh_cmd = json["ssh_command"].as_str().unwrap();
    assert_eq!(
        ssh_cmd,
        "ssh -t p_alice@panel.example.com \"cd '/home/p_alice' && exec \\$SHELL -l\""
    );

    let sftp_cmd = json["sftp_command"].as_str().unwrap();
    assert_eq!(sftp_cmd, "sftp p_alice@panel.example.com:'/home/p_alice'");

    let sftp_url = json["sftp_url"].as_str().unwrap();
    assert_eq!(sftp_url, "sftp://p_alice@panel.example.com/home/p_alice");

    let config_snippet = json["ssh_config_snippet"].as_str().unwrap();
    assert!(config_snippet.contains("User p_alice"));
    assert!(config_snippet.contains("HostName panel.example.com"));
}

#[tokio::test]
async fn test_ssh_connection_info_with_path_and_custom_port() {
    let ctx = setup_test_context().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/ssh/connection-info?path=/sites/my-app/public&port=2222&host=custom.ssh.host")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.alice_session),
        )
        .body(Body::empty())
        .unwrap();

    let res = ctx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["host"], "custom.ssh.host");
    assert_eq!(json["port"], 2222);
    assert_eq!(json["active_path"], "/home/p_alice/sites/my-app/public");
    assert_eq!(json["relative_path"], "sites/my-app/public");

    let ssh_cmd = json["ssh_command"].as_str().unwrap();
    assert_eq!(
        ssh_cmd,
        "ssh -p 2222 -t p_alice@custom.ssh.host \"cd '/home/p_alice/sites/my-app/public' && exec \\$SHELL -l\""
    );

    let sftp_cmd = json["sftp_command"].as_str().unwrap();
    assert_eq!(
        sftp_cmd,
        "sftp -P 2222 p_alice@custom.ssh.host:'/home/p_alice/sites/my-app/public'"
    );

    let sftp_url = json["sftp_url"].as_str().unwrap();
    assert_eq!(
        sftp_url,
        "sftp://p_alice@custom.ssh.host:2222/home/p_alice/sites/my-app/public"
    );
}

#[tokio::test]
async fn test_ssh_connection_info_single_quote_path_escaping() {
    let ctx = setup_test_context().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/ssh/connection-info?path=sites/alice's%20blog")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.alice_session),
        )
        .header(header::HOST, "localhost")
        .body(Body::empty())
        .unwrap();

    let res = ctx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["active_path"], "/home/p_alice/sites/alice's blog");
    let ssh_cmd = json["ssh_command"].as_str().unwrap();
    assert_eq!(
        ssh_cmd,
        "ssh -t p_alice@localhost \"cd '/home/p_alice/sites/alice'\\''s blog' && exec \\$SHELL -l\""
    );
}

#[tokio::test]
async fn test_ssh_connection_info_with_site_context() {
    let ctx = setup_test_context().await;
    let site_repo = SqliteSiteRepository::new(ctx.pool.clone());
    let domain_repo = SqliteDomainRepository::new(ctx.pool.clone());

    let site = site_repo
        .create(&CreateSite {
            user_id: ctx.alice_id,
            kind: "static".to_string(),
            docroot: "/sites/super-app/public".to_string(),
            runtime: None,
            version: None,
            status: Some("active".to_string()),
            settings_json: None,
        })
        .await
        .unwrap();

    let _domain = domain_repo
        .create(&CreateDomain {
            site_id: site.id,
            name: "superapp.com".to_string(),
            unicode_name: "superapp.com".to_string(),
            is_primary: true,
            redirect_mode: Some("none".to_string()),
            redirect_target: None,
            status: Some("active".to_string()),
        })
        .await
        .unwrap();

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/ssh/connection-info?site_id={}", site.id))
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.alice_session),
        )
        .body(Body::empty())
        .unwrap();

    let res = ctx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["host"], "superapp.com");
    assert_eq!(json["site_domain"], "superapp.com");
    assert_eq!(json["active_path"], "/home/p_alice/sites/super-app/public");
    assert_eq!(json["relative_path"], "sites/super-app/public");
}

#[tokio::test]
async fn test_ssh_connection_info_sftp_policy_bob() {
    let ctx = setup_test_context().await;

    // Bob has shell_policy: "sftp"
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/ssh/connection-info")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.bob_session),
        )
        .header(header::HOST, "vps.example.com")
        .body(Body::empty())
        .unwrap();

    let res = ctx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["username"], "p_bob");
    assert_eq!(json["shell_policy"], "sftp");
    assert_eq!(json["can_shell"], false);
    assert_eq!(json["can_sftp"], true);
    assert!(json["ssh_command"].is_null());
    assert!(!json["sftp_command"].is_null());
    assert_eq!(
        json["sftp_command"],
        "sftp p_bob@vps.example.com:'/home/p_bob'"
    );
}

#[tokio::test]
async fn test_ssh_connection_info_none_policy() {
    let ctx = setup_test_context().await;
    let user_repo = SqliteUserRepository::new(ctx.pool.clone());

    // Update Bob to "none"
    user_repo
        .update_shell_policy(ctx.bob_id, Some("none"))
        .await
        .unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/ssh/connection-info")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.bob_session),
        )
        .body(Body::empty())
        .unwrap();

    let res = ctx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["shell_policy"], "none");
    assert_eq!(json["can_shell"], false);
    assert_eq!(json["can_sftp"], false);
    assert!(json["ssh_command"].is_null());
    assert!(json["sftp_command"].is_null());
    assert!(json["sftp_url"].is_null());
}

#[tokio::test]
async fn test_ssh_connection_info_rbac_and_isolation() {
    let ctx = setup_test_context().await;

    // 1. Alice tries to inspect Bob's connection info -> 403 Forbidden
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/ssh/connection-info?tenant=bob")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.alice_session),
        )
        .body(Body::empty())
        .unwrap();

    let res = ctx.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Admin inspects Bob's connection info -> 200 OK
    let req_admin = Request::builder()
        .method("GET")
        .uri("/api/v1/ssh/connection-info?tenant=bob")
        .header(
            header::COOKIE,
            format!("{}={}", SESSION_COOKIE_NAME, ctx.admin_session),
        )
        .body(Body::empty())
        .unwrap();

    let res_admin = ctx.app.clone().oneshot(req_admin).await.unwrap();
    assert_eq!(res_admin.status(), StatusCode::OK);

    let body = res_admin.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["username"], "p_bob");
}
