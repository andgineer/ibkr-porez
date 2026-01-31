# ibkr-porez

Аутоматизовано генерисање пореске пријаве ППДГ-3Р (Капитална добит) за кориснике Interactive Brokers у Србији.
Програм аутоматски преузима податке о вашим трансакцијама и генерише XML фајл спреман за отпремање, са свим ценама конвертованим у РСД.

![ППДГ-3Р](images/ppdg-3r.png)

[Инсталирај ibkr-porez](installation.md)

[Конфигурација](setup_ibkr.md): Сачувајте ваше Interactive Brokers Flex Query креденцијале и пореске податке.
    ```bash
    ibkr-porez config
    ```

[Преузимање података](usage.md/#fetch-data-get): Преузмите историју трансакција са Interactive Brokers и курсеве валута са Народне банке Србије.
    ```bash
    ibkr-porez get
    ```

[Генерисање извештаја](usage.md/#generate-capital-gains-ppdg-3r-tax-report-report): Генеришите ППДГ-3Р XML фајл.
    ```bash
    ibkr-porez report
    ```

> Једноставно отпремите генерисани XML на портал **еПорези** (секција ППДГ-3Р).
