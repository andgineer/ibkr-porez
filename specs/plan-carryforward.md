# Capital Loss Carryforward Plan

## Context

This is a convenience and completeness feature for `PPDG-3R` capital gains reporting.

The application already calculates gains and losses from transaction data. However, Serbian tax law allows capital losses to be carried forward only after they are officially recognized by the Tax Administration.

The tax authority may approve amounts that differ from the application's calculations due to CPI adjustments, corrections, or assessment decisions.

Therefore the system must distinguish between:

* application-calculated values
* tax-authority-recognized values

Carryforward calculations must always use recognized values when available.

---

## Goals

Implement support for:

* tax-authority assessments attached to declarations
* recognized capital loss carryforward
* 5-year loss expiration
* automatic application of eligible carryforward losses to future gains
* complete audit trail of carryforward creation and consumption

---

## Scope

In scope:

* persist tax authority assessment data
* persist recognized capital loss vintages
* apply carryforward losses to future gain periods
* track remaining balances
* preserve historical snapshots on declarations
* manual entry of assessment data

Out of scope:

* FIFO matching changes
* CPI calculation logic
* OCR of tax rulings
* automatic parsing of official documents
* PP-OPO income declarations

---

## Design Principles

### Tax Authority Is The Source Of Truth

Application calculations are preliminary.

Whenever assessment data exists:

* recognized values override calculated values
* carryforward creation uses recognized values only
* tax reporting screens should display both values

### Immutable History

Historical declarations must not be silently modified.

Carryforward consumption should be recorded as snapshots at declaration creation time.

### Explicit Recalculation

Recomputation must be user initiated.

Sync operations must never automatically rewrite historical carryforward usage.

---

## Data Model

### Declaration Assessment Data

Extend declaration metadata with optional assessment information.

```text
assessment_date
assessment_reference
assessment_notes

assessed_tax_rsd

recognized_capital_gain_rsd
recognized_capital_loss_rsd
```

All three monetary fields (`assessed_tax_rsd`, `recognized_capital_gain_rsd`, `recognized_capital_loss_rsd`) are entered together in a single assessment command and stored atomically.

Purpose:

* preserve official ruling results
* explain differences between calculated and recognized values
* support future audits

Example:

```text
Calculated gain/loss:  -22,575.17 RSD  (loss)
Recognized gain/loss:  -44,471.30 RSD  (loss)
Assessed tax:               0.00 RSD
```

Both the application-calculated values and the authority-recognized values must be retained. The recognized value is the amount used for carryforward and for any display showing the official result. The calculated value is kept for audit comparison only.

---

### Carryforward Vintage

Add a dedicated model:

```text
id

origin_declaration_id

assessment_reference

origin_period_start
origin_period_end

recognized_loss_rsd

remaining_loss_rsd

created_at
expiration_tax_year

notes
```

Definitions:

* recognized_loss_rsd = loss approved by tax authority
* remaining_loss_rsd = unused balance
* expiration_tax_year = last year in which loss may be used

---

### Carryforward Ledger

Persist a separate ledger:

```text
capital_losses.json
```

The ledger is the authoritative carryforward store.

Carryforward balances must not be reconstructed from declaration metadata during normal operation.

---

## Declaration Snapshot Fields

Store carryforward usage on every declaration.

```text
opening_carryforward_rsd
carryforward_used_rsd
closing_carryforward_rsd

carryforward_sources
```

Where:

```text
carryforward_sources = [
  {
    vintage_id,
    amount_used
  }
]
```

This guarantees historical traceability.

---

## Workflow

### 1. Declaration Creation

Generate declaration normally.

Store:

```text
calculated_gain_rsd
calculated_loss_rsd
```

No carryforward is created yet.

The declaration is still awaiting tax authority assessment.

---

### 2. Assessment Entry

When a ruling is received, the user records all of the following in a single command:

```text
assessed_tax_rsd           # tax amount as determined by the authority
recognized_capital_gain_rsd  # gain recognized by the authority (may differ from calculated)
recognized_capital_loss_rsd  # loss recognized by the authority (may differ from calculated)
assessment_reference
assessment_date
```

All three monetary fields are optional individually, but at least one of `assessed_tax_rsd`, `recognized_capital_gain_rsd`, or `recognized_capital_loss_rsd` must be provided.

The recognized gain/loss amounts may differ from the application's own calculation due to CPI adjustments, corrections, or the authority's own assessment methodology. Both values (calculated and recognized) must be preserved and displayed side by side.

If:

```text
recognized_capital_loss_rsd > 0
```

create a new carryforward vintage.

Carryforward must never be created from calculated losses.

Only recognized losses create carryforward.

---

### 3. Future Gain Declaration

When generating a new gain report:

1. Calculate current-period gain/loss.
2. Perform normal same-period netting.
3. Load all active carryforward vintages.
4. Exclude expired vintages.
5. Apply eligible vintages oldest-first.
6. Reduce taxable base.
7. Record consumption snapshot.

Output should show:

```text
Calculated tax base

Carryforward applied

Adjusted tax base

Estimated tax
```

---

## Eligibility Rules

### Eligible Losses

A loss is eligible only if:

* it was recognized by the tax authority
* it has remaining balance
* it has not expired

### Expiration

Under Serbian tax law (Zakon o porezu na dohodak građana, Article 79), capital losses may be carried forward for up to **five tax years** following the year in which the loss was recognized.

Specifically:

* `expiration_tax_year = origin_tax_year + 5`
* A vintage created from a loss recognized in tax year Y may be used in declarations for years Y+1 through Y+5 inclusive.
* If a vintage is partially consumed across multiple declarations, the remaining balance continues to be eligible until the expiration year or until fully consumed.

Expired vintages:

* remain visible in history
* cannot be consumed

### Consumption Order

Consumption must always be deterministic.

Rule:

```text
Oldest eligible vintage first.
```

No alternative ordering is allowed.

### Negative Or Zero Base

If:

```text
current taxable base <= 0
```

then:

```text
carryforward_used = 0
```

Existing carryforward balances remain untouched.

### Partial Consumption

A vintage may be partially consumed.

The remaining balance stays available until:

* fully consumed
* expired

---

## Recompute Strategy

Normal sync operations must not modify:

* historical assessments
* carryforward balances
* carryforward consumption snapshots

Provide a future maintenance tool for:

* ledger rebuild
* validation
* mismatch detection

This action must be explicit and user initiated.

---

## Migration

Existing users start with:

```text
empty carryforward ledger
```

Users may manually enter prior assessments.

Future backfill tooling may suggest carryforward vintages from historical declarations, but user confirmation is required because:

```text
calculated loss != recognized loss
```

in many real-world cases.

---

## Test Plan

Add tests for:

* assessment entry creates carryforward vintage
* recognized loss differs from calculated loss
* carryforward uses recognized loss amount
* oldest-first consumption
* partial consumption
* multiple active vintages
* expiration after five years
* zero-gain periods
* loss-only periods
* historical snapshot preservation
* sync idempotency
* empty-ledger migration

---

## Files To Modify

```text
src/models.rs
src/storage.rs
src/report_gains.rs
src/declaration_manager.rs
src/sync.rs

plans/plan-rust-4-tax-reports.md
```

---

## Key Risks

* recognized amounts differ from calculated amounts
* accidental historical recomputation
* double consumption during sync
* storing carryforward only in metadata
* using calculated losses instead of recognized losses

---

## Recommendation

Implement after the primary gains-reporting workflow is stable.

The critical rule is:

Tax-authority-recognized losses are the only losses that may generate carryforward balances.

Whenever recognized values exist, they override application-calculated values for carryforward purposes.
