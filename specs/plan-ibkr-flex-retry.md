# Redesign IBKR Flex Query retry logic (two-level scheme)

## Context

`IBKRClient::fetch_latest_report` (`src/ibkr_flex.rs`) currently retries the
**entire** `SendRequest` + `GetStatement` cycle up to `MAX_RETRIES = 3` times,
2 seconds apart. Each retry calls `SendRequest` again, which IBKR answers with
a **brand-new `ReferenceCode`** — i.e. a brand-new statement-generation job.
For accounts whose statements take a while to generate, this perpetually
restarts the job before it can finish: a self-defeating loop. It also never
even looks at `<ErrorCode>` in the `GetStatement` response — a "still
generating" reply is returned as `Ok(error_xml)` and only fails later, in
`parse_flex_report`.

The correct pattern: call `SendRequest` **once**, obtain a `ReferenceCode`,
then poll `GetStatement` **with that same code** until the statement is ready,
classifying error codes into "keep polling" vs "give up now".

## Experiment log (live test against the real account — continue here if restarting)

A throwaway script `poll_demo.py` (repo root, **not part of the build** — created
for this investigation, safe to delete once the redesign is verified) reads the
real token from `~/Library/Application Support/ibkr-porez/config.json` and
hits IBKR directly. **Make sure the GUI app is NOT running** while using it —
concurrent clients on the same token recreate the "two clients fighting over
one ReferenceCode" problem (this happened for real on 2026-06-08, PID 644).

Run with: `cd ~/projects/ibkr-porez-rs && python3 -u poll_demo.py` (the `-u`
flag is required — buffered stdout means a backgrounded run shows nothing
until exit).

**Run on 2026-06-08 ~15:22–15:24 (Belgrade time) — result:**
- Scenario 1 (`SendRequest`, retried up to 10× with 10 s pauses = 96 s budget):
  **all 10 attempts** returned `ErrorCode 1001: Statement could not be
  generated at this time. Please try again shortly.` The script exhausted its
  budget and crashed with `RuntimeError: SendRequest kept failing -- giving up`
  — Scenario 2 (poll-the-same-code) never got to run, because **no
  `ReferenceCode` was ever issued**.
- This is live, reproducible proof of the user's reported ~2-hour outage, and
  it happens **before** the stage our redesign targets: the account/token is
  currently unable to even start generating a statement.
- **Note**: `1001` appeared at the `SendRequest` stage, confirming it is a
  transient API error. However the decision is to not retry `SendRequest` —
  each hourly GUI attempt or CLI invocation makes one `SendRequest` call and
  lets the outer loop (GUI scheduler / user) handle the next try.

**Rerun on 2026-06-08 ~16:03–16:05 (Belgrade time) — identical result:**
- Confirmed the GUI app was not running first (no `ibkr-porez` process in `ps aux`).
- Scenario 1: all 10 `SendRequest` attempts (93.9 s elapsed) again returned
  `ErrorCode 1001: Statement could not be generated at this time. Please try
  again shortly.` — byte-for-byte the same message as the 2026-06-08 ~15:22 run.
  Script crashed the same way: `RuntimeError: SendRequest kept failing --
  giving up`.
- **Scenario 2 (poll-the-same-`ReferenceCode`) again never ran** — no
  `ReferenceCode` was ever issued, so there is still no answer to "on which
  retry of `poll_for_statement` does it succeed after a successful
  `send_statement_request`". The outage/limitation is still blocking us before
  that stage, ~45 minutes after the first observation — looks more like a
  standing account/token-level issue than a brief blip.
- No new information was gained.

**Rerun on 2026-06-08 ~16:11 (Belgrade time), right after a successful manual
FlexQuery in the IBKR web UI — same `1001`, aborted by the user:**
- The user logged into the IBKR web interface and ran the same Flex Query
  manually — it returned an XML file in under a second.
- Immediately afterwards, `poll_demo.py`'s `SendRequest` (the *API* endpoint,
  same token/query) **still** returned `ErrorCode 1001` on attempts #1–#4
  (~32 s in) before the user stopped the run.
- **Conclusion (user's, and clearly correct): this is an API-side problem,
  not a "statement not yet generated" or GUI-conflict issue.** The web UI's
  on-demand generation path and the Flex Web Service API path are evidently
  decoupled — a fast, successful manual run proves the account/query/data are
  fine, yet the API keeps refusing with "could not be generated at this time".
  Likely an IBKR-side API outage/throttle/bug specific to the Flex Web
  Service, separate from statement-generation timing. Re-running this
  experiment won't add information; **wait for IBKR's API to recover** (or for
  IBKR support to confirm/fix it) before trying again.

**Rerun on 2026-06-09 (Belgrade time) — API recovered, new information:**
- **Scenario 1**: `SendRequest #1` immediately returned `ReferenceCode
  5998932621`; `GetStatement` called ~3 s later → **`READY` on the first
  poll** (FlexQueryResponse, 141 692 bytes). The API is working again.
- **Scenario 2**: all 10 `SendRequest` attempts (93.6 s elapsed) returned
  `ErrorCode 1001` again — but this time the cause is **rate limiting**, not
  a global outage. Scenario 1 had just consumed the token's quota, and IBKR
  refused to issue a second `ReferenceCode` for the same token/query within
  ~100 s.
- **Key finding about `poll_for_statement` timing**: on this run the statement
  was already ready at the very first `GetStatement` call (3 s after
  `SendRequest`). The "in-progress → ready" transition we wanted to measure
  did not occur — generation was instantaneous. This is the best-case scenario
  for our design; the poll budget is confirmed to be more than sufficient.
- **Rate-limit implication**: `1001` at the `SendRequest` stage can be either
  a global outage or a token-level rate limit. `SendRequest` is not retried —
  the GUI's hourly schedule handles it.
- **`poll_for_statement` in-progress path still untested live**: we have not
  observed `GetStatement` returning "in progress" and then eventually returning
  the report. The unit test `ibkr_client_get_statement_exhausts_retries` /
  `ibkr_client_polls_same_reference_code` covers it synthetically.

**Rerun on 2026-06-09 (Belgrade time), ~100 s after the previous run — rate limit still active:**
- `poll_demo.py` now matches production: `SendRequest` once, no retry.
- `SendRequest` returned `ErrorCode 1001` immediately → script exited.
- Confirms the new script behaviour is correct: no wasted retries, result in
  under 1 second instead of 93+ seconds.
- `poll_for_statement` path still unreachable until rate limit clears.

## Error code classification for GetStatement retries

`poll_for_statement` classifies IBKR error codes into:
- **Fatal** — abort immediately, no retry: `1010` (legacy query), `1011`
  (inactive account), `1012` (token expired), `1013` (IP restriction), `1014`
  (invalid query), `1015` (invalid token), `1016` (invalid account), `1017`
  (invalid reference code), `1020` (invalid request).
- **Everything else** — retry with delay. A blacklist of fatal codes is the
  safer default: an unanticipated code falls through to "keep trying", bounded
  by the 5-attempt budget.

`send_statement_request` does not retry — on any error (IBKR or network) it
returns `Err` immediately. The GUI's hourly schedule is the outer retry loop.

## Level 1 — `src/ibkr_flex.rs` (shared by GUI and CLI — this is the actual fix)

### Remove
- `const MAX_RETRIES: u32 = 3;` (line 15)
- `const RETRY_DELAY: std::time::Duration = ...;` (line 16)
- `fn try_fetch_report` (lines 69–113) and the loop in `fetch_latest_report`
  (lines 51–67) that restarts it from scratch.

### Add constants (replace the two removed above)

```rust
/// Delay after a transient IBKR error (server busy / generation in progress /
/// unknown transient code).
const POLL_DELAY_BUSY: std::time::Duration = std::time::Duration::from_secs(5);

/// Delay after ErrorCode 1018 ("Too many requests from this token").
const POLL_DELAY_THROTTLED: std::time::Duration = std::time::Duration::from_secs(10);

/// Up to 5 polls, 5–10 s apart, ~25–50 s worst-case budget per attempt.
/// Riding out multi-hour outages is the GUI's hourly retry's job (Level 2).
const MAX_POLL_ATTEMPTS: u32 = 5;

/// IBKR error codes that mean "retrying will never help" (auth/config
/// problems). Everything else — including codes not in IBKR's published list,
/// like the `1001` observed live on 2026-06-08 — is treated as transient:
/// IBKR's own wording for the broad "try again shortly" family makes a
/// blacklist of permanent failures the safer default than a whitelist of
/// transient ones.
const FATAL_ERROR_CODES: &[&str] = &[
    "1010", // Legacy Flex Queries no longer supported
    "1011", // Service account is inactive
    "1012", // Token has expired
    "1013", // IP restriction
    "1014", // Query is invalid
    "1015", // Token is invalid
    "1016", // Account is invalid
    "1017", // Reference code is invalid
    "1020", // Invalid request or unable to validate request
];
```

### Replace `try_fetch_report` with two focused steps

```rust
/// `SendRequest` once — no retries.
fn send_statement_request(&self) -> Result<(String, String)> {
    let resp = self.http.get(&self.request_url)
        .query(&[("t", self.token.as_str()), ("q", self.query_id.as_str()), ("v", VERSION)])
        .send()
        .context("IBKR SendRequest failed")?;
    resp.error_for_status_ref().context("IBKR SendRequest HTTP error")?;
    let body = resp.text()?;
    let req_resp: XmlRequestResponse = quick_xml::de::from_str(&body)
        .context("Failed to parse IBKR SendRequest response")?;
    if let Some(code) = &req_resp.error_code {
        let msg = req_resp.error_message.as_deref().unwrap_or("Unknown");
        bail!("IBKR API Error {code}: {msg}");
    }
    let reference_code = req_resp.reference_code.context("No ReferenceCode in IBKR response")?;
    let base_url = req_resp.url.filter(|u| !u.is_empty())
        .unwrap_or_else(|| self.get_url.clone());
    Ok((reference_code, base_url))
}

/// Poll the same `ReferenceCode` up to `max_poll_attempts` times.
fn poll_for_statement(&self, reference_code: &str, base_url: &str) -> Result<String> {
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..self.max_poll_attempts {
        let body = self.http.get(base_url)
            .query(&[("q", reference_code), ("t", self.token.as_str()), ("v", VERSION)])
            .send()
            .context("IBKR GetStatement request failed")
            .and_then(|r| {
                r.error_for_status_ref().context("IBKR GetStatement HTTP error")?;
                Ok(r.text()?)
            });
        match body {
            Err(e) => {
                debug!(attempt, error = %e, "GetStatement failed, retrying");
                if attempt + 1 < self.max_poll_attempts {
                    std::thread::sleep(self.poll_delay_override.unwrap_or(POLL_DELAY_BUSY));
                }
                last_err = Some(e);
            }
            Ok(body) => {
                if body.contains("<ErrorCode>")
                    && let Ok(err_resp) = quick_xml::de::from_str::<XmlErrorResponse>(&body)
                    && let Some(code) = &err_resp.error_code
                {
                    let msg = err_resp.error_message.as_deref().unwrap_or("Unknown");
                    let e = anyhow::anyhow!("IBKR API Error {code}: {msg}");
                    if FATAL_ERROR_CODES.contains(&code.as_str()) {
                        return Err(e);
                    }
                    let delay = if code == "1018" { POLL_DELAY_THROTTLED } else { POLL_DELAY_BUSY };
                    debug!(attempt, error = %e, "GetStatement not ready, retrying");
                    if attempt + 1 < self.max_poll_attempts {
                        std::thread::sleep(self.poll_delay_override.unwrap_or(delay));
                    }
                    last_err = Some(e);
                    continue;
                }
                return Ok(body);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        anyhow::anyhow!("GetStatement: not ready after {} attempts", self.max_poll_attempts)
    }))
}
```

```rust
pub fn fetch_latest_report(&self) -> Result<String> {
    let (reference_code, base_url) = self.send_statement_request()?;
    self.poll_for_statement(&reference_code, &base_url)
}
```

### Make poll timing overridable for tests

Add two fields to `IBKRClient` (struct at line 18) and set them in both
constructors (`new` / `with_base_url`) to the production constants:

```rust
pub struct IBKRClient {
    token: String,
    query_id: String,
    request_url: String,
    get_url: String,
    http: reqwest::blocking::Client,
    /// `None` in production — delay is chosen per error code. Set in tests to avoid real sleeps.
    /// Set in tests to avoid real sleeps.
    poll_delay_override: Option<std::time::Duration>,
    max_poll_attempts: u32,
}
```
```rust
// in new() / with_base_url():
poll_delay_override: None,
max_poll_attempts: MAX_POLL_ATTEMPTS,
```

Add a `#[cfg(test)]`-only builder so tests run in milliseconds:

```rust
#[cfg(test)]
fn with_poll_params(mut self, delay_override: std::time::Duration, max_attempts: u32) -> Self {
    self.poll_delay_override = Some(delay_override);
    self.max_poll_attempts = max_attempts;
    self
}
```

## Level 2 — `src/gui/app.rs`: flatten the auto-sync backoff to plain hourly retries

Per the user's correction: the GUI no longer needs a 5/10/20/30/60-minute
ramp-up — once `fetch_latest_report` itself polls correctly for several
minutes, a quick retry 5 minutes later adds little. Just retry once an hour.

### Replace the backoff table + helper (lines 21–25)

```rust
// before:
const AUTO_SYNC_BACKOFF_MINUTES: &[i64] = &[5, 10, 20, 30, 60];
fn auto_sync_backoff(idx: usize) -> chrono::Duration {
    let minutes = AUTO_SYNC_BACKOFF_MINUTES[idx.min(AUTO_SYNC_BACKOFF_MINUTES.len() - 1)];
    chrono::Duration::minutes(minutes)
}

// after:
const AUTO_SYNC_RETRY_INTERVAL: chrono::Duration = chrono::Duration::hours(1);
```

### Remove `auto_sync_backoff_idx` entirely

- Field declaration, `pub auto_sync_backoff_idx: usize,` (line 142)
- Initializer in `App::new`, `auto_sync_backoff_idx: 0,` (line 199)
- Reset on success in `handle_sync_done`, `self.auto_sync_backoff_idx = 0;` (line 486)
- Reset in `maybe_auto_sync` when `synced_today`, `self.auto_sync_backoff_idx = 0;` (line 524)

### Simplify the failure branch of `handle_sync_done` (lines 506–508)

```rust
// before:
let idx = self.auto_sync_backoff_idx;
self.next_auto_sync = Some(now + auto_sync_backoff(idx));
self.auto_sync_backoff_idx = (idx + 1).min(AUTO_SYNC_BACKOFF_MINUTES.len() - 1);

// after:
self.next_auto_sync = Some(now + AUTO_SYNC_RETRY_INTERVAL);
```

## Spec update — `specs/spec-auto-sync.md`

Replace the "Retry schedule" section (lines 29–33):

```
## Retry schedule

On failure, the next attempt is scheduled with backoff: 5, 10, 20, 30, then 60
minutes, repeating at 60-minute intervals until the sync succeeds or a new
local day begins — at which point the schedule restarts from the beginning.
```

with:

```
## Retry schedule

On failure, the next attempt is scheduled exactly one hour later, repeating
hourly until the sync succeeds or a new local day begins.
```

## Test updates

### `tests/test_gui.rs`

- Fixture `app_with_decls` (around line 105–106): remove the
  `auto_sync_backoff_idx: 0,` initializer (field no longer exists).
- Test around lines 664–665 (`sync failure schedules a retry` or similar):
  replace
  ```rust
  assert!(app.next_auto_sync.is_some());
  assert_eq!(app.auto_sync_backoff_idx, 1);
  ```
  with an assertion that `next_auto_sync` is set to (approximately) `now +
  AUTO_SYNC_RETRY_INTERVAL` — e.g. capture `now` right before
  `app.poll_background()` and assert
  `app.next_auto_sync.unwrap() - now == chrono::Duration::hours(1)` (or use a
  tolerance window if exact equality is too brittle given clock reads).

### `src/ibkr_flex.rs` — HTTP-level tests (implemented)

- **`ibkr_client_fetch_success`** — happy path: one `SendRequest`, one `GetStatement`, both succeed.
- **`ibkr_client_send_request_error_fails_immediately`** — any IBKR error on `SendRequest` → `.expect(1)`, no retry.
- **`ibkr_client_get_statement_fatal_error_aborts_immediately`** — fatal code on `GetStatement` → `.expect(1)`, no retry.
- **`ibkr_client_get_statement_exhausts_retries`** — transient `1019` on every `GetStatement` → `with_poll_params(ZERO, 3)`, mock `.expect(3)`, error contains "1019".
- **`ibkr_client_polls_same_reference_code`** — `SendRequest` returns `REF123`; `GetStatement` mock uses regex `q=REF123` to verify the same code is reused.
- **`ibkr_client_http_error_retries`** — HTTP 500 on `GetStatement` is retried; `SendRequest` hit exactly once (`mock_send_request_success`).
- **`ibkr_client_uses_default_get_url_when_response_url_empty`** — empty `<Url>` falls back to `get_url`.

## Verification

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test ibkr_flex::
cargo test --test test_gui
```

Then manually: run the GUI (`cargo run --features gui --bin ibkr-porez-gui`),
trigger "Sync now" while watching `tracing::debug!` output (set
`RUST_LOG=ibkr_porez=debug`) to confirm a single `SendRequest` is followed by
repeated `GetStatement` polls carrying the same `ReferenceCode`.

## Cleanup

Delete `poll_demo.py` from the repo root once the redesign is implemented and
verified — it was a throwaway investigation script, not part of the product
(reads the user's live token from the local config and hits production IBKR
endpoints directly; should not ship or be committed long-term).
