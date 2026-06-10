> [English](../en/ibkr.html) | [Русский](../ru/ibkr.html) | [Українська](../uk/ibkr.html) | **Srpski** | [Српски](../rs-cyr/ibkr.html)

# Interactive Brokers (IBKR)

## Flex Web Service

1. **Performance & Reports** > **Flex Queries**.
2. Kliknite na ikonu **Settings** (zupčanik) u "Flex Web Service Configuration".
3. Omogućite **Flex Web Service**.
4. Generišite **Token** (Generate Token).
    *   **Važno**: Odmah kopirajte ovaj token. Nećete moći ponovo da ga vidite u celosti.
    *   Postavite rok trajanja (preporučeno max - 1 godina).

## Flex Query

1. **Performance & Reports** > **Flex Queries**.
2. Kliknite **+** da kreirate novi **Activity Flex Query**.
3. **Name**: npr. `ibkr-porez-data`.
4.  **Delivery Configuration** (na dnu stranice):
    *   **Period**: Izaberite **Last 365 Calendar Days**.
5.  **Format**: **XML**.

### Sekcije za uključivanje (Sections):

Omogućite sledeće sekcije i označite **Select All** (Izaberi sve) za kolone.

Ako nikome ne verujete 8-) umesto **Select All** izaberite bar polja navedena u `Obavezne kolone`.

### Trades - Transakcije
Nalazi se pod Trade Confirmations ili Activity.

<details>
<summary>Obavezne kolone</summary>

*   `Symbol`
*   `Description`
*   `Currency`
*   `Quantity`
*   `TradePrice`
*   `TradeDate`
*   `TradeID`
*   `OrigTradeDate`
*   `OrigTradePrice`
*   `AssetClass`
*   `Buy/Sell`

</details>

### Cash Transactions - Novčane transakcije

<details>
<summary>Obavezne kolone</summary>

*   `Type`
*   `Amount`
*   `Currency`
*   `DateTime` / `Date`
*   `Symbol`
*   `Description`
*   `TransactionID`

</details>

## Sačuvajte i preuzmite Query ID

Zabeležite **Query ID** (broj koji se obično pojavljuje pored imena upita u listi).

Trebaće vam **Token** i **Query ID** za konfiguraciju `ibkr-porez`.

## Dokument potvrde

Za **Tačku 8 (Dokazi uz prijavu)** poreske prijave PPDG-3R potreban vam je PDF izveštaj od brokera.
Mora se ručno priložiti na portalu ePorezi nakon uvoza XML-a.

Kako preuzeti odgovarajući izveštaj:

1.  U IBKR idite na **Performance & Reports** > **Statements** > **Activity Statement**.
2.  **Period**: Izaberite **Custom Date Range**.
3.  Navedite datume koji odgovaraju vašem poreskom periodu (npr. `01-01-2024` do `30-06-2024` za prvo polugodište).
4.  Kliknite **Download PDF**.
5.  Na portalu ePorezi, u sekciji **8. Dokazi uz prijavu**, otpremite ovaj fajl.

## Preuzimanje Flex Query XML-a (za `sync --file`)

Ako IBKR API (`sync` / `fetch`) privremeno nije dostupan, možete pokrenuti Flex Query ručno sa IBKR sajta:

1. U IBKR idite na **Performance & Reports** > **Statements** > **Flex Queries**.
2. Pronađite upit koji ste kreirali za `ibkr-porez` (npr. `ibkr-porez-data`).
3. Kliknite **Run** (plava strelica desno od naziva upita).
4. Izaberite format **XML** i preuzmite fajl.
5. Koristite fajl sa komandom [`sync --file` ↗](usage.md#sinhronizacija-iz-preuzetog-xml-fajla-sync---file) ili opcijom **Sync from Flex Query XML…** u ☰ meniju u GUI-u.

## Izvoz pune istorije (za import komandu)

Ako treba da učitate istoriju transakcija za period duži od 1 godine (nedostupno preko Flex Web Service-a),
izvezite podatke u CSV:

1.  U IBKR idite na **Performance & Reports** > **Statements** > **Activity Statement**.
2.  **Period**: Izaberite **Custom Date Range** i navedite ceo period od otvaranja računa.
3.  Kliknite **Download CSV**.
4.  Ovaj fajl se može koristiti sa komandom [import ↗](usage.md#uvoz-istorijskih-podataka-import).
