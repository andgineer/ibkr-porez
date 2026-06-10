# Plan: Single persistent background-sync scheduler

## Goal

Get a sync that checks for new transactions once a day, retries sensibly on
failure, and is simple, reliable, and easy on battery. There is no
minimal-diff constraint — prefer whatever is simplest and most robust, even
if it touches more lines than a narrow patch would.

Replace the current per-event one-shot scheduling (`schedule_auto_sync_after`,
spawned anew on every success/failure/config-save — each with its own sleep
duration) with a single persistent scheduler thread that lives for the whole
app lifetime and ticks hourly. The UI thread (`poll_background`) keeps
deciding *what* to do on each tick; the scheduler only provides the
heartbeat. One thread, one wakeup per hour, one repaint per hour — that's the
floor for "battery impact" and is unavoidable given the hourly-retry
requirement for transient errors.

The fatal/transient distinction in `classify_sync_error` must be preserved
and made more robust, not removed:

- Transient IBKR errors (statement not ready yet, network hiccups) are
  retried every hour by the persistent tick — that's the actual point of the
  hourly heartbeat.
- Errors that indicate a broken configuration (expired/invalid token, invalid
  query ID) will fail again and again with an unchanged token. Retrying them
  hourly spawns a background thread that does real network I/O (IBKR + NBS
  calls) 24 times a day for a guaranteed failure — that's the part worth
  avoiding for battery/resource reasons. These should be retried at most once
  per day; no other automation is needed — if the user fixes the
  configuration, they are expected to click "Sync now" themselves (which
  always runs immediately, regardless of any fatal-error gating).

## Design agreed

- One thread, spawned lazily on first `render()` call (so it captures the
  real `egui::Context`, not `egui::Context::default()`), guarded by a new
  `scheduler_started: bool` field.
- Body: `loop { sleep(1h); send(()) to auto_sync_tx; ctx.request_repaint(); }`
  — no shared mutable state between threads.
- `poll_background` drains all pending `()` signals and decides per signal:
  `synced_today` → no-op; invalid config → banner only; valid config,
  `!bg_busy`, and not `already_failed_fatally_today` → `start_sync(false)`;
  otherwise no-op (next hourly tick re-evaluates).
- `already_failed_fatally_today` = `self.last_sync_fatal` is true **and**
  `self.last_sync_issue` is dated today. `last_sync_fatal` is set by
  `handle_sync_done`: `true` for a fatal error, `false` on success or a
  transient error. It is **not persisted** — an app restart on the same day
  after a fatal error causes one extra attempt, which is an acceptable,
  rare cost (restarting is itself a "let's see if it works now" signal from
  the user).
- No config-save automation: saving a new configuration does not bypass
  `already_failed_fatally_today` on its own. After fixing the configuration,
  the user clicks "Sync now" (`start_sync`, called directly from
  `main_window.rs`), which is unaffected by this gating since it doesn't go
  through `poll_background`.
- Net effect: transient errors retry hourly; fatal errors retry at most once
  a day (self-healing across midnight even if the app stays open) — without
  any extra threads, channels, or signal types.
- `classify_sync_error` keeps its `(String, bool)` signature; the `bool`
  now means "retry automatically every hour" (transient) vs. "retry at most
  once a day" (fatal).

## Status-line wording

Two small wording/UI tweaks address a real confusion case: the user fixes a
fatal error (e.g. an expired token) in Config and saves, but
`last_sync_issue` keeps showing the *old* attempt's message until the next
sync — and a stale "Token has expired" could read as "the token I just
entered is wrong".

- **Fatal messages get an explicit retry hint.** `classify_sync_error`'s
  fatal branch appends `" — won't retry automatically; click \"Sync now\" to
  try again."` to the raw error. This makes the message self-explanatory
  regardless of how stale it is: it always describes a past attempt that
  won't repeat on its own, with a clear next action — without guessing
  *what* needs fixing (we don't know that the error is config-related at
  all in the generic case, so we don't tell the user to "fix" anything).
- **The "Not configured" banner becomes a clickable link** to the Config
  dialog, mirroring the existing `show_import_hint` pattern. This makes the
  most important first-run action (set up the IBKR token) immediately
  actionable and visually distinct from a plain warning message.

Both are independent of the scheduler change above and could in principle
ship separately, but are bundled here since they were found while designing
this plan and touch the same files.

## Steps

### 1. `src/gui/app.rs` — new `App` fields

In the `App` struct, add `last_sync_fatal` next to `last_sync_issue`
(around line 133-134; private — only `handle_sync_done` and
`poll_background` touch it):

```rust
    pub last_sync_issue: Option<(chrono::NaiveDateTime, String)>,
    last_sync_fatal: bool,
    pub pending_new_declarations: u32,
```

And add `scheduler_started` next to `auto_sync_rx` (around line 135-137,
private — only `render()` touches it):

```rust
    ctx: egui::Context,
    auto_sync_tx: mpsc::Sender<()>,
    auto_sync_rx: mpsc::Receiver<()>,
    scheduler_started: bool,
```

### 2. `App::new()` — unconditional initial check + init new fields

Lines 177-180 currently:

```rust
        let (auto_sync_tx, auto_sync_rx) = mpsc::channel::<()>();
        if last_sync_success.is_none_or(|dt| dt.date() != chrono::Local::now().date_naive()) {
            let _ = auto_sync_tx.send(());
        }
```

Replace with an unconditional send (the persistent thread is now the only
long-term heartbeat, so priming the channel at startup is always safe —
`poll_background` is a no-op if already synced today):

```rust
        let (auto_sync_tx, auto_sync_rx) = mpsc::channel::<()>();
        let _ = auto_sync_tx.send(());
```

In the struct literal, add `last_sync_fatal: false,` after `last_sync_issue,`
(around line 197):

```rust
            last_sync_success,
            last_sync_issue,
            last_sync_fatal: false,
            pending_new_declarations,
```

And add `scheduler_started: false,` after `auto_sync_rx,` (around line 201):

```rust
            auto_sync_rx,
            scheduler_started: false,
```

### 3. `new_for_test()` — init new fields

Add `last_sync_fatal: false,` after `last_sync_issue: None,` (around line 626):

```rust
            last_sync_success: None,
            last_sync_issue: None,
            last_sync_fatal: false,
            pending_new_declarations: 0,
```

And add `scheduler_started: true,` after `auto_sync_rx,` (around line 642):

```rust
            auto_sync_tx,
            auto_sync_rx,
            scheduler_started: true,
```

`tests/test_e2e_gui.rs` *does* call `render()` (via the `egui_kittest`
harness, see `harness_for`). `scheduler_started: true` is what stops the
persistent thread from being spawned there — without it, every e2e test
would spawn a thread sleeping for an hour. This mirrors the existing
`app.last_sync_success = Some(now)` workaround in `setup_app`, which exists
for the same reason (to keep `poll_background` quiet on the first render).

### 4. `render()` — lazily spawn the persistent scheduler thread

Lines 751-753 currently:

```rust
    pub fn render(&mut self, ctx: &egui::Context) {
        self.ctx = ctx.clone();
        self.poll_background();
```

Insert the lazy spawn between the two lines:

```rust
    pub fn render(&mut self, ctx: &egui::Context) {
        self.ctx = ctx.clone();

        if !self.scheduler_started {
            self.scheduler_started = true;
            let tx = self.auto_sync_tx.clone();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_hours(1));
                    let _ = tx.send(());
                    ctx.request_repaint();
                }
            });
        }

        self.poll_background();
```

### 5. `poll_background()` — rewrite the `auto_sync_rx` handling

Lines 449-471 currently:

```rust
    pub fn poll_background(&mut self) {
        if let Ok(()) = self.auto_sync_rx.try_recv() {
            let now = chrono::Local::now().naive_local();
            let synced_today = self
                .last_sync_success
                .is_some_and(|dt| dt.date() == now.date());
            if synced_today {
                let tomorrow = now.date().succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap();
                let until_midnight = (tomorrow - now)
                    .to_std()
                    .unwrap_or(std::time::Duration::from_hours(1));
                self.schedule_auto_sync_after(until_midnight);
            } else if app_config::validate_config(&self.config).is_empty() {
                if self.bg_busy {
                    self.schedule_auto_sync_after(std::time::Duration::from_mins(1));
                } else {
                    self.start_sync(false);
                }
            } else {
                self.warning_banner =
                    Some("Not configured \u{2014} open Config to set up IBKR token".to_string());
            }
        }
```

Replace with (drain all queued signals with `while`; the persistent thread
provides the next tick regardless, so there's nothing left to schedule):

```rust
    pub fn poll_background(&mut self) {
        while let Ok(()) = self.auto_sync_rx.try_recv() {
            let now = chrono::Local::now().naive_local();
            let synced_today = self
                .last_sync_success
                .is_some_and(|dt| dt.date() == now.date());
            if synced_today {
                continue;
            }
            if app_config::validate_config(&self.config).is_empty() {
                let already_failed_fatally_today = self.last_sync_fatal
                    && self
                        .last_sync_issue
                        .as_ref()
                        .is_some_and(|(dt, _)| dt.date() == now.date());
                if !self.bg_busy && !already_failed_fatally_today {
                    self.start_sync(false);
                }
            } else {
                self.warning_banner =
                    Some("Not configured \u{2014} open Config to set up IBKR token".to_string());
            }
        }
```

The rest of `poll_background` (bg_receiver / export_channel handling,
lines 473-536) stays unchanged.

### 6. `handle_sync_done()` — drop scheduling, track fatal state

**Success branch** (lines 541-566) — drop the `until_midnight` /
`schedule_auto_sync_after` block and clear `last_sync_fatal` (a successful
sync proves the current configuration works, so any earlier fatal block no
longer applies):

```rust
            Ok(r) => {
                let _ = self.storage.set_last_sync_success(now);
                self.last_sync_success = Some(now);
                self.last_sync_fatal = false;
                if let Some(msg) = &r.income_error {
                    let _ = self.storage.set_last_sync_issue(now, msg);
                    self.last_sync_issue = Some((now, msg.clone()));
                } else {
                    let _ = self.storage.clear_last_sync_issue();
                    self.last_sync_issue = None;
                }
                self.warning_banner = check_holiday_warning(&self.config);

                let count = r.created_declarations.len();
                if count > 0 {
                    let count_u32 = u32::try_from(count).unwrap_or(u32::MAX);
                    let _ = self.storage.add_pending_new_declarations(count_u32);
                    self.pending_new_declarations = self.storage.get_pending_new_declarations();
                    notify_new_declarations(count);
                }
            }
```

**Error branch** (lines 567-574) currently:

```rust
            Err(e) => {
                let (display_message, should_retry) = classify_sync_error(&e);
                let _ = self.storage.set_last_sync_issue(now, &display_message);
                self.last_sync_issue = Some((now, display_message));
                if should_retry {
                    self.schedule_auto_sync_after(std::time::Duration::from_hours(1));
                }
            }
```

becomes (no scheduling; record whether this was a fatal error so
`poll_background` can suppress same-day hourly retries):

```rust
            Err(e) => {
                let (display_message, should_retry) = classify_sync_error(&e);
                let _ = self.storage.set_last_sync_issue(now, &display_message);
                self.last_sync_issue = Some((now, display_message));
                self.last_sync_fatal = !should_retry;
            }
```

### 7. `classify_sync_error()` — add retry hint to fatal messages

Keeps its `(String, bool)` signature (lines 579-601); the `bool` now means
"retry automatically every hour" (transient) vs. "retry at most once a day"
(fatal).

The fatal (`else`) branch (lines 598-600) currently:

```rust
    } else {
        (e.to_string(), false)
    }
```

becomes (see "Status-line wording" above for rationale):

```rust
    } else {
        (
            format!("{e} \u{2014} won't retry automatically; click \"Sync now\" to try again."),
            false,
        )
    }
```

### 8. Remove `schedule_auto_sync_after` entirely

Delete the function at lines 646-654:

```rust
    fn schedule_auto_sync_after(&self, delay: std::time::Duration) {
        let tx = self.auto_sync_tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            let _ = tx.send(());
            ctx.request_repaint();
        });
    }

```

`trigger_sync_check()` (lines 656-659) stays exactly as-is — it remains a
test-only helper (used in `tests/test_gui.rs` to simulate the persistent
hourly tick).

### 9. `src/gui/config_dialog.rs` — drop the auto-sync trigger on config save

No signal should be sent on config save: the user either clicks "Sync now"
themselves, or the next persistent hourly tick picks up the corrected
configuration automatically (within an hour) — same as for any other
fatal/invalid-config recovery. Remove the call at line 73:

```rust
        if let Err(e) = app_config::save_config(&cfg) {
            app.error_dialog = Some(e.to_string());
        } else {
            app.config = cfg;
            app.refresh_declarations();
            app.trigger_sync_check();
        }
```

becomes:

```rust
        if let Err(e) = app_config::save_config(&cfg) {
            app.error_dialog = Some(e.to_string());
        } else {
            app.config = cfg;
            app.refresh_declarations();
        }
```

### 10. `src/gui/main_window.rs` — make the "Not configured" banner clickable

Lines 9-20 currently:

```rust
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    toolbar(ui, app);

    if let Some(ref banner) = app.warning_banner {
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.colored_label(ui.visuals().warn_fg_color, banner);
            });
        ui.add_space(4.0);
    }
```

Replace with (special-case the "Not configured" banner — set in
`poll_background`, step 5 — to render as text + clickable link, mirroring
the existing `show_import_hint` pattern at lines 55-72):

```rust
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    toolbar(ui, app);

    if let Some(ref banner) = app.warning_banner {
        let is_config_banner = banner.starts_with("Not configured");
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(4.0)
            .show(ui, |ui| {
                let warn = ui.visuals().warn_fg_color;
                if is_config_banner {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.colored_label(warn, "Not configured \u{2014} click ");
                        if ui.link("Config").clicked() {
                            app.config_dialog =
                                Some(super::config_dialog::ConfigDialog::new(&app.config));
                        }
                        ui.colored_label(
                            warn,
                            " to set up your IBKR token and other required fields.",
                        );
                    });
                } else {
                    ui.colored_label(warn, banner);
                }
            });
        ui.add_space(4.0);
    }
```

The underlying `warning_banner` string (`"Not configured \u{2014} open
Config to set up IBKR token"`, set in `poll_background` and checked via
`starts_with("Not configured")` in `start_sync`, app.rs:386-391) is
unchanged — only the rendering for this specific banner is replaced.

### 11. Tests in `tests/test_gui.rs` and `tests/test_e2e_gui.rs` — verify, adjust, add

Walk through each existing test that touches this area:

- `poll_sync_error_ibkr_transient_shows_friendly_message` (line 973),
  `poll_sync_error_network_shows_friendly_message` (993),
  `classify_does_not_match_raw_number_in_unrelated_message` (1094): all only
  inspect `app.last_sync_issue` message text for transient/unrelated cases,
  which is unchanged — pass as-is.
- `poll_sync_error_fatal_shows_raw_message` (1014): the exact-match assertion
  must be updated for the new suffix from step 7:

  ```rust
  assert_eq!(
      msg,
      "IBKR API Error 1012: Token has expired \u{2014} won't retry automatically; click \"Sync now\" to try again."
  );
  ```
- `poll_background_sets_banner_when_config_invalid` (1033): calls
  `trigger_sync_check()` then `poll_background()` with invalid config →
  still sets the "Not configured" banner via the rewritten branch — passes.
- `start_sync_clears_config_banner_when_config_valid` (1049): unaffected,
  doesn't touch `auto_sync_rx`.
- `poll_sync_success_schedules_midnight_check` (1066): after a successful
  sync, calls `trigger_sync_check()` + `poll_background()` again and asserts
  `!app.bg_busy`. With the rewrite, `synced_today` is true on the second
  call → `continue` → no `start_sync` → `bg_busy` stays `false`. Test still
  passes unchanged. Optional: rename to
  `poll_sync_success_then_recheck_does_not_resync_same_day` and update the
  inline comment at lines 1080-1082 — not required for the test to pass.
- `poll_sync_success_after_error_clears_visible_issue` (1117),
  `stale_issue_has_earlier_timestamp_than_success` (1150): unaffected.

Add a new test covering the fatal-error gating, near
`poll_sync_error_fatal_shows_raw_message`:

```rust
#[test]
fn poll_background_skips_retry_after_fatal_error_same_day() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;

    tx.send(BackgroundResult::SyncDone(Err(
        "IBKR API Error 1012: Token has expired".into(),
    )))
    .unwrap();
    app.poll_background();
    assert!(!app.bg_busy);

    // Simulates the persistent hourly tick.
    app.trigger_sync_check();
    app.poll_background();

    assert!(
        !app.bg_busy,
        "fatal error from earlier today should not be retried automatically"
    );
}

#[test]
fn poll_background_resumes_hourly_retry_after_transient_error_following_fatal() {
    let (mut app, _tmp) = app_with_decls(Vec::new());

    // First attempt today: fatal error (e.g. expired token).
    let (tx1, rx1) = mpsc::channel();
    app.bg_receiver = Some(rx1);
    app.bg_busy = true;
    tx1.send(BackgroundResult::SyncDone(Err(
        "IBKR API Error 1012: Token has expired".into(),
    )))
    .unwrap();
    app.poll_background();

    // User fixes the config and clicks "Sync now"; this attempt hits a
    // transient error instead.
    let (tx2, rx2) = mpsc::channel();
    app.bg_receiver = Some(rx2);
    app.bg_busy = true;
    tx2.send(BackgroundResult::SyncDone(Err(
        "IBKR API Error 1001: Statement not ready".into(),
    )))
    .unwrap();
    app.poll_background();
    assert!(!app.bg_busy);

    // The next hourly tick should retry, since the latest error was
    // transient — the earlier fatal classification must not stick.
    app.trigger_sync_check();
    app.poll_background();

    assert!(
        app.bg_busy,
        "transient error after a fixed fatal error should resume hourly retries"
    );
}
```

In `tests/test_e2e_gui.rs`, add a test for the new clickable banner from
step 10, near `config_button_opens_dialog` (line 175):

```rust
#[test]
fn not_configured_banner_link_opens_config_dialog() {
    let (mut app, _tmp) = setup_app(vec![], vec![]);
    app.warning_banner =
        Some("Not configured \u{2014} open Config to set up IBKR token".into());
    let mut harness = harness_for(app);
    harness.run();

    assert!(harness.state().config_dialog.is_none());

    // Exact match: the toolbar button is labeled "\u{2699} Config", the
    // banner link is plain "Config" — `get_by_label` (exact) picks the link.
    harness.get_by_label("Config").click();
    harness.run();

    assert!(
        harness.state().config_dialog.is_some(),
        "clicking the Config link in the banner should open the config dialog"
    );
}
```

Run `cargo test` after the change to confirm; fix any unexpected failures
before moving on.

### 12. `specs/spec-auto-sync.md` — update behavior description

**"Retry schedule" section** — current:

```markdown
## Retry schedule

On failure, the next attempt is scheduled one hour later, repeating hourly
until the sync succeeds.
```

New:

```markdown
## Retry schedule

A single persistent background timer ticks hourly while there is no
successful sync recorded for the current day.

- Transient errors (Flex Query not ready yet, network issues) are retried on
  every tick until they succeed.
- Errors that indicate a configuration problem (expired/invalid token,
  invalid query ID, and similar) are not retried hourly, since repeating the
  same request would just fail again. Such an error is retried at most once
  a day (the cycle self-heals automatically across midnight, even if the app
  stays open). The user can also force an immediate retry at any time with
  "Sync now".
```

**"Manual vs. automatic sync" section** — current closing paragraph:

```markdown
Transient IBKR errors (the "statement generation in progress" family) and
network connectivity errors are shown with friendly wording rather than the
raw error text, since the app retries them automatically without the user
needing to do anything. Fatal errors (invalid token, expired token, invalid
query ID, and similar configuration problems) are shown as-is and are not
retried — they require the user to take action.
```

New:

```markdown
Transient IBKR errors (the "statement generation in progress" family) and
network connectivity errors are shown with friendly wording rather than the
raw error text, since the app retries them automatically every hour without
the user needing to do anything. Errors that indicate a configuration problem
(invalid token, expired token, invalid query ID, and similar) are shown with
their original IBKR error text plus a note that automatic retries are paused
and the user should use "Sync now" to try again. This phrasing also holds up
once the user has since changed the configuration: the message still
correctly describes a past attempt and the action needed, rather than
appearing to comment on the just-saved configuration. Such errors are not
retried hourly with the same configuration — the app picks them up again
automatically at most once a day, or immediately via "Sync now".
```

**"Configuration gate" section** — current:

```markdown
## Configuration gate

If the IBKR configuration is incomplete or invalid, the auto-cycle does not
run. A permanent status-line banner informs the user that configuration is
required. As soon as the user saves a valid configuration, the auto-cycle
starts immediately — no manual "Sync now" needed. Triggering a sync manually
with an invalid configuration still shows the configuration-validation error
as before.
```

New:

```markdown
## Configuration gate

If the IBKR configuration is incomplete or invalid, the auto-cycle does not
run. A permanent status-line banner informs the user that configuration is
required. Saving a valid configuration does not by itself trigger a sync —
the next hourly tick picks it up automatically (within an hour), or the user
can use "Sync now" for an immediate attempt. Triggering a sync manually with
an invalid configuration still shows the configuration-validation error as
before.
```

## Verification

1. `cargo fmt`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test`

Do not commit — leave changes staged/unstaged for review.
