# Quick Start

Automated generation of PPDG-3R tax report (Capital Gains) for Interactive Brokers users in Serbia.
The program automatically retrieves transaction data and creates a ready-to-upload XML file, converting all prices to Dinars (RSD).

1. [Install ibkr-porez](installation.md)

2. [Configuration](usage.md):
    ```bash
    ibkr-porez config
    ```

3. [Fetch Data](usage.md/#fetch-data-get): Download transaction history from Interactive Brokers and official exchange rates from the National Bank of Serbia.
    ```bash
    ibkr-porez get
    ```

    > To calculate gains, the application needs full history for sold securities.
    > Since Flex Query allows downloading data for no more than a year, use [<u>import</u>](usage.md/#import-historical-data-import) from CSV files for older data.

    > ⚠️ Do not forget to run `get` after `import` so the application adds maximum details at least for the last year
    > into the less detailed data loaded from CSV.

4. [Generate Report](usage.md/#generate-capital-gains-tax-report-report): Generate PPDG-3R XML file.
    ```bash
    ibkr-porez report
    ```

5. Upload the generated XML to the **ePorezi** portal (PPDG-3R section).

    ![PPDG-3R](images/ppdg-3r.png)
