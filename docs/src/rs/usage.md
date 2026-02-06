# Upotreba

## Konfiguracija

Kreiranje ili izmena ličnih podataka i podešavanja pristupa IBKR-u.

```bash
ibkr-porez config
```

Biće vam zatraženo:

*   **IBKR Flex Token**: [Preuzimanje tokena](ibkr.md/#flex-web-service)
*   **IBKR Query ID**: [Kreiranje Flex Query-a](ibkr.md/#flex-query)
*   **Personal ID**: JMBG / EBS
*   **Full Name**: Ime i Prezime
*   **Address**: Adresa prebivališta
*   **City Code**: Trocefrni kod opštine. Primer: `223` (Novi Sad). Kod možete naći u [šifarniku](https://www.apml.gov.rs/uploads/useruploads/Documents/1533_1_pravilnik-javni_prihodi_prilog-3.pdf) (videti kolonu "Šifra"). Takođe dostupan u padajućem meniju na portalu ePorezi.
*   **Phone**: Telefon
*   **Email**: Email

## Preuzimanje podataka (`fetch`)

Preuzima najnovije podatke sa IBKR i sinhronizuje kurseve sa NBS (Narodna banka Srbije).

Čuva ih u lokalno skladište.

```bash
ibkr-porez fetch
```

## Uvoz istorijskih podataka (`import`)

Učitavanje istorije transakcija starije od 365 dana, koja se ne može preuzeti putem Flex Query-a (`fetch`).

1.  Preuzmite CSV: [Izvoz pune istorije](ibkr.md/#izvoz-pune-istorije-za-import-komandu)
2.  Uvezite fajl:

```bash
ibkr-porez import /path/to/activity_statement.csv
```

> ⚠️ Ne zaboravite da pokrenete `fetch` nakon `import` kako bi aplikacija dodala maksimum detalja bar za poslednju godinu
> u manje detaljne podatke učitane iz CSV-a.

### Logika sinhronizacije (`import` + `fetch`)
Pri učitavanju podataka iz CSV-a (`import`) i Flex Query-a (`fetch`), sistem daje prioritet potpunijim Flex Query podacima:

*   Podaci Flex Query-a (`fetch`) su izvor istine. Oni prepisuju CSV podatke za bilo koje podudarne datume.
*   Ako se zapis Flex Query-a semantički poklapa sa CSV zapisom (Datum, Tiker, Cena, Količina), to se računa kao ažuriranje (zamena zvaničnim ID-em).
*   Ako se struktura podataka razlikuje (npr. split nalozi u Flex Query-u protiv "spojenog" zapisa u CSV-u), stari CSV zapis se uklanja, a novi Flex Query zapisi se dodaju.
*   Potpuno identični zapisi se preskaču.

## Prikaz statistike (`show`)

```bash
ibkr-porez show
```

Prikazuje:

*   Primljene dividende (u RSD).
*   Broj prodaja (poreski događaji).
*   Procenu realizovanog P/L (Kapitalna dobit) (u RSD).

## Generisanje Poreskog izveštaja za kapitalnu dobit (`report`)

```bash
# Izveštaj za poslednje puno polugodište
ibkr-porez report

# Izveštaj za drugo polugodište 2025 (1. jul - 31. dec)
ibkr-porez report --year 2025 --half 2
```

* **Rezultat**: `ppdg3r_2025_H1.xml`
* Uvezite ovaj fajl na portal Poreske uprave Srbije (ePorezi)
* Ručno otpremite fajl iz [Dokument potvrde](ibkr.md#dokument-potvrde) u Tačku 8
