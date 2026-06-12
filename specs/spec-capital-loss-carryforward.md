# Capital Loss Carryforward (PPDG-3R)

## Why

The application calculates capital gains and losses from transaction data,
but under Serbian tax law (Zakon o porezu na dohodak građana, Article 79) a
capital loss can only be carried forward to future tax years once it has been
officially recognized by the Tax Administration. The amount the authority
recognizes may differ from the application's own calculation due to CPI
adjustments, corrections, or the authority's own assessment methodology.

The system therefore distinguishes between application-calculated values and
tax-authority-recognized values. Carryforward is always based on recognized
values; calculated values are retained alongside them for audit comparison.

## Assessment

When a tax-authority ruling is received for a PPDG-3R declaration, the user
records it in a single step (CLI `assess` command, or the GUI assessment
dialog opened from a declaration row):

- assessed tax due
- recognized capital gain (if any)
- recognized capital loss (if any)
- a free-text reference and date for the ruling, plus optional notes
- whether the assessed tax has already been paid

At least one of assessed tax / recognized gain / recognized loss must be
given. A single assessment cannot recognize both a gain and a loss. Recognized
gain/loss only applies to PPDG-3R declarations (not PP-OPO income
declarations). Both the calculated and recognized gain/loss are preserved and
shown side by side wherever a declaration's details are displayed.

If the assessment recognizes a capital loss greater than zero, a carryforward
vintage is created (or updated) for that declaration's period. Carryforward
is **never** created from calculated losses — only from recognized losses.

## Carryforward Vintages and the Ledger

A carryforward vintage represents one originating declaration's recognized
loss: the recognized amount, the remaining (unused) balance, the originating
period, and the tax year in which it expires.

- Expiration: a loss recognized for a declaration whose period ends in tax
  year Y can be applied to declarations for tax years Y+1 through Y+5
  inclusive (`expiration_tax_year = Y + 5`). After Y+5 the vintage is expired
  and can no longer be consumed, though it remains visible in history.
- Vintages are stored in a dedicated, persistent ledger that is the
  authoritative source of carryforward balances. Balances are never
  reconstructed from declaration metadata during normal operation.
- If an assessment for a declaration is corrected before its vintage has been
  consumed at all, the vintage is updated in place (no duplicate). Once a
  vintage has any consumption, its recognized loss can no longer be changed
  through the assessment flow — correcting it requires manual editing of the
  ledger plus a future ledger-rebuild/validation tool (not yet built).

## Applying Carryforward to Future Gains

When a new gains declaration is generated:

1. The current period's gain/loss is calculated as before (including normal
   same-period netting), giving a calculated tax base.
2. All non-expired vintages with remaining balance, eligible for the
   declaration's tax year, are loaded.
3. Eligible vintages are applied oldest-origin-period-first (ties broken by
   creation order) until either the tax base reaches zero or the vintages are
   exhausted. This consumption order is fixed — no alternative ordering is
   supported.
4. The tax base is reduced by the amount consumed, producing an adjusted tax
   base and the resulting estimated tax.
5. If the calculated tax base is zero or negative, no carryforward is
   consumed and existing balances are left untouched.
6. A vintage may be partially consumed; its remaining balance stays eligible
   until fully consumed or expired.

Every declaration permanently records its own carryforward snapshot: the
opening and closing carryforward balances, the amount used, the adjusted tax
base, and which vintage(s) the consumption came from. This snapshot is never
recomputed for past declarations — sync operations only ever affect the
declaration currently being created. Re-running sync for a period that
already has a saved declaration does not consume carryforward again
(idempotent).

## Visibility

- CLI: `ibkr-porez carryforward` lists all vintages with their recognized and
  remaining amounts, expiration tax year, and status (Active / Exhausted /
  Expired). `ibkr-porez show` displays the opening/used/adjusted/closing
  carryforward figures, the recognized vs. calculated gain/loss, and the
  audit trail of which vintages a declaration drew from. The gains report
  preview shows only the used/adjusted/closing figures, since the full
  audit trail is recorded permanently once the declaration is created.
- GUI: the burger menu has a read-only "Capital loss carryforward..." view
  showing the same vintage list and statuses as the CLI command.

## Out of Scope

- Changes to FIFO lot-matching for gains/losses.
- CPI calculation logic.
- OCR or automatic parsing of official tax-authority documents.
- PP-OPO income declarations.
- Automatic recomputation/rebuild of the carryforward ledger — any such tool
  must be explicit and user-initiated, never run implicitly during sync.
