# Plan: coverage-based income declaration generation

## Problem

`determine_income_period` (src/sync.rs:218) starts the income window at
`last_declaration_date + 1`, and `generate_declarations` (src/sync.rs:175-181)
unconditionally advances `last_declaration_date` to yesterday — even when the
IBKR fetch failed, when IBKR's Flex report has not yet included transactions
for that date (dividends appear with a 1+ business-day lag), or when income
generation was skipped due to a missing NBS rate. Any income transaction that
arrives in storage *after* the watermark has passed its date is silently and
permanently skipped. Real instance: VOO dividend 2026-06-30 (3639.27 RSD
gross) has no PP-OPO declaration.

## Approach

Drop the calendar watermark for income entirely. Instead, every sync scans a
fixed coverage window `[income_coverage_start, yesterday]`, builds all income
groups in it, and relies on the existing filename-based `is_duplicate` check
(src/sync.rs:374) to skip groups that already have a declaration. A
transaction arriving late is simply picked up by the next sync — no state can
"move past" it.

Why this is the most reliable option considered:
- "don't advance watermark on fetch failure" still loses transactions that
  IBKR reports late on a *successful* fetch;
- "overlap buffer of N days" fails whenever IBKR lag + downtime exceeds N;
- "mark new income transactions as pending at import, clear on declaration"
  is precise and bounded, but reintroduces state that can drift from reality
  (lost flag, deleted declaration without re-queue) — the same bug class as
  the watermark, per transaction;
- coverage scan has no failure mode of this class: the set of missing
  declarations is recomputed from data every time, so even corrupted or
  rolled-back state self-heals on the next sync.

Known residual limitation: a correction transaction booked later into an
already-declared (date, symbol) group cannot be auto-declared (the filename
slot is taken and the original may already be submitted). It is surfaced as a
warning instead of being silently absorbed — see step 4a.

Deleting a declaration and re-running sync regenerates it — this matches the
existing delete-to-regenerate flow.

## Changes

### 1. Storage: `income_coverage_start`

`src/models.rs` — add to `DeclarationsFile` (next to `last_declaration_date`,
which stays in the struct for file compatibility but is no longer read/written
by sync):

```rust
#[serde(default)]
pub income_coverage_start: Option<String>,   // "%Y-%m-%d"
```

`src/storage.rs` — next to `get_last_declaration_date` (line 390) add:

```rust
pub fn get_income_coverage_start(&self) -> Option<NaiveDate>
pub fn set_income_coverage_start(&self, date: NaiveDate) -> Result<()>
```

Same parse/format pattern as `get/set_last_declaration_date`.

### 2. sync.rs: window determination

Replace `determine_income_period` (src/sync.rs:218-238) with:

```rust
fn determine_income_period(
    storage: &Storage,
    end_period: NaiveDate,
    options: &SyncOptions,
) -> Option<(NaiveDate, NaiveDate)>
```

Logic:
1. `forced_lookback_days` override — unchanged (start = end − lookback + 1).
2. If `income_coverage_start` is stored → `start = income_coverage_start`.
3. Else (first run after upgrade, or cold start) compute and **persist** it:
   - if `last_declaration_date` exists (legacy watermark):
     `start = last_declaration_date + 1` — seamless continuation, no rescan;
   - else `start = end_period - Duration::days(DEFAULT_LOOKBACK_DAYS - 1)`.
   - `storage.set_income_coverage_start(start)`.
4. `start > end_period` → `None` (unchanged).

The coverage start is written once and never advanced. The scan stays cheap:
it is a filter over already-loaded transactions plus a filename comparison per
group.

Delete the watermark-advance block in `generate_declarations`
(src/sync.rs:175-181) and remove `set_last_declaration_date` /
`get_last_declaration_date` calls from sync. Keep the storage methods (used by
migration read in step 3 above via `get_last_declaration_date`).

### 3. report_income.rs: per-group failures instead of batch bail

With a fixed coverage start, one problematic group must not block all others.
Currently:
- `build_income_reports` (src/report_income.rs:181) `bail!`s the whole batch
  on the first zero-WHT group;
- `build_income_groups` (src/report_income.rs:138) propagates the first
  missing-NBS-rate error and kills the batch.

Change `generate_income_reports` to return per-group outcomes:

```rust
pub struct IncomeReportBatch {
    pub reports: Vec<IncomeReport>,
    pub skipped: Vec<SkippedIncomeGroup>,   // { date, symbol_or_currency, reason }
}
```

Zero-WHT handling — a group must never stay skipped forever (securities with
legitimately no withholding exist, e.g. Irish-domiciled ETFs). The WHT
matching window is `income date + 7 days` (src/report_income.rs:237), so
after it closes no WHT can match anymore:
- WHT window still open (`income date + 7 days >= today`) and zero WHT →
  push to `skipped` with reason "waiting for withholding tax", continue with
  other groups; retried automatically next sync.
- WHT window closed and still zero WHT → **generate with zero WHT**. This is
  fiscally safe (full 15% due, no foreign credit claimed) and removes the
  permanent-warning failure mode. Trade-off: WHT that IBKR books more than
  7 days after the income date is not credited; the user can delete the
  declaration and re-sync.

The WHT wait does **not** consult `force`. Forcing an early zero-WHT
declaration is strictly harmful (WHT arriving within the window would be
silently uncreditable once the group is covered) and saves at most 7 days
against the 30-day PP-OPO filing deadline. `force` is narrowed to its other
existing role only: exchange-rate fallback to the nearest cached NBS rate
(`get_rate_or_force`). This matches what the GUI force-sync confirmation
dialog already tells the user (src/gui/app.rs:902 mentions only rate
lookback).

Testability: the window-closed check must not read `Local::now()` inside
report_income — pass "today" in from the caller (sync already computes
`end_period = today - 1`; the check is `income_date + 7 days < end_period + 1`),
so the wait/finalize boundary is drivable from tests.

Missing NBS rate for a group's income or WHT transaction → push to `skipped`
with the rate error, continue with other groups. This requires moving the
rate/WHT resolution from `build_income_groups` into the per-group loop, or
catching the error per transaction and tagging the group as skipped.

### 4. sync.rs: surface skipped groups

`SyncResult.income_error: Option<String>` becomes the joined summary of
`batch.skipped` (one line per group: date, symbol, reason), `None` when empty.
This flows into the existing `SyncIssue` persistence and GUI notification
unchanged. `income_skipped` stays: true when the window produced neither
reports nor skips.

In `generate_and_save_income`, the special-case error matching in
`generate_declarations` (src/sync.rs:161-172, the `"no NBS exchange rate"` and
`"withholding tax"` string matching) is deleted — those conditions no longer
arrive as errors.

Efficiency: move the `is_duplicate` check ahead of report generation — filter
already-declared group keys out before building XML (the filename is
derivable from the group key: `ppopo-{key_lower}-{Y-md}.xml`,
src/report_income.rs:214), so the ever-growing coverage window does not
regenerate historical reports each sync.

### 4a. Correction detection on duplicate skip

When a group is skipped as a duplicate, compare its recomputed gross
(`total_bruto`) and matched WHT (`total_wht_rsd`) with the existing
declaration's `metadata["gross_income_rsd"]` / `metadata["foreign_tax_paid_rsd"]`.
On mismatch, record a `SyncIssue`-style warning naming the declaration and
both amounts — a correction transaction (income or late WHT) was booked into
an already-declared group and needs manual review. No automatic regeneration:
the original may already be submitted. This also surfaces WHT that arrives
after the 7-day window closed a zero-WHT declaration.

### 5. Tests

Update in `src/sync.rs`:
- `test_income_period_no_last_date` → asserts 45-day window **and** that
  `income_coverage_start` was persisted.
- `test_income_period_with_last_date` → legacy watermark migration:
  `last_declaration_date = 2026-02-15` → start `2026-02-16`, persisted.
- `test_income_period_last_date_equals_end` → unchanged semantics (legacy
  watermark = end → start > end → `None`); also holds for a stored coverage
  start after `end_period`.
- `test_forced_lookback_overrides_start` → unchanged.

New tests:
- `late_arriving_income_gets_declared_on_next_sync`: sync once (no income txn
  in storage) → add income+WHT txn dated inside the already-scanned range →
  sync again → declaration created. This is the regression test for the VOO
  case.
- `coverage_start_does_not_advance`: two syncs, `income_coverage_start`
  identical after both.
- `zero_wht_waits_while_window_open`: dividend dated within 7 days of "today",
  no WHT → skipped with warning, no declaration; other groups unaffected.
- `zero_wht_wait_ignores_force`: same setup with `force: true` → still
  skipped (force must not produce an early zero-WHT declaration).
- `zero_wht_finalizes_after_window_closes`: dividend dated more than 7 days
  ago, no WHT → declaration generated with `porez_placen_drugoj_drzavi = 0`.
- `wht_arriving_within_window_is_credited`: skip on day 1, WHT txn added,
  next sync generates with the credit.
- `missing_rate_group_skips_without_blocking_others`: rate present for one of
  two dates → one declaration, one skip entry.
- `deleted_income_declaration_is_regenerated`: create, delete via
  `storage.delete_declaration`, sync → recreated.
- `duplicate_with_changed_gross_warns`: declaration exists, extra income txn
  added to the same (date, symbol) → no new declaration, mismatch warning
  recorded.

Update `src/report_income.rs` tests (if any) and `tests/` integration tests
touching `generate_income_reports`'s signature.

### 6. Docs

Update `specs/spec-auto-sync.md`: income declarations are generated for every
undeclared income transaction inside the coverage window; the window start is
fixed at first-sync time; groups missing WHT or exchange rates are skipped
with a warning and retried on later syncs.

## Out of scope

- PPDG-3R generation (already period-based with dedup, no watermark).
- Backfilling income older than the migration window (user can run
  `sync --lookback N` manually; dedup makes it safe).

## Verification

```
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Manual: on real data, `ibkr-porez sync` after upgrade must create no
duplicates for existing declarations, persist `income_coverage_start =
last_declaration_date + 1`, and subsequent syncs must keep it fixed.
