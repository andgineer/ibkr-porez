# Podešavanje Interactive Brokers (IBKR)

Da biste koristili `ibkr-porez`, potrebno je da konfigurišete **Flex Query** u vašem Interactive Brokers nalogu.
Ovo omogućava `ibkr-porez` da automatski preuzme istoriju vaših transakcija.

## 1. Omogućite Flex Web Service

1.  Prijavite se na **Interactive Brokers** (Account Management).
2.  Idite na **Performance & Reports** > **Flex Queries**.
3.  Kliknite na ikonu **Settings** (zupčanik) (ili potražite "Flex Web Service").
4.  Omogućite **Flex Web Service**.
5.  Generišite **Token**.
    *   **Važno**: Odmah kopirajte ovaj Token. Nećete moći ponovo da ga vidite u celosti.
    *   Postavite rok trajanja (npr. 1 godina).

## 2. Kreiranje Flex Query-a

1.  U **Performance & Reports** > **Flex Queries**, kliknite na **+** da kreirate novi **Trade Confirmation Flex Query** (ili "Activity Flex Query", ali striktno su nam potrebne specifične sekcije).
    *   *Napomena*: Obično se preferira "Activity Flex Query" za šire podatke, ali proverite sekcije ispod.
2.  **Name**: npr. `ibkr-porez-data`.
3.  **Delivery Configuration** (na dnu):
    *   **Period**: Izaberite **Last 365 Calendar Days**.
    *   *Savet*: `ibkr-porez` preuzima ono što je dostupno. Postavljanje "Last 365 Calendar Days" je standard za automatizaciju.
4.  **Format**: **XML**.

### Sekcije koje treba uključiti:

Omogućite sledeće sekcije i označite **Select All** kolone da biste osigurali kompatibilnost (ili barem specifične potrebne kolone navedene ispod).

#### A. Trades (Executions)
*   **Select**: `Trades` (pod Trade Confirmations ili slično u Activity).
*   **Potrebne kolone**:
    *   `Symbol`
    *   `Description`
    *   `Currency`
    *   `Quantity`
    *   `TradePrice` (ili Price)
    *   `TradeDate` (Date)
    *   `TradeID`
    *   `OrigTradeDate` (Potrebno za P/L uparivanje)
    *   `OrigTradePrice` (Potrebno za P/L uparivanje)
    *   `AssetClass` (preporučeno)
    *   `Buy/Sell` (preporučeno)

#### B. Cash Transactions
*   **Select**: `Cash Transactions`.
*   **Potrebne kolone**:
    *   `Type` (npr. Dividend, Withholding Tax)
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## 3. Sačuvajte i preuzmite Query ID

1.  Sačuvajte upit (query).
2.  Zabeležite **Query ID** (broj koji se obično pojavljuje pored imena upita u listi).

Koristićete **Token** i **Query ID** za konfiguraciju `ibkr-porez`.

## 4. Dokument potvrde (Za poresku prijavu)

Za **Deo 8 (Dokazi uz prijavu)** poreske prijave, potreban vam je PDF Izveštaj o aktivnostima (Activity Report) iz IBKR. `ibkr-porez` generiše XML sa mestom za fajl, ali morate ručno preuzeti dokazni fajl i otpremiti ga na ePorezi portal.

Kako preuzeti ispravan izveštaj:

1.  U IBKR idite na **Performance & Reports** > **Statements** > **Activity**.
2.  **Period**: Izaberite **Custom Date Range**.
3.  Odredite datume koji odgovaraju vašem poreskom periodu (npr. `01-01-2024` do `30-06-2024` za H1).
4.  **Format**: **PDF**.
5.  Kliknite **Run**.
6.  Preuzmite **PDF**.
7.  Na portalu ePorezi, u sekciji **8. Dokazi uz prijavu**, obrišite unos (ako postoji) i otpremite ovaj fajl.

## 5. Izvoz pune istorije (za `import` komandu)

Ako treba da učitate istoriju transakcija stariju od 1 godine (nedostupno preko redovnog Flex Web Service-a), koristite CSV izvoz:

1.  U IBKR idite na **Performance & Reports** > **Statements** > **Activity**.
2.  **Period**: Izaberite **Complete Date Range** ili **Custom Date Range** (odredite ceo period od otvaranja računa).
3.  **Format**: **CSV**.
4.  Kliknite **Run**.
5.  Preuzmite fajl izveštaja. Ovaj fajl se može koristiti sa `ibkr-porez import` komandom.
