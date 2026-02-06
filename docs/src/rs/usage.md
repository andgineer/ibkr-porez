# Upotreba

## Konfiguracija (config)
```bash
ibkr-porez config
```

Kreiranje ili izmena ličnih podataka i podešavanja pristupa IBKR-u.

Biće vam zatraženo:

*   **IBKR Flex Token**: [Preuzimanje tokena ↗](ibkr.md/#flex-web-service)
*   **IBKR Query ID**: [Kreiranje Flex Query-a ↗](ibkr.md/#flex-query)
*   **Personal ID**: JMBG / EBS
*   **Full Name**: Ime i Prezime
*   **Address**: Adresa prebivališta
*   **City Code**: Trocefrni kod opštine. Primer: `223` (Novi Sad). Kod možete naći u [šifarniku](https://www.apml.gov.rs/uploads/useruploads/Documents/1533_1_pravilnik-javni_prihodi_prilog-3.pdf) (videti kolonu "Šifra"). Takođe dostupan u padajućem meniju na portalu ePorezi.
*   **Phone**: Telefon
*   **Email**: Email
*   **Data Directory**: Apsolutna putanja do foldera sa fajlovima podataka (`transactions.json`, `declarations.json`, `rates.json`, itd.). Podrazumevano: `ibkr-porez-data` u folderu aplikacije.
*   **Output Folder**: Apsolutna putanja do foldera za čuvanje fajlova iz komandi `sync`, `export`, `export-flex`, `report`. Podrazumevano: folder Downloads vašeg sistema.

## Preuzimanje podataka (`fetch`)
```bash
ibkr-porez fetch
```

Preuzima najnovije podatke sa IBKR i sinhronizuje kurseve sa NBS (Narodna banka Srbije).

Čuva ih u lokalno skladište.

## Uvoz istorijskih podataka (`import`)
```bash
ibkr-porez import /path/to/activity_statement.csv
```

Učitavanje istorije transakcija starije od 365 dana, koja se ne može preuzeti putem Flex Query-a (`fetch`).

Da biste kreirali fajl sa transakcijama na portalu Interactive Brokers pogledajte [Izvoz pune istorije ↗](ibkr.md/#izvoz-pune-istorije-za-import-komandu)

> ⚠️ Ne zaboravite da pokrenete `fetch` nakon `import` kako bi aplikacija dodala maksimum detalja bar za poslednju godinu
> u manje detaljne podatke učitane iz CSV-a.

### Logika sinhronizacije (`import` + `fetch`)
Pri učitavanju podataka iz CSV-a (`import`) i Flex Query-a (`fetch`), sistem daje prioritet potpunijim Flex Query podacima:

*   Podaci Flex Query-a (`fetch`) su izvor istine. Oni prepisuju CSV podatke za bilo koje podudarne datume.
*   Ako se zapis Flex Query-a semantički poklapa sa CSV zapisom (Datum, Tiker, Cena, Količina), to se računa kao ažuriranje (zamena zvaničnim ID-em).
*   Ako se struktura podataka razlikuje (npr. split nalozi u Flex Query-u protiv "spojenog" zapisa u CSV-u), stari CSV zapis se uklanja, a novi Flex Query zapisi se dodaju.
*   Potpuno identični zapisi se preskaču.

## Sinhronizacija podataka i kreiranje prijava (`sync`)

```bash
ibkr-porez sync
```

Radi sve isto što i [fetch](#preuzimanje-podataka-fetch):

*   Preuzima najnovije transakcije sa IBKR putem Flex Query-a
*   Sinhronizuje kurseve valuta sa NBS

Nakon toga kreira sve potrebne prijave za poslednjih 45 dana (ako već nisu kreirane).

Zatim možete [Upravljati kreiranim prijavama](#_2).

> 💡 Ako ste pokrenuli `sync` prvi put i ona je kreirala prijave koje ste već podali pre početka korišćenja aplikacije,
> možete brzo da ih sve označite kao plaćene i uklonite iz izlaza [list](#spisak-prijava-list):
> ```bash
> ibkr-porez pay
> ```

## Prikaz statistike (`stat`)

```bash
ibkr-porez stat --year 2025
ibkr-porez stat --ticker AAPL
ibkr-porez stat --month 2025-01
```

Prikazuje:

*   Primljene dividende (u RSD)
*   Broj prodaja (poreski događaji)
*   Procenu realizovanog P/L (Kapitalna dobit) (u RSD)
*   Detaljnu podelu po tikerima ili mesecima (pri korišćenju filtera)

## Generisanje poreskog izveštaja (`report`)
```bash
ibkr-porez report
```

Ako ne navedete tip izveštaja i period, podrazumevano se generiše PPDG-3R za poslednje puno polugodište

* Kreira `ppdg3r_XXXX_HY.xml` u [Output Folder](#konfiguracija-config)
* Uvezite ovaj fajl na portal Poreske uprave Srbije (ePorezi)
* Ručno otpremite fajl iz [Dokument potvrde](ibkr.md#dokument-potvrde) u Tačku 8

Da biste izabrali drugi tip prijave ili vremenski period pogledajte dokumentaciju

```bash
ibkr-porez report --help
```

## Upravljanje prijavama

Nakon kreiranja prijava putem komande [sync](#sinhronizacija-podataka-i-kreiranje-prijava-sync) možete ih pregledati, menjati status i izvoziti za otpremanje na poreski portal.

### Spisak prijava (`list`)

Prikazuje spisak svih prijava sa mogućnošću filtriranja po statusu.

```bash
# Prikaži samo nacrte (podrazumevano)
ibkr-porez list

# Prikaži sve prijave
ibkr-porez list --all

# Filter po statusu
ibkr-porez list --status draft
ibkr-porez list --status submitted
ibkr-porez list --status paid

# Samo ID prijava (za korišćenje u cevima)
ibkr-porez list --ids-only
ibkr-porez list --status draft -1
```

Primer korišćenja u linux-stilu:
```bash
# Podneti sve nacrte
ibkr-porez list --status draft -1 | xargs -I {} ibkr-porez submit {}
```

### Pregled detalja prijave (`show`)

Prikazuje detaljne informacije o određenoj prijavi.

```bash
ibkr-porez show <declaration_id>
```

Prikazuje:

*   Tip prijave (PPDG-3R ili PP OPO)
*   Period prijave
*   Status (nacrt, podneta, plaćena)
*   Detalje transakcija i proračuna
*   Priložene fajlove

### Podnošenje prijave (`submit`)
```bash
ibkr-porez submit <declaration_id>
```

Označava prijavu kao podnetu (uvezenu na poreski portal).

### Plaćanje prijave (`pay`)
```bash
ibkr-porez pay <declaration_id>
```

Označava prijavu kao plaćenu.

Nakon toga će nestati sa spiska prikazanog [list](#spisak-prijava-list) (bez `--all`)

### Izvoz prijave (`export`)
```bash
ibkr-porez export <declaration_id>
ibkr-porez export <declaration_id> -o /path/to/output
```

Kopira XML i sve priložene fajlove ([attach](#prilozavanje-fajla-uz-prijavu-attach)) u [Output Folder](#konfiguracija-config) ili u katalog naveden u parametrima.

### Povratak statusa prijave (`revert`)
```bash
# Vratiti na nacrt (podrazumevano)
ibkr-porez revert <declaration_id>

# Vratiti na podnetu
ibkr-porez revert <declaration_id> --to submitted
```

Vraća status prijave.

### Priložavanje fajla uz prijavu (`attach`)
```bash
# Priložiti fajl
ibkr-porez attach <declaration_id> /path/to/file.pdf

# Obrisati priloženi fajl
ibkr-porez attach <declaration_id> <file_id> --delete
ibkr-porez attach <declaration_id> --delete --file-id <file_id>
```

Priložava fajl uz prijavu ili uklanja priloženi fajl iz skladišta prijava.

Za čuvanje u skladištu prijava koristi se samo ime fajla (putanja se odbacuje), tako da imena moraju
biti jedinstvena - inače će fajl sa istim imenom prepisati ranije učitan fajl sa istim imenom čak i iz druge putanje


> 💡 Priloženi fajlovi se kopiraju zajedno sa XML prijave tokom [izvoza (export)](#izvoz-prijave-export)

## Izvoz Flex Query (`export-flex`)
```bash
ibkr-porez export-flex 2025-01-15
ibkr-porez export-flex 2025-01-15 -o /path/to/output.xml
ibkr-porez export-flex 2025-01-15 -o -  # Izlaz u stdout (za cevi)
```

Izvoz XML fajla Flex Query dobijenog tokom [fetch](#preuzimanje-podataka-fetch) ili [sync](#sinhronizacija-podataka-i-kreiranje-prijava-sync) na navedeni datum.

Primer korišćenja u linux-stilu:
```bash
ibkr-porez export-flex 2025-01-15 | ibkr-porez import
```
