use std::sync::OnceLock;

use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, SqlitePool};
use ulid::Ulid;

use crate::internal::domain::{
    AuthError, InvitedAccount, IssuedSession, SESSION_LIFETIME_DAYS, User, UserId, validate_email,
    validate_password,
};

use super::password::{hash_password, verify_password};
use super::token::{generate_token, hash_token};
use crate::internal::domain::{OAuthProfile, OAuthProvider};

/// Owns every SQL statement the auth feature runs. Pure local
/// persistence, not a hexagonal port (scoped that pattern
/// to the external tracker boundary) — nothing external to swap
/// here.
#[derive(Clone)]
pub struct AuthStore {
    pool: SqlitePool,
}

impl AuthStore {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Bootstraps the first (and only ever first) account. Any call
    /// after the table already has a row fails with
    /// `AlreadyBootstrapped` — this is not a general signup
    /// endpoint (single-tenant, self-hosted, no public
    /// registration surface).
    ///
    /// # Errors
    /// `AlreadyBootstrapped` if a user already exists,
    /// `InvalidEmail`/`PasswordTooShort` on bad input, `Storage` on
    /// any SQL failure.
    pub async fn register(
        &self,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<IssuedSession, AuthError> {
        validate_email(email)?;
        validate_password(password)?;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        if count > 0 {
            return Err(AuthError::AlreadyBootstrapped);
        }

        let password_hash = hash_password(password).map_err(AuthError::Storage)?;
        let id = UserId(Ulid::new().to_string());
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO users (id, email, password_hash, display_name, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id.0)
        .bind(email)
        .bind(&password_hash)
        .bind(display_name)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        let user = User {
            id,
            email: email.to_owned(),
            display_name: display_name.to_owned(),
            created_at: now,
            updated_at: now,
            must_change_password: false,
        };
        self.issue_session(user).await
    }

    /// Creates a new account with a server-generated temporary
    /// password, never chosen by the inviter — so the inviter never
    /// learns the invitee's working password, only the one-time
    /// value they must relay and the invitee must immediately
    /// replace. This is the only way to create an account after the
    /// one-time bootstrap: unlike [`Self::register`] (no auth) and
    /// [`Self::login_via_oauth`] (never creates), this requires an
    /// already-authenticated caller — enforced by the router, not
    /// here.
    ///
    /// # Errors
    /// `InvalidEmail` on bad input, `EmailAlreadyRegistered` if the
    /// email is taken, `Storage` on any SQL failure.
    pub async fn invite_user(
        &self,
        email: &str,
        display_name: &str,
    ) -> Result<InvitedAccount, AuthError> {
        validate_email(email)?;

        let existing: Option<i64> = sqlx::query_scalar("SELECT 1 FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await?;
        if existing.is_some() {
            return Err(AuthError::EmailAlreadyRegistered);
        }

        let temporary_password = generate_token();
        let password_hash = hash_password(&temporary_password).map_err(AuthError::Storage)?;
        let id = UserId(Ulid::new().to_string());
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO users \
             (id, email, password_hash, display_name, created_at, updated_at, must_change_password) \
             VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&id.0)
        .bind(email)
        .bind(&password_hash)
        .bind(display_name)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(InvitedAccount {
            user: User {
                id,
                email: email.to_owned(),
                display_name: display_name.to_owned(),
                created_at: now,
                updated_at: now,
                must_change_password: true,
            },
            temporary_password,
        })
    }

    /// # Errors
    /// `InvalidCredentials` if `current_password` doesn't match.
    /// `PasswordTooShort` if `new_password` is under the minimum.
    /// `UserNotFound` if `user_id` doesn't exist. `Storage` on any
    /// SQL failure.
    pub async fn change_password(
        &self,
        user_id: &UserId,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        let password_hash: Option<String> =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
                .bind(&user_id.0)
                .fetch_optional(&self.pool)
                .await?;
        let Some(password_hash) = password_hash else {
            return Err(AuthError::UserNotFound);
        };
        if !verify_password(current_password, &password_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        validate_password(new_password)?;

        let new_hash = hash_password(new_password).map_err(AuthError::Storage)?;
        sqlx::query(
            "UPDATE users SET password_hash = ?, must_change_password = 0, updated_at = ? \
             WHERE id = ?",
        )
        .bind(&new_hash)
        .bind(Utc::now().to_rfc3339())
        .bind(&user_id.0)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// # Errors
    /// `InvalidCredentials` on any mismatch — deliberately identical
    /// whether the email doesn't exist or the password is wrong.
    /// `Storage` on any SQL failure.
    pub async fn login(&self, email: &str, password: &str) -> Result<IssuedSession, AuthError> {
        let row = sqlx::query(
            "SELECT id, email, password_hash, display_name, created_at, updated_at, disabled_at, \
                    must_change_password \
             FROM users WHERE email = ?",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            // Run a hash verification anyway against a real (but
            // otherwise unused) Argon2id hash so a missing-email
            // response takes about as long as a wrong-password one
            // — cheap defense against timing-based email
            // enumeration.
            let _ = verify_password(password, dummy_hash());
            return Err(AuthError::InvalidCredentials);
        };

        let password_hash: String = row.get("password_hash");
        if !verify_password(password, &password_hash) {
            return Err(AuthError::InvalidCredentials);
        }

        // Deliberately the same error as a wrong password — a
        // disabled account should read to the caller exactly like
        // one that never existed.
        if is_disabled(&row) {
            return Err(AuthError::InvalidCredentials);
        }

        let user = user_from_row(&row)?;
        self.issue_session(user).await
    }

    /// # Errors
    /// `InvalidSession` if the token is unknown or expired —
    /// deliberately the same error for both. `Storage` on any SQL
    /// failure.
    pub async fn resolve_session(&self, token: &str) -> Result<User, AuthError> {
        let token_hash = hash_token(token);
        let row = sqlx::query(
            "SELECT u.id, u.email, u.password_hash, u.display_name, u.created_at, u.updated_at, \
                    u.disabled_at, u.must_change_password, s.expires_at \
             FROM session_tokens s JOIN users u ON u.id = s.user_id \
             WHERE s.token_hash = ?",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AuthError::InvalidSession)?;

        let expires_at: String = row.get("expires_at");
        let expires_at = parse_ts(&expires_at)?;
        if expires_at < Utc::now() {
            return Err(AuthError::InvalidSession);
        }

        // A token issued before the account was disabled must stop
        // working the instant it is — deleting session_tokens rows
        // isn't needed, this join catches it every time.
        if is_disabled(&row) {
            return Err(AuthError::InvalidSession);
        }

        user_from_row(&row)
    }

    /// # Errors
    /// `CannotDisableSelf` if `target_id == caller_id`. `UserNotFound`
    /// if no user has `target_id`. `Storage` on any SQL failure.
    pub async fn disable_user(&self, target_id: &str, caller_id: &UserId) -> Result<(), AuthError> {
        if target_id == caller_id.0 {
            return Err(AuthError::CannotDisableSelf);
        }
        let now = Utc::now();
        let result = sqlx::query("UPDATE users SET disabled_at = ?, updated_at = ? WHERE id = ?")
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .bind(target_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AuthError::UserNotFound);
        }
        Ok(())
    }

    /// # Errors
    /// `UserNotFound` if no user has `target_id`. `Storage` on any
    /// SQL failure.
    pub async fn enable_user(&self, target_id: &str) -> Result<(), AuthError> {
        let now = Utc::now();
        let result =
            sqlx::query("UPDATE users SET disabled_at = NULL, updated_at = ? WHERE id = ?")
                .bind(now.to_rfc3339())
                .bind(target_id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(AuthError::UserNotFound);
        }
        Ok(())
    }

    /// Logs in through a provider, **linking only, never creating**
    ///.
    ///
    /// A returning login is matched on the provider's own account id.
    /// A first one is matched on the email, which is why every adapter
    /// must hand over an address that provider has verified — see
    /// [`OAuthProfile::email`].
    ///
    /// Every failure to find an account is the same error, whether the
    /// identity is unknown, the email matches nobody, or the account
    /// is disabled. Distinguishing them would answer "does this person
    /// have an account here" to anyone with a provider account.
    ///
    /// # Errors
    /// `OAuthAccountNotLinked` when no enabled account matches,
    /// `Storage` on any SQL failure.
    pub async fn login_via_oauth(
        &self,
        provider: OAuthProvider,
        profile: OAuthProfile,
        scopes: &str,
    ) -> Result<IssuedSession, AuthError> {
        if let Some(row) = sqlx::query(
            "SELECT u.id, u.email, u.password_hash, u.display_name, u.created_at, u.updated_at, \
                    u.disabled_at, u.must_change_password \
             FROM oauth_identities o JOIN users u ON u.id = o.user_id \
             WHERE o.provider = ? AND o.provider_user_id = ?",
        )
        .bind(provider.as_str())
        .bind(&profile.provider_user_id)
        .fetch_optional(&self.pool)
        .await?
        {
            if is_disabled(&row) {
                return Err(AuthError::OAuthAccountNotLinked);
            }
            let user = user_from_row(&row)?;
            self.touch_oauth_identity(provider, &user.id, scopes)
                .await?;
            return self.issue_session(user).await;
        }

        let row = sqlx::query(
            "SELECT id, email, password_hash, display_name, created_at, updated_at, disabled_at, \
                    must_change_password \
             FROM users WHERE email = ?",
        )
        .bind(&profile.email)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AuthError::OAuthAccountNotLinked)?;

        if is_disabled(&row) {
            return Err(AuthError::OAuthAccountNotLinked);
        }
        let user = user_from_row(&row)?;

        self.link_oauth_identity(provider, &user.id, &profile.provider_user_id, scopes)
            .await?;
        self.issue_session(user).await
    }

    async fn link_oauth_identity(
        &self,
        provider: OAuthProvider,
        user_id: &UserId,
        provider_user_id: &str,
        scopes: &str,
    ) -> Result<(), AuthError> {
        sqlx::query(
            "INSERT INTO oauth_identities \
             (id, user_id, provider, provider_user_id, scopes, linked_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(Ulid::new().to_string())
        .bind(&user_id.0)
        .bind(provider.as_str())
        .bind(provider_user_id)
        .bind(scopes)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Refreshes the recorded scopes on an already-linked identity.
    ///
    /// The provider's access token is deliberately not stored: it was
    /// written here and read nowhere, which made it a live third-party
    /// credential earning nothing. The scopes are kept because they
    /// describe what the link permits, which is a property of the
    /// link rather than a secret.
    async fn touch_oauth_identity(
        &self,
        provider: OAuthProvider,
        user_id: &UserId,
        scopes: &str,
    ) -> Result<(), AuthError> {
        sqlx::query(
            "UPDATE oauth_identities SET scopes = ? \
             WHERE user_id = ? AND provider = ?",
        )
        .bind(scopes)
        .bind(&user_id.0)
        .bind(provider.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn issue_session(&self, user: User) -> Result<IssuedSession, AuthError> {
        let token = generate_token();
        let token_hash = hash_token(&token);
        let now = Utc::now();
        let expires_at = now + Duration::days(SESSION_LIFETIME_DAYS);

        sqlx::query(
            "INSERT INTO session_tokens (id, user_id, token_hash, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Ulid::new().to_string())
        .bind(&user.id.0)
        .bind(&token_hash)
        .bind(now.to_rfc3339())
        .bind(expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(IssuedSession {
            token,
            user,
            expires_at,
        })
    }
}

/// A real Argon2id hash of a value nobody will ever type, computed
/// once and cached, used only to burn roughly the same time on the
/// "no such email" path in `login` as a real verification would.
fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        hash_password("atlas-auth-timing-mitigation-dummy-password")
            .expect("hashing a fixed dummy password cannot fail")
    })
}

fn is_disabled(row: &sqlx::sqlite::SqliteRow) -> bool {
    row.get::<Option<String>, _>("disabled_at").is_some()
}

fn user_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<User, AuthError> {
    Ok(User {
        id: UserId(row.get("id")),
        email: row.get("email"),
        display_name: row.get("display_name"),
        created_at: parse_ts(&row.get::<String, _>("created_at"))?,
        updated_at: parse_ts(&row.get::<String, _>("updated_at"))?,
        must_change_password: row.get("must_change_password"),
    })
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, AuthError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| AuthError::Storage(format!("stored timestamp unparsable: {e}")))
}
