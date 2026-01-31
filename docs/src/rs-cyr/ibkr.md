# Interactive Brokers (IBKR)

## Flex Web Service

1. **Performance & Reports** > **Flex Queries**.
2. Кликните на икону **Settings** (зупчаник) у "Flex Web Service Configuration".
3. Омогућите **Flex Web Service**.
4. Генеришите **Token**.
    *   **Важно**: Одмах копирајте овај токен. Нећете моћи поново да га видите у целости.
    *   Поставите рок трајања (препоручено max - 1 година).

## Flex Query

1. **Performance & Reports** > **Flex Queries**.
2. Кликните **+** да креирате нови **Activity Flex Query**.
3. **Name**: нпр. `ibkr-porez-data`.
4.  **Delivery Configuration** (на дну):
    *   **Period**: Изаберите **Last 365 Calendar Days**.
5.  **Format**: **XML**.

### Секције за укључивање (Sections):

Омогућите следеће секције и означите **Select All** (Изабери све) за колоне.

Ако никоме не верујете 8-) уместо **Select All** изаберите бар поља наведена у `Потребне колоне`.

### Trades - Трговине
Налази се под Trade Confirmations или Activity.

??? Info "Потребне колоне"
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

### Cash Transactions - Новчане трансакције
??? Info "Потребне колоне"
    *   `Type`
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## Сачувајте и преузмите Query ID

Забележите **Query ID** (број који се обично појављује поред имена упита у листи).

Требаће вам **Token** и **Query ID** за конфигурацију `ibkr-porez`.

## Документ потврде

За **Тачку 8 (Докази уз пријаву)** пореске пријаве ППДГ-3Р потребан вам је PDF извештај од брокера.
Мора се ручно приложити на порталу еПорези након увоза XML-а.

Како преузети одговарајући извештај:

1.  У IBKR идите на **Performance & Reports** > **Statements** > **Activity Statement**.
2.  **Period**: Изаберите **Custom Date Range**.
3.  Наведите датуме који одговарају вашем пореском периоду (нпр. `01-01-2024` до `30-06-2024` за прво полугодиште).
4.  Кликните **Download PDF**.
5.  На порталу еПорези, у секцији **8. Докази уз пријаву**, отпремите овај фајл.

## Извоз пуне историје (за import команду)

Ако треба да учитате историју трансакција за период дужи од 1 године (недоступно преко Flex Web Service-а),
извезите податке у CSV:

1.  У IBKR идите на **Performance & Reports** > **Statements** > **Activity Statement**.
2.  **Period**: Изаберите **Custom Date Range** и наведите цео период од отварања рачуна.
3.  Кликните **Download CSV**.
4.  Овај фајл се може користити са командом [import](usage.md#import).
