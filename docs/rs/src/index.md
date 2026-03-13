> [English](../en/) | [Русский](../ru/) | [Українська](../uk/) | **Srpski** | [Српски](../rs-cyr/)

# Brzi start

Automatizovano generisanje poreske prijave PPDG-3R (porez na kapitalnu dobit) i PP OPO (porez na prihode od kapitala) za korisnike Interactive Brokers u Srbiji.
Program automatski preuzima podatke o transakcijama i kreira gotov XML fajl za otpremanje, konvertujući sve cene u dinare (RSD).

[Instalirajte ibkr-porez ↗](installation.md)

Ako koristite grafički interfejs, podesite svoje podatke (dugme `Config`) i za osvežavanje podataka i kreiranje prijava samo koristite dugme `Sync`.

Ako ste instalirali GUI + CLI, možete koristiti i grafički interfejs (pokreće se kao `ibkr-porez` bez parametara) i komandnu liniju, vidi ispod.

Grafički interfejs i komandna linija koriste istu bazu podataka.

> ⚠️ Dok je grafički interfejs pokrenut, ne koristite komandnu liniju,
> jer istovremeni rad može izazvati greške u bazi podataka.

Ako želite sve da radite kroz komandnu liniju, nastavite sa:

- [Konfiguracija (config) ↗](usage.md#konfiguracija-config)
- [Uvoz istorijskih podataka (import) ↗](usage.md#uvoz-istorijskih-podataka-import)

> ⚠️ **Uvoz je potreban samo ako imate više od godinu dana istorije transakcija u Interactive Brokers.** Flex Query omogućava preuzimanje podataka za najviše poslednju godinu, pa stariji podaci moraju biti učitani iz [punog izvoza u CSV fajl ↗](ibkr.md#izvoz-pune-istorije-za-import-komandu).

### Brzo kreirati potrebnu prijavu

Ako želite brzo da kreirate konkretnu prijavu.

[Preuzimanje najnovijih podataka (fetch) ↗](usage.md#preuzimanje-podataka-fetch)

[Kreiranje izveštaja (report) ↗](usage.md#generisanje-poreskog-izvestaja-report)

Otpremite kreirani XML na portal **ePorezi** (sekcija PPDG-3R).

![PPDG-3R](images/ppdg-3r.png)

### Automatsko kreiranje prijava

Ako želite automatski da dobijate sve potrebne prijave i pratite njihov status (podneta, plaćena).

[Preuzimanje najnovijih podataka i kreiranje prijava (sync) ↗](usage.md#sinhronizacija-podataka-i-kreiranje-prijava-sync)

[Upravljanje prijavama ↗](usage.md#upravljanje-prijavama)
