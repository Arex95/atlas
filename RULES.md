# Atlas — Coding Rules

Patterns enforced across all audits. Every rule exists because a bug was found in production code.

---

## Rust — Async Safety

**Never hold a lock across an `.await` point.**
Clone the Arc or the data you need, drop the guard, then await.

```rust
// ❌ WRONG — MutexGuard held across await
let mut sessions = self.sessions.lock().await;
tokio::time::sleep(Duration::from_millis(50)).await; // deadlock risk

// ✅ CORRECT — release before awaiting
{ let mut sessions = self.sessions.lock().await; session.writer.write_all(...); }
tokio::time::sleep(Duration::from_millis(50)).await;
{ let mut sessions = self.sessions.lock().await; session.writer.write_all(b"\r"); }
```

**Never call `std::thread::sleep` in async code.** Use `tokio::time::sleep(...).await`.

**No blocking I/O on the async executor.** Use `tokio::fs::*` or `tokio::task::spawn_blocking`.

```rust
// ❌ WRONG
std::fs::read_to_string(&path)?;

// ✅ CORRECT
tokio::fs::read_to_string(&path).await?;
```

---

## Rust — Mutex Choice

`Arc<std::sync::RwLock<T>>` for data accessed from both sync threads and async tasks when reads dominate.
`Arc<tokio::sync::Mutex<T>>` for data accessed only from async tasks.

Never lock `std::sync::RwLock` while holding a `tokio::sync::Mutex` guard — the sync lock can block the thread.

---

## Rust — No Side-Effects in `From` / `Into`

`From<T>` must be pure: no I/O, no subprocess calls, no DB queries. Move those to explicit methods.

```rust
// ❌ WRONG — git subprocess inside From
impl From<AiSession> for AiSessionResponse {
    fn from(s: AiSession) -> Self {
        Self { git: get_git_info(&s.working_directory), .. }
    }
}

// ✅ CORRECT — explicit call at the handler site
let response = AiSessionResponse::from(session).with_git();
```

---

## Rust — SQL Patterns

**ON CONFLICT for upserts.** Never INSERT then catch the unique error, then SELECT.

```sql
-- ❌ WRONG (3 queries, race condition)
INSERT INTO projects (...) VALUES (...)
-- catch unique error
SELECT * FROM projects WHERE slug = ?

-- ✅ CORRECT (1 query, atomic)
INSERT INTO projects (...) VALUES (...)
ON CONFLICT(slug) DO UPDATE SET name = excluded.name, ...
RETURNING *
```

**COALESCE UPDATE for partial updates.** Never SELECT, merge in Rust, then UPDATE.

```sql
-- ❌ WRONG (N+1: SELECT then UPDATE)
SELECT * FROM ai_sessions WHERE id = ?
-- merge in Rust
UPDATE ai_sessions SET custom_name = ? WHERE id = ?

-- ✅ CORRECT (1 query)
UPDATE ai_sessions
SET custom_name = COALESCE(?, custom_name),
    color       = COALESCE(?, color)
WHERE id = ? RETURNING *
```

**Use correct table names.** Verify against `db.rs` before every query. The `ai_sessions` table is not `sessions`.

---

## Rust — Clippy Compliance

All code must pass `cargo clippy -- -D warnings`. Common violations to avoid:

- Use `let`-chain syntax (`if let Some(x) = foo && bar`) instead of nested `if let`
- Use `.first()` instead of `.get(0)`
- Use `['\n', '\r']` array instead of a closure for multi-char splits
- Use `.strip_prefix("$ ")` instead of manual indexing
- Define type aliases for complex types (`type SpawnResult = Result<(Box<...>, ...), String>`)
- Do not call `.ok()` on a `Future` — it does not compile; use `let _ = expr.await`

---

## Rust — Bounded Collections

Every collection that grows from external input must have a cap. Uncapped collections are memory bombs.

```rust
const MAX_DIR_ENTRIES: usize = 1_000;
const SCROLLBACK_MAX_BYTES: usize = 100_000;
```

---

## Rust — DB Connection Pool

Always configure timeouts:

```rust
.acquire_timeout(Duration::from_secs(5))
.idle_timeout(Duration::from_secs(300))
.max_lifetime(Duration::from_secs(1800))
```

---

## TypeScript / Vue — No `any`

All props, events, store state, and API responses must have explicit types. `any` defeats the type system.

```ts
// ❌ WRONG
function handleEvent(data: any) { ... }

// ✅ CORRECT
interface SessionUpdatedEvent { sessionId: string; workingDirectory: string }
function handleEvent(data: SessionUpdatedEvent) { ... }
```

---

## TypeScript / Vue — Watcher Cleanup

Always store the return value of `watch()` and call it in `onBeforeUnmount`.

```ts
// ❌ WRONG — memory leak
onMounted(() => {
  watch(() => props.isVisible, handler);
});

// ✅ CORRECT
let unwatch: (() => void) | null = null;
onMounted(() => {
  unwatch = watch(() => props.isVisible, handler);
});
onBeforeUnmount(() => {
  unwatch?.();
});
```

---

## TypeScript / Vue — Store Mutation via Actions

Never mutate Pinia store state from a component watcher or template. Use store actions.

```ts
// ❌ WRONG — component mutates store directly
watch(something, () => {
  store.injectedCommands[id].shift();
});

// ✅ CORRECT — call a store action
watch(something, () => {
  const cmd = store.consumeCommand(id);
});
```

---

## TypeScript / Vue — Bounded Store Collections

Pinia collections that grow from socket events or user actions must be capped.

```ts
const INBOX_MAX = 100;
const INJECTED_COMMANDS_MAX = 50;

function addInboxMessage(sessionId: string, message: InboxMessage) {
  inboxMessages.value[sessionId].unshift(message);
  if (inboxMessages.value[sessionId].length > INBOX_MAX) {
    inboxMessages.value[sessionId].length = INBOX_MAX;
  }
}
```

---

## TypeScript / Vue — No Duplicate Computed

A store must not expose two computed properties that return the same value. One name, one source of truth.

---

## TypeScript / Vue — Merge, Never Replace

When loading remote data into a collection that may already have local entries, merge — never replace.

```ts
// ❌ WRONG — wipes any locally-created tabs
tabs.value = saved;

// ✅ CORRECT
const existingIds = new Set(tabs.value.map((t) => t.id));
const incoming = saved.filter((s) => !existingIds.has(s.id));
tabs.value = [...tabs.value, ...incoming];
```

---

## TypeScript / Vue — Field Name Contracts

Domain types use camelCase (`lastSyncedAt`). API responses from Axum use `#[serde(rename_all = "camelCase")]`. Never use snake_case field names in templates or components.

```vue
<!-- ❌ WRONG — field doesn't exist on the type -->
{{ project.last_synced_at }}

<!-- ✅ CORRECT -->
{{ project.lastSyncedAt }}
```

---

## Rust — String Constants in `constants.rs`

Every hardcoded string literal that has semantic meaning must live in `apps/atlas-server/src/constants.rs`, not inline in handler/business code. Organize by semantic category, not by file.

```rust
// constants.rs — namespaced modules
pub mod env {
    pub const DATABASE_URL: &str = "DATABASE_URL";
    pub const ATLAS_MCP_TOKEN: &str = "ATLAS_MCP_TOKEN";
    // ...
}
pub mod defaults {
    pub const PORT: u16 = 4000;
    pub const WEB_ORIGIN: &str = "http://localhost:3000";
    // ...
}
pub mod errors {
    pub const SESSION_NOT_FOUND: &str = "Session not found";
    pub const FILE_TOO_LARGE: &str = "File exceeds 5 MB limit";
    // ...
}
pub mod response {
    pub const STATUS_SUCCESS: &str = "success";
    pub const MESSAGE_SENT: &str = "Message sent";
    // ...
}
pub mod terminal {
    pub const MCP_AGENT_ID: &str = "MCP_AGENT";
    pub const DANGEROUS_COMMANDS: &[&str] = &["rm -rf /", "mkfs", ":(){ :|:& };:"];
    // ...
}
```

Usage: `use crate::constants::{env, errors};` then `env::DATABASE_URL`, `errors::SESSION_NOT_FOUND`.

Do NOT spread magic strings across handler files — a rename then requires a grep across the whole codebase.

---

## Rust — Declarative Validation with `garde`

Never write manual if-chain validation in handlers. Use `garde` to express constraints on the DTO struct itself.

```rust
// ❌ WRONG — manual if-chain in handler
if payload.name.is_empty() || payload.name.len() > 200 {
    return err(StatusCode::UNPROCESSABLE_ENTITY, "name: 1–200 chars").into_response();
}
if !payload.slug.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
    return err(StatusCode::UNPROCESSABLE_ENTITY, "slug: only a-z 0-9 - _").into_response();
}

// ✅ CORRECT — declare on the struct, validate in one line in the handler
#[derive(Deserialize, Validate)]
pub struct CreateProjectRequest {
    #[garde(length(min = 1, max = 200))]
    pub name: String,
    #[garde(length(min = 1, max = 100), pattern(r"^[a-zA-Z0-9_-]+$"))]
    pub slug: String,
    #[garde(inner(length(max = 2000)))]       // Option<String>
    pub description: Option<String>,
    #[garde(skip)]                             // no constraint
    pub color: Option<String>,
}
```

`Cargo.toml`: `garde = { version = "0.21", features = ["derive", "regex"] }` — the `regex` feature is required for `pattern()`.

The `validate()` helper in `handlers/mod.rs` bridges garde's error to the standard `ApiResponse<()>` format with HTTP 422:

```rust
pub fn validate<T: garde::Validate>(
    payload: &T,
) -> Result<(), (StatusCode, Json<ApiResponse<()>>)>
where
    T::Context: Default,
{
    payload.validate().map_err(|e| err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()))
}

// in any handler:
if let Err(rejection) = validate(&payload) {
    return rejection.into_response();
}
```

Note: garde 0.21 — `.validate()` takes **no arguments** (not `&()`). The `where T::Context: Default` bound is mandatory on the generic helper.

---

## Rust — Middleware with `tower` / `axum`

Never implement auth/logging/timeout logic inline inside handlers. Use `tower` middleware.

**Scoped middleware (auth on specific routes):**

```rust
// routes.rs
let mcp = Router::new()
    .route("/api/mcp", post(handle_mcp_request))
    .route_layer(middleware::from_fn(mcp_auth));   // applies only to /api/mcp
```

**Global middleware via `ServiceBuilder`:**

```rust
// main.rs — order matters: innermost layer wraps the app first
.layer(
    ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|e: BoxError| async move {
            // HandleErrorLayer MUST come before TimeoutLayer
            if e.is::<tower::timeout::error::Elapsed>() { StatusCode::REQUEST_TIMEOUT }
            else { StatusCode::INTERNAL_SERVER_ERROR }
        }))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(cors),
)
```

`HandleErrorLayer` **must** precede `TimeoutLayer` in the chain. `TimeoutLayer` returns `BoxError` on expiry; without `HandleErrorLayer`, the type system rejects the chain (`Infallible: From<BoxError>` is not satisfied).

`Cargo.toml` additions: `tower = { version = "0.5", features = ["timeout"] }` and `tower-http` features must include `"cors", "trace", "request-id", "util"`.

Constant-time comparison for tokens (avoid timing attacks):

```rust
fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
```

---

## No Magic Strings — Extract to Enums / Constants

Any string that has semantic meaning and appears (or could appear) in more than one place must be extracted to a typed constant. This applies to both Rust and TypeScript.

**Candidates:** socket event names, status values, provider names, session modes, HTTP paths, DB column values, error codes.

**Rust:**
```rust
// ❌ WRONG — scattered literals; a typo silently breaks the socket contract
socket.on("session:receive_message", ...);
io.emit("session:receive_message", ...);

// ✅ CORRECT — one module, one truth
// socket_events.rs
pub const SESSION_RECEIVE_MESSAGE: &str = "session:receive_message";

// usage
socket.on(socket_events::SESSION_RECEIVE_MESSAGE, ...);
io.emit(socket_events::SESSION_RECEIVE_MESSAGE, ...);
```

**TypeScript:**
```ts
// ❌ WRONG
socket.emit("terminal:input", { ... });
socket.on("terminal:input", handler);

// ✅ CORRECT — const object + derived type (compatible with erasableSyntaxOnly)
// packages/domain/src/socket-events.ts
export const SocketEvent = {
  TERMINAL_INPUT: "terminal:input",
} as const;
export type SocketEventName = (typeof SocketEvent)[keyof typeof SocketEvent];

// usage
socket.emit(SocketEvent.TERMINAL_INPUT, { ... });
socket.on(SocketEvent.TERMINAL_INPUT, handler);
```

**TypeScript enums vs const objects:** Never use `enum` — it is forbidden by `erasableSyntaxOnly`. Always use a `const` object with a companion type alias:

```ts
// ❌ WRONG — blocked by erasableSyntaxOnly
export enum SessionMode { Interactive = "interactive" }

// ✅ CORRECT
export const SessionMode = { Interactive: "interactive" } as const;
export type SessionMode = (typeof SessionMode)[keyof typeof SessionMode];
```

**Cross-boundary strings (Rust ↔ TypeScript):** Socket event names live in both sides. The Rust module (`socket_events.rs`) and the TS module (`packages/domain/src/socket-events.ts`) must stay in sync — same string values, same names. When adding a new event, update both files.

---

## Vue — i18n String Externalization

All user-visible strings must be in locale files (`src/i18n/locales/es.json`, `en.json`), never hardcoded in templates or scripts.

**In templates:** use `$t('key.path')` — no import needed.

**In `<script setup>`:** only import `useI18n()` when you actually call `t()` programmatically (e.g., inside `term.write()` or computed values). If you only use `$t()` in the template, do NOT add `useI18n()` — it causes TS6133 ("declared but never read").

```vue
<!-- ✅ Template-only: no useI18n() needed -->
<template>
  <span>{{ $t('session.noDescription') }}</span>
</template>
<script setup lang="ts">
// no useI18n() here
</script>

<!-- ✅ Script also uses t(): import is justified -->
<script setup lang="ts">
const { t } = useI18n()
term.write(`\r\n${t('terminal.messages.sent', { target })}\r\n`)
</script>
```

Interpolations use the named-argument syntax: `$t('key', { date: value })`. The locale file must declare the placeholder with the same name: `"key": "Last synced: {date}"`.

---

## Vite Build — Zero Warnings

`pnpm --filter web build` must produce zero warnings. Two common warning sources:

**Chunk size > 500 kB:** split vendor code with `manualChunks` in `vite.config.ts`, then set `chunkSizeWarningLimit` to cover remaining app-code size:

```ts
build: {
  chunkSizeWarningLimit: 1100,   // raise after vendor extraction; keeps the check meaningful
  rollupOptions: {
    output: {
      manualChunks(id) {
        if (id.includes('node_modules/@xterm')) return 'vendor-xterm';
        if (id.includes('node_modules/socket.io') || id.includes('node_modules/engine.io')) return 'vendor-socket';
        if (id.includes('node_modules/vue-i18n') || id.includes('node_modules/@intlify')) return 'vendor-i18n';
        if (id.includes('node_modules/vue') || id.includes('node_modules/pinia')) return 'vendor-vue';
      },
    },
  },
},
```

**TypeScript unused variable (TS6133):** delete the declaration entirely — do not prefix with `_`. If a variable is unused, it should not exist.

---

## TypeScript / Vue — No Dead Code

Functions defined in `<script setup>` must be called in the template or by other functions. Delete unused functions instead of leaving them.

---

## TypeScript / Vue — No `console.log`

No `console.log` in committed code. Use structured error handling or a logger.

---

## Design Tokens

**Never hardcode colors, sizes, or spacing.** The only source of truth is `apps/web/src/assets/main.css` (`@theme` block).

```vue
<!-- ❌ WRONG -->
<div style="background: #1e293b; padding: 16px;">

<!-- ❌ WRONG -->
<div class="bg-[#1e293b] p-[16px]">

<!-- ✅ CORRECT -->
<div class="bg-bg-sidebar p-4">
```

---

## Socket.io (Client)

Always configure reconnection on `io()`:

```ts
socket = io(SERVER_URL, {
  transports: ["websocket"],
  reconnection: true,
  reconnectionAttempts: Infinity,
  reconnectionDelay: 1000,
  reconnectionDelayMax: 10000,
});
socket.on("connect_error", () => { isConnected.value = false; });
```

---

## HTTP Error Handling

Handler errors must be propagated to callers. Never log an error and silently return success when the operation's outcome matters to the caller.

```rust
// ❌ WRONG — caller thinks it succeeded
Err(e) => { error!("[ORCH] Failed: {}", e); ok(()).into_response() }

// ✅ CORRECT — return the error
Err(e) => err_internal("orchestrator", e).into_response()
```

Exception: fire-and-forget operations (socket emissions to notify clients) may log and return success because the primary operation (DB write) already succeeded.
