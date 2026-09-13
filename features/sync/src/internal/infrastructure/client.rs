//! Client side of the sync engine: one on-demand push-then-pull pass
//! against a remote `atlas-server`'s `/api/sync/**` surface, plus the
//! `live`-mode event stream that says when to run one.
//!
//! The background loops that drive either live in `supervisor.rs` —
//! this type only knows how to perform a pass and how to listen.

use atlas_memory::api::{MemoryEntry, MemoryScope, MemoryStore};
use atlas_notes::api::{Note, NoteStore};
use atlas_sessions::api::{Session, SessionStore};
use futures_util::{Stream, StreamExt};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use url::Url;

use crate::internal::domain::{ChangeKind, SyncError, SyncReport};

pub struct SyncClient {
    inner: Client,
    base_url: Url,
}

#[derive(Deserialize)]
struct SessionsBody {
    sessions: Vec<Session>,
}

#[derive(Deserialize)]
struct MemoryBody {
    entries: Vec<MemoryEntry>,
}

#[derive(Deserialize)]
struct NotesBody {
    notes: Vec<Note>,
}

impl SyncClient {
    /// # Errors
    /// `BadUrl` if `remote_url` doesn't parse, or if it would send this
    /// developer's state across a network in the clear.
    pub fn new(remote_url: &str) -> Result<Self, SyncError> {
        let base_url = Url::parse(remote_url).map_err(|e| SyncError::BadUrl(e.to_string()))?;
        reject_cleartext(&base_url)?;
        Ok(Self {
            inner: Client::new(),
            base_url,
        })
    }

    /// Pushes every session `local` has under `owner_id`, applies
    /// the authoritative (post-last-write-wins) result of each back
    /// locally, then pulls and applies anything the remote has for
    /// that owner that wasn't just pushed. A full pull every call —
    /// no persisted cursor yet, this is `focus` mode, not `auto`.
    ///
    /// # Errors
    /// `Unauthorized` on a bad bearer token, `RemoteError` on any
    /// other non-success HTTP status, `Transport`/`Malformed` on
    /// network or decoding failure, `Local` on a local storage
    /// failure applying the results.
    pub async fn sync_sessions(
        &self,
        bearer_token: &str,
        owner_id: &str,
        local: &SessionStore,
    ) -> Result<SyncReport, SyncError> {
        let to_push = local
            .list_since(owner_id, None)
            .await
            .map_err(|e| SyncError::Local(e.to_string()))?;
        let pushed_count = to_push.len();

        let push_url = self
            .base_url
            .join("sessions/push")
            .map_err(|e| SyncError::BadUrl(e.to_string()))?;
        let push_response = self
            .inner
            .post(push_url)
            .bearer_auth(bearer_token)
            .json(&json!({ "sessions": to_push }))
            .send()
            .await
            .map_err(|e| SyncError::Transport(e.to_string()))?;
        let push_result: SessionsBody = decode(push_response).await?;

        for authoritative in push_result.sessions {
            local
                .upsert_for_sync(authoritative, owner_id)
                .await
                .map_err(|e| SyncError::Local(e.to_string()))?;
        }

        let pull_url = self
            .base_url
            .join("sessions/pull")
            .map_err(|e| SyncError::BadUrl(e.to_string()))?;
        let pull_response = self
            .inner
            .get(pull_url)
            .bearer_auth(bearer_token)
            .send()
            .await
            .map_err(|e| SyncError::Transport(e.to_string()))?;
        let pull_result: SessionsBody = decode(pull_response).await?;
        let pulled_count = pull_result.sessions.len();

        for remote_session in pull_result.sessions {
            local
                .upsert_for_sync(remote_session, owner_id)
                .await
                .map_err(|e| SyncError::Local(e.to_string()))?;
        }

        Ok(SyncReport {
            pushed: pushed_count,
            pulled: pulled_count,
        })
    }

    /// Pushes every memory entry this machine holds for `owner_id`
    /// plus all project memory, applies the authoritative results
    /// back, then pulls whatever the remote has that this machine
    /// doesn't.
    ///
    /// Personal entries are stored under `owner_id` regardless of what
    /// the remote echoes back, mirroring the server's own rule — a
    /// remote cannot use this path to plant rows under another owner.
    ///
    /// # Errors
    /// `Unauthorized` on a bad bearer token, `RemoteError` on any
    /// other non-success HTTP status, `Transport`/`Malformed` on
    /// network or decoding failure, `Local` on a local storage
    /// failure applying the results.
    pub async fn sync_memory(
        &self,
        bearer_token: &str,
        owner_id: &str,
        local: &MemoryStore,
    ) -> Result<SyncReport, SyncError> {
        let mut to_push = local
            .list_project_since(None)
            .await
            .map_err(|e| SyncError::Local(e.to_string()))?;
        to_push.extend(
            local
                .list_personal_since(owner_id, None)
                .await
                .map_err(|e| SyncError::Local(e.to_string()))?,
        );
        let pushed_count = to_push.len();

        let push_url = self
            .base_url
            .join("memory/push")
            .map_err(|e| SyncError::BadUrl(e.to_string()))?;
        let push_response = self
            .inner
            .post(push_url)
            .bearer_auth(bearer_token)
            .json(&json!({ "entries": to_push }))
            .send()
            .await
            .map_err(|e| SyncError::Transport(e.to_string()))?;
        let push_result: MemoryBody = decode(push_response).await?;
        self.apply_memory(push_result.entries, owner_id, local)
            .await?;

        let pull_url = self
            .base_url
            .join("memory/pull")
            .map_err(|e| SyncError::BadUrl(e.to_string()))?;
        let pull_response = self
            .inner
            .get(pull_url)
            .bearer_auth(bearer_token)
            .send()
            .await
            .map_err(|e| SyncError::Transport(e.to_string()))?;
        let pull_result: MemoryBody = decode(pull_response).await?;
        let pulled_count = pull_result.entries.len();
        self.apply_memory(pull_result.entries, owner_id, local)
            .await?;

        Ok(SyncReport {
            pushed: pushed_count,
            pulled: pulled_count,
        })
    }

    /// Push-then-pull for notes (personal state throughout).
    ///
    /// Simpler than memory in exactly one way and it is the important
    /// one: there are no buckets to route between, because there is no
    /// shared variant. Every note is somebody's, so every note on both
    /// legs is forced under `owner_id`.
    ///
    /// # Errors
    /// `Local` on a storage failure, `Transport` or `BadUrl` on the
    /// network legs, `Remote` when the server refuses.
    pub async fn sync_notes(
        &self,
        bearer_token: &str,
        owner_id: &str,
        local: &NoteStore,
    ) -> Result<SyncReport, SyncError> {
        let to_push = local
            .list_since(owner_id, None)
            .await
            .map_err(|e| SyncError::Local(e.to_string()))?;
        let pushed_count = to_push.len();

        let push_url = self
            .base_url
            .join("notes/push")
            .map_err(|e| SyncError::BadUrl(e.to_string()))?;
        let push_response = self
            .inner
            .post(push_url)
            .bearer_auth(bearer_token)
            .json(&json!({ "notes": to_push }))
            .send()
            .await
            .map_err(|e| SyncError::Transport(e.to_string()))?;
        let push_result: NotesBody = decode(push_response).await?;
        self.apply_notes(push_result.notes, owner_id, local).await?;

        let pull_url = self
            .base_url
            .join("notes/pull")
            .map_err(|e| SyncError::BadUrl(e.to_string()))?;
        let pull_response = self
            .inner
            .get(pull_url)
            .bearer_auth(bearer_token)
            .send()
            .await
            .map_err(|e| SyncError::Transport(e.to_string()))?;
        let pull_result: NotesBody = decode(pull_response).await?;
        let pulled_count = pull_result.notes.len();
        self.apply_notes(pull_result.notes, owner_id, local).await?;

        Ok(SyncReport {
            pushed: pushed_count,
            pulled: pulled_count,
        })
    }

    /// Stores what came back under **our** owner, not the one the row
    /// claims.
    ///
    /// The server already forces this on its side; doing it again here
    /// is deliberate rather than redundant. A remote that is wrong,
    /// compromised, or simply an older version cannot make this
    /// machine file another developer's notes as its own.
    async fn apply_notes(
        &self,
        notes: Vec<Note>,
        owner_id: &str,
        local: &NoteStore,
    ) -> Result<(), SyncError> {
        for note in notes {
            local
                .upsert_for_sync(note, owner_id)
                .await
                .map_err(|e| SyncError::Local(e.to_string()))?;
        }
        Ok(())
    }

    async fn apply_memory(
        &self,
        entries: Vec<MemoryEntry>,
        owner_id: &str,
        local: &MemoryStore,
    ) -> Result<(), SyncError> {
        for entry in entries {
            let applied = match entry.scope {
                MemoryScope::Project => local.upsert_project_for_sync(&entry).await,
                MemoryScope::Personal => local.upsert_personal_for_sync(&entry, owner_id).await,
            };
            applied.map_err(|e| SyncError::Local(e.to_string()))?;
        }
        Ok(())
    }

    /// Opens the remote's `live`-mode stream, yielding one item per
    /// change notification addressed to this bearer's own owner.
    /// Kinds this build doesn't recognise are dropped rather than
    /// guessed at (see [`ChangeKind::parse`]).
    ///
    /// The returned stream ends when the connection does. Deciding
    /// whether that warrants a reconnect is the caller's — the
    /// supervisor's — business, not this method's.
    ///
    /// # Errors
    /// `Unauthorized` on a bad bearer token, `RemoteError` on any
    /// other non-success status, `Transport` if the connection can't
    /// be opened at all. Failures *during* streaming surface as
    /// `Err` items inside the stream instead.
    pub async fn events(
        &self,
        bearer_token: &str,
    ) -> Result<impl Stream<Item = Result<ChangeKind, SyncError>> + use<>, SyncError> {
        let url = self
            .base_url
            .join("events")
            .map_err(|e| SyncError::BadUrl(e.to_string()))?;
        let response = self
            .inner
            .get(url)
            .bearer_auth(bearer_token)
            .send()
            .await
            .map_err(|e| SyncError::Transport(e.to_string()))?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(SyncError::Unauthorized);
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(SyncError::RemoteError {
                status: status.as_u16(),
                body,
            });
        }

        let mut decoder = SseDecoder::default();
        let stream = response.bytes_stream().flat_map(move |chunk| {
            let items = match chunk {
                Ok(bytes) => match std::str::from_utf8(&bytes) {
                    Ok(text) => decoder
                        .push(text)
                        .iter()
                        .filter_map(|payload| ChangeKind::parse(payload))
                        .map(Ok)
                        .collect(),
                    Err(e) => vec![Err(SyncError::Malformed(format!(
                        "event stream was not utf-8: {e}"
                    )))],
                },
                Err(e) => vec![Err(SyncError::Transport(e.to_string()))],
            };
            futures_util::stream::iter(items)
        });

        Ok(stream)
    }
}

/// Reassembles SSE frames from transport chunks.
///
/// The whole reason this exists rather than a full SSE library: a
/// chunk boundary falls wherever the network puts it — mid-frame,
/// mid-line, mid-`data:` — so anything not yet terminated by a blank
/// line has to wait for the next chunk. That is the only part of the
/// format that is easy to get wrong, and both ends of this stream are
/// ours, so the fields we never emit (`id:`, `retry:`, multi-line
/// `event:`) are not parsed. Server-sent keep-alive comments (`:`
/// lines) fall out naturally: they contribute no `data:` field.
#[derive(Default)]
struct SseDecoder {
    buffer: String,
}

impl SseDecoder {
    /// Feeds one chunk, returning every `data:` payload it completed.
    fn push(&mut self, chunk: &str) -> Vec<String> {
        self.buffer.push_str(chunk);
        let mut out = Vec::new();
        while let Some(idx) = self.buffer.find("\n\n") {
            let frame: String = self.buffer.drain(..idx + 2).collect();
            let data: Vec<&str> = frame
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(|value| value.strip_prefix(' ').unwrap_or(value))
                .collect();
            if !data.is_empty() {
                out.push(data.join("\n"));
            }
        }
        out
    }
}

async fn decode<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, SyncError> {
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED {
        return Err(SyncError::Unauthorized);
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(SyncError::RemoteError {
            status: status.as_u16(),
            body,
        });
    }
    response
        .json::<T>()
        .await
        .map_err(|e| SyncError::Malformed(e.to_string()))
}

#[cfg(test)]
mod sse_decoder_tests {
    use super::SseDecoder;

    #[test]
    fn a_whole_frame_in_one_chunk_yields_its_payload() {
        let mut decoder = SseDecoder::default();
        assert_eq!(decoder.push("data: sessions\n\n"), vec!["sessions"]);
    }

    #[test]
    fn a_frame_split_across_chunks_waits_for_the_rest() {
        let mut decoder = SseDecoder::default();
        // Every one of these boundaries is one the network can pick.
        assert!(decoder.push("data: ses").is_empty());
        assert!(decoder.push("sions").is_empty());
        assert!(decoder.push("\n").is_empty());
        assert_eq!(decoder.push("\n"), vec!["sessions"]);
    }

    #[test]
    fn several_frames_in_one_chunk_all_come_out_in_order() {
        let mut decoder = SseDecoder::default();
        assert_eq!(
            decoder.push("data: a\n\ndata: b\n\ndata: c\n\n"),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn keep_alive_comments_produce_nothing_and_do_not_disturb_the_stream() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push(":\n\n").is_empty());
        assert!(decoder.push(": keep-alive\n\n").is_empty());
        assert_eq!(decoder.push("data: sessions\n\n"), vec!["sessions"]);
    }

    #[test]
    fn the_event_field_is_ignored_and_only_data_is_returned() {
        let mut decoder = SseDecoder::default();
        assert_eq!(
            decoder.push("event: change\ndata: sessions\n\n"),
            vec!["sessions"]
        );
    }

    #[test]
    fn an_unterminated_trailing_frame_is_not_emitted_early() {
        let mut decoder = SseDecoder::default();
        assert_eq!(
            decoder.push("data: first\n\ndata: incomplete"),
            vec!["first"]
        );
        assert_eq!(decoder.push("\n\n"), vec!["incomplete"]);
    }
}

/// Refuse to sync over a connection that carries the state in the clear.
///
/// Every request carries a bearer token in a header, plus notes, memory
/// and messages in the body. Over plain HTTP to another machine, all of
/// it is readable by anything on the path — and nothing said so: the
/// client accepted `http://` in silence, which made the unsafe choice
/// the quiet default.
///
/// Loopback is exempt because the traffic never reaches a network, and
/// requiring a certificate for a server talking to itself would push
/// people towards disabling the check rather than towards TLS.
fn reject_cleartext(url: &Url) -> Result<(), SyncError> {
    if url.scheme() == "https" {
        return Ok(());
    }

    let loopback = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
    );
    if url.scheme() == "http" && loopback {
        return Ok(());
    }

    Err(SyncError::BadUrl(format!(
        "refusing to sync to {} over {}: the bearer token and everything it carries would \
         cross the network in the clear. Use https, or point at a loopback address.",
        url.host_str().unwrap_or("that host"),
        url.scheme()
    )))
}

#[cfg(test)]
mod cleartext_tests {
    use super::*;

    fn check(u: &str) -> Result<(), SyncError> {
        reject_cleartext(&Url::parse(u).unwrap())
    }

    #[test]
    fn https_to_anywhere_is_fine() {
        assert!(check("https://atlas.example.com/api/sync/").is_ok());
    }

    #[test]
    fn plain_http_to_another_machine_is_refused() {
        // The failure this exists for: the quiet default used to be the
        // unsafe one.
        let err = check("http://atlas.example.com/api/sync/").unwrap_err();
        assert!(format!("{err}").contains("in the clear"), "{err}");
    }

    #[test]
    fn plain_http_to_loopback_is_fine() {
        // It never reaches a network, and demanding a certificate here
        // teaches people to switch the check off.
        for u in [
            "http://localhost:4000/api/sync/",
            "http://127.0.0.1:4000/api/sync/",
        ] {
            assert!(check(u).is_ok(), "{u}");
        }
    }

    #[test]
    fn anything_that_is_not_http_or_https_is_refused() {
        assert!(check("ftp://example.com/").is_err());
    }
}
