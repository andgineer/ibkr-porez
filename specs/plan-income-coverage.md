# Plan: deadline-driven income declaration generation

## Problem

Income declarations are generated for the window `[last_declaration_date + 1,
yesterday]` (`determine_income_period`, src/sync.rs:218) and the watermark is
then advanced to yesterday (src/sync.rs:175-181). The advance happens on every
run that does not return an error, regardless of whether IBKR had actually
booked everything for those dates. Any income transaction that lands in storage
after the watermark has passed its date is silently and permanently skipped.

The paths that lose data today:

- **late Flex data on a successful fetch** — dividends and their withholding
  tax appear in the Flex report with a 1+ business-day lag; the watermark has
  already moved past their date;
- **failed fetch** — `run_sync` deliberately still generates from stored
  transactions (src/sync.rs:53-70) and the watermark advances anyway;
- **missing NBS rate** — the error is collected into `income_error`
  (src/sync.rs:163-165) and generation continues, so the watermark advances
  over a period that produced nothing.

The zero-withholding-tax path does *not* lose data (`build_income_reports`
bails, src/report_income.rs:181, and `generate_declarations` returns `Err`
before reaching the watermark advance), but it has its own failure mode: a
single group without withholding tax fails the whole sync — every sync, forever,
for securities that legitimately never withhold (Irish-domiciled ETFs), which
also blocks every other income group in the window.

A third loss exists independently of the watermark: withholding tax booked more
than 7 days after the income date is never matched (src/report_income.rs:237)
and its foreign-tax credit is forfeited. Today that is masked — the group bails
instead of being declared — but it becomes a real, permanent loss as soon as
zero-WHT groups start being finalized.
`test_wht_not_found_beyond_7_day_window` (tests/test_reports.rs:616) documents
exactly this case with a year-end dividend whose tax arrives 9 days later.

## Design

### The rule

The PP-OPO filing deadline is 30 days from the income date. That single fact,
not a stored watermark, decides everything:

- a group **inside its deadline** is the app's job → generate;
- a group **past its deadline** with no declaration → report it, never
  auto-generate;
- a group inside its deadline with no withholding tax yet → hold it for a
  bounded wait, then declare without a foreign-tax credit.

Nothing about this rule is specific to the upgrade. The same behaviour covers a
vacation, a long outage, a restore from backup, and the import of an old Flex
file — cases a migration-time pin would each have to handle separately.

### Three passes

Every sync, after import, income handling is three passes over stored
transactions, in this order:

1. **Claims.** Read PP-OPO declarations once
   (`storage.get_declarations(None, Some(&DeclarationType::Ppo))`) and build
   two sets: declared group keys, and withholding-tax transaction ids already
   credited by some declaration.
2. **Generate.** Only groups dated `>= end_period - (PPOPO_DEADLINE_DAYS - 1)`.
   Resolve rates, match unclaimed withholding tax, decide
   declare / wait / skip-with-reason. **This is the only pass that touches the
   network, and it is bounded to 30 days of history** no matter how long the
   app has been in use or how long it was not run.
3. **Report.** Everything else, from raw transactions only — no rates, no
   network:
   - undeclared group older than the deadline → `PastDeadline` notice;
   - unclaimed withholding tax whose owning income group is already declared →
     `LateWht` notice.

### Why not a coverage watermark

An earlier draft of this plan pinned a coverage start (`income_coverage_start`)
and rescanned `[pin, yesterday]` every sync, floored at a rolling maximum. It
was dropped because the pin is state whose only job is to defend a scope
decision, and it dragged in a migration from `last_declaration_date`, a
"the pin never moves" invariant with its own tests, an interaction with
`--lookback`, a rolling floor constant, and a 400-day-wide network scan in which
a group with a permanently unresolvable rate would re-walk `nbs.get_rate`'s
10-day HTTP lookback (src/nbs.rs:80-127, no negative caching) on every sync for
over a year. The deadline rule needs no stored boundary and bounds the
network pass to 30 days.

### Why claims instead of a matching-window cap

`find_withholding_tax` sums every candidate in range
(src/report_income.rs:239-277), so two payments of one symbol closer together
than the matching window would each claim the other's tax. Capping the window at
the day before the next same-key payment fixes that in one direction and breaks
it in the other: with payments 3 days apart and the first payment's tax booked
on day 5, the first group is capped to `[d, d+2]` and loses its own credit while
the second claims it.

Instead, each issued declaration records the ids of the withholding-tax
transactions it credited, and only *unclaimed* taxes are candidates. Attribution
is one universal rule: **a withholding tax belongs to the nearest preceding
income of the same key.** This removes the cap constant, makes double-crediting
across two declarations impossible by construction, and makes `LateWht`
detection a pure set operation with no rates, no second horizon and no amount
comparison.

## Constants

```rust
const PPOPO_DEADLINE_DAYS: i64 = 30;   // src/sync.rs — statutory filing deadline
const WHT_WAIT_DAYS: i64 = 20;         // src/report_income.rs — hold a zero-tax group this long
```

The matching window has **no constant of its own**: it is
`[income date, min(today, income date + WHT_WAIT_DAYS)]`. The moment a
declaration is issued *is* the end of its matching window, so the two cannot
drift apart — they are one number, not two kept in sync by a comment.

`WHT_WAIT_DAYS` must stay comfortably below `PPOPO_DEADLINE_DAYS`; 20 leaves 10
days of margin. It replaces both hardcoded `Duration::days(7)` occurrences
(src/report_income.rs:115 pool bound, :237 matching window). The value goes from
7 to 20 **because** zero-WHT groups are now finalized instead of bailing: at 7
days the year-end pattern already in the test suite (dividend 2025-12-24, its US
tax 2026-01-02) silently forfeits the credit, which was harmless while the group
bailed and is a permanent loss once it is declared.

A group is declared **as soon as its matched tax is non-zero**, not at the end
of the wait — this keeps today's latency. A later tranche of tax for an
already-declared group surfaces as `LateWht`.

## Changes

### 1. models.rs / storage.rs

`DeclarationsFile` (src/models.rs:354) gains two fields, both
`#[serde(default)]`:

```rust
pub income_notices: Vec<IncomeNotice>,          // cache, rewritten whole each sync
pub income_overdue_dismissed_before: Option<String>,   // "%Y-%m-%d"
```

`income_notices` is a **cache of derived data**, not a source of truth: it is
recomputed from transactions and declarations on every sync and rewritten
wholesale, so the worst it can be is stale until the next sync. It exists so the
GUI can render the banner on startup, before any sync has run in that session.

`last_declaration_date` stays in the struct and keeps its accessors, but nothing
in `sync.rs` reads or writes it any more (verified: the only non-test readers
are src/sync.rs:175, :228). Freezing it keeps the file loadable by older builds.

New storage accessors next to `get_pending_new_declarations`
(src/storage.rs:443-457), same shape:
`get_income_notices` / `set_income_notices`,
`get_income_overdue_dismissed_before` / `set_income_overdue_dismissed_before`.

### 2. report_income.rs

`IncomeReport` gains `pub wht_txn_ids: Vec<String>`, and `metadata()`
(src/report_income.rs:33-82) inserts `"withholding_tax_txn_ids"` as a JSON array
of those ids.

New public types:

```rust
pub struct IncomeGenOptions {
    pub today: NaiveDate,      // wait/deadline boundary; never read Local::now() in this module
    pub force_rates: bool,     // nearest-cached NBS fallback (get_rate_or_force)
    pub skip_wht_wait: bool,   // manual override, see step 6
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum NoticeKind { WaitingForWht, MissingRate, PastDeadline, LateWht }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IncomeNotice {
    pub kind: NoticeKind,
    pub date: NaiveDate,
    pub symbol_or_currency: String,
    pub income_type: String,
    pub text: String,          // rendered once, at construction
}

#[derive(Default)]
pub struct IncomeReportBatch {
    pub reports: Vec<IncomeReport>,
    pub notices: Vec<IncomeNotice>,
}

pub type DeclaredKey = (NaiveDate, String, String);  // (date, SYMBOL upper, income_type)

pub struct DeclaredState {
    pub keys: HashSet<DeclaredKey>,
    pub claimed_wht_ids: HashSet<String>,
    /// Declarations that predate the `withholding_tax_txn_ids` metadata key.
    /// Their groups are excluded from `LateWht` detection.
    pub keys_without_claims: HashSet<DeclaredKey>,
}

pub fn generate_income_reports(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    start: NaiveDate,
    end: NaiveDate,
    declared: &DeclaredState,
    opts: &IncomeGenOptions,
) -> IncomeReportBatch
```

`generate_income_reports` becomes infallible at the batch level — per-group
problems are notices, not errors. It replaces the current
`build_income_groups` → `build_income_reports` split, where the first propagates
the first rate error and the second bails on the first zero-WHT group.

**Withholding-tax attribution.** Build it once, over all
`TransactionType::WithholdingTax` in storage (no date bound — the per-group
window does the bounding, so the `end + 7` pool bound at :115 disappears
entirely). A tax matches an income transaction when:

- income is a dividend and the tax description contains the income's entity name
  or ISIN as captured by `ENTITY_RE` (src/report_income.rs:22-23) — **or**, if
  no entity match exists for that tax, the tax's `symbol` equals the income's
  `symbol`. Entity/ISIN matches take precedence over symbol matches, preserving
  today's two-tier behaviour (`test_wht_matched_by_entity_isin_not_symbol`,
  `test_wht_fallback_to_symbol_match`);
- income is interest and the currencies are equal.

Among matching incomes with `income.date <= tax.date`, the tax belongs to the
one with the greatest date. A tax with no preceding match belongs to nobody and
is never reported.

**Pipeline:**

1. Bucket income transactions in `[start, end]` by group key
   `(date, symbol|currency, income_type)`. The key comes from the raw
   transaction — **no exchange rate needed yet.**
2. Key in `declared.keys` → skip silently. Never regenerate.
3. Otherwise resolve rates and sum the taxes attributed to this group whose date
   falls in `[date, min(opts.today, date + WHT_WAIT_DAYS)]` and whose id is not
   in `declared.claimed_wht_ids`:
   - any rate error (income or tax leg) → `MissingRate` notice, continue with
     the other groups;
   - matched total is zero, `date + WHT_WAIT_DAYS >= opts.today`, and
     `!opts.skip_wht_wait` → `WaitingForWht` notice, retried on the next sync;
   - matched total is zero and the wait has elapsed → **generate with zero
     credit.** Fiscally safe (full 15% due, no foreign credit claimed) and it
     removes the permanent-failure mode;
   - otherwise generate, recording the matched ids in `wht_txn_ids`.

The wait must **not** consult `force_rates`: forcing an early zero-credit
declaration is strictly harmful (tax arriving inside the window would become
uncreditable once the slot is taken) and saves at most `WHT_WAIT_DAYS` against a
30-day deadline. Note this is *not* what the GUI force dialog currently
describes (src/gui/app.rs:686-705 talks about re-fetching and overwriting local
edits, and src/gui/app.rs:412-414 also enables holiday fallback) — update that
dialog text in the same change.

Assumption to state in review, not enforceable without new state: the wait is
measured from the *income date*, not from when the transaction first appeared in
storage. Income imported long after its date is therefore past its deadline on
arrival and is reported rather than declared, which is the correct outcome.

`HolidayError::MissingYear` from `nbs.get_rate` (via
`holidays.is_serbian_holiday(target)?`, src/nbs.rs:93) stays a **hard error**
and is not turned into a `MissingRate` notice: it means the app has run out of
holiday data, and generating declarations with wrong holiday handling is worse
than failing. Distinguish it before mapping rate failures to notices.

### 3. sync.rs

`determine_income_period` (src/sync.rs:218-238) becomes pure — no `Storage`
argument, no watermark read:

```rust
fn determine_income_period(
    end_period: NaiveDate,
    options: &SyncOptions,
) -> (NaiveDate, NaiveDate)
```

1. `options.forced_lookback_days` → `start = end_period - (lookback - 1)`;
2. else `start = end_period - (PPOPO_DEADLINE_DAYS - 1)`.

It can no longer return `None`: with `end_period = today - 1` the default window
is always `[today - 30, today - 1]`.

Delete the watermark-advance block (src/sync.rs:175-181) and every
`set_last_declaration_date` call in sync.

`generate_and_save_income` builds `DeclaredState` once — currently
`is_duplicate` calls `storage.get_declarations` (src/sync.rs:380 → whole-file
read and parse, src/storage.rs:339-358) once per group. Key each declaration by
`(period_start, metadata["symbol"].to_uppercase(), metadata["income_type"])`.
Deliberately **not** by filename stem, which is what `is_duplicate` does today:
a filename key freezes the generated filename format forever (changing it makes
every existing declaration invisible to the dedup check) and misses any
declaration whose `file_path` is `None`. Every declaration this app writes
carries `symbol` and `income_type` (src/report_income.rs:38-55), so a
declaration missing either key is hand-edited: log it at `warn` and leave it out
of the map. `is_duplicate` stays for PPDG-3R (src/sync.rs:261).

`opts.today` is `end_period + Duration::days(1)`, not a second `Local::now()`
call — `generate_declarations` derives `end_period` from `Local::now()` at
src/sync.rs:116 and a second read can land on the other side of midnight.

Pass 3 runs in `generate_and_save_income` after generation, over all stored
transactions, and appends `PastDeadline` and `LateWht` notices. Both kinds are
suppressed for groups dated before `income_overdue_dismissed_before`;
`WaitingForWht` and `MissingRate` are never suppressed (they only ever arise
inside the 30-day window).

`SyncResult` (src/sync.rs:28-38): `income_error: Option<String>` is replaced by

```rust
pub income_notices: Vec<IncomeNotice>,
pub income_skipped: bool,   // no reports and no notices at all
```

Persist the notices with `storage.set_income_notices(&notices)` before
returning, `.with_context(|| storage.io_error_hint())?`.

Delete the error-string matching in `generate_declarations`
(src/sync.rs:161-172): `"no NBS exchange rate"` and `"withholding tax"` no
longer arrive as errors. Errors that still propagate (declaration save, IO,
missing holiday year) keep the `"PP-OPO generation failed"` context and remain
the only thing that reaches `set_last_sync_issue`.

`IncomeOutcome` (src/sync.rs:311-314) is replaced by a struct carrying
`created: Vec<Declaration>`, `notices: Vec<IncomeNotice>`, `empty: bool`.

The special-case check for an XML import that predates the window is **not
needed**: an old Flex file simply produces `PastDeadline` notices like any other
undeclared history.

### 4. cli/sync.rs

`print_sync_result` prints every notice to stdout, grouped by kind:

- `PastDeadline`, `LateWht` → `output::warning`;
- `MissingRate`, `WaitingForWht` → `output::dim`.

Replace `"Income report generation failed: {err_msg}"` (src/cli/sync.rs:119) —
it no longer fits a per-group list. Reword `"no income in period"`
(src/cli/sync.rs:126) to `"no undeclared income in period"`, which is what the
flag now means.

Add `sync --dismiss-overdue`: sets `income_overdue_dismissed_before` to today so
a CLI-only user is not walled by the same historical list on every sync. Without
it the CLI would have no equivalent of the GUI's Close button.

### 5. gui

New banner in `src/gui/main_window.rs`, modelled on `new_declarations_banner`
(:171-194) and rendered next to it from `show` (:37-38): lists the notices with
a Close button. **Not** the `last_sync_issue` pill (:130-168) — that pill holds
one short message and is cleared by the next successful sync, whereas these
notices survive a successful sync by construction.

`App` (src/gui/app.rs:130-137) gains `income_notices: Vec<IncomeNotice>`, loaded
from storage in the constructor next to `pending_new_declarations` (:181) and
refreshed from `SyncResult` after each sync. Close calls a new
`dismiss_overdue_income_notices` alongside `dismiss_pending_new_declarations`
(:678-681): it writes `income_overdue_dismissed_before = today` and drops the
`PastDeadline` / `LateWht` entries from the in-memory list.

src/gui/app.rs:576-580 no longer maps income problems to `set_last_sync_issue`;
only fetch and hard generation errors do.

### 6. cli/report.rs

`run_income` (src/cli/report.rs:169-176) must be updated for the new signature:
drop the `match` on `Result`, pass an **empty** `DeclaredState` (this command
writes to a destination directory and does not consult declaration state) and
`IncomeGenOptions { today: Local::now().date_naive(), force_rates: force,
skip_wht_wait: force }`.

`skip_wht_wait: force` preserves today's escape hatch: `--force` is currently
the only way to produce XML for a zero-WHT group (it bypasses the bail at
src/report_income.rs:181).

Two behaviour changes to document rather than hide:

- for a period whose groups are all older than `WHT_WAIT_DAYS`, plain
  `report income` now *emits* zero-credit XML where today it errors out;
- `--force` means different things in the two commands: rates only in `sync`,
  rates *and* skipping the wait in `report income`. Say so in the command help
  and in the docs.

Print `batch.notices` after the reports, and warn instead of overwriting when
two reports in one run resolve to the same destination filename.

### 7. Tests

**Rewrite in `src/sync.rs`** (`determine_income_period` is now pure and total,
so these lose their `Storage` and their `Option`):

- `test_income_period_no_last_date`, `test_income_period_with_last_date`,
  `test_income_period_last_date_equals_end` → one
  `income_period_is_the_deadline_window`: `start == end - 29`.
- `test_forced_lookback_overrides_start` → keep, minus `Storage`.

**New in `src/sync.rs`:**

- `late_arriving_income_gets_declared_on_next_sync`: sync once → add income+tax
  transactions dated inside the already-scanned range → sync again →
  declaration created. The regression test for the whole plan.
- `income_past_deadline_is_reported_not_declared`: undeclared income dated
  `end - 40` → one `PastDeadline` notice, zero declarations.
- `dismiss_suppresses_older_overdue_only`: two overdue groups, dismiss dated
  between them → only the newer is still reported.
- `missing_rate_group_does_not_block_others`: rate present for one of two dates
  → one declaration, one `MissingRate` notice.
- `deleted_income_declaration_is_regenerated`: create, `delete_declaration`,
  sync → recreated (inside the deadline window).
- `declaration_without_file_path_is_not_regenerated`: PP-OPO whose `file_path`
  is `None` but whose metadata carries symbol/type → no duplicate. Guards the
  dedup-key change.
- `notices_are_persisted_and_reloaded`: notices written by a sync are readable
  via `get_income_notices`.
- `no_watermark_is_written`: after a sync, `get_last_declaration_date()` is
  unchanged.

**New in `tests/test_reports.rs`** (batch-level, no `Storage` watermark
involved). All of these pass an explicit `opts.today`:

- `zero_wht_waits_while_window_open`: dividend within `WHT_WAIT_DAYS` of `today`,
  no tax → `WaitingForWht`, no report.
- `zero_wht_wait_ignores_force_rates`: same with `force_rates: true` → still
  waiting.
- `zero_wht_finalizes_after_wait_elapses`: dividend older than `WHT_WAIT_DAYS`,
  no tax → report with `porez_placen_drugoj_drzavi = 0`.
- `wht_arriving_within_window_is_credited`: waiting on day 1, tax added, next
  run generates with the credit and records its id.
- `claimed_wht_is_not_credited_twice`: tax id already in
  `declared.claimed_wht_ids` → the group generates with zero credit.
- `wht_belongs_to_nearest_preceding_payment`: two dividends of one symbol 10
  days apart, one tax after the second → credited to the second only.
- `late_wht_on_declared_group_is_reported`: zero-credit declaration recorded, a
  matching unclaimed tax dated beyond its window → `LateWht`.
- `late_wht_ignores_declarations_without_claim_metadata`: same, but the
  declaration lacks `withholding_tax_txn_ids` → no notice.
- `orphan_wht_is_never_reported`: tax with no preceding same-key income → no
  notice.

**Existing `tests/test_reports.rs` work** — there are **12** call sites (lines
438, 508, 554, 605, 653, 695, 739, 783, 841, 872, 926, 991). All need the new
signature and an explicit `today` far enough past their fixtures that zero-tax
groups finalize rather than wait; do not leave them reading the real clock. Two
need more:

- `test_zero_wht_force_false_errors` (:852) asserts the removed `Err` and
  becomes `zero_wht_finalizes_after_wait_elapses`;
- `test_wht_not_found_beyond_7_day_window` (:616) has its tax 9 days out, which
  `WHT_WAIT_DAYS = 20` now matches. Rename to
  `test_wht_not_found_beyond_wait_window`, move the tax past 20 days, and add
  `test_wht_found_at_year_end_gap` asserting the 9-day case *is* credited —
  that pattern is the reason the constant changed.

`tests/test_gui.rs:55` and the `SyncResult` literal in the `src/cli/sync.rs`
test helper (:143-155) both need the new fields; `print_income_skipped` (:190)
and `print_income_error` (:202) need reworking into per-kind notice cases.

`tests/test_storage.rs`: round-trips for `income_notices` and
`income_overdue_dismissed_before` next to `test_storage_last_declaration_date`
(:257).

### 8. Docs

New `specs/spec-income-declarations.md`: income declarations are generated for
every undeclared income group whose filing deadline has not passed; a group
whose deadline has passed without a declaration is reported, never generated
automatically; a group without withholding tax is held for a bounded wait and
then declared without a foreign-tax credit; a withholding tax belongs to the
nearest preceding income of the same key and can be credited by at most one
declaration; a tax that appears after its group was declared is reported for
manual review; groups missing an exchange rate are reported and retried; a
deleted declaration is rebuilt by the next sync while it is inside the deadline.

This does not belong in `specs/spec-auto-sync.md`, which is about the GUI
background retry cycle. That file gets one sentence: income notices are not the
"issue" shown in the permanent status line.

`docs/*/src/usage.md` (`sync` section) — the same in user terms, plus
`sync --lookback N` as the way to generate for a group whose deadline has
passed, and `sync --dismiss-overdue`. The `delete` → `sync` rebuild promise is
now true within the deadline and needs `--lookback` beyond it: usage.md:340,
`src/cli/delete.rs:27` and `src/gui/delete_dialog.rs:60`. Five locales are kept
in parallel (en, ru, rs, rs-cyr, uk).

## Out of scope

- PPDG-3R generation (already period-based with dedup, no watermark).
- Tracking when a transaction was first seen, which would let the wait start
  from import time instead of the income date.
- Splitting a single withholding tax across several income groups. Attribution
  is whole-tax to one group; revisit only if real data shows partial taxes.

## Verification

```
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Manual, on real data after upgrade:

1. first `sync` creates no duplicates and reports the pre-existing undeclared
   history as `PastDeadline` instead of generating for it;
2. Close in the GUI (or `sync --dismiss-overdue`) hides that history, and a
   later overdue group still appears;
3. adding an income transaction dated inside the deadline window (e.g. via
   `sync --file` with an older Flex XML) produces its declaration on the next
   sync;
4. `sync --lookback 365` generates for the reported historical groups;
5. `declarations.json` still loads in the previous app version (both new keys
   are additive).
