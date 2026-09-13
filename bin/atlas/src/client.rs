//! Talking to an Atlas server over its MCP endpoint.
//!
//! The CLI is a client of the same surface agents use, not a second
//! way in. Everything below goes through `POST /api/mcp` as JSON-RPC,
//! so anything this can do an agent can do, and a change that breaks
//! agents breaks this too instead of being hidden behind a shared
//! type.

use serde::Deserialize;
use serde_json::{Value, json};

/// Where the credential came from, so an error can say what to fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credential {
    /// `ATLAS_SESSION_TOKEN` — identifies a session, and through it a
    /// developer. What a terminal spawned by Atlas is given.
    Session,
    /// `ATLAS_MCP_TOKEN` — identifies a permitted client and nobody in
    /// particular. Fine on a single-developer install; on a team
    /// server it reaches none of that team's data.
    Shared,
}

pub struct Client {
    base_url: String,
    token: String,
    pub credential: Credential,
    http: reqwest::Client,
}

#[derive(Debug)]
pub enum ClientError {
    NoCredential,
    Transport(String),
    /// The server answered, and said no.
    Rpc {
        code: i64,
        message: String,
    },
    /// The server answered something that is not JSON-RPC — usually a
    /// proxy or the wrong port.
    NotAnAtlasServer(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCredential => write!(
                f,
                "no credential: set ATLAS_SESSION_TOKEN (from sessions.create, and \
                 already set inside a terminal Atlas spawned) or ATLAS_MCP_TOKEN"
            ),
            Self::Transport(e) => write!(f, "could not reach the server: {e}"),
            Self::Rpc { code, message } => write!(f, "{message} (code {code})"),
            Self::NotAnAtlasServer(what) => {
                write!(
                    f,
                    "that address did not answer like an Atlas server: {what}"
                )
            }
        }
    }
}

#[derive(Deserialize)]
struct RpcResponse {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

impl Client {
    /// # Errors
    /// `NoCredential` when neither environment variable is set.
    pub fn from_env(base_url: String) -> Result<Self, ClientError> {
        // The session token first: on a machine where both are set —
        // which is every terminal Atlas spawns — the specific identity
        // is the one worth using, and falling back to the shared token
        // would silently act as nobody.
        let (token, credential) = std::env::var("ATLAS_SESSION_TOKEN")
            .ok()
            .filter(|t| !t.trim().is_empty())
            .map(|t| (t, Credential::Session))
            .or_else(|| {
                std::env::var("ATLAS_MCP_TOKEN")
                    .ok()
                    .filter(|t| !t.trim().is_empty())
                    .map(|t| (t, Credential::Shared))
            })
            .ok_or(ClientError::NoCredential)?;

        Ok(Self {
            base_url,
            token,
            credential,
            http: reqwest::Client::new(),
        })
    }

    /// Calls one MCP tool and returns whatever it produced.
    ///
    /// # Errors
    /// `Transport` if the server is unreachable, `Rpc` if it refused,
    /// `NotAnAtlasServer` if the answer was not JSON-RPC.
    pub async fn call(&self, tool: &str, arguments: Value) -> Result<Value, ClientError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments },
        });
        self.rpc(body).await
    }

    /// Every tool the server exposes.
    ///
    /// # Errors
    /// As [`Self::call`].
    pub async fn tools(&self) -> Result<Value, ClientError> {
        self.rpc(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
            .await
    }

    async fn rpc(&self, body: Value) -> Result<Value, ClientError> {
        let url = format!("{}/api/mcp", self.base_url.trim_end_matches('/'));
        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        // 401 is worth its own message: the difference between "your
        // token is wrong" and "the tool refused" is the difference
        // between two entirely different things to go and fix.
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClientError::Rpc {
                code: 401,
                message: "the server rejected this credential".to_owned(),
            });
        }

        let text = response
            .text()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        let parsed: RpcResponse = serde_json::from_str(&text)
            .map_err(|_| ClientError::NotAnAtlasServer(summarise(&text)))?;

        if let Some(error) = parsed.error {
            return Err(ClientError::Rpc {
                code: error.code,
                message: error.message,
            });
        }
        let result = parsed.result.unwrap_or(Value::Null);

        // A tool's own failure arrives as a *successful* JSON-RPC
        // response carrying `isError`. Reporting that as success and
        // exiting 0 would make a shell script think it worked.
        if result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(ClientError::Rpc {
                code: 0,
                message: tool_text(&result).unwrap_or_else(|| "the tool reported an error".into()),
            });
        }

        Ok(unwrap_tool_result(result))
    }
}

/// MCP wraps a tool's answer in `content[0].text` holding JSON as a
/// string. Unwrapped here so every caller sees the value the tool
/// actually returned rather than the envelope carrying it.
fn unwrap_tool_result(result: Value) -> Value {
    let Some(text) = tool_text(&result) else {
        return result;
    };
    serde_json::from_str(&text).unwrap_or(Value::String(text))
}

fn tool_text(result: &Value) -> Option<String> {
    result
        .get("content")?
        .get(0)?
        .get("text")?
        .as_str()
        .map(ToOwned::to_owned)
}

/// Bounded, because an unexpected body may be an entire HTML page.
fn summarise(body: &str) -> String {
    const MAX: usize = 120;
    let trimmed = body.trim();
    match trimmed.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &trimmed[..cut]),
        None => trimmed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_result_is_unwrapped_from_its_envelope() {
        let enveloped = json!({
            "content": [{ "type": "text", "text": "{\"id\":\"abc\",\"n\":2}" }],
            "isError": false,
        });
        assert_eq!(
            unwrap_tool_result(enveloped),
            json!({ "id": "abc", "n": 2 })
        );
    }

    #[test]
    fn a_non_json_tool_result_survives_as_a_string() {
        let enveloped = json!({ "content": [{ "type": "text", "text": "plain words" }] });
        assert_eq!(unwrap_tool_result(enveloped), json!("plain words"));
    }

    #[test]
    fn a_result_with_no_envelope_is_returned_as_is() {
        assert_eq!(
            unwrap_tool_result(json!({ "tools": [] })),
            json!({ "tools": [] })
        );
    }

    #[test]
    fn an_unexpected_body_is_bounded_before_it_reaches_a_message() {
        assert!(summarise(&"x".repeat(9_000)).len() < 200);
        // A byte-index cut would panic here rather than truncate.
        let _ = summarise(&"é".repeat(400));
    }
}
