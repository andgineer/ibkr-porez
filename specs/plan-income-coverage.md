# Plan: income coverage — match by distribution, net by sign, amend when it changes

## Problem

Three defects in the PP-OPO path, in order of fiscal severity.

**1. `.abs()` on withholding tax** (src/report_income.rs:254, :263, :272) counts an
IBKR reversal as a second credit instead of cancelling the first. US funds whose
distributions turn out to be interest-related dividends under IRC §871(k) —
Treasury ETFs above all — have their withholding reversed, so the pair nets to
zero. With `.abs()` the same amount is credited twice, and the declaration claims
a foreign tax credit that does not exist.

**2. The watermark loses late-arriving income.** Declarations are generated for
`[last_declaration_date + 1, yesterday]` (`determine_income_period`,
src/sync.rs:218) and the watermark advances to yesterday (src/sync.rs:175-181) on
every run that does not error. Income reaching storage after the watermark passed
its date is silently and permanently skipped — on a failed fetch (src/sync.rs:53-70
generates anyway), on a missing NBS rate (src/sync.rs:163-165 collects and
continues), and on ordinary Flex reporting lag.

**3. A zero-withholding group aborts the whole sync** (src/report_income.rs:181).
One security that legitimately never withholds fails every sync, forever, and
blocks every other group. Defect 1 masks this today: a reversal pair looks like
tax, so the bail does not fire.

Underneath all three, withholding tax is attributed by date proximity plus a
description substring (`find_withholding_tax`, src/report_income.rs:229-278) —
a heuristic that needs a window constant, can give one tax to two incomes, and
always returns an answer.

## Design

### Match by distribution, not by date

```rust
fn distribution_key(txn: &Transaction) -> Option<String>
```

An ordered fallback chain, no other logic:

1. `actionID` from the Flex report — the identifier of the corporate action.
   A dividend and every withholding row belonging to it, reversals included,
   share it.
2. the description prefix up to and including `PER SHARE`. IBKR builds the tax
   description by appending `- US TAX` to the income's, and the per-share rate
   distinguishes one distribution of a security from the next.
3. the currency paired with the `CREDIT INT FOR <MON>-<YYYY>` token —
   broker-interest rows carry no `actionID`, so this is their permanent key, not
   a fallback. The currency comes from the transaction's own field, present on
   both sides; the tax description does not name it, and the token alone would
   give one month's USD tax to that month's EUR interest as well.

The token is a substring, not a prefix: the income reads `USD CREDIT INT FOR
JUL-2025`, its tax `WITHHOLDING @ 30% ON CREDIT INT FOR JUL-2025` and its
cancellation `CANCEL WITHHOLDING ON CREDIT INT FOR JUL-2025`.

Matching is a hash join on that key, summing **signed** amounts so a reversal
cancels its original. A transaction with no key joins nothing — two keyless rows
must never be treated as a match.

**Income is summed with its sign too**, within the group and for the same reason:
a reversed dividend must cancel its original rather than be counted twice.
`.abs()` on the income amount (src/report_income.rs:139) goes with the three on
the tax side.

Verified on the real reports under `raw_reports/` in the configured `data_dir`:
24 of 24 withholding rows in `ibkr-porez-data-2026-05-08.xml` (2025-06-13 …
2026-03-31) and 13 of 13 in `flex_report_1770123204.xml` resolve to exactly one
owner, none ambiguous, none unmatched, no income row without a key. Two payments
of one symbol 20 days apart carry different `actionID`s; a cancellation arriving
15 days after its interest still finds its owner, because the date takes no part
in the join.

**Settled: `action_id` is recorded going forward and never backfilled.** Storage
and its merge are not touched. `is_identical_to` (src/models.rs:461) does not
compare new fields and the merge skips identical rows (src/storage.rs:594), so a
re-fetched report never writes `action_id` onto transactions already in
`transactions.json` — they keep `None` for good. This is the decision, not a
limitation to engineer around, and it is not to be reopened:

- backfilling changes nothing that matters. The field serves income declarations
  only; capital-gains declarations never consult it, and every dividend group
  older than this version already has its declaration;
- it cannot be done completely in any case. A CSV activity statement carries no
  such field, and a Flex Query does not reach back far enough to re-supply those
  years, so `None` is permanent for part of the history no matter what;
- the alternative is worse. Forcing a rewrite means reaching into the merge — XML
  supremacy, key matching, CSV handling, the most delicate code in the repo — to
  buy a property branch 2 already provides in three lines.

**Branch 2 is therefore permanent and must not be dropped as dead code.** It is
the key for every row without an `actionID`: everything already in storage,
everything from CSV, and anything IBKR emits without one. `transactions.json`
currently holds eight undeclared income groups in that state (2025-07-02 …
2025-12-04, all older than the app's first declaration), and `sync --lookback` —
*Verification* step 6 — is what reaches them. Without branch 2 they would each be
declared with a zero credit while their withholding sits in the same file.

Verified across all 45 income and withholding rows in `transactions.json`: branch
2 alone resolves every one of them to exactly one owner, no bucket spanning two
distributions, no row without a key.

This removes: the matching window and its constant, the tax→owner map, the
entity/ISIN-then-symbol two-tier fallback, the "nearest preceding income" rule,
and the tax pool's date bounds.

### No detection machinery for a broken join

If the key ever stops working, groups see zero tax and are declared with a zero
credit — the taxpayer overpays. That failure announces itself: dividend
declarations that have always been zero suddenly demand payment, which is a
stronger signal than any notice the app could print, and it errs toward
overpaying rather than underpaying. No unmatched-tax notice, no ambiguity check,
no extra state.

### Two horizons, and only one of them is a window

Drop the watermark. Two separate spans replace it, and conflating them is the
mistake to avoid.

**The creation window** — `DEFAULT_LOOKBACK_DAYS`, or `--lookback N`. A group
inside it with no declaration gets one. A group outside it with no declaration is
left alone. Late data inside the window is picked up next run, whatever made it
late.

**The scan horizon** — how far back transactions are read at all, so that an
already-declared group can be re-checked. Withholding for a distribution of tax
year Y is corrected between January and March of Y+1, capped by the 1042-S filing
(`specs/spec-transaction-sources.md`), so the horizon is **1 January of the
previous calendar year**, or the creation window's start if `--lookback` reaches
further back:

```rust
fn scan_start(end_period: NaiveDate, creation_start: NaiveDate) -> NaiveDate
// min(NaiveDate::from_ymd(end_period.year() - 1, 1, 1), creation_start)
```

Tight, not arbitrary: a distribution paid 2 January 2025 can be corrected
31 March 2026, and on that date the rule yields 1 January 2025. Confirmed against
the archived reports in `flex-queries/` — every §871(k) reversal for tax year 2025
appeared between 1 and 11 February 2026, 49 to 159 days after its income, all
beyond any 45-day window and all inside this horizon.

Groups are keyed on the income group, never on a stored boundary and never on a
generated filename.

### Hold a group without tax for a bounded wait

The wait applies to a group that matched **no withholding row at all** — the
answer has not arrived yet. A group that matched rows which net to zero has its
answer: the tax was withheld and reversed, and it is declared immediately with a
zero credit. That is the normal shape of every §871(k) distribution, so
conflating the two would delay the most common case by `WHT_WAIT_DAYS` for
nothing.

A group with no matched row is held until its income is
`WHT_WAIT_DAYS` old, then declared with a zero credit — the full 15% is declared
as due, which is fiscally safe, and the permanent-failure mode is gone. A group
is declared as soon as any withholding row matches it, so the common case keeps
today's latency.

The wait is not load-bearing: matching does not depend on it, and a tax that
changes later is caught by the amendment path. A wrong constant costs a delayed
declaration, never a wrong credit.

### Amend when a declared group changes

When a declared group's source amounts differ from what was declared, generate an
измењена пријава. No range is involved and none is needed: a reversal reaches the
income it reverses through the distribution key, and the difference shows up in
the comparison. The scan horizon above is the only bound, and it exists to avoid
reading transactions that provably cannot change — not to limit which declarations
may be amended.

Comparison is against amounts recorded in the declaration's metadata **in the
income currency**, never in RSD — a shifted exchange rate must not look like a
change.

Every declared group inside the horizon is recomputed and compared on every sync.
Deliberately not driven by "which rows arrived in this fetch": a reversal already
in storage would never be new again, so a fetch that failed, a sync that stopped
between import and generation, or an import through `sync --file` would lose it
permanently — the same failure class as defect 2.

Nothing from before this change is ever touched: only declarations created by the
new code carry those metadata fields, so earlier ones are invisible to the
mechanism. That is the whole cutoff — no install timestamp, no migration flag, no
version stamp, and no pass over historical periods.

### CSV-sourced transactions never produce an income declaration

The CSV importer exists to load history older than a Flex query reaches, and its
only purpose is the purchase side of the capital-gains calculation. Income it
carries is not a declaration source: `generate_income_reports` filters out
`is_csv_sourced()` (src/models.rs:457) transactions along with the type and date
filter, for both income and withholding rows.

This is a behaviour fix, not just documentation — today a CSV dividend inside the
period would produce a PP-OPO. It also settles the CSV descriptions, which do not
follow the Flex shape at all (`tests/resources/complex_activity.csv` carries
`Dividend Payment` and `Tax`) and would never carry an `actionID`: they are simply
never asked to match.

### Income predating this version

Nothing here is built to work for dividend income that arrived before this
version, and nothing needs to be. The version being replaced generated a
declaration as soon as the income appeared, so every such group already has one.

The one consequence, accepted: a withholding reversal arriving **after** the
upgrade for a dividend that arrived **before** it carries an `actionID` its
dividend does not have, so it keys on branch 1 while the dividend keys on branch 2
and the two do not join. Such a reversal is ignored — the declaration keeps the
credit it was created with, and no amendment is generated. That declaration
predates the metadata the amendment path compares, so it is invisible to that path
regardless. See *Settled* above for why this is not closed by backfilling.

### Accepted limitations

- income older than the window is not generated; `sync --lookback N` reaches it.
- a group that never resolves its exchange rate leaves the window after
  `DEFAULT_LOOKBACK_DAYS` and is forgotten. `Currency` is a closed enum of
  USD/EUR/GBP/RSD (src/models.rs:39-44), all published daily by NBS, and
  `get_rate` already looks back 10 days.
- declarations predating this change are never amended.
- income predating this version, and a late reversal belonging to it, are out of
  scope; see above.
- an amendment is generated, not filed; the taxpayer submits it.

## Constants

```rust
const WHT_WAIT_DAYS: i64 = 7;   // src/report_income.rs
```

How long a group for which no withholding row matched at all is held before being
declared with a zero credit. A group whose matched rows net to zero never waits. Replaces both `Duration::days(7)` occurrences
(src/report_income.rs:115, :237), whose meanings — pool bound and matching window
— both disappear. The PP-OPO deadline is 30 days from the income date, so 7
leaves 23 to file. The deadline is not a code constant; it is the reason for this
number and belongs in the spec.

`DEFAULT_LOOKBACK_DAYS = 45` (src/sync.rs:20) stays and becomes the only window.

## Changes

### 1. ibkr_flex.rs

Add to `XmlCashTransaction` (src/ibkr_flex.rs:404-419) and carry into the
`Transaction` built by `convert_cash_transaction` (src/ibkr_flex.rs:282-307):

```rust
#[serde(rename = "@actionID")]
action_id: Option<String>,
```

Trades do not need it.

### 2. models.rs

```rust
#[serde(default)]
pub action_id: Option<String>,
```

on `Transaction` (src/models.rs:198-220). `#[serde(default)]` keeps existing
`transactions.json` loading unchanged. `is_identical_to` is **not** modified —
see *Design*.

`Transaction` derives no `Default`, so the new field breaks all 31 struct
literals across 13 files — `src/fetch.rs`, `src/ibkr_csv.rs`, `src/ibkr_flex.rs`,
`src/models.rs`, `src/sync.rs` and seven test files including
`tests/test_python_compat.rs`. Mechanical `action_id: None`, but it is the bulk of
the diff.

### 3. report_income.rs

```rust
#[derive(Default)]
pub struct IncomeReportBatch {
    pub reports: Vec<IncomeReport>,
    pub notices: Vec<String>,      // rendered at construction, one per group
}

pub type GroupKey = (NaiveDate, String, String);  // (date, SYMBOL upper, income_type)

pub struct IncomeGenOptions {
    pub today: NaiveDate,    // wait boundary; never read Local::now() in this module
    pub force_rates: bool,   // nearest-cached NBS fallback (get_rate_or_force)
}
```

Notices are plain strings. There is no kind enum: the two remaining cases —
a missing rate and a group waiting for its tax — are both informational, both
resolve themselves on a later sync, and nothing branches on which is which.

`generate_income_reports` takes `scan_start`, `creation_start`, `end`, the
already-declared groups with their recorded source amounts, and
`IncomeGenOptions`. Two starts, one end: `scan_start` decides what is read and
compared, `creation_start` decides what may be created. It replaces the
`build_income_groups` → `build_income_reports` split, where the first propagates
the first rate error and the second bails on the first zero-WHT group.

`Result` is not for per-group problems — those are notices. It is kept because
`HolidayError::MissingYear` must stay a hard error: it means the app ran out of
holiday data, and generating with wrong holiday handling is worse than failing.
It arrives through `holidays.is_serbian_holiday(target)?` inside `nbs.get_rate`
(src/nbs.rs:93), which returns `anyhow::Result`, so distinguish it with
`e.downcast_ref::<HolidayError>()` before mapping any other rate failure to a
notice. Save/IO errors also stay `Err`. The guarantee holds for
`force_rates: false` only — in force mode `get_rate_or_force`
(src/report_income.rs:297-300) already swallows every `Err` into a cached-rate
lookup. Accepted: `--force` is an explicit request for approximate rates.

**Pipeline:**

1. Index every `WithholdingTax` transaction dated `>= scan_start`, excluding
   CSV-sourced rows, by `distribution_key`. Rows whose key is `None` are dropped
   from the index, not bucketed together — two keyless rows are not a match.
2. Bucket income transactions in `[scan_start, end]`, excluding CSV-sourced rows,
   by group key, summing their signed amounts. The key comes from the raw
   transaction, so already-declared groups never touch NBS.
3. Sum the signed taxes whose key matches an income in the group, and keep
   whether *any* row matched — that, not the sum being zero, is what the wait
   in step 5 tests.
4. Already declared: source amounts unchanged → skip silently; changed → emit a
   report with `amends` set to the original's id. The group's date is irrelevant
   here; only the horizon of step 2 bounds this.
5. Not declared **and `date >= creation_start`** — a group older than that is
   dropped silently, it was only read so that step 4 could check it. Resolve
   rates:
   - any rate error → notice, continue with the other groups;
   - no withholding row matched and `date + WHT_WAIT_DAYS >= opts.today` →
     notice, retried next sync;
   - no withholding row matched and the wait elapsed → generate with a zero
     credit;
   - otherwise generate — including when the matched rows net to zero, which
     needs no wait.

The wait is measured from the income date, not from when the transaction appeared
in storage, so income imported long after its date finalizes immediately. It does
not consult `force_rates`: forcing an early zero-credit declaration is strictly
harmful and saves at most `WHT_WAIT_DAYS` against a 30-day deadline.

**Metadata: the source amounts in the broker's own currency.**
`IncomeReport::metadata()` (src/report_income.rs:33-82) today records only RSD
figures, which cannot be reconciled against what IBKR shows on screen — the user
sees dollars there and dinars in `show`, with an exchange rate in between. Record
the group's own numbers next to them:

- `gross_income_ccy` — gross in the income currency;
- `foreign_tax_paid_ccy` — net withholding in the income currency, after signed
  netting;
- `currency` — the code, so the pair is unambiguous;
- `exchange_rate` — the NBS rate used, which closes the loop between the two.

These serve two purposes at once. They are what step 4 compares — never the RSD
figures, because a shifted rate must not look like a change — and they are what
makes a declaration checkable against the broker.

Step 4 compares the **formatted `{:.2}` strings**, as stored, not values parsed
back into `Decimal`. That fixes the threshold at 0.01 implicitly and correctly:
anything finer never reaches the declaration anyway.

A change in `foreign_tax_paid_ccy` produces an amendment even when
`porez_za_uplatu` does not move, which happens when withholding exceeds 15% and
the credit is already capped. The declared foreign-tax figure changed, and the
filed declaration has to match reality.

Add all four to `METADATA_KEY_ORDER` (src/cli/show.rs:89-112), each beside its
RSD counterpart, so `show` prints them in place rather than sorting them into the
unordered extras at the end. The GUI details dialog
(src/gui/details_dialog.rs:76-79) iterates metadata as-is and needs no change.

**Amendments.** An amendment is an ordinary `IncomeReport` with one extra field:

```rust
pub amends: Option<String>,   // declaration_id being amended
```

No separate type and no separate list — an amendment is a declaration, so it
travels the same path, is saved by the same code, lands in the same output
directory and is counted the same way. The filename is
`ppopo-izmena-{sym}-{YYYY-MMDD}.xml`, matching the existing shape
(src/report_income.rs:212-214), so the two documents never collide on disk.

`amends` names the **newest** declaration of the group, not the first — see
*The declared-group map keys on the newest* below. A group's tax can change more
than once: IXUS 2025-12-19 carries `-12.25`, `+12.25` and `-12.23`.

### PP-OPO schema: what actually marks an amendment

Sources are recorded in `specs/spec-income-declarations.md`; do not re-derive
them. `PodaciOPrijavi` is an `xs:sequence`, so element order is fixed, and these
are the accepted enumerations as the schema states them:

```
KlijentskaOznakaDeklaracije   optional
VrstaPrijave                  required   1 | 3 | 4 | 5
ObracunskiPeriod              optional
DatumOstvarivanjaPrihoda      required
Rok                           required   1 | 2
DatumDospelostiObaveze        required
DatumObracunaKamate           optional
VrstaIzmenePrijave            optional   1 | 2 | 3 | 9
IdentifikatorPrijave          optional   unsignedLong, ≤ 19 digits
PoNalazuKontroleSuda          optional
OsnovIzmene                   optional   1 | 2 | 3
```

`VrstaPrijave` does **not** mark an amendment — it states when the return is
filed relative to its deadline. An amendment is marked by two elements appended
after `DatumDospelostiObaveze`, which is exactly where
`write_podaci_o_prijavi` (src/declaration_income_xml.rs:46-63) currently stops,
so nothing existing moves:

- `VrstaIzmenePrijave` = `1` — измена по члану 40. ЗПППА, the voluntary
  correction. `2` is by audit order and `9` is a storno; neither is ours.
  Confirmed against the schema, the rulebook and the ePorezi entry form, all
  three of which agree on `1`, `2` and `9`.
- `IdentifikatorPrijave` — the PURS number of the declaration being amended.
  Numeric, so `submit --number` must reject anything that is not up to 19
  digits. Optional in the schema, so it is simply omitted when unknown; ePorezi
  will not register the amendment without it and the taxpayer completes it there.

`PoNalazuKontroleSuda` and `OsnovIzmene` belong to audit and court cases. Never
written.

### VrstaPrijave states timeliness, and must stop lying

`VrstaPrijave` is hardcoded `"1"` today (src/declaration_income_xml.rs:52).
Per the PP-OPO rulebook, `1` is an општа пријава — filed **before or on** the due
date — and `3` is a пријава по члану 182б ЗПППА, filed after it has passed. Both
are confirmed against the schema, the rulebook and the ePorezi entry form. The
three sources diverge only on codes this app never writes; see
`specs/spec-income-declarations.md`.

Hardcoding `1` therefore asserts timeliness that may be false. It is already
wrong today for a declaration generated on day 40 of a 30-day deadline, and this
plan makes it routinely wrong: every amendment is by nature late, and every
declaration produced by `sync --lookback` over 2025 income is years late.

The rule is a date comparison, not a tax judgement, and both dates are already
computed:

```
VrstaPrijave = if opts.today <= DatumDospelostiObaveze { 1 } else { 3 }
```

`DatumDospelostiObaveze` is `next_working_day(income_date, holidays)` — income
plus 30 days, advanced past weekends and holidays (src/due_date.rs:10).

The rule can only err toward `1` — when the app generates in time but the
taxpayer files late — which is the assumption the app already makes. It never
claims timeliness for a return the app itself knows is overdue. Amendments need
no special case: generated months after the income, they come out as `3`
naturally.

`VrstaPrijave` and `VrstaIzmenePrijave` are independent — settled, do not
re-derive. Neither codebook references the other and neither value constrains the
other, so an amendment filed after the deadline writes `3` and `1` together and
the date comparison above is the whole rule. See `specs/spec-income-declarations.md`,
*Sources*, for the trap that makes this look otherwise.

`generate_income_xml` therefore takes `today: NaiveDate`, threaded from
`IncomeGenOptions.today`. **This is not optional for the golden tests:** without
it the function would have to read the clock, and every golden fixture — all
dated 2025-12 to 2026-03 — would flip to `3` the moment its deadline passed, and
back again for any fixture added with a recent date. Golden tests pass an
explicit date on or before the fixture's due date, so existing goldens keep
`VrstaPrijave = 1` and stay deterministic.

Interest is out of scope. A `3` return carries `DatumObracunaKamate` and interest
amounts; the app writes neither and leaves the `Kamata` block at zeros as it does
today. ePorezi computes and the taxpayer completes them, exactly as with
`IdentifikatorPrijave`.

### 4. sync.rs

`determine_income_period` (src/sync.rs:218-238) becomes pure — no `Storage`, no
watermark, no `Option`:

```rust
fn determine_income_period(end_period: NaiveDate, options: &SyncOptions)
    -> (NaiveDate, NaiveDate, NaiveDate)   // (scan_start, creation_start, end)
```

`creation_start = end_period - (lookback - 1)`, from
`options.forced_lookback_days` or `DEFAULT_LOOKBACK_DAYS`.
`scan_start = min(Jan 1 of end_period.year() - 1, creation_start)`.

Both are pure functions of `end_period` and `options` — no `Storage`, no
watermark, no `Option`.

Delete the watermark-advance block (src/sync.rs:175-181).
`last_declaration_date` stays in `DeclarationsFile` with both accessors
(src/storage.rs:388-401) so the file keeps loading in either direction; nothing
outside tests then touches it (tests/test_storage.rs:257,
tests/test_models.rs:151, tests/test_python_compat.rs:123,
tests/resources/declarations.json:411). The accessors are `pub` on a lib crate,
so nothing goes dead. **src/storage.rs is not modified.**

**The declared-group map keys on the newest.** Once an amendment exists, its
group has **two** declarations under the same `(period_start, symbol,
income_type)` key — the original and the amendment. Comparing against the
original would find the amounts still changed and emit a second amendment, then a
third, on every sync forever. The map therefore keeps, for each group key, the
declaration with the highest numeric `declaration_id`, and that is both what step
4 compares against and what `amends` points at. After an amendment the comparison
matches and nothing more is produced; a genuine second change produces exactly one
further amendment, which then references the first.

`generate_and_save_income` builds the declared-group map once, before the loop —
today `is_duplicate` re-reads and re-parses the whole declarations file per group
(src/sync.rs:347 → :380 → src/storage.rs:338-358). Key each existing PP-OPO by
`(period_start, metadata["symbol"].to_uppercase(), metadata["income_type"])`,
where `period_start` is the `Declaration` field, not the metadata string; the
value is the recorded source amounts, absent for declarations created before this
change. Deliberately not keyed by filename stem, which is what `is_duplicate`
does: that freezes the generated filename format forever and misses declarations
whose `file_path` is `None`. Every PP-OPO this app has ever written carries both
metadata keys, so no fallback is needed; a declaration missing one is hand-edited
and is logged at `warn` and skipped. `is_duplicate` stays for PPDG-3R
(src/sync.rs:261).

`opts.today` is `end_period + Duration::days(1)`, not a second `Local::now()` —
`end_period` comes from `Local::now()` at src/sync.rs:116 and a second read can
land past midnight.

`SyncResult` (src/sync.rs:28-38): `income_error: Option<String>` becomes
`income_notices: Vec<String>`. `income_skipped` keeps its meaning: no reports, no
notices. Delete the error-string matching (src/sync.rs:161-172). Errors that
still propagate keep the `"PP-OPO generation failed"` context.

`IncomeOutcome` (src/sync.rs:311-314) becomes a struct with `created`,
`notices`, `empty`.

Amendments go through `save_declaration` unchanged, like every other report: a
`Declaration` record of type `Ppo`, XML written to the declarations directory and
to the configured output directory, `amends` carried into its metadata. Two
documents really do go to PURS, and one declaration stays one file.

### 5. submit — declaration number

`submit` is bulk (`Vec<String>`, src/cli/submit.rs:6), so `--number <N>` is
accepted only when exactly one id is given; more than one is an error. Stored in
the declaration's `metadata`, which already holds `symbol` and `income_type` and
needs no schema change. In the GUI, an optional field in the submit confirmation.

`IdentifikatorPrijave` is `xs:unsignedLong` with at most 19 digits, so `--number`
rejects anything that is not 1–19 digits at parse time rather than writing XML
that fails validation at ePorezi.

### 6. cli/sync.rs

Print notices with `output::dim`. Replace `"Income report generation failed:
{err_msg}"` (src/cli/sync.rs:118-120); reword `"no income in period"`
(src/cli/sync.rs:126) to `"no undeclared income in period"`.

An amendment appears in the created-declarations list exactly like a first-time
declaration — same "created" message, same XML in the output directory, same
`list` / `show` / `submit` / `pay` handling. It is a declaration that has to be
filed; nothing about it is a side channel. The only addition is a hint line
carrying the two things that locate the original in the ePorezi table, where
every other column is uninformative — the date of income realization, and the
number when it was recorded at submit:

```
Amended PP-OPO: датум остваривања прихода 24.12.2025, SGOV
  original: declaration 003, number not recorded — find it by date
  credit 5225.01 → 0.00 RSD, now due 1306.01 RSD
```

### 7. cli/report.rs

`run_income` (src/cli/report.rs:169-176) passes an empty declared map — this
command writes to a destination directory and does not consult declaration state
— and `IncomeGenOptions { today: Local::now().date_naive(), force_rates: force }`.
Keep the `match` on `Result`. Print `batch.notices` after the reports.

`--force` now means the same thing in both commands: approximate rates, nothing
else. The old zero-WHT escape hatch (src/report_income.rs:181) disappears with
the bail it existed to bypass — a group older than `WHT_WAIT_DAYS` is emitted
with a zero credit without any flag. One behaviour change to document: for a
period whose groups are all older than the wait, plain `report income` now emits
zero-credit XML where today it errors out.

### 8. gui

No new banner, no new `App` field, no dismissal. Notices are recomputed every
sync and disappear when their cause does — the contract of the existing
`last_sync_issue` pill, which `handle_sync_done` (src/gui/app.rs:573-583) already
sets and clears. Only the message source changes: `"3 income groups pending"`
from `r.income_notices.len()`. Do not join the lines — `status_pill`
(src/gui/main_window.rs:120-127) holds one short string and an NBS outage
produces a notice per group. The detail is in the CLI.

A fetch error keeps its priority: `handle_sync_done` sets the pill from
`fetch_error` first and never looks at the income side (src/gui/app.rs:565-572).
Leave that branch alone — a broken connection is the more actionable message and
it causes income notices anyway.

Amendments are declarations, so they arrive through `created_declarations` and
are counted by `pending_new_declarations` like any other.

## Tests

**Rewrite in `src/sync.rs`:**

- `test_income_period_no_last_date`, `test_income_period_with_last_date`,
  `test_income_period_last_date_equals_end` → one `income_period_is_fixed_window`
  asserting `creation_start == end - 44`. Their `set_last_declaration_date` calls
  (src/sync.rs:496, :513, :525) go with them.
- `test_forced_lookback_overrides_start` → keep, minus `Storage`, asserting
  `creation_start`.

**New for the horizon, in `src/sync.rs`:**

- `scan_horizon_is_previous_calendar_year` — `end = 2026-07-26` → `scan_start ==
  2025-01-01`, and `end = 2026-01-02` → `2025-01-01` too.
- `scan_horizon_follows_a_deeper_lookback` — `--lookback 3650` pushes
  `scan_start` back to `creation_start`, not the other way round.
- `income_outside_creation_window_is_not_declared_but_is_compared` — a group
  inside the horizon and outside the window, already declared, whose tax then
  changes, produces an amendment; the same group undeclared produces nothing.
  The two horizons in one test.

**New in `src/sync.rs`:**

- `late_arriving_income_gets_declared_on_next_sync` — sync, add income+tax dated
  inside the already-scanned range, sync again, declaration created. The
  regression test for defect 2.
- `missing_rate_group_does_not_block_others`.
- `deleted_income_declaration_is_regenerated` — inside the window.
- `declaration_without_file_path_is_not_regenerated` — PP-OPO with
  `file_path: None` but metadata carrying symbol/type.
- `no_watermark_is_written`.
- `predating_declaration_is_never_amended` — a declaration without the new
  metadata whose tax then changes produces nothing. Guards the cutoff.

**New in `tests/test_reports.rs`,** all with an explicit `opts.today`:

- `wht_matched_by_action_id` — two dividends of one symbol inside one wait window,
  each with its own tax under its own `actionID` → each credited its own.
- `wht_matched_by_description_when_action_id_absent` — income and tax both with
  `action_id: None`, matched on the `PER SHARE` prefix. Covers a distribution
  IBKR emits without an `actionID`, and a distribution wholly predating the
  upgrade. A mixed pair — one side `None`, the other `Some` — is deliberately not
  tested, because it is out of scope.
- `wht_matched_by_interest_token`.
- `interest_tax_does_not_cross_currencies` — USD and EUR interest for the same
  month, one USD tax → credited to the USD group only.
- `reversed_income_nets_to_zero` — a dividend and its reversal in one group give
  zero gross, not `2X`.
- `wht_reversal_nets_to_zero` — `-X` and `+X` under one key give zero, not `2X`.
  The regression test for defect 1.
- `late_reversal_produces_amendment` — and a variant with the reversal dated five
  months after its dividend, the shape the archived reports actually show.
- `amendment_compares_source_currency_not_rsd` — same amounts, different NBS rate
  → no amendment.
- `metadata_carries_source_currency_amounts` — `gross_income_ccy`,
  `foreign_tax_paid_ccy`, `currency` and `exchange_rate` present and matching the
  broker's figures, not the converted ones.
- `no_wht_finalizes_after_wait_elapses` — credit `0`.
- `no_wht_waits_while_window_open`, and the same with `force_rates: true` still
  waiting.
- `netted_wht_does_not_wait` — a reversal pair inside the wait window is declared
  at once with a zero credit, not held. Distinguishes "no answer yet" from "the
  answer is zero".
- `already_declared_group_is_skipped`.
- `csv_income_produces_no_declaration` — a `csv-` dividend with a `csv-`
  withholding row inside the period yields no report.
- `amendment_is_not_regenerated_on_the_next_sync` — sync, change the tax, sync,
  sync again: exactly one amendment, not two. The regression test for the
  declared-group map keying on the newest declaration.
- `second_change_produces_a_second_amendment_referencing_the_first` — the IXUS
  shape: `-X`, then `+X`, then `-Y`.

**New in `tests/test_xml.rs` / `tests/test_golden_xml.rs`:**

- `vrsta_prijave_is_1_before_the_due_date` and
  `vrsta_prijave_is_3_after_the_due_date` — `today` on the due date and one day
  past it.
- `amendment_writes_izmena_elements` — `VrstaIzmenePrijave = 1`, and
  `IdentifikatorPrijave` present only when a number was recorded, in schema
  order after `DatumDospelostiObaveze`.
- `submit_number_rejects_non_numeric_and_over_19_digits`.

**Existing `tests/test_reports.rs` work** — 12 call sites (lines 438, 508, 554,
605, 653, 695, 739, 783, 841, 872, 926, 991) need the new signature and an
explicit `today` past their fixtures; none may read the real clock. Two need
more:

- `test_zero_wht_force_false_errors` (:852) asserts the removed `Err` and becomes
  `no_wht_finalizes_after_wait_elapses`;
- `test_wht_not_found_beyond_7_day_window` (:616) tests a window that no longer
  exists. Its fixture — dividend 2025-12-24, tax 2026-01-02 — is the year-end
  gap, so rewrite it as `wht_across_year_end_is_credited`, asserting the tax **is**
  credited despite the 9-day gap.

**Golden files.** Every `generate_income_xml` call site in the golden tests gains
an explicit `today` on or before the fixture's due date, so `VrstaPrijave` stays
`1` and the goldens do not drift with the calendar. Passing the real clock would
make them depend on the date the suite runs.

`tests/resources/golden-003-ppopo-sgov-2025-1224.xml` records
`PorezPlacenDrugojDrzavi = 5225.01` with `PorezZaUplatu = 0.00` — a reversal pair
double-counted by `.abs()`. Fixing defect 1 turns it into `0.00` / `1306.01`.
Regenerate the affected goldens and read the diff: it is the proof the bug is
gone, not noise to wave through.

`tests/test_gui.rs:55` and `:594` (`poll_sync_done_with_income_error`, which
asserts the exact pill text) and the `SyncResult` literal in the `src/cli/sync.rs`
test helper (:143-155) plus `print_income_error` (:202) need the new field.
**No `tests/test_storage.rs` changes.**

## Docs

`specs/spec-income-declarations.md` and `specs/spec-transaction-sources.md`
already exist — written alongside this plan, because the rules outlive it and this
file gets deleted once implemented. Between them they hold: the distribution key
and its three branches, signed netting on both sides, the interest key's currency,
the two horizons and why they differ, the wait and why a netted zero does not
wait, the amendment rule and its cutoff, the CSV rule, and the accepted
limitations. Nothing to add there during implementation — verify the code matches
them instead.

`specs/spec-auto-sync.md` gets one sentence: income notices reuse the sync-issue
status line and are recomputed from scratch on every sync.

`docs/*/src/ibkr.md` states the CSV importer's purpose plainly where it describes
the import: it supplies purchase history older than a Flex query reaches, for the
capital-gains calculation, and never produces income declarations.

`docs/*/src/usage.md` (`sync`, `submit`, `delete`) — the same in user terms, plus
`sync --lookback N` for older income, the optional declaration number on
`submit`, and what to do when an amendment appears. The `delete` → `sync` rebuild
promise is true within the window and needs `--lookback` beyond it: usage.md:340,
src/cli/delete.rs:27, src/gui/delete_dialog.rs:60. Five locales (en, ru, rs,
rs-cyr, uk).

## Out of scope

- PPDG-3R generation (already period-based with dedup, no watermark).
- Reporting income older than the window.
- Amending declarations created before this change; any retroactive pass over
  historical periods.
- Income predating this version, including a withholding reversal that arrives
  after the upgrade for a dividend that arrived before it. See *Design*.
- Declaring income imported from CSV. See *Design*.
- Detecting or reporting a broken join. See *Design*.
- Submitting an amendment.
- Splitting one withholding tax across several income groups — the distribution
  key makes it meaningless.

## Verification

```
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Manual, on real data:

1. two syncs over the same window create no duplicates;
2. an income transaction dated inside the window (`sync --file` with an older
   Flex XML) produces its declaration on the next sync — defect 2;
3. a Treasury-ETF distribution whose withholding is reversed is declared with a
   zero credit and the full 15% due, immediately and without the wait — defect 1;
4. a dividend younger than `WHT_WAIT_DAYS` with no tax row at all shows the
   waiting notice; once its tax arrives the next sync declares it with the credit;
5. a reversal arriving after its declaration produces an amendment naming the
   income date;
6. `sync --lookback 365` generates for the eight income groups of 2025-07-02 …
   2025-12-04 that sit undeclared in `transactions.json`, each with the credit
   its stored withholding gives it — the branch-2 path end to end. Only two carry
   a non-zero credit (VOO 2025-07-02 → 0.52, VOO 2025-10-01 → 5.22); the other
   six net to zero and must come out with a zero credit and the full 15% due;
7. a plain `sync` reads back to 1 January of the previous year but creates
   nothing outside the 45-day window — those same eight groups stay undeclared
   until step 6 is asked for explicitly.
