use reqwest::{Client, Response, StatusCode};
use url::Url;

use crate::internal::domain::TrackerError;

/// Thin HTTP wrapper around `reqwest::Client`.
///
/// Owns the base URL, the PAT header (`PRIVATE-TOKEN`) and the
/// user-agent; every call goes through `get_json`, which centralises
/// the status→[`TrackerError`] mapping so the adapter body only
/// deals in domain-shaped errors.
pub(super) struct HttpClient {
    inner: Client,
    base_url: Url,
    token: String,
}

impl HttpClient {
    pub(super) fn new(base_url: Url, token: String, user_agent: String) -> Result<Self, String> {
        let inner = Client::builder()
            .user_agent(user_agent)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            inner,
            base_url,
            token,
        })
    }

    /// GET `path` (a `/api/v4/...` absolute-path), attach the
    /// PAT, decode the JSON body.
    ///
    /// # Errors
    /// Maps 401 → `Unauthorized`, 404 → `NotFound`,
    /// 429 → `RateLimited { retry_after }`, other non-success
    /// → `Transport(...)`. Malformed JSON → `Malformed(...)`.
    pub(super) async fn get_json<T>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, TrackerError>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| TrackerError::Transport(format!("bad path {path}: {e}")))?;
        let response = self
            .inner
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("Accept", "application/json")
            .query(query)
            .send()
            .await
            .map_err(|e| TrackerError::Transport(e.to_string()))?;
        let response = check_status(response)?;
        response
            .json::<T>()
            .await
            .map_err(|e| TrackerError::Malformed(e.to_string()))
    }

    /// POST `path` with a form-encoded body, attach the PAT, decode
    /// the JSON body. Same status mapping as [`Self::get_json`],
    /// plus 422 → `Invalid` and 409 → `Conflict`.
    ///
    /// GitLab's issue write endpoints accept `application/x-www-form-urlencoded`
    /// or query params; form body keeps this off the URL length limit
    /// for a long `description`.
    ///
    /// # Errors
    /// See [`Self::get_json`], plus the additional write-error mapping above.
    pub(super) async fn post_json<T>(
        &self,
        path: &str,
        form: &[(&str, String)],
    ) -> Result<T, TrackerError>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| TrackerError::Transport(format!("bad path {path}: {e}")))?;
        let response = self
            .inner
            .post(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("Accept", "application/json")
            .form(form)
            .send()
            .await
            .map_err(|e| TrackerError::Transport(e.to_string()))?;
        let response = check_status(response)?;
        response
            .json::<T>()
            .await
            .map_err(|e| TrackerError::Malformed(e.to_string()))
    }

    /// PUT `path` with a form-encoded body. See [`Self::post_json`].
    ///
    /// # Errors
    /// See [`Self::post_json`].
    pub(super) async fn put_json<T>(
        &self,
        path: &str,
        form: &[(&str, String)],
    ) -> Result<T, TrackerError>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| TrackerError::Transport(format!("bad path {path}: {e}")))?;
        let response = self
            .inner
            .put(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("Accept", "application/json")
            .form(form)
            .send()
            .await
            .map_err(|e| TrackerError::Transport(e.to_string()))?;
        let response = check_status(response)?;
        response
            .json::<T>()
            .await
            .map_err(|e| TrackerError::Malformed(e.to_string()))
    }
}

fn check_status(response: Response) -> Result<Response, TrackerError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(TrackerError::Unauthorized),
        StatusCode::NOT_FOUND => Err(TrackerError::NotFound),
        StatusCode::CONFLICT => Err(TrackerError::Conflict(format!(
            "tracker returned HTTP {}",
            status.as_u16()
        ))),
        StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST => Err(TrackerError::Invalid(
            format!("tracker returned HTTP {}", status.as_u16()),
        )),
        StatusCode::TOO_MANY_REQUESTS => {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(std::time::Duration::from_secs);
            Err(TrackerError::RateLimited { retry_after })
        }
        other => Err(TrackerError::Transport(format!(
            "tracker returned HTTP {}",
            other.as_u16()
        ))),
    }
}
