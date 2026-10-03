#![allow(clippy::unwrap_used, clippy::expect_used)]

use panel_db::{
    CreateApiToken, CreateAuditLog, CreateJob, CreateSession, CreateUser, SqliteApiTokenRepository,
    SqliteAuditLogRepository, SqliteJobRepository, SqliteSessionRepository,
    SqliteSettingRepository, SqliteUserRepository, init_pool, run_migrations,
};

async fn setup_test_db() -> panel_db::DbPool {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("test_panel.db");
    let db_url = format!("sqlite://{}", db_path.to_str().expect("path to str"));

    let pool = init_pool(&db_url).await.expect("init pool");
    run_migrations(&pool).await.expect("run migrations");
    pool
}

#[tokio::test]
async fn test_user_repository_crud() {
    let pool = setup_test_db().await;
    let repo = SqliteUserRepository::new(pool);

    // 1. Create User
    let input = CreateUser {
        username: "admin".to_string(),
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$test$test".to_string(),
        role: "admin".to_string(),
        email: Some("admin@example.com".to_string()),
        linux_username: None,
        linux_uid: None,
        plan_id: None,
    };
    let user = repo.create(&input).await.expect("create user");
    assert_eq!(user.username, "admin");
    assert_eq!(user.role, "admin");
    assert_eq!(user.status, "active");

    // 2. Conflict on duplicate username
    let conflict_res = repo.create(&input).await;
    assert!(conflict_res.is_err());

    // 3. Find by ID and username
    let by_id = repo
        .find_by_id(user.id)
        .await
        .expect("find by id")
        .expect("user exists");
    assert_eq!(by_id.id, user.id);

    let by_username = repo
        .find_by_username("admin")
        .await
        .expect("find by username")
        .expect("user exists");
    assert_eq!(by_username.id, user.id);

    // 4. Update password
    repo.update_password(user.id, "new_hash")
        .await
        .expect("update password");
    let updated_user = repo
        .find_by_id(user.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(updated_user.password_hash, "new_hash");

    // 5. Update TOTP
    repo.update_totp(user.id, Some("JBSWY3DPEHPK3PXP"), true)
        .await
        .expect("update totp");
    let totp_user = repo
        .find_by_id(user.id)
        .await
        .expect("find")
        .expect("exists");
    assert!(totp_user.totp_enabled);
    assert_eq!(totp_user.totp_secret, Some("JBSWY3DPEHPK3PXP".to_string()));

    // 6. Update Status
    repo.update_status(user.id, "suspended")
        .await
        .expect("update status");
    let suspended = repo
        .find_by_id(user.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(suspended.status, "suspended");

    // 7. List all
    let list = repo.list_all().await.expect("list all");
    assert_eq!(list.len(), 1);

    // 8. Delete
    repo.delete(user.id).await.expect("delete");
    let after_delete = repo.find_by_id(user.id).await.expect("find");
    assert!(after_delete.is_none());
}

#[tokio::test]
async fn test_session_repository_crud() {
    let pool = setup_test_db().await;
    let user_repo = SqliteUserRepository::new(pool.clone());
    let session_repo = SqliteSessionRepository::new(pool);

    let user = user_repo
        .create(&CreateUser {
            username: "alice".to_string(),
            password_hash: "hash".to_string(),
            role: "user".to_string(),
            email: None,
            linux_username: Some("p_alice".to_string()),
            linux_uid: Some(10001),
            plan_id: None,
        })
        .await
        .expect("create user");

    let now = 1000000;
    let session_input = CreateSession {
        id: "sess_123".to_string(),
        user_id: user.id,
        token_hash: "token_hash_abc".to_string(),
        ip_address: Some("127.0.0.1".to_string()),
        user_agent: Some("Mozilla/5.0".to_string()),
        expires_at: now + 3600,
    };

    // 1. Create
    let session = session_repo
        .create(&session_input)
        .await
        .expect("create session");
    assert_eq!(session.id, "sess_123");

    // 2. Find by token hash
    let found = session_repo
        .find_by_token_hash("token_hash_abc")
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(found.user_id, user.id);

    // 3. List by user id
    let user_sessions = session_repo.list_by_user_id(user.id).await.expect("list");
    assert_eq!(user_sessions.len(), 1);

    // 4. Delete expired
    let expired_count = session_repo
        .delete_expired(now + 4000)
        .await
        .expect("delete expired");
    assert_eq!(expired_count, 1);

    let after_expire = session_repo
        .find_by_token_hash("token_hash_abc")
        .await
        .expect("find");
    assert!(after_expire.is_none());
}

#[tokio::test]
async fn test_audit_log_repository() {
    let pool = setup_test_db().await;
    let repo = SqliteAuditLogRepository::new(pool);

    let log_input = CreateAuditLog {
        user_id: None,
        actor_username: "admin".to_string(),
        action: "site.create".to_string(),
        target_resource: "example.com".to_string(),
        details_json: Some(r#"{"kind":"php"}"#.to_string()),
        ip_address: Some("192.168.1.1".to_string()),
        status: "success".to_string(),
    };

    let log = repo.create(&log_input).await.expect("create audit log");
    assert_eq!(log.action, "site.create");
    assert_eq!(log.target_resource, "example.com");

    let list = repo.list(10, 0).await.expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, log.id);
}

#[tokio::test]
async fn test_job_repository_and_recovery() {
    let pool = setup_test_db().await;
    let repo = SqliteJobRepository::new(pool);

    let job_input = CreateJob {
        id: "job_deploy_1".to_string(),
        user_id: None,
        name: "site.deploy".to_string(),
        payload_json: r#"{"domain":"example.com"}"#.to_string(),
        max_retries: 3,
    };

    // 1. Create
    let job = repo.create(&job_input).await.expect("create job");
    assert_eq!(job.status, "queued");
    assert_eq!(job.progress, 0);

    // 2. Update progress
    repo.update_progress(&job.id, 50).await.expect("progress");
    let updated = repo
        .find_by_id(&job.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(updated.progress, 50);

    // 3. Mark running
    repo.update_status(&job.id, "running", None, None)
        .await
        .expect("running");

    // 4. Test recovery: Server restarts unexpectedly while job is running
    let recovered = repo.recover_interrupted_jobs().await.expect("recover");
    assert_eq!(recovered, 1);

    let failed_job = repo
        .find_by_id(&job.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(failed_job.status, "failed");
    assert!(failed_job.error_message.unwrap().contains("restart"));
}

#[tokio::test]
async fn test_setting_repository_upsert() {
    let pool = setup_test_db().await;
    let repo = SqliteSettingRepository::new(pool);

    // 1. Set
    repo.set("panel.title", "Nusa Panel").await.expect("set");
    let val = repo.get("panel.title").await.expect("get");
    assert_eq!(val, Some("Nusa Panel".to_string()));

    // 2. Upsert
    repo.set("panel.title", "Nusa Control Panel")
        .await
        .expect("upsert");
    let updated_val = repo.get("panel.title").await.expect("get");
    assert_eq!(updated_val, Some("Nusa Control Panel".to_string()));

    // 3. List
    let list = repo.list().await.expect("list");
    assert_eq!(list.len(), 1);

    // 4. Delete
    repo.delete("panel.title").await.expect("delete");
    let after_del = repo.get("panel.title").await.expect("get");
    assert!(after_del.is_none());
}

#[tokio::test]
async fn test_api_token_repository() {
    let pool = setup_test_db().await;
    let user_repo = SqliteUserRepository::new(pool.clone());
    let token_repo = SqliteApiTokenRepository::new(pool);

    let user = user_repo
        .create(&CreateUser {
            username: "bob".to_string(),
            password_hash: "hash".to_string(),
            role: "user".to_string(),
            email: None,
            linux_username: Some("p_bob".to_string()),
            linux_uid: Some(10002),
            plan_id: None,
        })
        .await
        .expect("create user");

    let token_input = CreateApiToken {
        id: "tok_1".to_string(),
        user_id: user.id,
        name: "CI Deploy Token".to_string(),
        token_hash: "sha256_hash_value".to_string(),
        scopes_json: r#"["sites:read", "sites:deploy"]"#.to_string(),
        expires_at: Some(2000000000),
    };

    let token = token_repo.create(&token_input).await.expect("create token");
    assert_eq!(token.name, "CI Deploy Token");

    let found = token_repo
        .find_by_token_hash("sha256_hash_value")
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(found.id, "tok_1");

    token_repo
        .update_last_used("tok_1", 1700000000)
        .await
        .expect("update");
    let after_used = token_repo
        .find_by_token_hash("sha256_hash_value")
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(after_used.last_used_at, Some(1700000000));

    let user_tokens = token_repo.list_by_user_id(user.id).await.expect("list");
    assert_eq!(user_tokens.len(), 1);

    token_repo.delete("tok_1").await.expect("delete");
    let after_del = token_repo
        .find_by_token_hash("sha256_hash_value")
        .await
        .expect("find");
    assert!(after_del.is_none());
}
