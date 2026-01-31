# Uputstvo za upotrebu

## Konfiguracija

Pre prve upotrebe, pokrenite config komandu da sačuvate vaše IBKR kredencijale i lične podatke.

```bash
ibkr-porez config
```

Biće vam zatraženo:

*   **IBKR Flex Token**: (Iz IBKR podešavanja)
*   **IBKR Query ID**: (Iz vašeg sačuvanog Flex Query-a)
*   **Personal ID**: (JMBG za poreske obrasce)
*   **Full Name**: (Puno ime za poreske obrasce)
*   **Address**: (Adresa za poreske obrasce)
*   **City Code**: Trocefrni kod opštine (Šifra opštine). Primer: `223` (Novi Sad). Kod možete naći u [sifarniku](https://www.apml.gov.rs/uploads/useruploads/Documents/1533_1_pravilnik-javni_prihodi_prilog-3.pdf) (videti kolonu "Šifra"). Kod je takođe dostupan u padajućem meniju na portalu ePorezi.
*   **Phone**: (Kontakt telefon)
*   **Email**: (Kontakt email)

## Preuzimanje podataka (`get`)

Preuzima najnovije podatke sa IBKR i sinhronizuje kurseve sa NBS (Narodna banka Srbije).

```bash
ibkr-porez get

# Prisilno puno ažuriranje (ignoriše lokalnu istoriju i preuzima poslednjih 365 dana)
ibkr-porez get --force
```

*   Preuzima XML izveštaj koristeći vaš Token/Query ID.
*   Parsira nove transakcije (Sdelke, Dividende).
*   Čuva ih u lokalno skladište.
*   Preuzima istorijske kurseve za sve datume transakcija.
*   **Podrazumevano**: Inkrementalno preuzima samo nove podatke (od vaše poslednje transakcije).
*   **--force**: koristite ovo ako želite da osvežite starije podatke.

## Uvoz istorijskih podataka (`import`)

Koristite ovu komandu za učitavanje istorije transakcija starije od 365 dana (koja se ne može preuzeti putem Flex Query-a).

1.  Preuzmite **Custom Statement** sa IBKR (Activity Statement -> Custom -> Format: CSV) koji pokriva nedostajući period.
2.  Uvezite ga:

```bash
ibkr-porez import /path/to/activity_statement.csv
```

### Logika sinhronizacije podataka (`import` + `get`)
Kada kombinujete CSV podatke (Import) i XML podatke (Get), sistem automatski rešava preklapanja:
*   **Prioritet XML-a**: Zvanični XML podaci iz `get` su izvor istine. Oni uvek zamenjuju CSV podatke za bilo koje podudarne datume.
*   **Updated**: Ako se XML zapis semantički poklapa sa CSV zapisom (isti Datum, Simbol, Cena, Količina), to se računa kao **Ažuriranje** (nadogradnja na zvanični ID).
*   **New**: Ako se struktura XML-a razlikuje (npr. Split Orders u XML-u protiv "spojenog" CSV zapisa), stari CSV zapis se zamenjuje novim XML zapisima. Ovo se prikazuje kao **Novo (New)**.
*   **Identical**: Ako su zapisi identični, preskaču se.

## Prikaz statistike (`show`)

Prikazuje sažetak aktivnosti vašeg portfolia grupisan po mesecima.

```bash
ibkr-porez show
```

Prikazuje:
*   Primljene dividende (u RSD).
*   Broj prodaja (poreski događaji).
*   Procenu realizovanog P/L (Kapitalna dobit) (u RSD).

## Generisanje poreskog izveštaja PPDG-3R (`report`)

```bash
# Poslednje puno polugodište
ibkr-porez report

# Izveštaj za drugo polugodište 2024 (1. jul - 31. dec)
ibkr-porez report --year 2024 --half 2
```

*   **Rezultat**: `ppdg3r_2024_H1.xml`
*   Uvezite ovaj fajl na portal Poreske uprave Srbije (ePorezi).
