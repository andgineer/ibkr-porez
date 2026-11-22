# Plan to reconcile carryforward when recording a PURS assessment

## Goal

When a PURS decision recognizes zero capital gain and a new capital loss, `assess` should restore any prior loss the declaration provisionally used. The existing `--gain`, `--loss`, and `--tax` options are sufficient for this case. The [capital loss carryforward specification](spec-capital-loss-carryforward.md) describes how the app records each deduction against its originating decision.

## Worked example

Declaration 11 estimated a gain of 9,488.47 RSD and used that amount from the 2025 loss. PURS recognized a new loss of 3,945.67 RSD and used zero of the 2025 loss. The first assessment entry added the new loss to the ledger but left the provisional deduction in place.

| Remaining loss (RSD) | After the first assessment entry | Per PURS |
|---|---:|---:|
| 2025 decision (CF-1) | 34,982.83 | 44,471.30 |
| 2026 decision (CF-11) | 3,945.67 | 3,945.67 |
| Total available | 38,928.50 | 48,416.97 |

## Implementation

1. Preserve the submitted declaration and its original carryforward calculation. Record the assessed use of prior losses separately, with an amount for each originating decision. An explicitly empty list means the assessment used zero prior loss. An absent list in an older record means the ledger still reflects the original deductions.
2. Reconcile the ledger in the assessment logic shared by the CLI and GUI. Compare the deductions currently reflected in the ledger with those established by the decision. Restore each difference to its original loss balance and record the new period loss under its own decision. For declaration 11, restore 9,488.47 RSD to CF-1 and retain the existing 3,945.67 RSD in CF-11. A tax-only assessment of zero cannot establish prior-loss usage because an earlier loss may have covered a gain.
3. Make updates recoverable and safe to repeat. Save the corrected balances and assessment together, or provide recovery if a write fails between the two files. Reapplying an assessment must use the last recorded deductions and apply only the difference. If the user later deletes a declaration, reverse only the deductions still reflected in the ledger. Keep the original estimate available for comparison in declaration details.

## Verification

- Reproduce declaration 11 with a finalized legacy record and the 38,928.50 RSD balance shown above. Apply the assessment twice. After each run, verify the remaining amount for each decision, the 48,416.97 RSD total, and zero tax due.
- Test provisional deductions from two earlier decisions. Verify that each decision receives its own amount back and that unrelated declarations retain their balances.
- Test a tax-only assessment, a retry after an interrupted write, and deletion after reconciliation. Verify that each path preserves the ledger total expected from the recorded deductions.

## Apply to declaration 11

After the regression tests pass, back up the live declaration and ledger files. Run the corrected command and inspect the saved assessment and balances.

```sh
ibkr-porez assess 11 --gain 0 --loss 3945.67 --tax 0
ibkr-porez show 11
ibkr-porez carryforward
```

The correction is complete when the two remaining balances match the PURS column above and the declaration retains its original estimate, decision reference, date, and attachments.
