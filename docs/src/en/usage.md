# Usage

## Configuration

Creating or modifying personal data and IBKR access settings.

```bash
ibkr-porez config
```

You will be prompted for:

*   **IBKR Flex Token**: [Get Token](ibkr.md/#flex-web-service)
*   **IBKR Query ID**: [Create Flex Query](ibkr.md/#flex-query)
*   **Personal ID**: JMBG / EBS
*   **Full Name**: First and Last Name
*   **Address**: Registered Address
*   **City Code**: 3-digit municipality code. Example: `223` (Novi Sad). Code can be found in the [list](https://www.apml.gov.rs/uploads/useruploads/Documents/1533_1_pravilnik-javni_prihodi_prilog-3.pdf) (see column "Šifra"). Also available in the dropdown on the ePorezi portal.
*   **Phone**: Phone
*   **Email**: Email

## Fetch Data (`fetch`)

Downloads latest data from IBKR and syncs exchange rates from NBS (National Bank of Serbia).

Saves them to local storage.

```bash
ibkr-porez fetch
```

## Import Historical Data (`import`)

Loading transaction history older than 365 days, which cannot be retrieved via Flex Query (`fetch`).

1.  Download CSV: [Export Full History](ibkr.md/#export-full-history-for-import-command)
2.  Import file:

```bash
ibkr-porez import /path/to/activity_statement.csv
```

> ⚠️ Do not forget to run `fetch` after `import` so the application adds maximum details at least for the last year
> into the less detailed data loaded from CSV.

### Synchronization Logic (`import` + `fetch`)
When loading data from CSV (`import`) and Flex Query (`fetch`), the system prioritizes more complete Flex Query data:

*   Flex Query (`fetch`) data is the source of truth. It overwrites CSV data for any matching dates.
*   If a Flex Query record matches a CSV record semantically (Date, Ticker, Price, Quantity), it counts as an update (replacing with official ID).
*   If data structure differs (e.g. split orders in Flex Query vs "bundled" record in CSV), the old CSV record is removed, and new Flex Query records are added.
*   Completely identical records are skipped.

## Show Statistics (`show`)

```bash
ibkr-porez show
```

Shows:

*   Dividends received (in RSD).
*   Number of sales (taxable events).
*   Estimated Realized P/L (Capital Gains) (in RSD).

## Generate Capital Gains Tax Report (`report`)

```bash
# Report for the last full half-year
ibkr-porez report

# Report for the second half of 2025 (Jul 1 - Dec 31)
ibkr-porez report --year 2025 --half 2
```

* **Output**: `ppdg3r_2025_H1.xml`
* Import this file into the Serbian Tax Administration portal (ePorezi)
* Manually upload the file from [Confirmation Document](ibkr.md#confirmation-document) to Item 8
