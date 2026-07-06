# Plan: `regenerate` command (rebuild an erroneous declaration)

## Goal

New CLI command `ibkr-porez regenerate <ID>` that deletes an erroneous
declaration, restores the carryforward ledger to the state before it
existed, and re-runs the standard generation for its period from
already-stored transactions.

Use cases:
1. A calculation bug was found and fixed — the declaration must be rebuilt.
2. The user forgot to record the tax-authority assessment (recognized loss)
   for a prior PPDG-3R before the next PPDG-3R was generated, so the next
   one was generated without part 7 / carryforward. After running `assess`
   on the prior declaration, `regenerate` on the next one rebuilds it with
   carryforward applied.

Only the most recent PPDG-3R can be regenerated: if any PPDG-3R with a
later period exists, the command errors (rebuilding an old declaration
would invalidate every later one through the carryforward chain; that
scenario is not supported). PP-OPO declarations are independent of each
other, so any PP-OPO can be regenerated regardless of age.

Out of scope: GUI button (CLI only), IBKR re-fetch (regeneration works from
locally stored transactions; if the bug was in import, the user re-runs
`sync`/`import` first), regenerating a PPDG-3R that has later PPDG-3R
declarations.

## CLI contract

```
ibkr-porez regenerate <declaration_id>            # dry run: print the plan, change nothing
ibkr-porez regenerate <declaration_id> --yes      # execute
ibkr-porez regenerate <declaration_id> --yes --force
```

- Single declaration id (no bulk/stdin — this is a destructive command).
- Without `--yes`: print what would be deleted (id, type, period, status,
  attachment count, whether it has a carryforward vintage) and the period
  that would be regenerated, then exit 0. This is the confirmation
  mechanism — no interactive prompt.
- `--force` is required when the declaration to be deleted is not in
  `Draft` status (deleting it loses submitted/paid/assessment history).
  `--force` is also passed into report generation as the existing
  `force` flag (same meaning as in `sync`/`report`).
- The regenerated declaration comes back as a new `Draft` declaration; the
  user must `submit`/`assess` it again.

## Semantics

### Target is PP-OPO (`DeclarationType::Ppo`)

Delete that declaration, then regenerate income declarations for its own
date: `generate_income_reports(storage, nbs, config, holidays,
decl.period_start, decl.period_end, force)` and save each report that is
not a duplicate. No ledger interaction. (`period_start == period_end ==
declaration_date` for PP-OPO — see `src/sync.rs` `generate_and_save_income`.)

### Target is PPDG-3R (`DeclarationType::Ppdg3r`)

1. **Guard**: if any other PPDG-3R declaration has
   `period_start > target.period_start`, bail:
   "later PPDG-3R declarations exist; regenerating {id} would invalidate
   them — not supported". (Carryforward flows forward in time, so an
   error in the target contaminates all later gains declarations.)
2. Two ledger effects of the target must be undone
   (see `specs/spec-capital-loss-carryforward.md`):
   - **Its consumption**: metadata key `carryforward_sources` (array of
     `{"vintage_id": String, "amount_used": String-decimal}`, written by
     `CarryforwardApplication::apply_to_metadata` in `src/report_gains.rs`).
     Reverse it by calling `storage.apply_carryforward_consumption` with the
     same sources but negated `amount_used` (the rollback in
     `src/sync.rs:274-284` already uses this pattern).
   - **Its own vintage** `CF-{declaration_id}` (created by `assess` when a
     loss was recognized): remove it from the ledger. Since no later
     PPDG-3R exists (guard above), the vintage cannot have been consumed;
     if `remaining_loss_rsd != recognized_loss_rsd` anyway (manually edited
     ledger), bail before mutating anything. When a vintage is deleted,
     print a warning at the end: "assessment data was deleted; re-run
     assess on the regenerated declaration".
3. Regenerate the target's own half-year period
   (`target.period_start`..`target.period_end`).

## Implementation steps

### Step 1 — fix declaration id assignment (prerequisite bug)

`src/sync.rs` `save_declaration` (~line 385):

```rust
let next_id = existing.len() + 1;
```

After deleting a declaration from the middle of the list (PP-OPO case)
this collides with a surviving declaration and `storage.save_declaration`
would silently overwrite it (it replaces by matching `declaration_id`).
Change to max+1:

```rust
let next_id = existing
    .iter()
    .filter_map(|d| d.declaration_id.parse::<usize>().ok())
    .max()
    .unwrap_or(0)
    + 1;
```

Note: when the deleted declaration was the last one, max+1 reuses its id.
That is intended and harmless — the old record is gone and everything that
referenced it (vintage `CF-{id}`, attachments dir, XML files) is removed by
the same regenerate run.

Unit test in `src/sync.rs` tests: save 3 declarations, delete id "2",
generate again, assert new id is "4" and declaration "3" is untouched.

### Step 2 — storage: delete a declaration

`src/storage.rs`, next to `save_declaration`:

```rust
pub fn delete_declaration(&self, declaration_id: &str) -> Result<()>
```

Load `DeclarationsFile`, remove the entry whose `declaration_id` matches
(bail with "declaration {id} not found" if absent), save. Follow the
load/filter/save pattern of `save_declaration` (src/storage.rs:289).

Tests in `tests/test_storage.rs`: deletes the right one, keeps others,
errors on unknown id.

### Step 3 — sync.rs: expose per-period generation

The target's period is usually not the period `sync` would generate today
(`determine_gains_period` derives it from the current date), so regenerate
needs generation functions that take an explicit period.

Extract from `generate_and_save_gains` (src/sync.rs:222):

```rust
pub fn generate_and_save_gains_for_period(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    period_start: NaiveDate,
    period_end: NaiveDate,
    output_dir: &Path,
    force: bool,
) -> Result<Vec<Declaration>>
```

It contains everything currently in `generate_and_save_gains` after the
`determine_gains_period` call (duplicate check, carryforward consumption +
rollback-on-save-failure, `save_declaration`). `generate_and_save_gains`
becomes a thin wrapper calling it with `determine_gains_period(end_period)`.

Similarly extract the per-report save loop of `generate_and_save_income`
(src/sync.rs:298) into:

```rust
pub fn generate_and_save_income_for_period(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    start: NaiveDate,
    end: NaiveDate,
    output_dir: &Path,
    force: bool,
) -> Result<Vec<Declaration>>
```

Behavior of `sync` must not change — existing tests stay green.

### Step 4 — core logic: `src/regenerate.rs`

New module (registered in `src/lib.rs`), so it is testable without the CLI
and reusable by the GUI later.

```rust
pub struct RegenerationPlan {
    pub to_delete: Declaration,
    pub period_to_generate: (NaiveDate, NaiveDate),
    pub deletes_vintage: bool,   // CF-{id} exists and will be removed
}

pub fn plan_regeneration(storage: &Storage, declaration_id: &str) -> Result<RegenerationPlan>;

pub fn execute_regeneration(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    plan: &RegenerationPlan,
    force: bool,
) -> Result<Vec<Declaration>>;  // created declarations
```

`plan_regeneration` (no mutation):
1. `storage.get_declaration(id)` or bail "declaration {id} not found".
2. If Ppdg3r: bail if any other Ppdg3r declaration has
   `period_start > target.period_start` (guard from Semantics).
3. If `CF-{id}` exists (`storage.find_carryforward_vintage`) and
   `remaining_loss_rsd != recognized_loss_rsd`, bail:
   "carryforward vintage CF-{id} has been consumed; fix the ledger
   manually first".
4. `period_to_generate = (target.period_start, target.period_end)`.

`execute_regeneration`:
1. Parse `carryforward_sources` from the target's metadata (absent/empty →
   skip); negate each `amount_used`;
   `storage.apply_carryforward_consumption`.
2. If `CF-{id}` exists: re-check the guard from `plan_regeneration` step 3,
   then `storage.remove_carryforward_vintage`.
3. Delete files, best-effort (`let _ = fs::remove_file(...)`):
   - `decl.file_path` (XML in declarations dir),
   - copy in the effective output dir
     (`config::get_effective_output_dir_path(config)` joined with the
     `file_path` basename),
   - attachments dir `storage.declarations_dir().join(&decl.declaration_id)`
     (remove_dir_all).
4. `storage.delete_declaration(&decl.declaration_id)`.
5. Call `generate_and_save_gains_for_period` (Ppdg3r) or
   `generate_and_save_income_for_period` (Ppo) for `period_to_generate`.
   Treat a gains error whose message contains "no taxable sales" as success
   with no declarations (after the bug fix the period may genuinely have
   nothing to declare) — same string-match convention as
   `generate_declarations` (src/sync.rs:121). Other errors propagate; the
   deletion is kept and the user can rerun `sync` later (`is_duplicate`
   prevents doubles).
6. Do NOT touch `last_declaration_date`.

The status/`--force` gate lives in the CLI layer (step 5), not here.

### Step 5 — CLI: `src/cli/regenerate.rs` + wiring

`src/main.rs`: add to `Commands`:

```rust
/// Delete an erroneous declaration and regenerate it from stored data
Regenerate {
    declaration_id: String,
    /// Actually execute (without this flag only prints the plan)
    #[arg(long)]
    yes: bool,
    /// Allow deleting a non-Draft declaration; also passed to report generation
    #[arg(long)]
    force: bool,
},
```

Dispatch: `cli::regenerate::run(&declaration_id, yes, force)`. Register the
module in `src/cli/mod.rs`.

`run` mirrors `cli/sync.rs::run` scaffolding:
1. `load_config_or_exit`, `make_storage`, calendar, `make_nbs`. Reuse
   `init_calendar_with_sync` by moving it (and its threshold helper +
   their tests) from `src/cli/sync.rs` to `src/cli/mod.rs` as `pub(crate)`.
2. `plan_regeneration`.
3. Print the plan with `output` helpers: the deletion line
   (id, type, period, status, "N attachments", "has carryforward vintage")
   and the period to regenerate.
4. If `to_delete.status != Draft` and !force → bail
   "declaration {id} is {status}; pass --force to delete it".
5. If !yes → print "Dry run. Pass --yes to execute." and return Ok.
6. `execute_regeneration`, print created declarations with
   `output::success` lines (full table rendering not required).
7. If `deletes_vintage`, print the warning about lost assessment data
   (see Semantics).

### Step 6 — tests

`tests/test_regenerate.rs` (mirror the setup helpers from `src/sync.rs`
tests around line 745: temp-dir `Storage`, seeded transactions, seeded
vintages, offline `NBSClient`/`HolidayCalendar` as done in
`carryforward_test_setup`; if those helpers are private, replicate them):

1. **Forgot-assessment scenario**: H1 PPDG-3R with recognized loss (vintage
   CF-x untouched), H2 generated WITHOUT consumption; regenerate H2 →
   old H2 gone, new H2 consumes the vintage
   (`carryforward_used_rsd` > 0, vintage remaining reduced).
2. **Consumption reversal**: target consumed vintage CF-origin; regenerate
   it with the vintage seeded differently (or transactions changed) →
   remaining restored before regeneration, then re-consumed by the new
   declaration; ledger ends consistent with the new declaration's
   `carryforward_sources`.
3. **Vintage removal**: target had its own unconsumed vintage → vintage
   removed from ledger, warning flag (`deletes_vintage`) set.
4. **Later-declaration guard**: PPDG-3R with a later-period PPDG-3R present
   → `plan_regeneration` bails, nothing deleted.
5. **Consumed-vintage guard**: target's own vintage has
   `remaining != recognized` (manually seeded) → bails, nothing deleted.
6. **PP-OPO**: regenerate one income declaration from the middle of the
   list → only it is deleted and recreated, new id does not collide with
   survivors; ledger untouched.
7. **Files**: XML file and attachments dir removed from disk.
8. CLI-level test in `tests/test_e2e_cli.rs` style: dry run changes
   nothing; `--yes` without `--force` on non-Draft fails.

### Step 7 — docs and spec

- Add a short `regenerate` entry to `docs/{en,ru,rs,rs-cyr,uk}/src/usage.md`
  next to `revert` (follow the existing per-language wording style).
- Spec: add a "Regeneration" section to
  `specs/spec-capital-loss-carryforward.md` stating the business rule only:
  a declaration can be deleted and regenerated from stored data; for
  PPDG-3R this is allowed only when no later PPDG-3R exists; the ledger
  balances it consumed are restored, its own vintage is removed, and its
  assessment must be re-entered.

## Verification

```
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Manual smoke test: with a data dir containing an H1 loss declaration,
`assess` it, then `regenerate` the H2 declaration and check `show` displays
part-7/carryforward figures and `carryforward` shows reduced remaining.
