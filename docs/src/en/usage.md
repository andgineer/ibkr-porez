# Usage Guide

## Configuration

Before first use, run the config command to save your IBKR credentials and personal details.

```bash
ibkr-porez config
```

You will be prompted for:

*   **IBKR Flex Token**: (From IBKR Settings)
*   **IBKR Query ID**: (From your saved Flex Query)
*   **Personal ID**: (JMBG for tax forms)
*   **Full Name**: (For tax forms)
*   **Address**: (For tax forms)
*   **City Code**: The 3-digit municipality code (Šifra opštine). Example: `223` (Novi Sad). You can find the code in the [list](https://www.apml.gov.rs/uploads/useruploads/Documents/1533_1_pravilnik-javni_prihodi_prilog-3.pdf) (see column "Šifra"). The code is also available in the dropdown on the ePorezi portal.
*   **Phone**: (Contact phone)
*   **Email**: (Contact email)

## Fetch Data (`get`)

Downloads the latest data from IBKR and syncs exchange rates from NBS (National Bank of Serbia).

```bash
ibkr-porez get

# Force full update (ignore local history and fetch last 365 days)
ibkr-porez get --force
```

*   Downloads XML report using your Token/Query ID.
*   Parses new transactions (Trades, Dividends).
*   Saves them to local storage.
*   Fetches historical exchange rates for all transaction dates.
*   **Default**: Incrementally fetches only new data (since your last transaction).
*   **--force**: use this if you want to refresh older data.

## Import Historical Data (`import`)

Use this command to load transaction history older than 365 days (which cannot be fetched via Flex Query).

1.  Download a **Custom Statement** from IBKR (Activity Statement -> Custom -> Format: CSV) covering the missing period.
2.  Import it:

```bash
ibkr-porez import /path/to/activity_statement.csv
```

### Data Synchronization Logic (`import` + `get`)
When you mix CSV data (Import) and XML data (Get), the system handles overlaps automatically:
*   **XML Supremacy**: Official XML data (`get`) is the source of truth. It overwrites CSV data for any matching dates.
*   **Updated**: If an XML record matches a CSV record semantically (same Date, Symbol, Price, Quantity), it counts as an **Update** (upgrading to the official ID).
*   **New**: If XML structure differs (e.g. Split Orders vs Bundled CSV), the old CSV record is replaced by new XML records. These appear as **New**.
*   **Identical**: If records are identical, they are skipped.

## Show Statistics (`show`)

Displays a summary of your portfolio activity grouped by month.

```bash
ibkr-porez show
```

Shows:
*   Dividends received (in RSD).
*   Number of sales (taxable events).
*   Realized P/L (Capital Gains) estimate (in RSD).

## Generate Capital Gains PPDG-3R Tax Report (`report`)

```bash
# Latest full half a year
ibkr-porez report

# Report for the second half of 2024 (Jul 1 - Dec 31)
ibkr-porez report --year 2024 --half 2
```

*   **Output**: `ppdg3r_2024_H1.xml`
*   Import this file into the Serbian Tax Administration portal (ePorezi).
