# Plan: replace the income watermark with a fixed rescan window

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
  already moved past their date. This is the everyday case, not an edge case;
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

## Design

Drop the watermark. Every sync rescans a fixed window and skips the groups that
already have a declaration. Rescanning is safe because the skip is keyed on the
income group, not on a stored boundary — late data inside the window is picked
up on the next run, whatever the reason it was late.

Three supporting changes make that work:

- **dedup by group key, not filename.** A filename key freezes the generated
  filename format forever and misses declarations whose `file_path` is `None`.
- **per-group problems become notices, not batch aborts.** One group missing a
  rate, or one group without withholding tax, must not take the other groups
  down with it.
- **a group with no withholding tax is held for a bounded wait, then declared
  with a zero foreign-tax credit.** Fiscally safe (the full 15% is declared as
  due, no credit is claimed) and it removes the permanent-failure mode.

### Accepted limitations

Stated here so they are deliberate, and repeated in the spec:

- income older than the window is not generated. `sync --lookback N` is the way
  to reach it. The app says nothing about it — the main causes of undeclared
  history (the zero-WHT abort and the missing-rate abort) are what this plan
  fixes, so it stops accumulating.
- a withholding tax arriving more than `WHT_WAIT_DAYS` after its income is not
  credited and the foreign-tax credit is forfeited. Pre-existing.
- a group that never resolves its exchange rate leaves the window after 45 days
  and is forgotten silently. `Currency` is a closed enum of USD/EUR/GBP/RSD
  (src/models.rs:39-44), all published daily by NBS, and `get_rate` already
  looks back 10 days, so this needs NBS to be unreachable for 45 consecutive
  days — during which the notice is shown on every single sync and the fetch is
  almost certainly failing through its own channel. Today the same group is lost
  after one day, not 45.

## Constants

```rust
const WHT_WAIT_DAYS: i64 = 20;   // src/report_income.rs
```

Replaces both hardcoded `Duration::days(7)` occurrences (src/report_income.rs:115
pool bound, :237 matching window). The PP-OPO filing deadline is 30 days from
the income date, so waiting 20 leaves 10 days to file. The deadline itself is
not a code constant — it is the reason for this number and belongs in the spec.

The value goes from 7 to 20 **because** zero-WHT groups are now finalized
instead of bailing: at 7 days the year-end pattern already in the test suite
(dividend 2025-12-24, its US tax 2026-01-02) silently forfeits the credit, which
was harmless while the group bailed and is a permanent loss once it is declared.

`DEFAULT_LOOKBACK_DAYS = 45` (src/sync.rs:20) stays as it is and becomes the
only window. No other new constants.

A group is declared **as soon as its matched tax is non-zero**, not at the end of
the wait — this keeps today's latency.

## Changes

### 1. report_income.rs

New public types:

```rust
pub enum IncomeNoticeKind { MissingRate, WaitingForWht }

pub struct IncomeNotice {
    pub kind: IncomeNoticeKind,
    pub text: String,      // rendered once, at construction
}

#[derive(Default)]
pub struct IncomeReportBatch {
    pub reports: Vec<IncomeReport>,
    pub notices: Vec<IncomeNotice>,
}

pub type GroupKey = (NaiveDate, String, String);  // (date, SYMBOL upper, income_type)

pub struct IncomeGenOptions {
    pub today: NaiveDate,      // wait boundary; never read Local::now() in this module
    pub force_rates: bool,     // nearest-cached NBS fallback (get_rate_or_force)
    pub skip_wht_wait: bool,   // manual override, see step 5
}

pub fn generate_income_reports(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    start: NaiveDate,
    end: NaiveDate,
    declared: &HashSet<GroupKey>,
    opts: &IncomeGenOptions,
) -> Result<IncomeReportBatch>
```

This replaces the current `build_income_groups` → `build_income_reports` split,
where the first propagates the first rate error and the second bails on the
first zero-WHT group.

The `Result` is **not** for per-group problems — those are notices. It is kept
because `HolidayError::MissingYear` must stay a hard error: it means the app has
run out of holiday data, and generating declarations with wrong holiday handling
is worse than failing. It arrives through `holidays.is_serbian_holiday(target)?`
inside `nbs.get_rate` (src/nbs.rs:94), which returns `anyhow::Result`, so
distinguish it with `e.downcast_ref::<HolidayError>()` before mapping any other
rate failure to a `MissingRate` notice. Save/IO errors also stay `Err`.

That guarantee holds for `force_rates: false` only. In force mode
`get_rate_or_force` (src/report_income.rs:297-300) already swallows *every*
`Err` into a nearest-cached-rate lookup, so `MissingYear` never reaches the
downcast and degrades to a `MissingRate` notice. Accepted rather than fixed:
`--force` is an explicit "use approximate rates" request, and the group it
touches is the one the user asked to push through anyway.

**Withholding-tax attribution.** `find_withholding_tax` currently sums *every*
candidate in range (src/report_income.rs:239-277). Widening the matching window
from 7 to 20 days makes that unsafe on its own: two payments of one symbol closer
together than the window would each claim the other's tax. Fix it with a rule,
built once as a `tax -> owning group` map, not with a window cap.

A tax matches an income transaction when:

- income is a dividend and the tax description contains the income's entity name
  or ISIN as captured by `ENTITY_RE` (src/report_income.rs:22-23) — **or**, if
  no entity match exists for that tax, the tax's `symbol` equals the income's
  `symbol`. Entity/ISIN matches take precedence over symbol matches, preserving
  today's two-tier behaviour (`test_wht_matched_by_entity_isin_not_symbol`,
  `test_wht_fallback_to_symbol_match`);
- income is interest and the currencies are equal.

Among matching incomes with `income.date <= tax.date`, the tax belongs to the one
with the **greatest date**. A tax with no preceding match belongs to nobody.

Build the map over the incomes in `[start, end + WHT_WAIT_DAYS]` — wider than the
range that gets reports. The two boundaries are not symmetric:

- **below `start`** the restriction is safe. If a tax's true owner lies before
  `start`, no income at or after `start` can have a date `<= tax.date` — such an
  income would be later than the owner and would itself be the owner. The tax is
  left ownerless rather than misattributed.
- **above `end`** it is not. The pool holds taxes up to `end + WHT_WAIT_DAYS`,
  and an income just past `end` can own one of them. Cut the incomes off at
  `end` and that tax falls to the last matching income *inside* the window — a
  wrong credit, not a dropped one. `report income --half 1` is the everyday
  case: a dividend on Jul 1 with its tax on Jul 2 hands its credit to the
  previous payment of the same symbol in June. Widening the match window from 7
  to 20 days widens this hole with it.

`end + WHT_WAIT_DAYS` is exactly enough and no more: it is the largest date the
pool can hold, so no income beyond it can precede a pool tax. The pool itself
does not grow — keep the filter at src/report_income.rs:110-117, changing its
upper bound from `end + 7` to `end + WHT_WAIT_DAYS`.

Groups past `end` take part in ownership only. They are never reported and never
produce a notice.

**Pipeline:**

1. Bucket income transactions in `[start, end + WHT_WAIT_DAYS]` by group key
   `(date, symbol|currency uppercased, income_type)`. The key comes from the raw
   transaction — **no exchange rate needed yet.**
2. Build the tax → owning group map over **all** of those groups, before any
   filtering. A group that is already declared, or that sits past `end`, still
   owns its taxes. Drop it first and its tax falls through to an earlier group
   with no claim to it — a double credit on the second payment of a symbol
   inside one wait window.
3. Walk the groups in `[start, end]` only. Key in `declared` → skip silently.
   Never regenerate.
4. Otherwise resolve rates and sum the taxes whose owner is this group and whose
   date falls in `[date, min(opts.today, date + WHT_WAIT_DAYS)]`:
   - any rate error (income or tax leg) → `MissingRate` notice, continue with
     the other groups;
   - matched total is zero, `date + WHT_WAIT_DAYS >= opts.today`, and
     `!opts.skip_wht_wait` → `WaitingForWht` notice, retried on the next sync;
   - matched total is zero and the wait has elapsed → generate with zero credit;
   - otherwise generate.

The wait must **not** consult `force_rates`: forcing an early zero-credit
declaration is strictly harmful (tax arriving inside the window would become
uncreditable once the declaration exists) and saves at most `WHT_WAIT_DAYS`
against a 30-day deadline.

The wait is measured from the *income date*, not from when the transaction first
appeared in storage. Income imported long after its date therefore finalizes
immediately with a zero credit rather than waiting, which is the correct outcome.

### 2. sync.rs

`determine_income_period` (src/sync.rs:218-238) becomes pure — no `Storage`
argument, no watermark read, no `Option`:

```rust
fn determine_income_period(
    end_period: NaiveDate,
    options: &SyncOptions,
) -> (NaiveDate, NaiveDate)
```

1. `options.forced_lookback_days` → `start = end_period - (lookback - 1)`;
2. else `start = end_period - (DEFAULT_LOOKBACK_DAYS - 1)`.

Delete the watermark-advance block (src/sync.rs:175-181) and its
`set_last_declaration_date` call. `last_declaration_date` stays in
`DeclarationsFile` with both its accessors (src/storage.rs:389-401) so the file
keeps loading in either direction; after this change nothing outside tests
touches it (tests/test_storage.rs:257 exercises the accessors,
tests/test_python_compat.rs:123 sets the field in a fixture). The accessors are
`pub` on a lib crate, so nothing goes dead. **src/storage.rs and src/models.rs
are not modified at all** — no new persisted state, no migration.

`generate_and_save_income` builds the declared-key set once, before the loop.
Today `is_duplicate` calls `storage.get_declarations` per group (src/sync.rs:347
→ :380 → whole-file read and parse, src/storage.rs:339-358). Key each existing
PP-OPO declaration by
`(period_start, metadata["symbol"].to_uppercase(), metadata["income_type"])`.
Deliberately **not** by filename stem, which is what `is_duplicate` does today:
a filename key freezes the generated filename format forever (changing it makes
every existing declaration invisible to the dedup check) and misses any
declaration whose `file_path` is `None`. Every declaration this app writes
carries `symbol` and `income_type` (src/report_income.rs:38-55), so a
declaration missing either key is hand-edited: log it at `warn` and leave it out
of the set. `is_duplicate` stays for PPDG-3R (src/sync.rs:261).

`opts.today` is `end_period + Duration::days(1)`, not a second `Local::now()`
call — `generate_declarations` derives `end_period` from `Local::now()` at
src/sync.rs:116 and a second read can land on the other side of midnight.

`SyncResult` (src/sync.rs:28-38): `income_error: Option<String>` is replaced by

```rust
pub income_notices: Vec<IncomeNotice>,
```

`income_skipped` stays and keeps its meaning: no reports and no notices at all.

Delete the error-string matching in `generate_declarations` (src/sync.rs:161-172):
`"no NBS exchange rate"` and `"withholding tax"` no longer arrive as errors.
Errors that still propagate (declaration save, IO, missing holiday year) keep the
`"PP-OPO generation failed"` context and remain the only thing that reaches
`set_last_sync_issue` as a failure.

`IncomeOutcome` (src/sync.rs:311-314) is replaced by a struct carrying
`created: Vec<Declaration>`, `notices: Vec<IncomeNotice>`, `empty: bool`.

### 3. cli/sync.rs

`print_sync_result` prints every notice with `output::dim` — both kinds are
informational and resolve themselves on a later sync. Print the per-group text,
which is the detail a CLI user wants.

Replace `"Income report generation failed: {err_msg}"` (src/cli/sync.rs:119) —
it no longer fits a per-group list. Reword `"no income in period"`
(src/cli/sync.rs:126) to `"no undeclared income in period"`, which is what the
flag now means.

### 4. gui

**No new banner, no new `App` field, no dismissal.** The notices are recomputed
from scratch on every sync and disappear only when their cause does, which is
exactly the contract of the existing `last_sync_issue` pill: one short message
that the next successful sync replaces or clears. `handle_sync_done`
(src/gui/app.rs:576-580) already does both — only the message source changes.

Build that message by aggregating `r.income_notices` **by kind**, e.g.
`"2 income groups waiting for withholding tax, 1 without an NBS rate"`. Do not
join the per-group lines: `status_pill` (src/gui/main_window.rs:119-128) holds
one short string, and an NBS outage produces a notice for every group in the
window.

**A fetch error keeps its priority.** `handle_sync_done` sets the pill from
`fetch_error` first and never looks at the income side (src/gui/app.rs:565-572);
leave that branch alone. Both messages want the same one-line pill, and a broken
IBKR connection is the more actionable of the two — it also *causes* income
notices, since a report that never arrived cannot carry the withholding tax
anyone is waiting for. Consequence to accept: while the fetch is failing the
notices are invisible in the GUI. They are still in `SyncResult`, still printed
by the CLI, and reappear in the pill on the first sync that fetches cleanly.

Nothing is lost across syncs: while a cause holds, the pill is re-set every sync;
when a `WaitingForWht` cause clears, a declaration was created, and created
declarations are counted by `pending_new_declarations`, which persists and is
cleared only by its own Close button.

### 5. cli/report.rs

`run_income` (src/cli/report.rs:169-176) needs the new signature: pass an
**empty** `HashSet<GroupKey>` (this command writes to a destination directory and
does not consult declaration state) and
`IncomeGenOptions { today: Local::now().date_naive(), force_rates: force,
skip_wht_wait: force }`. Keep the `match` on `Result` — `MissingYear` and IO
still arrive as errors. Print `batch.notices` after the reports.

`skip_wht_wait: force` preserves today's escape hatch: `--force` is currently the
only way to produce XML for a zero-WHT group (it bypasses the bail at
src/report_income.rs:181).

Two behaviour changes to document rather than hide:

- for a period whose groups are all older than `WHT_WAIT_DAYS`, plain
  `report income` now *emits* zero-credit XML where today it errors out;
- `--force` means different things in the two commands: rates only in `sync`,
  rates *and* skipping the wait in `report income`. Say so in the command help
  and in the docs.

### 6. Tests

**Rewrite in `src/sync.rs`** (`determine_income_period` is now pure and total,
so these lose their `Storage` and their `Option`):

- `test_income_period_no_last_date`, `test_income_period_with_last_date`,
  `test_income_period_last_date_equals_end` → one
  `income_period_is_fixed_window`: `start == end - 44`. Their
  `set_last_declaration_date` calls (src/sync.rs:496, :513, :525) go with them.
- `test_forced_lookback_overrides_start` → keep, minus `Storage`.

**New in `src/sync.rs`:**

- `late_arriving_income_gets_declared_on_next_sync`: sync once → add income+tax
  transactions dated inside the already-scanned range → sync again →
  declaration created. The regression test for the whole plan.
- `missing_rate_group_does_not_block_others`: rate present for one of two dates
  → one declaration, one `MissingRate` notice.
- `deleted_income_declaration_is_regenerated`: create, `delete_declaration`,
  sync → recreated (inside the window).
- `declaration_without_file_path_is_not_regenerated`: PP-OPO whose `file_path`
  is `None` but whose metadata carries symbol/type → no duplicate. Guards the
  dedup-key change.
- `no_watermark_is_written`: after a sync, `get_last_declaration_date()` is
  unchanged.

**New in `tests/test_reports.rs`.** All of these pass an explicit `opts.today`:

- `zero_wht_waits_while_window_open`: dividend within `WHT_WAIT_DAYS` of `today`,
  no tax → `WaitingForWht`, no report.
- `zero_wht_wait_ignores_force_rates`: same with `force_rates: true` → still
  waiting.
- `zero_wht_finalizes_after_wait_elapses`: dividend older than `WHT_WAIT_DAYS`,
  no tax → report with `porez_placen_drugoj_drzavi = 0`.
- `wht_arriving_within_window_is_credited`: waiting on day 1, tax added, next
  run generates with the credit.
- `wht_belongs_to_nearest_preceding_payment`: two dividends of one symbol 15 days
  apart, one tax after the second → credited to the second only. Guards the
  7 → 20 widening.
- `wht_owner_after_window_end_is_not_stolen`: `end` = 2026-06-30, dividends of
  one symbol on 06-25 and 07-01, tax on 07-02. One report (06-25) with a **zero**
  credit; the 07-01 group owns the tax and is not reported. Guards the
  `end + WHT_WAIT_DAYS` ownership range — cut the incomes at `end` and June
  silently takes July's credit.
- `wht_of_declared_group_is_not_recredited`: dividends of one symbol on 06-05 and
  06-15, tax on 06-16, `declared` holding the 06-15 key. One report (06-05) with
  a **zero** credit. Guards building the ownership map before the `declared`
  filter — filter first and 06-05 claims a tax that is already spent.
- `already_declared_group_is_skipped`: key present in `declared` → no report.

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

`tests/test_gui.rs:55` and the `SyncResult` literal in the `src/cli/sync.rs` test
helper (:143-155) need the new field; `print_income_error` (:202) becomes a
notices case. **No `tests/test_storage.rs` changes** — storage is untouched.

### 7. Docs

New `specs/spec-income-declarations.md`, rules only:

- an income declaration is generated for every undeclared income group inside the
  rescan window; a group that already has a declaration is never regenerated;
- a group without withholding tax is held for a bounded wait and then declared
  without a foreign-tax credit;
- a withholding tax belongs to the nearest preceding income of the same key;
- a group missing an exchange rate is reported and retried on the next sync;
- a deleted declaration is rebuilt by the next sync while its group is inside the
  window;
- the accepted limitations listed under *Design* above.

`specs/spec-auto-sync.md` gets one sentence: income notices reuse the sync-issue
status line and are recomputed from scratch on every sync.

`docs/*/src/usage.md` (`sync` section) — the same in user terms, plus
`sync --lookback N` as the way to generate for income older than the window. The
`delete` → `sync` rebuild promise is now true within the window and needs
`--lookback` beyond it: usage.md:340, `src/cli/delete.rs:27` and
`src/gui/delete_dialog.rs:60`. Five locales are kept in parallel (en, ru, rs,
rs-cyr, uk).

## Out of scope

- PPDG-3R generation (already period-based with dedup, no watermark).
- Reporting income older than the window, and any mechanism to dismiss such a
  report. See *Accepted limitations*.
- Recording which withholding taxes a declaration credited, and reporting a tax
  that arrives after its group was declared. Attribution alone already prevents
  two groups from crediting one tax; recording the claim would only add the
  late-tax report, at the cost of new declaration metadata and a dependency on
  `transaction_id` stability, which csv → xml supremacy (src/storage.rs:570-582,
  :612-618) does not provide.
- Splitting a single withholding tax across several income groups.
- `.abs()` on withholding-tax amounts (src/report_income.rs:254, :263, :272)
  counts an IBKR reversal as a second credit. Pre-existing and adjacent to the
  rewritten code, but a separate bug with its own fixture needs; left as is
  deliberately rather than by omission.

## Verification

```
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Manual, on real data:

1. two syncs in a row over the same window create no duplicates;
2. adding an income transaction dated inside the window (e.g. via `sync --file`
   with an older Flex XML) produces its declaration on the next sync — this is
   the bug being fixed;
3. a dividend younger than `WHT_WAIT_DAYS` with no tax yet shows the waiting
   notice; once its tax arrives, the next sync declares it *with* the credit;
4. an Irish-domiciled ETF dividend no longer fails the whole sync, and after the
   wait elapses it is declared with a zero credit;
5. `sync --lookback 365` generates for income older than the window.
