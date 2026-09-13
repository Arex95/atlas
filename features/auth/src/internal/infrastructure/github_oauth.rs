//! GitHub side of "Continue with GitHub".
//!
//! The same three steps as the GitLab adapter — authorize, exchange,
//! fetch the profile — and one difference that is the whole reason
//! this file is not a copy of that one.
//!
//! **GitHub's profile email cannot be trusted, so it is not used.**
//! `GET /user` returns the account's *public* email, which may be
//! absent, and which GitHub does not guarantee to have verified. A
//! first login links by matching that address against an existing
//! account, so accepting an unverified one is an account takeover:
//! add the victim's address to your own GitHub account, click
//! consent, and Atlas attaches your identity to their account.
//!
//! So the address comes from `GET /user/emails`, and only an entry
//! that is **both primary and verified** is accepted. No fallback to
//! the profile field — a fallback would restore the hole on exactly
//! the accounts that keep their address private.

use serde::Deserialize;
use url::Url;

use crate::internal::domain::{AuthError, OAuthProfile};

/// `read:user` for the profile, `user:email` for the address list.
///
/// Deliberately no `repo`: Atlas does not read GitHub repositories, so
/// asking for them would be a scope it never uses on a consent screen
/// a human has to believe.
const SCOPE: &str = "read:user user:email";

/// GitHub rejects API requests without one, and asks that it identify
/// the application.
const USER_AGENT: &str = "atlas-server";

#[derive(Clone)]
pub struct GithubOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub state_secret: String,
    /// The OAuth endpoints (`github.com`). Separate from `api_base_url`
    /// because GitHub splits them across two hosts, unlike GitLab.
    pub base_url: String,
    /// The REST API (`api.github.com`).
    pub api_base_url: String,
}

impl GithubOAuthConfig {
    #[must_use]
    pub fn github_com(
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
            base_url: "https://github.com".to_owned(),
            api_base_url: "https://api.github.com".to_owned(),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    scope: String,
}

#[derive(Deserialize)]
struct UserResponse {
    id: i64,
    /// GitHub's display name is optional; the login handle is not.
    #[serde(default)]
    name: Option<String>,
    login: String,
}

#[derive(Deserialize)]
struct EmailResponse {
    email: String,
    primary: bool,
    verified: bool,
}

pub struct GithubOAuthClient {
    config: GithubOAuthConfig,
    http: reqwest::Client,
}

impl GithubOAuthClient {
    #[must_use]
    pub fn new(config: GithubOAuthConfig) -> Self {
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
            .join("/login/oauth/authorize")
            .expect("static path");
        url.query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri)
            .append_pair("scope", SCOPE)
            .append_pair("state", state);
        url
    }

    /// # Errors
    /// `OAuthProviderError` on any transport, non-2xx, or decode
    /// failure.
    pub async fn exchange_code(&self, code: &str) -> Result<(String, String), AuthError> {
        let token_url = format!("{}/login/oauth/access_token", self.config.base_url);
        let response = self
            .http
            .post(token_url)
            // Without this GitHub answers form-encoded, and the JSON
            // decode below fails on a response that was otherwise fine.
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("code", code),
                ("redirect_uri", self.config.redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(|e| AuthError::OAuthProviderError(e.to_string()))?;

        // GitHub answers 200 with an `error` body for a bad code, so a
        // status check alone would let a failed exchange through as a
        // malformed-token-response error further down.
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| AuthError::OAuthProviderError(e.to_string()))?;
        if !status.is_success() {
            return Err(AuthError::OAuthProviderError(format!(
                "token exchange returned {status}: {body}"
            )));
        }

        let parsed: TokenResponse = serde_json::from_str(&body).map_err(|_| {
            AuthError::OAuthProviderError(format!("token exchange failed: {}", summarise(&body)))
        })?;
        Ok((parsed.access_token, parsed.scope))
    }

    /// # Errors
    /// `OAuthProviderError` on any transport, non-2xx, or decode
    /// failure, or if the account has no verified primary email.
    pub async fn fetch_profile(&self, access_token: &str) -> Result<OAuthProfile, AuthError> {
        let user: UserResponse = self.get("/user", access_token).await?;
        let emails: Vec<EmailResponse> = self.get("/user/emails", access_token).await?;

        // Both conditions, and no fallback. `primary` alone would take
        // an address the account has not proven it controls; `verified`
        // alone would pick an arbitrary confirmed address, which may not
        // be the one the account is known by.
        let email = emails
            .into_iter()
            .find(|e| e.primary && e.verified)
            .map(|e| e.email)
            .ok_or_else(|| {
                AuthError::OAuthProviderError(
                    "this GitHub account has no verified primary email; verify one on GitHub and \
                     try again"
                        .to_owned(),
                )
            })?;

        Ok(OAuthProfile {
            provider_user_id: user.id.to_string(),
            email,
            // GitHub's display name is optional; the handle always
            // exists and is what other people see.
            name: user.name.unwrap_or(user.login),
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        access_token: &str,
    ) -> Result<T, AuthError> {
        let url = format!("{}{path}", self.config.api_base_url);
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| AuthError::OAuthProviderError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AuthError::OAuthProviderError(format!(
                "{path} returned {status}: {}",
                summarise(&body)
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AuthError::OAuthProviderError(format!("malformed {path} response: {e}")))
    }
}

/// Trims a provider body for an error message.
///
/// Bounded because it reaches a log and an HTTP response, and a
/// provider that answers with a page of HTML should not put a page of
/// HTML in either.
fn summarise(body: &str) -> String {
    const MAX: usize = 200;
    let trimmed = body.trim();
    match trimmed.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &trimmed[..cut]),
        None => trimmed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> GithubOAuthClient {
        GithubOAuthClient::new(GithubOAuthConfig::github_com(
            "client-id".to_owned(),
            "secret".to_owned(),
            "https://atlas.example/api/auth/oauth/github/callback".to_owned(),
            "state-secret".to_owned(),
        ))
    }

    #[test]
    fn the_authorize_url_carries_the_state_and_asks_for_no_repo_scope() {
        let url = client().authorize_url("signed-state");
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

        assert_eq!(url.path(), "/login/oauth/authorize");
        assert_eq!(query.get("state").map(String::as_str), Some("signed-state"));
        assert_eq!(
            query.get("client_id").map(String::as_str),
            Some("client-id")
        );

        let scope = query.get("scope").expect("no scope requested");
        assert!(scope.contains("user:email"), "{scope}");
        assert!(
            !scope.contains("repo"),
            "asked for repository access it never uses: {scope}"
        );
    }

    #[test]
    fn a_long_provider_body_is_bounded_before_it_reaches_a_log() {
        let long = "x".repeat(5_000);
        assert!(summarise(&long).len() < 260, "{}", summarise(&long).len());
        assert_eq!(summarise("  short  "), "short");
    }

    #[test]
    fn summarising_does_not_split_a_character() {
        // A byte-index cut would panic here rather than truncate.
        let body = "é".repeat(500);
        let _ = summarise(&body);
    }
}
