# Quick Start
Automated PPDG-3R (Capital Gains) and PP-OPO (Dividends / Interest) tax reports generation for Interactive Brokers in Serbia.
The application automatically fetches transaction data and creates a ready-to-upload XML file, converting all prices to dinars (RSD).

1. [Install ibkr-porez ↗](installation.md)

2. [Configuration (config) ↗](usage.md#config)

3. [Import Historical Data (import) ↗](usage.md/#import-historical-data-import)

    > ⚠️ **Import is only necessary if you have more than a year of transaction history in Interactive Brokers.** Flex Query allows downloading data for no more than the last year, so older data must be loaded from a [full export to CSV file ↗](ibkr.md/#export-full-history-for-import-command).

=== "Quickly create a specific declaration"

    If you want to quickly create a specific declaration.

    [Fetch Latest Data (fetch) ↗](usage.md/#fetch-data-fetch)

    [Generate Report (report) ↗](usage.md/#generate-tax-report-report)

    Upload the created XML to the **ePorezi** portal (PPDG-3R section).
        ![PPDG-3R](images/ppdg-3r.png)

=== "Automatic declaration creation"

    If you want to automatically receive all necessary declarations and track their status (submitted, paid).

    [Fetch Latest Data and Create Declarations (sync) ↗](usage.md/#sync-data-and-create-declarations-sync)

    [Declaration Management ↗](usage.md/#declaration-management)
