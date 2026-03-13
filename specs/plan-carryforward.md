# Capital Loss Carryforward Plan

## Context
This is a good-to-have enhancement, not a core correctness blocker for the current app. The existing Python app works without it because taxpayers can still file `PPDG-3R` and the tax authority ultimately issues the official ruling. The value of this feature is convenience and better preliminary calculations, especially for users who already have approved loss rulings from earlier periods.

The feature should be tracked as a separate follow-up from the main tax-report port.

## Why It Matters
Serbian capital losses can be carried forward for up to 5 years, but only once they are recognized by the tax authority. Today the app computes only the current declaration period and ignores prior approved loss balances. That means:

- current-period `PPDG-3R` previews can overstate tax due
- users must manually remember and apply prior loss rulings
- the app has no audit trail for what carryforward was consumed and what remains

Because the tax authority is the source of truth, the implementation should model approved loss carryforward, not just raw transaction losses.

## Scope
Implement carryforward only for `PPDG-3R` capital gains declarations.

In scope:

- persist approved carryforward loss vintages with 5-year expiry
- let gains reporting consume eligible prior losses in chronological order
- store per-declaration opening balance, amount used, and closing balance
- support manual/admin entry of tax-authority-approved loss amounts when a ruling arrives

Out of scope for this feature:

- changing current FIFO trade matching
- changing CPI/inflation handling done by the tax authority
- automatic OCR/parsing of official rulings
- reworking `PP-OPO` income flows

## Design Principles
- Treat tax-authority-approved loss amounts as canonical. Raw computed losses are not enough.
- Keep the feature additive: existing declarations and sync flows should still work if no carryforward data exists.
- Prefer a dedicated typed ledger over ad hoc `Declaration.metadata` keys for the core state.
- Make recomputation explicit. Do not silently rewrite historical declarations on re-sync.

## Proposed Data Model
Add a typed carryforward record in `src/models.rs`, for example:

- `origin_declaration_id`
- `origin_period_start`
- `origin_period_end`
- `recognized_loss_rsd`
- `remaining_loss_rsd`
- `used_loss_rsd`
- `expires_after_year`
- `assessment_reference` or ruling note fields

Add a top-level persisted collection in storage, separate from declarations, for example `capital_losses.json`, managed by `src/storage.rs`.

Keep declaration-level snapshots in `Declaration.metadata` for traceability:

- `opening_carryforward_rsd`
- `carryforward_used_rsd`
- `closing_carryforward_rsd`
- `carryforward_sources` (list of origin declarations or vintages used)

## Workflow Integration
### 1. Assessment Entry
Extend the future declaration manager flow in `src/declaration_manager.rs` so that when a `PPDG-3R` ruling is recorded, the app can also save:

- approved loss amount from the ruling
- whether the ruling created a new carryforward vintage
- optional attachment or reference to the ruling document

This should be the point where carryforward becomes available for later periods.

### 2. Gains Report Generation
Extend the future gains flow in `src/report_gains.rs`:

- compute current-period gains and losses as today
- load all non-expired carryforward vintages from storage
- apply them only after current-period same-period netting is computed
- consume oldest eligible vintages first
- record exactly which vintages were consumed and by how much

The resulting declaration should preserve both:

- raw same-period result
- carryforward-adjusted tax base shown to the user

## Eligibility Rules
Model these rules explicitly:

- only approved prior losses are eligible
- loss carryforward expires after 5 years
- zero or negative current tax base means no prior carryforward is consumed
- partially consumed vintages remain available with reduced balance
- fully consumed or expired vintages remain in history but are no longer active

Open legal and implementation note:

- within-period chronological loss-offset rules remain separate from this feature and should not be mixed into the carryforward ledger logic

## Sync and Recompute Strategy
Integrate with the future sync flow in `src/sync.rs` conservatively:

- new declarations should use the current carryforward ledger at creation time
- existing declarations should not be silently recalculated on re-sync
- if historical transactions change, require an explicit recompute or backfill flow rather than automatic mutation

Add a dedicated maintenance path later if needed:

- rebuild carryforward ledger from prior declarations and assessment data
- detect mismatches between declaration snapshots and current ledger balances

## Migration and Backfill
For existing users:

- start with an empty carryforward ledger by default
- allow manual creation of carryforward vintages from already issued rulings
- optionally add a later backfill helper that derives candidate loss vintages from historical `PPDG-3R` declarations, but requires user confirmation because raw declaration losses may differ from officially recognized losses

## Test Plan
Add tests around:

- creating a new carryforward vintage from an approved ruling
- applying one prior loss vintage to a later gain period
- partial consumption across multiple future periods
- expiry after 5 years
- multiple vintages consumed oldest-first
- zero-gain or loss-only periods leaving prior balances untouched
- re-sync idempotency: same period must not double-consume carryforward
- migration with no carryforward data present

## Main Files To Touch Later
- `src/models.rs`
- `src/storage.rs`
- `src/report_gains.rs`
- `src/declaration_manager.rs`
- `src/sync.rs`
- `plans/plan-rust-4-tax-reports.md`

## Key Risks
- official rulings may approve a different loss amount than the app's raw computation
- silent recomputation could corrupt lifecycle history or double-apply losses
- storing carryforward only in metadata would make validation and reuse brittle

## Recommendation
Implement this only after the main gains and declaration flow is stable. It improves completeness and user convenience, but the absence of this feature does not block basic filing because users can still rely on tax authority rulings and manual follow-up.
