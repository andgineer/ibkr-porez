# CPI Acquisition Price Indexation

## Status: REPLANNED — original XML change cancelled, replaced by read-only estimator

### Conclusion

The application's current behavior is correct. The PPDG-3R declaration requires the taxpayer to report the **nominal acquisition price** (actual price paid, converted at the NBS exchange rate on the purchase date). CPI indexation is applied by the tax authority, not by the taxpayer, when issuing the formal assessment decision (Решење). Implementing CPI adjustment in the XML output would cause the tax authority to double-index the acquisition price.

### Procedure confirmed by research

The Serbian capital gains tax procedure for individuals works as follows:

1. The taxpayer submits PPDG-3R with factual transaction data: ticker, dates, quantities, actual purchase and sale prices in the original currency, converted to RSD at the NBS rate.
2. The tax authority reviews the declaration, applies CPI indexation per Article 74 paragraph 8 of the Law on Personal Income Tax, and issues a formal Решење (administrative decision) that establishes the capital gain or loss.
3. The obligation to pay (or to record the carry-forward loss) arises from the Решење, not from the declaration itself.
4. There is no field for the taxpayer-calculated tax amount in the PPDG-3R form — this is by design: the form is informational, not a self-assessment.

This is confirmed both by the procedure experienced directly (2025 H2 ruling received five months after filing, with no penalty, establishing a carry-forward loss) and by independent sources:

- **Sigma Solution** (specialist tax advisory): *"Nabavnom cenom smatra se cena po kojoj je HoV stečena, tačnije cena koja je dokumentovana kao stvarno plaćena."* The article describes the capital gain formula without any mention of taxpayer-side CPI indexation, and explicitly states that the tax authority issues the Решење on the basis of the submitted declaration.  
  Source: https://sigmasolution.rs/porez-na-dobitke-od-prodaje-akcija-i-drugih-hartija-od-vrednosti-i-podnosenje-poreske-prijave/

- **Official PURS instruction for PPDG-3R**: the instruction page on the Tax Authority portal describes the declaration as the basis on which the tax authority conducts its review and issues the assessment.  
  Source: https://purs.gov.rs/sr/fizicka-lica/pregled-propisa/uputstva/4579/uputstvo-za-podnosenje-poreske-prijave-za-utvrdjivanje-poreza-na-kapitalne-dobitke-na-obrascu-ppdg--3r.html

### What the 2025 H2 ruling confirms

The ruling (`~/Library/CloudStorage/OneDrive-EPAM/Documents/ibkr-porez-data/declarations/capital_loss.md`) itself is titled "РЕШЕЊЕ О КАПИТАЛНОМ ГУБИТКУ" and states that the Tax Authority "спровела поступак контроле" (conducted a review procedure) based on the submitted PPDG-3R. The CPI-adjusted column (4.11 "Усклађена набавна цена") appears only in the ruling, not in the declaration form — meaning the taxpayer never submits CPI-adjusted values. The ruling was issued without any penalty despite arriving five months after filing, consistent with the obligation arising from the Решење rather than from the declaration deadline.

### Consequence for the codebase

No changes are needed. The current `<ns1:NabavnaCena>` output (nominal acquisition price at NBS rate) is correct. If the CPI-adjusted figure were submitted in the XML, the tax authority would apply CPI indexation on top of an already-adjusted value, producing a doubly-inflated acquisition price and an overstated loss.

---

## Estimated Tax Authority Ruling

### Goal

Add a read-only section to the gains report CLI output that shows what the tax authority's Решење is expected to calculate: the CPI-adjusted acquisition price per row and the resulting estimated capital gain or loss. This lets the taxpayer anticipate the ruling before it arrives and verify it row-by-row when it does.

The declaration XML is not changed. `TaxReportEntry`, `TaxCalculator`, and the existing report pipeline are not changed.

---

### Scope

In scope:

* `RzsClient` for fetching and caching monthly CPI data from РЗС
* CPI storage in `cpi.json`
* Standalone `cpi_factor()` function (independent of `TaxCalculator`)
* `estimated_ruling()` function that takes `&[TaxReportEntry]` and produces display-only rows
* New "Estimated ruling" section printed after the existing gains report table
* Unit tests verifying the CPI algorithm

Out of scope:

* Any changes to `<ns1:NabavnaCena>` or any other XML field
* Any changes to `TaxReportEntry`, `TaxCalculator`, or `declaration_gains_xml`
* Carryforward ledger, assessment workflow, PP-OPO declarations

---

### Algorithm

```
cpi_factor(purchase_date, sale_date) =
    product of monthly_cpi[Y][M]
    for all (Y, M) where year_month(Y, M) > year_month(purchase_date)
                     AND year_month(Y, M) <= year_month(sale_date)
```

where `year_month(date)` is the `(year, month)` pair compared lexicographically. For single-year holdings this reduces to the chain product of monthly factors from `purchase_month + 1` through `sale_month` inclusive.

**Same-month edge case:** when purchase and sale fall in the same calendar month the strict formula yields 1.0 (empty product). The tax authority applies the sale month's CPI as a deliberate policy in this case (confirmed from the 2025 H2 ruling). The estimator matches this by applying the sale month's factor when the months are equal.

**Missing data:** if the CPI cache does not contain a required month, use factor 1.0 for that month and mark the affected rows in the output with a warning marker (`?`) so the user knows the estimate is incomplete.

---

### CPI Data Source

РЗС publishes monthly chain index data (индекс потрошачких цена, месец/претходни месец). The canonical URL must be confirmed from the official РЗС statistics portal before implementation.

Pre-embed H2 2025 values as a compile-time fallback so the estimator works offline and unit tests run without network access:

| Month    | Monthly factor |
|----------|----------------|
| Sep 2025 | 1.0020         |
| Oct 2025 | 1.0050         |
| Nov 2025 | 1.0020         |
| Dec 2025 | 1.0010         |

For periods outside the pre-embedded range the application falls back to factor 1.0 per missing month with a warning. For correct estimates on older holdings, historical data must be fetched from РЗС.

---

### Data Model

#### CpiRate (new)

```text
year:   u16
month:  u8        (1–12)
factor: Decimal   (monthly chain index, e.g. 1.0020 for +0.20%)
```

#### EstimatedRulingRow (new, display only — not serialized)

```text
ticker:                String
sale_date:             NaiveDate
purchase_date:         NaiveDate
nominal_purchase_rsd:  Decimal
cpi_factor:            Decimal
adjusted_purchase_rsd: Decimal
estimated_gain_rsd:    Decimal
data_complete:         bool      (false if any CPI month was missing)
```

#### Storage

Add `cpi.json` alongside `rates.json`. Key format: `"YYYY-MM"`, value: factor as decimal string.

---

### Files

```text
src/models.rs       — add CpiRate struct
src/storage.rs      — add cpi.json path; add load_cpi / save_cpi methods
src/rzs.rs          — new: RzsClient; fetch, cache, and look up monthly CPI values;
                      cpi_factor(purchase_date, sale_date) -> Decimal
src/report_gains.rs — add estimated_ruling(&[TaxReportEntry], &RzsClient) -> Vec<EstimatedRulingRow>
src/cli/report.rs   — print the estimated ruling table after the main gains table
```

No other files are modified.

---

### Output Format

```
Estimated tax authority ruling (indicative — does not affect declaration)
─────────────────────────────────────────────────────────────────────────
Ticker  Purchased   Sold        Nominal RSD   CPI factor  Adjusted RSD  Est. gain/loss
SGOV    2025-08-04  2025-09-05    477,336.05      1.0020    478,290.92       -4,310.11
...
─────────────────────────────────────────────────────────────────────────
Estimated total gain/loss:  -44,471.30 RSD
Estimated tax base:               0.00 RSD
Estimated tax (15%):              0.00 RSD

Rows marked ? used factor 1.0 for months with no CPI data.
```

---

### Test Plan

* `cpi_factor` for single-month gap (purchase Aug, sale Sep → factor = Sep CPI only)
* `cpi_factor` for multi-month gap (purchase Aug, sale Dec → Sep × Oct × Nov × Dec)
* `cpi_factor` for multi-year holding (chain across year boundary)
* same-month edge case: sale month CPI applied once
* missing CPI month: factor 1.0 used, `data_complete = false`, no panic
* end-to-end: reproduce all 14 rows from the 2025 H2 ruling; expected values taken from `~/Library/CloudStorage/OneDrive-EPAM/Documents/ibkr-porez-data/declarations/capital_loss.md` (not committed to git)
* existing declaration tests must pass unchanged — `estimated_ruling` is additive

---

### Integration Verification

After implementation, run `ibkr-porez report` for H2 2025 against the ibkr-porez database and compare the printed "Estimated ruling" section with `~/Library/CloudStorage/OneDrive-EPAM/Documents/ibkr-porez-data/declarations/capital_loss.md` row-by-row:

* Column "Adjusted RSD" must match ruling column 4.11 (Усклађена набавна цена)
* Column "Est. gain/loss" must match ruling column 4.13
* Total must match the ruling dispositive figure

All 14 rows (SGOV and IJH) must match to the nearest дин (last-digit rounding differences acceptable as observed in the ruling).

---

### Key Risks

* CPI data source URL or format may change — mitigated by pre-embedded fallback
* Same-month edge case is a tax authority policy, not derivable from the law text — documented and tested explicitly
* Missing historical CPI silently produces understated adjusted prices — the `?` marker and warning line make this visible to the user
* Multi-year holdings require chaining across year boundaries — covered by explicit test
