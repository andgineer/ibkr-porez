# ibkr-porez

Automated generation of the PPDG-3R tax report (Capital Gains) for Interactive Brokers users in Serbia.
It automatically fetches your transaction data and generates a ready-to-upload XML file with all prices converted to RSD.

![PPDG-3R](images/ppdg-3r.png)

[Install ibkr-porez](installation.md)

[Configure](setup_ibkr.md): Save your Interactive Brokers Flex Query credentials and taxpayer details.
    ```bash
    ibkr-porez config
    ```

[Fetch Data](usage.md/#fetch-data-get): Download transaction history from Interactive Brokers and exchange rates from the National Bank of Serbia.
    ```bash
    ibkr-porez get
    ```

[Generate Report](usage.md/#generate-capital-gains-ppdg-3r-tax-report-report): Generate the PPDG-3R XML file.
    ```bash
    ibkr-porez report
    ```

> Simply upload the generated XML to the **ePorezi** portal (PPDG-3R section).
