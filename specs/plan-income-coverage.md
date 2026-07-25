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

## Goal and scope

From the version that ships this change onward, no income transaction can be
missed because of fetch failures, IBKR reporting lag, missing rates or a
problematic neighbouring group.

Retroactively declaring income that the current watermark has already passed is
**out of scope**: the coverage window starts where the watermark stood at
upgrade time. Older gaps are the user's call via `sync --lookback N` (dedup
makes that safe).

## Approach

Drop the calendar watermark for income. Every sync scans a coverage window
`[coverage_start, yesterday]`, builds all income groups in it, and skips the
groups that already have a declaration (matched by generator filename). A
transaction arriving late is simply picked up by the next sync — no state can
"move past" it.

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

The window start is pinned once (`income_coverage_start`) so upgrading does not
trigger a backfill, and the effective start is additionally floored at
`end − COVERAGE_MAX_DAYS` so the per-sync work stays bounded as the years pass.

Deleting a declaration and re-running sync regenerates it. This is what
docs/en/src/usage.md:340 already promises and what `delete.rs`'s contract says
("run `sync` to rebuild it if needed"); today it is true for PPDG-3R only,
because the income watermark has moved past the deleted period.

## Constants

```rust
const COVERAGE_MAX_DAYS: i64 = 400;        // src/sync.rs — rolling floor of the window
const CORRECTION_HORIZON_DAYS: i64 = 90;   // src/report_income.rs — how far back changes are re-checked
pub const WHT_WAIT_DAYS: i64 = 7;          // src/report_income.rs — matching window *and* wait
```

`WHT_WAIT_DAYS` replaces both hardcoded `Duration::days(7)` occurrences
(src/report_income.rs:112 pool bound, :237 matching window) and drives the
finalize decision, so the matching window and the wait cannot drift apart.

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
1. stored `income_coverage_start` → return it;
2. else legacy watermark `last_declaration_date` present → `last + 1`
   (no backfill on upgrade; this is the deliberate scope limit above);
3. else (fresh install) → `end_period - (DEFAULT_LOOKBACK_DAYS - 1)`;
4. persist the result — including when it is later than `end_period`, which is
   the normal case for an existing daily-syncing user (watermark = yesterday →
   start = today). The window is empty that day and becomes `[today, today]`
   the next day; the pinned value never changes afterwards.

Persist with `.with_context(|| storage.io_error_hint())?` like the other
writes; the `Result` propagates out of `generate_and_save_income`.

`determine_income_period`:
1. `options.forced_lookback_days` → `start = end - (lookback - 1)`, ignoring
   the pinned start (manual backfill), no persistence;
2. else `start = max(coverage_start, end_period - (COVERAGE_MAX_DAYS - 1))`;
3. `start > end_period` → `None`.

The floor in (2) bounds the scan: the window grows to at most ~13 months and
then rolls. Consequence to accept explicitly: a group that stayed undeclared
for more than `COVERAGE_MAX_DAYS` (e.g. an NBS rate that never became
available) drops out of the window and stops being retried and reported.

Delete the watermark-advance block (src/sync.rs:175-181) and every
`set_last_declaration_date` call in sync.

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
    FilenameConflict(String),
}

pub struct SkippedIncomeGroup {
    pub date: NaiveDate,
    pub symbol_or_currency: String,
    pub reason: SkipReason,
}

/// Existing PP-OPO declarations, keyed by generator filename stem
/// ("ppopo-voo-2026-0630"), built once by the caller.
pub struct DeclaredIncome {
    pub declaration_id: String,
    pub gross_rsd: Option<Decimal>,
    pub foreign_tax_rsd: Option<Decimal>,
}

#[allow(clippy::too_many_arguments)]
pub fn generate_income_reports(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    start: NaiveDate,
    end: NaiveDate,
    declared: &HashMap<String, DeclaredIncome>,
    opts: &IncomeGenOptions,
) -> IncomeReportBatch
```

Pipeline (replaces the current `build_income_groups` → `build_income_reports`
split, where the first propagates the first rate error and the second bails on
the first zero-WHT group):

1. Filter income transactions to `[start, end]` and bucket them by group key
   `(date, symbol|currency, type)`. **The key is derived from the raw
   transaction — no exchange rate needed yet.**
2. Derive the filename stem per key (`ppopo-{key_lower}-{%Y-%m%d}`,
   src/report_income.rs:212-214). Keep this format: changing it would make
   every existing declaration invisible to the dedup check.
3. Key already in `declared`:
   - group date older than `opts.today - CORRECTION_HORIZON_DAYS` → drop the
     group without resolving rates. This is what keeps the growing window cheap
     and silent;
   - otherwise resolve the group and compare with the stored declaration
     (step 4a). Rate errors here are logged at debug and dropped — an
     already-declared group must not generate noise.
   Never regenerate.
4. Key not declared → resolve rates and match WHT per transaction:
   - any rate error (income or WHT leg) → `skipped(MissingRate)`, continue with
     the other groups;
   - `total_wht_rsd == 0` and `date + WHT_WAIT_DAYS >= opts.today` and
     `!opts.skip_wht_wait` → `skipped(WaitingForWithholdingTax)`, retried
     automatically on the next sync;
   - `total_wht_rsd == 0` and the window has closed → **generate with zero
     WHT**. Fiscally safe (full 15% due, no foreign credit claimed) and removes
     the permanent-failure mode. Trade-off: WHT that IBKR books later than
     `WHT_WAIT_DAYS` after the income date is not credited; step 4a surfaces it
     and the user can delete the declaration and re-sync.
   - two distinct group keys mapping to one filename (only possible if a
     dividend symbol equals a currency code) → the second is
     `skipped(FilenameConflict)` instead of silently overwriting.

The wait must **not** consult `force`: forcing an early zero-WHT declaration is
strictly harmful (WHT arriving inside the window would become uncreditable once
the slot is taken) and saves at most `WHT_WAIT_DAYS` against the 30-day PP-OPO
deadline. `force` in sync keeps only its rate-fallback role. Note this is *not*
what the GUI force dialog currently describes (src/gui/app.rs:686-710 talks
about re-fetching and overwriting local edits, and src/gui/app.rs:414 also
enables holiday fallback) — update that dialog text in the same change.

Assumption to state in the code review, not enforceable without new state: the
wait is measured from the *income date*, not from when the transaction first
appeared in storage. Income imported long after its date (CSV import, a very
late Flex report) is therefore finalized immediately, with zero WHT if its
withholding tax arrives in a later report. This holds because IBKR books income
and its withholding in the same statement; when it does not, step 4a reports
the discrepancy.

### 4. sync.rs: routing the batch

`generate_and_save_income` builds the `declared` map once — currently
`is_duplicate` calls `storage.get_declarations` (src/sync.rs:380 → whole-file
read and parse, src/storage.rs:339-358) once per group:

```rust
storage.get_declarations(None, Some(&DeclarationType::Ppo))
```
→ for each declaration with a `file_path`, take the file stem, strip an
optional leading `\d+-` id prefix (saved files are `001-ppopo-…`, legacy ones
may not have it) and read `metadata["gross_income_rsd"]` /
`metadata["foreign_tax_paid_rsd"]` (parsed to `Decimal`, `None` when the key is
missing or unparsable).

`is_duplicate` stays for PPDG-3R (src/sync.rs:261) and is no longer used for
income.

Outcome type replacing `IncomeOutcome` (src/sync.rs:311-314):

```rust
struct IncomeOutcome {
    created: Vec<Declaration>,
    pending: Vec<String>,    // WaitingForWithholdingTax, one line per group
    problems: Vec<String>,   // MissingRate | FilenameConflict | corrections
    empty: bool,             // no reports, no skips, no corrections
}
```

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

For declared groups inside `CORRECTION_HORIZON_DAYS`, compare the recomputed
`total_bruto` / `total_wht_rsd` with the stored `gross_rsd` /
`foreign_tax_rsd`. On mismatch push an `IncomeCorrection` (declaration id,
filename, date, symbol, both amount pairs) → `problems`.

Rules that keep this from crying wolf:
- a `None` stored value (declaration written by an older version or by the
  Python predecessor, which may not carry these metadata keys) → no comparison;
- comparison is on parsed `Decimal`s, not strings;
- a declaration originally generated with `force` used the nearest cached NBS
  rate; once the real rate lands, the recomputed gross legitimately differs.
  The warning text must therefore say "review", not "wrong".

No automatic regeneration — the original may already be submitted. The
documented remedy (docs/en/src/usage.md:340-342) is delete + re-sync, which now
actually rebuilds the PP-OPO; that also clears the warning, so no separate
acknowledge state is needed.

### 5. cli/report.rs: manual `report income`

src/cli/report.rs:170 must be updated for the new signature: pass an **empty**
`declared` map (this command writes to a destination directory and does not
consult declaration state) and `IncomeGenOptions { today: Local::now().date_naive(),
force_rates: force, skip_wht_wait: force }`.

`skip_wht_wait: force` preserves today's escape hatch: `--force` is currently
the only way to produce XML for a zero-WHT group (it bypasses the bail at
src/report_income.rs:181). Without it this command would lose the ability
entirely, since the sync-side wait is unconditional.

Print `batch.skipped` per group after the reports (`output::warning`).

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

New:
- `coverage_start_is_pinned_on_first_sync_and_never_moves`: two syncs, value
  identical, and equal to the migration value (not to `end_period`).
- `coverage_start_wins_over_legacy_watermark`: both stored → the pinned value
  is used and the watermark is ignored.
- `window_is_floored_at_coverage_max_days`: pinned start two years back →
  `start == end - (COVERAGE_MAX_DAYS - 1)`.
- `late_arriving_income_gets_declared_on_next_sync`: sync once → add income+WHT
  transactions dated inside the already-scanned range → sync again →
  declaration created. The regression test for the whole plan.
- `zero_wht_waits_while_window_open`: dividend within `WHT_WAIT_DAYS` of
  `today`, no WHT → `skipped(WaitingForWithholdingTax)`, no declaration, and it
  lands in `income_pending`, not `income_error`.
- `zero_wht_wait_ignores_force`: same with `force: true` → still skipped.
- `zero_wht_finalizes_after_window_closes`: dividend older than
  `WHT_WAIT_DAYS`, no WHT → declaration with
  `porez_placen_drugoj_drzavi = 0`.
- `wht_arriving_within_window_is_credited`: skipped on day 1, WHT added, next
  sync generates with the credit.
- `missing_rate_group_skips_without_blocking_others`: rate present for one of
  two dates → one declaration, one `MissingRate` entry in `income_error`.
- `deleted_income_declaration_is_regenerated`: create, `delete_declaration`,
  sync → recreated.
- `declared_group_outside_correction_horizon_is_not_recomputed`: declared group
  older than `CORRECTION_HORIZON_DAYS` whose rate is missing from the cache →
  no skip entry, no NBS lookup, no `income_error` (guards the "growing window
  stays cheap and silent" property).
- `duplicate_with_changed_gross_warns`: extra income transaction added to an
  already-declared (date, symbol) → no new declaration, correction in
  `income_error`.
- `late_wht_after_zero_declaration_warns`: zero-WHT declaration finalized, WHT
  arrives on day 9 → correction warning.
- `declaration_without_metadata_never_warns`: declared group whose metadata
  lacks `gross_income_rsd` → no correction.

`src/storage.rs` / `tests/test_storage.rs`: get/set round-trip for
`income_coverage_start` next to `test_storage_last_declaration_date`
(tests/test_storage.rs:257).

`tests/test_reports.rs` (8 call sites from line 438) and `tests/test_gui.rs:55`
(`SyncResult` literal) need the new signature/fields.

### 7. Docs

`specs/spec-auto-sync.md` — income declarations are generated for every
undeclared income transaction inside the coverage window; the window start is
pinned at first sync and only rolls once it is older than the retention bound;
a group without withholding tax is held back for a few days and then declared
without a foreign-tax credit; groups missing an exchange rate are reported and
retried; a deleted declaration is rebuilt by the next sync; a changed amount on
an already-declared group is reported for manual review, never auto-corrected.

`docs/*/src/usage.md` (`sync` section) — the same in user terms, plus
`sync --lookback N` as the way to reach income older than the window. Five
locales are kept in parallel (en, ru, rs, rs-cyr, uk).

## Out of scope

- Retroactively declaring income the current watermark has already passed
  (see "Goal and scope"); `sync --lookback N` covers it manually and dedup
  makes it safe.
- PPDG-3R generation (already period-based with dedup, no watermark).
- Tracking when a transaction was first seen, which would let the WHT wait
  start from import time instead of the income date.

## Verification

```
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Manual, on real data after upgrade:
1. first `sync` creates no duplicates and persists `income_coverage_start =
   last_declaration_date + 1`;
2. a second `sync` leaves that value untouched;
3. adding an income transaction dated inside the window (e.g. via
   `sync --file` with an older Flex XML) produces its declaration on the next
   sync;
4. `declarations.json` still loads in the previous app version (the new key is
   additive).
