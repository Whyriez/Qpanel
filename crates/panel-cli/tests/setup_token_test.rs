#![allow(clippy::unwrap_used, clippy::expect_used)]

use panel_cli::{
    SetupStatus, create_admin, generate_setup_token, get_setup_status, verify_setup_token,
};
use panel_db::pool::{init_pool, run_migrations};
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_setup_token_lifecycle() {
    let tmp = NamedTempFile::new().expect("create temp file");
    let db_path = tmp.path().to_str().expect("to_str");
    let pool = init_pool(&format!("sqlite://{}", db_path.replace('\\', "/")))
        .await
        .expect("init pool");
    run_migrations(&pool).await.expect("run migrations");

    // Initially uninitialized
    let initial_status = get_setup_status(&pool).await.expect("initial status");
    assert_eq!(initial_status, SetupStatus::Uninitialized);

    // Generate token
    let token = generate_setup_token(&pool).await.expect("generate token");
    assert_eq!(token.len(), 64);

    // Pending status
    let pending_status = get_setup_status(&pool).await.expect("pending status");
    match pending_status {
        SetupStatus::Pending {
            token: active_token,
            created_at,
        } => {
            assert_eq!(active_token, token);
            assert!(created_at.is_some());
        }
        _ => panic!("Expected Pending status"),
    }

    // Verify token
    let valid = verify_setup_token(&pool, &token)
        .await
        .expect("verify valid");
    assert!(valid);

    let invalid = verify_setup_token(&pool, "wrong-token-value")
        .await
        .expect("verify invalid");
    assert!(!invalid);

    // After admin is created, token cannot be generated anymore and verification fails
    create_admin(&pool, "bootstrap_admin", "Password123!", None)
        .await
        .expect("create admin");

    let status_completed = get_setup_status(&pool).await.expect("status completed");
    assert_eq!(status_completed, SetupStatus::Completed);

    let gen_err = generate_setup_token(&pool).await;
    assert!(gen_err.is_err());

    let verify_after = verify_setup_token(&pool, &token)
        .await
        .expect("verify after");
    assert!(!verify_after);
}
