# Plan: coverage-based income declaration generation

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

A fourth, narrower loss exists independently of the watermark: withholding tax
booked more than 7 days after the income date is never matched
(src/report_income.rs:237) and its foreign-tax credit is forfeited. Today that
is masked — the group bails instead of being declared — but it becomes a real,
permanent loss as soon as zero-WHT groups start being finalized (step 3).
`test_wht_not_found_beyond_7_day_window` (tests/test_reports.rs:616) documents
exactly this case with a year-end dividend whose tax arrives 9 days later.

## Goal and scope

From the first sync on the version that ships this change onward, no income
transaction dated on or after the pinned coverage start can be missed because
of fetch failures, IBKR reporting lag, missing rates or a problematic
neighbouring group.

**Nothing is done for transactions that existed before the upgrade.** The
coverage window is pinned at `last_declaration_date + 1`, i.e. exactly where
the old watermark stood, so the first sync on the new version scans the same
range the old code would have scanned and not one day more. Income dated before
that start — including withholding tax or dividends that IBKR reports late for
those pre-upgrade dates — is not scanned, not detected, not reported and not
declared. Known gaps of that kind are the user's call, by hand or via
`sync --lookback N` (dedup makes that safe).

This is deliberate, not an oversight: a backfill on upgrade would regenerate
declarations the user has already reconciled manually, and would surface
"corrections" for periods that are already settled.

Consequence to state plainly: for a user who syncs daily the pinned start is
the day of the upgrade, so a dividend dated the day before the upgrade and
reported by IBKR afterwards is still lost. That is the last instance of the old
bug, not a new one. Everything dated from the pinned start onward is covered.

## Approach

Drop the calendar watermark for income. Every sync scans a coverage window
`[coverage_start, yesterday]`, builds all income groups in it, and skips the
groups that already have a declaration. A transaction arriving late is simply
picked up by the next sync — no state can "move past" it.

Why this over the alternatives:

- "don't advance the watermark on fetch failure" still loses transactions that
  IBKR reports late on a *successful* fetch;
- "overlap buffer of N days" fails whenever IBKR lag + downtime exceeds N;
- "mark new income transactions as pending at import, clear on declaration" is
  precise and bounded, but reintroduces state that can drift from reality (lost
  flag, deleted declaration without re-queue) — the same bug class as the
  watermark, per transaction;
- the coverage scan recomputes the set of missing declarations from data every
  time, so even corrupted or rolled-back state self-heals on the next sync.

A second, unrelated benefit falls out of recomputing from data: today, if
saving the third of five declarations fails, the remaining two are lost with
the watermark still advancing. With the coverage scan the next sync recomputes
what is missing and finishes the job.

The window start is pinned once (`income_coverage_start`) so upgrading does not
trigger a backfill, and the effective start is additionally floored at
`end − COVERAGE_MAX_DAYS` so the per-sync work stays bounded as the years pass.

Deleting a declaration and re-running sync regenerates it, **as long as its date
is still inside the coverage window**. This is what docs/en/src/usage.md:340
already promises and what `delete.rs`'s contract says ("run `sync` to rebuild it
if needed"); today it is true for PPDG-3R only, because the income watermark has
moved past the deleted period. Beyond `COVERAGE_MAX_DAYS` the rebuild needs
`sync --lookback N`; the docs must say so rather than promising it
unconditionally.

## Constants

```rust
const COVERAGE_MAX_DAYS: i64 = 400;        // src/sync.rs — rolling floor of the window
const CORRECTION_HORIZON_DAYS: i64 = 90;   // src/report_income.rs — how far back declared groups are re-checked
const RATE_GRACE_DAYS: i64 = 3;            // src/report_income.rs — a rate this fresh may simply not be published yet
const WHT_MATCH_DAYS: i64 = 15;            // src/report_income.rs — matching window *and* wait
const WHT_LATE_MATCH_DAYS: i64 = 60;       // src/report_income.rs — recheck-only, never used for generation
```

`WHT_MATCH_DAYS` replaces both hardcoded `Duration::days(7)` occurrences
(src/report_income.rs:115 pool bound, :237 matching window) and drives the
finalize decision, so the matching window and the wait cannot drift apart:
waiting longer than the matcher looks would be pointless, matching further than
the wait would be unreachable.

The value goes from 7 to 15 **because** zero-WHT groups are now finalized
instead of bailing. At 7 days the year-end pattern already in the test suite
(dividend 2025-12-24, its US tax 2026-01-02) silently forfeits the credit; that
was harmless while the group bailed and is a permanent loss once it is
declared. 15 days still leaves half of the 30-day PP-OPO deadline.

Widening the window is only safe with one extra rule: both the matching window
and the recheck window are capped at the day before the next income transaction
**in storage** with the same group key —
`window_end = min(date + N, next_same_key_date - 1)`. `find_withholding_tax`
sums every candidate in range (src/report_income.rs:246-275), so without the
cap two payments of the same symbol closer together than the window would each
claim the other's tax. "In storage", not "in the window", so a group sitting at
the very start of the coverage window cannot absorb tax belonging to an earlier,
already-declared payment.

## Changes

### 1. Storage: `income_coverage_start`

`src/models.rs` — add to `DeclarationsFile` (`last_declaration_date` stays in
the struct for file compatibility, but is only *read*, by the migration in
step 2):

```rust
#[serde(default)]
pub income_coverage_start: Option<String>,   // "%Y-%m-%d"
```

`src/storage.rs` — next to `get_last_declaration_date` (line 390) add
`get_income_coverage_start` / `set_income_coverage_start`, same parse/format
pattern.

### 2. sync.rs: window determination

Split the side effect out of the pure calculation (the current
`determine_income_period` at src/sync.rs:218-238 is replaced by both):

```rust
/// Reads the pinned coverage start, or pins and persists it on first run.
fn resolve_coverage_start(storage: &Storage, end_period: NaiveDate) -> Result<NaiveDate>

/// Pure: no storage access, drivable from tests.
fn determine_income_period(
    coverage_start: NaiveDate,
    end_period: NaiveDate,
    options: &SyncOptions,
) -> Option<(NaiveDate, NaiveDate)>
```

`resolve_coverage_start`:
1. stored `income_coverage_start` → return it, no write;
2. else legacy watermark `last_declaration_date` present → `last + 1`
   (no backfill on upgrade; this is the deliberate scope limit above);
3. else (fresh install) → `end_period - (DEFAULT_LOOKBACK_DAYS - 1)`;
4. persist the result — including when it is later than `end_period`, which is
   the normal case for an existing daily-syncing user (watermark = yesterday →
   start = today). The window is empty that day and becomes `[today, today]`
   the next day; the pinned value never changes afterwards.

It runs before the empty-window check, so the pin happens even on a sync that
generates nothing.

Persist with `.with_context(|| storage.io_error_hint())?` like the other
writes; the `Result` propagates out of `generate_and_save_income`.

`determine_income_period`:
1. `options.forced_lookback_days` → `start = end - (lookback - 1)`, ignoring
   the pinned start (manual backfill), no persistence;
2. else `start = max(coverage_start, end_period - (COVERAGE_MAX_DAYS - 1))`;
3. `start > end_period` → `None`.

A forced-lookback run never moves the pin — not even when its start is earlier.
`--lookback` is a one-shot manual reach into the past, not a permanent change to
how far every future sync scans. `resolve_coverage_start` still runs (and still
pins on a fresh install whose very first command is `sync --lookback N`), it is
simply not consulted for the window that run.

The floor in (2) bounds the scan: the window grows to at most ~13 months and
then rolls. Consequence to accept explicitly: a group that stayed undeclared
for more than `COVERAGE_MAX_DAYS` (e.g. an NBS rate that never became
available) drops out of the window and stops being retried and reported.

Delete the watermark-advance block (src/sync.rs:175-181) and every
`set_last_declaration_date` call in sync. Nothing else in the codebase reads
`last_declaration_date` (checked: only src/sync.rs, plus storage accessors and
tests), so freezing it is safe and keeps the file readable by older versions.

### 3. report_income.rs: per-group outcomes instead of batch bail

`generate_income_reports` becomes infallible at the batch level — per-group
problems are data, not errors:

```rust
pub struct IncomeGenOptions {
    pub today: NaiveDate,      // wait/finalize boundary; never read Local::now() in this module
    pub force_rates: bool,     // nearest-cached NBS fallback (get_rate_or_force)
    pub skip_wht_wait: bool,   // manual override, see step 5
}

#[derive(Default)]
pub struct IncomeReportBatch {
    pub reports: Vec<IncomeReport>,
    pub skipped: Vec<SkippedIncomeGroup>,
    pub corrections: Vec<IncomeCorrection>,
}

pub enum SkipReason {
    WaitingForWithholdingTax,   // normal, retried next sync — not a sync issue
    MissingRate(String),
}

pub struct SkippedIncomeGroup {
    pub date: NaiveDate,
    pub symbol_or_currency: String,
    pub reason: SkipReason,
}

/// Existing PP-OPO declarations, keyed by group identity (see step 4).
pub struct DeclaredIncome {
    pub declaration_id: String,
    pub status: DeclarationStatus,
    pub gross_rsd: Option<Decimal>,
    pub foreign_tax_rsd: Option<Decimal>,
}

pub type DeclaredKey = (NaiveDate, String, String);  // (date, SYMBOL upper, income_type)

#[allow(clippy::too_many_arguments)]
pub fn generate_income_reports(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    start: NaiveDate,
    end: NaiveDate,
    declared: &HashMap<DeclaredKey, DeclaredIncome>,
    opts: &IncomeGenOptions,
) -> IncomeReportBatch
```

Pipeline (replaces the current `build_income_groups` → `build_income_reports`
split, where the first propagates the first rate error and the second bails on
the first zero-WHT group):

1. Filter income transactions to `[start, end]` and bucket them by group key
   `(date, symbol|currency, type)`. **The key is derived from the raw
   transaction — no exchange rate needed yet.**
2. The dedup key is that same group key with the symbol uppercased —
   *not* the generated filename (see step 4 for why).
3. Key already in `declared`:
   - group date older than `opts.today - CORRECTION_HORIZON_DAYS` → drop the
     group without resolving rates. This is what keeps the growing window cheap
     and silent;
   - otherwise compare with the stored declaration (step 4a), using
     **cache-only** rate lookups — never a network call, never a
     `force` fallback. Any leg whose rate is not already in the local cache
     silently skips the comparison for that group.
   Never regenerate.
4. Key not declared → resolve rates and match WHT per transaction, with the
   matching window `[date, min(date + WHT_MATCH_DAYS, next_same_key_date - 1)]`:
   - any rate error (income or WHT leg) → `skipped(MissingRate)`, continue with
     the other groups;
   - `total_wht_rsd == 0` and `date + WHT_MATCH_DAYS >= opts.today` and
     `!opts.skip_wht_wait` → `skipped(WaitingForWithholdingTax)`, retried
     automatically on the next sync;
   - `total_wht_rsd == 0` and the window has closed → **generate with zero
     WHT**. Fiscally safe (full 15% due, no foreign credit claimed) and removes
     the permanent-failure mode. WHT that IBKR books later than
     `WHT_MATCH_DAYS` is not credited; step 4a detects it up to
     `WHT_LATE_MATCH_DAYS` and the user can delete the declaration and re-sync.

The wait must **not** consult `force`: forcing an early zero-WHT declaration is
strictly harmful (WHT arriving inside the window would become uncreditable once
the slot is taken) and saves at most `WHT_MATCH_DAYS` against the 30-day PP-OPO
deadline. `force` in sync keeps only its rate-fallback role. Note this is *not*
what the GUI force dialog currently describes (src/gui/app.rs:687-705 talks
about re-fetching and overwriting local edits, and src/gui/app.rs:413-415 also
enables holiday fallback) — update that dialog text in the same change.

Assumption to state in the code review, not enforceable without new state: the
wait is measured from the *income date*, not from when the transaction first
appeared in storage. Income imported long after its date (CSV import, a very
late Flex report) is therefore finalized immediately, with zero WHT if its
withholding tax arrives in a later report. This holds because IBKR books income
and its withholding in the same statement; when it does not, step 4a reports
the discrepancy.

`FilenameConflict` from an earlier draft of this plan is gone: with a dedup key
that carries the income type, a dividend on a symbol literally named `USD` and
USD interest on the same day are two distinct keys, and `save_declaration`
already gives their files distinct `NNN-` prefixes. Only `report income`
(step 5), which writes bare filenames into a destination directory, can still
collide — it warns instead of overwriting.

### 4. sync.rs: routing the batch

`generate_and_save_income` builds the `declared` map once — currently
`is_duplicate` calls `storage.get_declarations` (src/sync.rs:380 → whole-file
read and parse, src/storage.rs:339-358) once per group:

```rust
storage.get_declarations(None, Some(&DeclarationType::Ppo))
```

Key each declaration by `(period_start, metadata["symbol"].to_uppercase(),
metadata["income_type"])` and carry `status`, `metadata["gross_income_rsd"]`
and `metadata["foreign_tax_paid_rsd"]` (parsed to `Decimal`, `None` when the
key is missing or unparsable).

Deliberately **not** keyed by filename stem, which is what `is_duplicate` does
today. A filename key would (a) freeze the generated filename format forever,
since changing it makes every existing declaration invisible to the dedup
check, and (b) miss any declaration whose `file_path` is `None` — harmless
today because the watermark blocked rescanning, but under a coverage scan such
a declaration would be regenerated on *every* sync for up to
`COVERAGE_MAX_DAYS`. `period_start`, `symbol` and `income_type` are exactly the
group identity, are present in every declaration written by this app and by the
Python predecessor (verified against tests/resources/declarations.json), and do
not depend on paths.

Fallback for a declaration that lacks either metadata key: parse its filename
stem (strip a leading `\d+-`, then `ppopo-{key}-{%Y-%m%d}`) and register it
under both `(date, KEY, "dividend")` and `(date, KEY, "coupon")`, so it cannot
be regenerated under either type. This keeps the change strictly no worse than
today's behaviour for any file we have not seen.

`is_duplicate` stays for PPDG-3R (src/sync.rs:261) and is no longer used for
income.

`opts.today` is `end_period + Duration::days(1)`, not a second
`Local::now()` call — `generate_declarations` derives `end_period` from
`Local::now()` at src/sync.rs:116 and a second read can land on the other side
of midnight.

Outcome type replacing `IncomeOutcome` (src/sync.rs:311-314):

```rust
struct IncomeOutcome {
    created: Vec<Declaration>,
    pending: Vec<String>,    // informational, retried or not actionable
    problems: Vec<String>,   // needs the user
    empty: bool,             // no reports, no skips, no corrections
}
```

Routing, so that normal transient states never park a permanent issue in the
GUI status line:

| outcome | goes to |
| --- | --- |
| `WaitingForWithholdingTax` | `pending` |
| `MissingRate`, group date `>= today - RATE_GRACE_DAYS` | `pending` |
| `MissingRate`, older | `problems` |
| correction on a `Draft` declaration | `problems` |
| correction on a `Submitted`/`Paid` declaration | `pending` |

A missing rate for a two-day-old dividend is as normal and as self-resolving as
waiting for withholding tax — NBS may simply not have published yet. Only a
rate still missing after the grace period is worth the user's attention.

A correction on an already-submitted declaration is not actionable through
delete + re-sync, so raising a permanent issue for it would leave the GUI
status line stuck for up to `CORRECTION_HORIZON_DAYS` with nothing the user can
do; its wording says so ("already submitted — file an amendment if needed").

`SyncResult` (src/sync.rs:28-38):

```rust
pub income_error: Option<String>,   // joined `problems`, None when empty
pub income_pending: Vec<String>,    // informational, never a SyncIssue
pub income_skipped: bool,           // nothing to do at all
```

Delete the error-string matching in `generate_declarations`
(src/sync.rs:161-172): `"no NBS exchange rate"` and `"withholding tax"` no
longer arrive as errors. Errors that still propagate (declaration save/IO)
keep the `"PP-OPO generation failed"` context.

`run_sync_from_xml` additionally compares the minimum income date among the
*imported* transactions (`fetch_result.transactions`) with
`storage.get_income_coverage_start()` read after generation. If the import
contains income older than the window, push one line into `income_pending`:
importing a year-old Flex file otherwise prints "no undeclared income in
period", which is true but actively misleading. Only on this path — a
long-standing user always has income before the window, so an unconditional
check would emit the line on every sync forever.

Consumers:
- src/gui/app.rs:577-579 keeps mapping `income_error` to `set_last_sync_issue`
  unchanged, and ignores `income_pending` — waiting for withholding tax on a
  three-day-old dividend is a normal state and must not park a permanent
  "issue" in the status line;
- src/cli/sync.rs:118 reword from "Income report generation failed" to
  something that fits a per-group list ("Income declarations need attention:"),
  and print `income_pending` with `output::dim`;
- src/cli/sync.rs:125 reword "no income in period" → "no undeclared income in
  period", which is what the flag now means.

### 4a. Correction detection on an already-declared group

For declared groups inside `CORRECTION_HORIZON_DAYS`, recompute `total_bruto`
and `total_wht_rsd` and compare with the stored `gross_rsd` /
`foreign_tax_rsd`. On mismatch push an `IncomeCorrection` (declaration id,
status, filename, date, symbol, both amount pairs), routed per the table above.

The WHT leg of this recompute uses `WHT_LATE_MATCH_DAYS`, not
`WHT_MATCH_DAYS` — capped by the next same-key transaction as everywhere else.
This is the only thing that makes the "declare with zero WHT once the window
closes" trade-off visible: with a 15-day recheck window, tax booked on day 20
would be invisible and the forfeited credit silent. The wider window is
warn-only and never feeds generation, so a false positive costs the user a
second look, not money.

Rules that keep this from crying wolf:
- a `None` stored value → no comparison;
- a rate not present in the local cache → no comparison. This also removes the
  force-mode noise: `force_fallback_rate` (src/report_income.rs:305-321)
  returns the nearest cached rate *without* writing a cache entry for the
  requested date, so a force-generated group has no cached rate for its own
  date, is silently skipped here, and — importantly — does not make
  `nbs.get_rate` walk its 10-day lookback with an HTTP request per day
  (src/nbs.rs:81-129) on every sync for 90 days;
- comparison is on parsed `Decimal`s, not strings;
- the warning text says "review", not "wrong".

No automatic regeneration — the original may already be submitted. The
documented remedy (docs/en/src/usage.md:340-342) is delete + re-sync, which now
actually rebuilds the PP-OPO; that also clears the warning, so no separate
acknowledge state is needed for the `Draft` case, and the non-`Draft` case is
downgraded to informational rather than nagged about.

### 5. cli/report.rs: manual `report income`

src/cli/report.rs:170-177 must be updated for the new signature: drop the
`match` on `Result`, pass an **empty** `declared` map (this command writes to a
destination directory and does not consult declaration state) and
`IncomeGenOptions { today: Local::now().date_naive(), force_rates: force,
skip_wht_wait: force }`.

`skip_wht_wait: force` preserves today's escape hatch: `--force` is currently
the only way to produce XML for a zero-WHT group (it bypasses the bail at
src/report_income.rs:181). Without it this command would lose the ability
entirely for recent periods, since the sync-side wait is unconditional.

Two behaviour changes to document rather than hide:
- for a period whose groups are all older than `WHT_MATCH_DAYS`, plain
  `report income` now *emits* zero-WHT XML where today it errors out. That is
  the intended new semantics, but it means `--force` is no longer needed for
  historical periods;
- `--force` now means different things in the two commands: rates only in
  `sync`, rates *and* skipping the wait in `report income`. Say so in the
  command help and in the docs.

Print `batch.skipped` per group after the reports (`output::warning`), and warn
instead of overwriting when two reports in one run resolve to the same
destination filename.

### 6. Tests

Rewrite in `src/sync.rs` (`determine_income_period` is now pure, so these lose
their `Storage`):
- `test_income_period_no_last_date` → move to `resolve_coverage_start`: fresh
  storage → `end - 44` **and** persisted.
- `test_income_period_with_last_date` → `resolve_coverage_start` migration:
  `last_declaration_date = 2026-02-15` → `2026-02-16`, persisted.
- `test_income_period_last_date_equals_end` → watermark = end → pinned start is
  `end + 1` and persisted, `determine_income_period` yields `None`.
- `test_forced_lookback_overrides_start` → forced lookback wins over the pinned
  start and does not overwrite it.

New in `src/sync.rs`:
- `coverage_start_is_pinned_on_first_sync_and_never_moves`: two syncs, value
  identical, and equal to the migration value (not to `end_period`).
- `coverage_start_wins_over_legacy_watermark`: both stored → the pinned value
  is used and the watermark is ignored.
- `upgrade_does_not_backfill_before_the_watermark`: watermark = `end - 5` with
  an undeclared income transaction dated `end - 10` → sync creates nothing for
  it and reports nothing about it. This is the scope limit from "Goal and
  scope", and it is a test so that a later change cannot quietly turn it into a
  backfill.
- `forced_lookback_does_not_move_the_pin`: `sync --lookback 365` on a pinned
  install → declarations created for the older range, `income_coverage_start`
  unchanged.
- `window_is_floored_at_coverage_max_days`: pinned start two years back →
  `start == end - (COVERAGE_MAX_DAYS - 1)`.
- `late_arriving_income_gets_declared_on_next_sync`: sync once → add income+WHT
  transactions dated inside the already-scanned range → sync again →
  declaration created. The regression test for the whole plan.
- `missing_rate_group_skips_without_blocking_others`: rate present for one of
  two dates → one declaration, one `MissingRate` entry.
- `fresh_missing_rate_is_pending_not_error`: rate missing for a group dated
  yesterday → `income_pending`, `income_error` is `None`.
- `stale_missing_rate_is_an_error`: same group dated
  `today - RATE_GRACE_DAYS - 1` → `income_error`.
- `deleted_income_declaration_is_regenerated`: create, `delete_declaration`,
  sync → recreated.
- `declaration_without_file_path_is_not_regenerated`: PP-OPO declaration whose
  `file_path` is `None` but whose metadata carries symbol/type → no duplicate.
  Guards the dedup-key change.
- `xml_import_older_than_window_is_reported`: `run_sync_from_xml` with income
  before `coverage_start` → one `income_pending` line, no declarations.

New in `tests/test_reports.rs` (batch-level, no `Storage` watermark involved):
- `zero_wht_waits_while_window_open`: dividend within `WHT_MATCH_DAYS` of
  `today`, no WHT → `skipped(WaitingForWithholdingTax)`, no report.
- `zero_wht_wait_ignores_force`: same with `force_rates: true` → still skipped.
- `zero_wht_finalizes_after_window_closes`: dividend older than
  `WHT_MATCH_DAYS`, no WHT → report with `porez_placen_drugoj_drzavi = 0`.
- `wht_arriving_within_window_is_credited`: skipped on day 1, WHT added, next
  run generates with the credit.
- `declared_group_outside_correction_horizon_is_not_recomputed`: declared group
  older than `CORRECTION_HORIZON_DAYS` whose rate is missing from the cache →
  no skip entry, no NBS lookup, no correction (guards the "growing window stays
  cheap and silent" property).
- `duplicate_with_changed_gross_warns`: extra income transaction added to an
  already-declared (date, symbol) → no new report, one correction.
- `correction_on_submitted_declaration_is_informational`: same, declaration
  status `Submitted` → lands in `income_pending`, not `income_error`.
- `late_wht_after_zero_declaration_warns`: zero-WHT declaration finalized, WHT
  arrives on day 20 (outside `WHT_MATCH_DAYS`, inside `WHT_LATE_MATCH_DAYS`)
  → correction warning.
- `declaration_without_metadata_never_warns`: declared group whose metadata
  lacks `gross_income_rsd` → no correction.
- `uncached_rate_never_warns`: declared group whose rate is absent from the
  cache → no correction and no network call.
- `wht_window_capped_by_next_same_symbol_payment`: two dividends of one symbol
  10 days apart, one WHT after the second → credited to the second only, not
  to both.

Existing `tests/test_reports.rs` work — the plan's earlier count of "8 call
sites" was wrong, there are **12** (lines 438, 508, 554, 605, 653, 695, 739,
783, 841, 872, 926, 991), and two of them need more than a signature update:
- `test_zero_wht_force_false_errors` (:852) asserts the removed `Err` and must
  become `zero_wht_finalizes_after_window_closes` / `..._waits_while_open`
  depending on the date it uses;
- `test_wht_not_found_beyond_7_day_window` (:616) has its WHT 9 days out, which
  the new `WHT_MATCH_DAYS = 15` now matches. Rename to
  `test_wht_not_found_beyond_match_window`, move the WHT past 15 days, and add
  `test_wht_found_at_year_end_gap` asserting the 9-day case *is* credited —
  that pattern is the reason the constant changed.

`tests/test_gui.rs:55` and the `SyncResult` literal in the `src/cli/sync.rs`
test helper (:149) both need the new fields; `print_income_skipped` (:190) and
`print_income_error` (:202) need the reworded strings plus a
`print_income_pending` case.

`src/storage.rs` / `tests/test_storage.rs`: get/set round-trip for
`income_coverage_start` next to `test_storage_last_declaration_date`
(tests/test_storage.rs:257).

### 7. Docs

New `specs/spec-income-declarations.md` — income declarations are generated for
every undeclared income transaction inside the coverage window; the window
start is pinned at the first sync of the version that introduced it and only
rolls once it is older than the retention bound; income dated before that pin
is out of scope by design; a group without withholding tax is held back for a
short wait and then declared without a foreign-tax credit; groups missing an
exchange rate are reported and retried; a deleted declaration is rebuilt by the
next sync while it is inside the window; a changed amount on an already-declared
group is reported for manual review, never auto-corrected.

This does not belong in `specs/spec-auto-sync.md`, which is specifically about
the GUI background retry cycle. That file gets one sentence only: waiting for
withholding tax and a just-published-yet-missing rate are normal states and do
not count as the "issue" shown in the permanent status line.

`docs/*/src/usage.md` (`sync` section) — the same in user terms, plus
`sync --lookback N` as the way to reach income older than the window, and the
bounded form of the `delete` → `sync` rebuild promise (usage.md:340-342 and the
"Run `sync` afterwards to rebuild the period if needed" line in
src/cli/delete.rs). Five locales are kept in parallel (en, ru, rs, rs-cyr, uk).

## Out of scope

- Retroactively declaring income the current watermark has already passed
  (see "Goal and scope"); `sync --lookback N` covers it manually and dedup
  makes it safe.
- PPDG-3R generation (already period-based with dedup, no watermark).
- Tracking when a transaction was first seen, which would let the WHT wait
  start from import time instead of the income date.
- Claim-based WHT matching, where each withholding-tax transaction is consumed
  by exactly one income group. The next-same-key cap gives most of that benefit
  for a fraction of the change; revisit if the widened window produces real
  mismatches.

## Verification

```
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Manual, on real data after upgrade:
1. first `sync` creates no duplicates, declares nothing for any date at or
   before the old watermark, and persists `income_coverage_start =
   last_declaration_date + 1`;
2. a second `sync` leaves that value untouched;
3. adding an income transaction dated inside the window (e.g. via
   `sync --file` with an older Flex XML) produces its declaration on the next
   sync;
4. `sync --lookback 365` reaches older income and still does not move
   `income_coverage_start`;
5. `declarations.json` still loads in the previous app version (the new key is
   additive).
