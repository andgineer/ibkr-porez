# Brzi start

Automatizovano generisanje poreske prijave PPDG-3R (Kapitalna dobit) za korisnike Interactive Brokers u Srbiji.
Program automatski preuzima podatke o transakcijama i kreira spreman XML fajl za otpremanje, konvertujući sve cene u dinare (RSD).

1. [Instalirajte ibkr-porez](installation.md)

2. [Konfiguracija](usage.md):
    ```bash
    ibkr-porez config
    ```

3. [Preuzimanje podataka](usage.md/#preuzimanje-podataka-get): Preuzmite istoriju transakcija sa Interactive Brokers i zvanične kurseve valuta sa Narodne banke Srbije.
    ```bash
    ibkr-porez get
    ```

    > Za proračun dobiti, aplikaciji je potrebna puna istorija za prodate hartije.
    > Pošto Flex Query omogućava preuzimanje podataka za ne više od godinu dana, za učitavanje starijih podataka koristite
    > [<u>import</u>](usage.md/#uvoz-istorijskih-podataka-import) iz CSV fajlova.

    > ⚠️ Ne zaboravite da pokrenete `get` nakon `import` kako bi aplikacija dodala maksimum detalja bar za poslednju godinu
    > u manje detaljne podatke učitane iz CSV-a.

4. [Kreiranje izveštaja](usage.md/#generisanje-poreskog-izvestaja-za-kapitalnu-dobit-report): Generišite PPDG-3R XML fajl.
    ```bash
    ibkr-porez report
    ```

5. Otpremite generisani XML na portal **ePorezi** (sekcija PPDG-3R).

    ![PPDG-3R](images/ppdg-3r.png)
