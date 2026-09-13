//! Integration tests for `AuthStore` against a real `SQLite`
//! database — real Argon2id hashing, no mocks.

use atlas_auth::api::{
    AuthError, AuthStore, OAuthProfile, OAuthProvider, SqlitePool, UserId, run_migrations,
};

async fn fresh_store() -> AuthStore {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    AuthStore::new(pool)
}

#[tokio::test]
async fn bootstrap_succeeds_once_and_only_once() {
    let store = fresh_store().await;

    let session = store
        .register("admin@example.com", "correct horse battery", "Admin")
        .await
        .unwrap();
    assert_eq!(session.user.email, "admin@example.com");
    assert!(!session.token.is_empty());

    let second = store
        .register("someone-else@example.com", "another password", "Someone")
        .await
        .unwrap_err();
    assert!(matches!(second, AuthError::AlreadyBootstrapped));
}

#[tokio::test]
async fn login_round_trips() {
    let store = fresh_store().await;
    store
        .register("admin@example.com", "correct horse battery", "Admin")
        .await
        .unwrap();

    let session = store
        .login("admin@example.com", "correct horse battery")
        .await
        .unwrap();
    assert_eq!(session.user.email, "admin@example.com");
}

#[tokio::test]
async fn login_rejects_wrong_password() {
    let store = fresh_store().await;
    store
        .register("admin@example.com", "correct horse battery", "Admin")
        .await
        .unwrap();

    let err = store
        .login("admin@example.com", "wrong password")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn login_rejects_unknown_email_identically_to_wrong_password() {
    let store = fresh_store().await;
    store
        .register("admin@example.com", "correct horse battery", "Admin")
        .await
        .unwrap();

    let err = store
        .login("nobody@example.com", "whatever")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn resolve_session_returns_the_user_for_a_valid_token() {
    let store = fresh_store().await;
    let session = store
        .register("admin@example.com", "correct horse battery", "Admin")
        .await
        .unwrap();

    let user = store.resolve_session(&session.token).await.unwrap();
    assert_eq!(user.email, "admin@example.com");
}

#[tokio::test]
async fn resolve_session_rejects_a_garbage_token_cleanly() {
    let store = fresh_store().await;
    let err = store.resolve_session("not-a-real-token").await.unwrap_err();
    assert!(matches!(err, AuthError::InvalidSession));
}

#[tokio::test]
async fn register_rejects_short_password() {
    let store = fresh_store().await;
    let err = store
        .register("admin@example.com", "short", "Admin")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::PasswordTooShort));
}

#[tokio::test]
async fn register_rejects_invalid_email() {
    let store = fresh_store().await;
    let err = store
        .register("not-an-email", "correct horse battery", "Admin")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidEmail));
}

fn gitlab_profile(provider_user_id: &str, email: &str, name: &str) -> OAuthProfile {
    OAuthProfile {
        provider_user_id: provider_user_id.to_owned(),
        email: email.to_owned(),
        name: name.to_owned(),
    }
}

#[tokio::test]
async fn gitlab_login_with_no_existing_match_is_rejected_and_creates_nothing() {
    let store = fresh_store().await;
    let err = store
        .login_via_oauth(
            OAuthProvider::Gitlab,
            gitlab_profile("42", "dev@example.com", "Dev"),
            "read_user read_api",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::OAuthAccountNotLinked));

    // No account was created — a subsequent password login against
    // that same email still fails as "no such user".
    let login_err = store
        .login("dev@example.com", "whatever")
        .await
        .unwrap_err();
    assert!(matches!(login_err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn gitlab_login_with_a_matching_email_links_the_existing_local_account() {
    let store = fresh_store().await;
    let registered = store
        .register("dev@example.com", "correct horse battery", "Dev (local)")
        .await
        .unwrap();

    let session = store
        .login_via_oauth(
            OAuthProvider::Gitlab,
            gitlab_profile("42", "dev@example.com", "Dev (gitlab)"),
            "read_user read_api",
        )
        .await
        .unwrap();

    // Linked to the *existing* user — same id, original display_name
    // untouched (this call only links an identity, it doesn't edit
    // the profile).
    assert_eq!(session.user.id, registered.user.id);
    assert_eq!(session.user.display_name, "Dev (local)");
}

#[tokio::test]
async fn a_second_gitlab_login_resolves_to_the_same_user() {
    let store = fresh_store().await;
    store
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let first = store
        .login_via_oauth(
            OAuthProvider::Gitlab,
            gitlab_profile("42", "dev@example.com", "Dev"),
            "read_user read_api",
        )
        .await
        .unwrap();

    // A returning login: same provider_user_id, possibly a different
    // email if they changed it on GitLab — resolution is by identity
    // first, not email, once linked.
    let second = store
        .login_via_oauth(
            OAuthProvider::Gitlab,
            gitlab_profile("42", "dev-new-email@example.com", "Dev"),
            "read_user read_api",
        )
        .await
        .unwrap();

    assert_eq!(first.user.id, second.user.id);
}

#[tokio::test]
async fn gitlab_login_against_a_disabled_account_is_rejected() {
    let store = fresh_store().await;
    let registered = store
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    store
        .disable_user(&registered.user.id.0, &UserId("someone-else".to_owned()))
        .await
        .unwrap();

    let err = store
        .login_via_oauth(
            OAuthProvider::Gitlab,
            gitlab_profile("42", "dev@example.com", "Dev"),
            "read_user read_api",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::OAuthAccountNotLinked));
}

#[tokio::test]
async fn disabled_user_cannot_login_by_password_or_use_an_existing_session() {
    let store = fresh_store().await;
    let session = store
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();

    let caller = UserId("someone-else".to_owned());
    store
        .disable_user(&session.user.id.0, &caller)
        .await
        .unwrap();

    let login_err = store
        .login("dev@example.com", "correct horse battery")
        .await
        .unwrap_err();
    assert!(matches!(login_err, AuthError::InvalidCredentials));

    let session_err = store.resolve_session(&session.token).await.unwrap_err();
    assert!(matches!(session_err, AuthError::InvalidSession));
}

#[tokio::test]
async fn disable_user_rejects_disabling_your_own_account() {
    let store = fresh_store().await;
    let session = store
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();

    let err = store
        .disable_user(&session.user.id.0, &session.user.id)
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::CannotDisableSelf));
}

#[tokio::test]
async fn disable_user_on_an_unknown_id_is_not_found() {
    let store = fresh_store().await;
    let err = store
        .disable_user("no-such-id", &UserId("caller".to_owned()))
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::UserNotFound));
}

#[tokio::test]
async fn enable_user_restores_access() {
    let store = fresh_store().await;
    let session = store
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let other = UserId("someone-else".to_owned());
    store
        .disable_user(&session.user.id.0, &other)
        .await
        .unwrap();
    store.enable_user(&session.user.id.0).await.unwrap();

    let session = store
        .login("dev@example.com", "correct horse battery")
        .await
        .unwrap();
    assert_eq!(session.user.email, "dev@example.com");
}

#[tokio::test]
async fn invite_user_creates_an_account_with_a_temporary_password_flagged_for_change() {
    let store = fresh_store().await;
    let invited = store
        .invite_user("teammate@example.com", "Teammate")
        .await
        .unwrap();

    assert_eq!(invited.user.email, "teammate@example.com");
    assert!(invited.user.must_change_password);
    assert!(!invited.temporary_password.is_empty());

    let session = store
        .login("teammate@example.com", &invited.temporary_password)
        .await
        .unwrap();
    assert!(session.user.must_change_password);
}

#[tokio::test]
async fn invite_user_rejects_an_email_that_already_has_an_account() {
    let store = fresh_store().await;
    store
        .register("teammate@example.com", "correct horse battery", "Teammate")
        .await
        .unwrap();

    let err = store
        .invite_user("teammate@example.com", "Teammate")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::EmailAlreadyRegistered));
}

#[tokio::test]
async fn change_password_replaces_the_password_and_clears_the_must_change_flag() {
    let store = fresh_store().await;
    let invited = store
        .invite_user("teammate@example.com", "Teammate")
        .await
        .unwrap();

    store
        .change_password(
            &invited.user.id,
            &invited.temporary_password,
            "a brand new password",
        )
        .await
        .unwrap();

    let session = store
        .login("teammate@example.com", "a brand new password")
        .await
        .unwrap();
    assert!(!session.user.must_change_password);

    let old_password_err = store
        .login("teammate@example.com", &invited.temporary_password)
        .await
        .unwrap_err();
    assert!(matches!(old_password_err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn change_password_rejects_a_wrong_current_password() {
    let store = fresh_store().await;
    let invited = store
        .invite_user("teammate@example.com", "Teammate")
        .await
        .unwrap();

    let err = store
        .change_password(&invited.user.id, "wrong current password", "new password!")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn change_password_rejects_a_short_new_password() {
    let store = fresh_store().await;
    let invited = store
        .invite_user("teammate@example.com", "Teammate")
        .await
        .unwrap();

    let err = store
        .change_password(&invited.user.id, &invited.temporary_password, "short")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::PasswordTooShort));
}
