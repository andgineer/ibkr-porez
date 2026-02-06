# Brzi start
Automatizovano generisanje poreske prijave PPDG-3R (Kapitalna dobit) i PP OPO (Prihodi od kapitala) za korisnike Interactive Brokers u Srbiji.
Program automatski preuzima podatke o transakcijama i kreira spreman XML fajl za otpremanje, konvertujući sve cene u dinare (RSD).

1. [Instalirajte ibkr-porez ↗](installation.md)

2. [Konfiguracija (config) ↗](usage.md#konfiguracija-config)

3. [Uvoz istorijskih podataka (import) ↗](usage.md/#uvoz-istorijskih-podataka-import)

    > ⚠️ **Uvoz je potreban samo ako imate više od godinu dana istorije transakcija u Interactive Brokers.** Flex Query omogućava preuzimanje podataka za ne više od poslednje godine, tako da stariji podaci moraju biti učitani iz [punog izvoza u CSV fajl ↗](ibkr.md/#izvoz-pune-istorije-za-import-komandu).

=== "Brzo kreirati određenu prijavu"

    Ako želite brzo da kreirate određenu prijavu.

    [Preuzimanje najnovijih podataka (fetch) ↗](usage.md/#preuzimanje-podataka-fetch)

    [Kreiranje izveštaja (report) ↗](usage.md/#generisanje-poreskog-izvestaja-report)

    Otpremite kreirani XML na portal **ePorezi** (sekcija PPDG-3R).
        ![PPDG-3R](images/ppdg-3r.png)

=== "Automatsko kreiranje prijava"

    Ako želite automatski da primate sve potrebne prijave i pratite njihov status (podneta, plaćena).

    [Preuzimanje najnovijih podataka i kreiranje prijava (sync) ↗](usage.md/#sinhronizacija-podataka-i-kreiranje-prijava-sync)

    [Upravljanje prijavama ↗](usage.md/#upravljanje-prijavama)
