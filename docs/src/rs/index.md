# ibkr-porez

Automatizovano generisanje poreske prijave PPDG-3R (Kapitalna dobit) za korisnike Interactive Brokers u Srbiji.
Program automatski preuzima podatke o vašim transakcijama i generiše XML fajl spreman za otpremanje, sa svim cenama konvertovanim u RSD.

![PPDG-3R](images/ppdg-3r.png)

[Instaliraj ibkr-porez](installation.md)

[Konfiguracija](setup_ibkr.md): Sačuvajte vaše Interactive Brokers Flex Query kredencijale i poreske podatke.
    ```bash
    ibkr-porez config
    ```

[Preuzimanje podataka](usage.md/#fetch-data-get): Preuzmite istoriju transakcija sa Interactive Brokers i kurseve valuta sa Narodne banke Srbije.
    ```bash
    ibkr-porez get
    ```

[Generisanje izveštaja](usage.md/#generate-capital-gains-ppdg-3r-tax-report-report): Generišite PPDG-3R XML fajl.
    ```bash
    ibkr-porez report
    ```

> Jednostavno otpremite generisani XML na portal **ePorezi** (sekcija PPDG-3R).
