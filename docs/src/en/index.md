# ibkr-porez

Automated generation of the PPDG-3R tax report (Capital Gains) for Interactive Brokers users in Serbia.
It automatically fetches your transaction data and generates a ready-to-upload XML file with all prices converted to RSD.

![PPDG-3R](images/ppdg-3r.png)

1. [Configure](https://andgineer.github.io/ibkr-porez/setup_ibkr/): Save your Interactive Brokers Flex Query credentials and taxpayer details.
    ```bash
    ibkr-porez config
    ```

2. [Fetch Data](https://andgineer.github.io/ibkr-porez/usage/#1-fetch-data-get): Download transaction history from Interactive Brokers and exchange rates from the National Bank of Serbia.
    ```bash
    ibkr-porez get
    ```

3. [Generate Report](https://andgineer.github.io/ibkr-porez/usage/#3-generate-tax-report-report): Generate the PPDG-3R XML file.
    ```bash
    ibkr-porez report
    ```

> Simply upload the generated XML to the **ePorezi** portal (PPDG-3R section).
