//! GitLab side of "Continue with GitLab": the OAuth
//! Authorization Code exchange and the profile fetch that follows
//! it. No PKCE — this is a confidential server-side client with its
//! own `client_secret`, not a public client, so PKCE adds no
//! protection PKCE was designed to provide.

use serde::Deserialize;
use url::Url;

use crate::internal::domain::{AuthError, OAuthProfile};

/// One combined grant for login *and* tracker reads — a
/// single OAuth prompt, not two.
const SCOPE: &str = "read_user read_api";

#[derive(Clone)]
pub struct GitlabOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub state_secret: String,
    /// Overridable for tests to point at a `wiremock` server instead
    /// of `https://gitlab.com`.
    pub base_url: String,
}

impl GitlabOAuthConfig {
    #[must_use]
    pub fn gitlab_com(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        state_secret: String,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
            state_secret,
            base_url: "https://gitlab.com".to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    scope: String,
}

#[derive(Deserialize)]
struct UserResponse {
    id: i64,
    email: Option<String>,
    name: String,
}

pub struct GitlabOAuthClient {
    config: GitlabOAuthConfig,
    http: reqwest::Client,
}

impl GitlabOAuthClient {
    #[must_use]
    pub fn new(config: GitlabOAuthConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    #[must_use]
    pub fn state_secret(&self) -> &str {
        &self.config.state_secret
    }

    /// # Panics
    /// If `base_url` isn't a valid URL — a startup-time configuration
    /// error, not a runtime one.
    #[must_use]
    pub fn authorize_url(&self, state: &str) -> Url {
        let mut url = Url::parse(&self.config.base_url)
            .expect("base_url is validated at startup")
            .join("/oauth/authorize")
            .expect("static path");
        url.query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", SCOPE)
            .append_pair("state", state);
        url
    }

    /// # Errors
    /// `OAuthProviderError` on any transport, non-2xx, or decode
    /// failure.
    pub async fn exchange_code(&self, code: &str) -> Result<(String, String), AuthError> {
        let token_url = format!("{}/oauth/token", self.config.base_url);
        let response = self
            .http
            .post(token_url)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.config.redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(|e| AuthError::OAuthProviderError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AuthError::OAuthProviderError(format!(
                "token exchange returned {status}: {body}"
            )));
        }

        let parsed: TokenResponse = response
            .json()
            .await
            .map_err(|e| AuthError::OAuthProviderError(format!("malformed token response: {e}")))?;
        Ok((parsed.access_token, parsed.scope))
    }

    /// # Errors
    /// `OAuthProviderError` on any transport, non-2xx, or decode
    /// failure, or if GitLab's profile has no verified email.
    pub async fn fetch_profile(&self, access_token: &str) -> Result<OAuthProfile, AuthError> {
        let user_url = format!("{}/api/v4/user", self.config.base_url);
        let response = self
            .http
            .get(user_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AuthError::OAuthProviderError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AuthError::OAuthProviderError(format!(
                "profile fetch returned {status}: {body}"
            )));
        }

        let parsed: UserResponse = response.json().await.map_err(|e| {
            AuthError::OAuthProviderError(format!("malformed profile response: {e}"))
        })?;
        let email = parsed.email.ok_or_else(|| {
            AuthError::OAuthProviderError(
                "GitLab profile has no email; grant a scope that includes it".to_owned(),
            )
        })?;

        // GitLab's profile email is the account's primary, which
        // GitLab requires to be confirmed — so unlike GitHub, no
        // second call is needed to get a verified address.
        Ok(OAuthProfile {
            provider_user_id: parsed.id.to_string(),
            email,
            name: parsed.name,
        })
    }
}
