# Подешавање Interactive Brokers (IBKR)

Да бисте користили `ibkr-porez`, потребно је да конфигуришете **Flex Query** у вашем Interactive Brokers налогу.
Ово омогућава `ibkr-porez` да аутоматски преузме историју ваших трансакција.

## 1. Омогућите Flex Web Service

1.  Пријавите се на **Interactive Brokers** (Account Management).
2.  Идите на **Performance & Reports** > **Flex Queries**.
3.  Кликните на икону **Settings** (зупчаник) (или потражите "Flex Web Service").
4.  Омогућите **Flex Web Service**.
5.  Генеришите **Token**.
    *   **Важно**: Одмах копирајте овај Токен. Нећете моћи поново да га видите у целости.
    *   Поставите рок трајања (нпр. 1 година).

## 2. Креирање Flex Query-а

1.  У **Performance & Reports** > **Flex Queries**, кликните на **+** да креирате нови **Trade Confirmation Flex Query** (или "Activity Flex Query", али стриктно су нам потребне специфичне секције).
    *   *Напомена*: Обично се преферира "Activity Flex Query" за шире податке, али проверите секције испод.
2.  **Name**: нпр. `ibkr-porez-data`.
3.  **Delivery Configuration** (на дну):
    *   **Period**: Изаберите **Last 365 Calendar Days**.
    *   *Савет*: `ibkr-porez` преузима оно што је доступно. Постављање "Last 365 Calendar Days" је стандард за аутоматизацију.
4.  **Format**: **XML**.

### Секције које треба укључити:

Омогућите следеће секције и означите **Select All** колоне да бисте осигурали компатибилност (или барем специфичне потребне колоне наведене испод).

#### A. Trades (Executions)
*   **Select**: `Trades` (под Trade Confirmations или слично у Activity).
*   **Потребне колоне**:
    *   `Symbol`
    *   `Description`
    *   `Currency`
    *   `Quantity`
    *   `TradePrice` (или Price)
    *   `TradeDate` (Date)
    *   `TradeID`
    *   `OrigTradeDate` (Потребно за P/L упаривање)
    *   `OrigTradePrice` (Потребно за P/L упаривање)
    *   `AssetClass` (препоручено)
    *   `Buy/Sell` (препоручено)

#### B. Cash Transactions
*   **Select**: `Cash Transactions`.
*   **Потребне колоне**:
    *   `Type` (нпр. Dividend, Withholding Tax)
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## 3. Сачувајте и преузмите Query ID

1.  Сачувајте упит (query).
2.  Забележите **Query ID** (број који се обично појављује поред имена упита у листи).

Користићете **Token** и **Query ID** за конфигурацију `ibkr-porez`.

## 4. Документ потврде (За пореску пријаву)

За **Део 8 (Докази уз пријаву)** пореске пријаве, потребан вам је PDF Извештај о активностима (Activity Report) из IBKR. `ibkr-porez` генерише XML са местом за фајл, али морате ручно преузети доказни фајл и отпремити га на еПорези портал.

Како преузети исправан извештај:

1.  У IBKR идите на **Performance & Reports** > **Statements** > **Activity**.
2.  **Period**: Изаберите **Custom Date Range**.
3.  Одредите датуме који одговарају вашем пореском периоду (нпр. `01-01-2024` до `30-06-2024` за Х1).
4.  **Format**: **PDF**.
5.  Кликните **Run**.
6.  Преузмите **PDF**.
7.  На порталу еПорези, у секцији **8. Докази уз пријаву**, обришите унос (ако постоји) и отпремите овај фајл.

## 5. Извоз пуне историје (за `import` команду)

Ако треба да учитате историју трансакција старију од 1 године (недоступно преко редовног Flex Web Service-а), користите CSV извоз:

1.  У IBKR идите на **Performance & Reports** > **Statements** > **Activity**.
2.  **Period**: Изаберите **Complete Date Range** или **Custom Date Range** (одредите цео период од отварања рачуна).
3.  **Format**: **CSV**.
4.  Кликните **Run**.
5.  Преузмите фајл извештаја. Овај фајл се може користити са `ibkr-porez import` командом.
