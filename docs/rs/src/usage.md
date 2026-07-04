> [English](../en/usage.html) | [Русский](../ru/usage.html) | [Українська](../uk/usage.html) | **Srpski** | [Српски](../rs-cyr/usage.html)

# Upotreba

## Brzi start

Ako želite brzo da kreirate konkretnu prijavu:

1. [Podesite podatke (config) ↗](#konfiguracija-config) — jednom pri prvom pokretanju
2. [Preuzmite najnovije podatke (fetch) ↗](#preuzimanje-podataka-fetch)
3. [Kreirajte izveštaj (report) ↗](#generisanje-poreskog-izvestaja-report)
4. Otpremite kreirani XML na portal **ePorezi** (sekcija PPDG-3R)

Za automatsko dobijanje svih prijava i praćenje njihovih statusa —
koristite [sync](#sinhronizacija-podataka-i-kreiranje-prijava-sync) umesto koraka 2–3.

---

## Konfiguracija (config)
```bash
ibkr-porez config
```

Kreiranje ili izmena ličnih podataka i podešavanja pristupa IBKR-u.

Biće vam zatraženo:

*   **IBKR Flex Token**: [Preuzimanje tokena ↗](ibkr.md#flex-web-service)
*   **IBKR Query ID**: [Kreiranje Flex Query-a ↗](ibkr.md#flex-query)
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

Da biste kreirali fajl sa transakcijama na portalu Interactive Brokers pogledajte [Izvoz pune istorije ↗](ibkr.md#izvoz-pune-istorije-za-import-komandu)

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

Zatim možete [Upravljati kreiranim prijavama](#upravljanje-prijavama).

> 💡 Ako ste pokrenuli `sync` prvi put i ona je kreirala prijave koje ste već podali pre početka korišćenja aplikacije,
> možete brzo da ih sve označite kao plaćene i uklonite iz izlaza [list](#spisak-prijava-list):
> ```bash
> ibkr-porez list --status submitted -1 | ibkr-porez pay
> ```

### Sinhronizacija iz preuzetog XML fajla (`sync --file`)

Ako IBKR API privremeno nije dostupan, možete ručno preuzeti Flex Query XML sa IBKR sajta i koristiti ga:

```bash
ibkr-porez sync --file /path/to/report.xml
```

Radi sve isto što i `sync` — čuva transakcije, kreira sve potrebne prijave — ali čita podatke iz lokalnog fajla umesto pozivanja IBKR API-ja.

Pogledajte [kako preuzeti Flex Query XML ↗](ibkr.md#preuzimanje-flex-query-xml-a-za-sync---file).

U GUI-u, ista opcija dostupna je u meniju **☰** kao **Sync from Flex Query XML…**.

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
# Prikaži aktivne prijave (podrazumevano):
# draft + submitted + pending
ibkr-porez list

# Prikaži sve prijave
ibkr-porez list --all

# Filter po statusu
ibkr-porez list --status draft
ibkr-porez list --status submitted
ibkr-porez list --status pending
ibkr-porez list --status finalized

# Samo ID prijava (za korišćenje u cevima)
ibkr-porez list --ids-only
ibkr-porez list --status draft -1
```

Primer korišćenja u linux-stilu:
```bash
# Podneti sve nacrte
ibkr-porez list --status draft -1 | ibkr-porez submit
```

### Pregled detalja prijave (`show`)

Prikazuje detaljne informacije o određenoj prijavi.

```bash
ibkr-porez show <declaration_id>
```

Prikazuje:

*   Tip prijave (PPDG-3R ili PP OPO)
*   Period prijave
*   Status (nacrt, podneta, na čekanju, završena)
*   Detalje transakcija i proračuna
*   Za PPDG-3R: dobitak/gubitak priznat od strane poreske uprave pored
    izračunatih vrednosti, iskorišćeni prenos kapitalnih gubitaka (početno/
    iskorišćeno/korigovano/krajnje stanje) i iz kojih "tranši" je iskorišćen
*   Priložene fajlove

### Podnošenje prijave (`submit`)
```bash
ibkr-porez submit <id> [<id> ...]
```

Označava prijavu kao podnetu (uvezenu na poreski portal).

Ponašanje zavisi od tipa prijave:

*   `PPDG-3R` nakon `submit` prelazi u status `pending` (čeka rešenje poreske uprave o iznosu poreza).
*   `PP OPO` nakon `submit`:
    *   prelazi u `submitted` ako postoji porez za plaćanje;
    *   prelazi direktno u `finalized` ako je porez `0`.

### Plaćanje prijave (`pay`)
```bash
ibkr-porez pay <id> [<id> ...]
ibkr-porez pay <id> --tax 1234.56
```

Označava prijavu kao završenu (`finalized`) i čuva datum plaćanja.

Opcija `--tax` omogućava da odmah zabeležite iznos poreza tokom plaćanja, bez posebnog koraka `assess`.

Nakon toga će nestati sa spiska prikazanog [list](#spisak-prijava-list) (bez `--all`)

### Evidencija iznosa po rešenju poreske (`assess`)
```bash
# Zabeleži zvaničan iznos poreza iz rešenja
ibkr-porez assess <declaration_id> --tax 1234.56

# Zabeleži iznos i odmah označi kao već plaćeno
ibkr-porez assess <declaration_id> --tax 1234.56 --paid

# Zabeleži gubitak priznat od strane poreske uprave (samo za PPDG-3R)
ibkr-porez assess <declaration_id> --loss 50000.00 \
    --reference "RES-123/2025" --date 2025-09-01

# Zabeleži dobitak priznat od strane poreske uprave (samo za PPDG-3R)
ibkr-porez assess <declaration_id> --gain 12000.00
```

Komanda je najvažnija za `PPDG-3R`, gde iznos poreza, kao i priznati kapitalni
dobitak/gubitak, određuje poreska uprava nakon podnošenja prijave.

Šta komanda radi:

*   upisuje zvaničan iznos poreza u metapodatke prijave (`--tax`);
*   sa `--paid` odmah prebacuje prijavu u `finalized`;
*   bez `--paid`:
    *   ako je iznos veći od nule, prijava ostaje aktivna (`submitted`) za naknadno plaćanje;
    *   ako je iznos nula, prijava prelazi u `finalized`.

Mora biti navedena bar jedna od opcija: `--tax`, `--gain`, `--loss`.

`--gain` i `--loss` su dostupni samo za `PPDG-3R` i upisuju kapitalni
dobitak/gubitak priznat od strane poreske uprave — čuvaju se pored izračunatih
vrednosti aplikacije i mogu se razlikovati od njih (zbog CPI korekcija ili
metodologije poreske uprave). Jedno rešenje ne može istovremeno priznati i
dobitak i gubitak.

`--reference`, `--date` i `--notes` su podaci o rešenju (broj, datum,
napomene). Prikazuju se u [show](#pregled-detalja-prijave-show), a za
priznati gubitak broj i datum rešenja se dodatno upisuju u deo 7 budućih
PPDG-3R prijava (vidi [prenos kapitalnih gubitaka](#prenos-kapitalnih-gubitaka-carryforward)),
pa ih vredi evidentirati.

Ako rešenje priznaje gubitak (`--loss` veći od nule), kreira se (ili ažurira)
zapis u registru [prenosa kapitalnih gubitaka](#prenos-kapitalnih-gubitaka-carryforward).
Prenos se uvek zasniva na gubitku priznatom od strane poreske uprave, a ne na
izračunatom.

> ⚠️ Nakon što je preneti gubitak makar delimično iskorišćen u jednoj od
> narednih prijava, priznati gubitak se više ne može menjati putem `assess` —
> komanda će vratiti grešku.

### Prenos kapitalnih gubitaka (`carryforward`)
```bash
ibkr-porez carryforward
```

Prikazuje listu svih "tranši" (vintages) kapitalnih gubitaka priznatih od
strane poreske uprave, dostupnih za prenos u buduće periode:

*   prijavu-izvor i period za koji je gubitak priznat;
*   priznat i preostali (neiskorišćeni) iznos;
*   poresku godinu nakon koje prenos ističe (gubitak se može preneti najviše 5
    godina unapred);
*   status: `Active` (može se koristiti), `Exhausted` (potpuno iskorišćen),
    `Expired` (istekao rok).

Ista lista je dostupna u GUI-ju u meniju **☰** → **Capital loss carryforward...**.

Svaka PPDG-3R prijava kreirana putem
[sync](#sinhronizacija-podataka-i-kreiranje-prijava-sync) automatski umanjuje
izračunatu poresku osnovicu koristeći raspoložive prenose (od starijih ka
novijim periodima), dok se osnovica ne svede na nulu ili se prenosi ne
iscrpe. Pregled [report](#generisanje-poreskog-izveštaja-report)-a prikazuje
iskorišćeni i preostali iznos prenosa nakon toga. Iznos se odbija iz registra
samo jednom — prilikom čuvanja prijave; ponovni `sync` za isti period ga ne
odbija ponovo.

Preneti gubici se takođe prijavljuju u samoj prijavi: u PPDG-3R XML-u se
popunjava deo 7 („Kapitalni gubici“) — po jedan red za svaki aktivan prenos,
sa brojem i datumom rešenja poreske uprave (7.2/7.3) i preostalim iznosom
gubitka (7.4). Konačni `Osnovica` i `PorezZaUplatu` u XML-u takođe uzimaju u
obzir primenjeni prenos. Prijavljivanje gubitka u delu 7 je obaveza samog
poreskog obveznika — bez toga ga poreska uprava neće primeniti u rešenju.

Broj i datum rešenja se uzimaju iz
[assess](#evidencija-iznosa-po-rešenju-poreske-assess) (`--reference` i
`--date`). Ako nisu evidentirani, polja 7.2/7.3 u XML-u ostaju prazna, a
`report` ispisuje upozorenje — evidentirajte ih kroz `assess` i ponovo
generišite izveštaj, ili popunite ta polja ručno na portalu.

### Izvoz prijave (`export`)
```bash
ibkr-porez export <declaration_id>
ibkr-porez export <declaration_id> -o /path/to/output
```

Kopira XML i sve priložene fajlove ([attach](#prilozavanje-fajla-uz-prijavu-attach)) u [Output Folder](#konfiguracija-config) ili u katalog naveden u parametrima.

### Povratak statusa prijave (`revert`)
```bash
# Vratiti na nacrt (podrazumevano)
ibkr-porez revert <id> [<id> ...]

# Vratiti na podnetu
ibkr-porez revert <id> [<id> ...] --to submitted
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
ibkr-porez export-flex 2025-01-15 | ibkr-porez sync --file -
```
