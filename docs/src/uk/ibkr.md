# Interactive Brokers (IBKR)

## Flex Web Service

1. **Performance & Reports** > **Flex Queries**.
2. Натисніть на значок **Налаштувань** (шестерня) у "Flex Web Service Configuration".
3. Увімкніть **Flex Web Service**.
4. Згенеруйте **Token** (Generate Token).
    *   **Важливо**: Скопіюйте цей токен одразу. Ви не зможете побачити його повністю ще раз.
    *   Встановіть термін дії (рекомендується максимум - 1 рік).

## Flex Query

1. **Performance & Reports** > **Flex Queries**.
2. Натисніть **+**, щоб створити новий **Activity Flex Query**.
3. **Name**: наприклад, `ibkr-porez-data`.
4. **Delivery Configuration** (внизу сторінки):
    *   **Period**: Виберіть **Last 365 Calendar Days**.
5. **Format**: **XML**.

### Розділи для увімкнення (Sections):

Увімкніть такі розділи і позначте **Select All** (Вибрати все) для колонок.

Якщо ви нікому не довіряєте 8-) замість **Select All** виберіть щонайменше поля, перелічені в `Обов'язкові колонки`.

### Trades - Угоди
Знаходиться у розділі Trade Confirmations або Activity.

??? Info "Обов'язкові колонки"
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

### Cash Transactions - Грошові транзакції
??? Info "Обов'язкові колонки"
    *   `Type`
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## Збережіть і отримайте Query ID

Запишіть **Query ID** (число, яке зазвичай відображається поруч із назвою запиту у списку).

Вам знадобляться **Token** і **Query ID** для налаштування `ibkr-porez`.

## Документ-підтвердження {#confirmation-document}

Для **Пункту 8 (Докази уз пријаву)** податкової декларації ППДГ-3Р вам знадобиться PDF-звіт від брокера.
Його потрібно прикріпити вручну на порталі ePorezi після імпорту XML.

Як завантажити відповідний звіт:

1. У IBKR перейдіть до **Performance & Reports** > **Statements** > **Activity Statement**.
2. **Period**: Виберіть **Custom Date Range**.
3. Вкажіть дати, що відповідають вашому податковому періоду (наприклад, `01-01-2024` до `30-06-2024` для першого півріччя).
4. Натисніть **Download PDF**.
5. На порталі ePorezi, у розділі **8. Докази уз пријаву** завантажте цей файл.

## Експорт повної історії (для команди import) {#export-full-history-for-import-command}

Якщо вам потрібно завантажити історію транзакцій за період понад 1 рік (що недоступно через Flex Web Service),
експортуйте дані в CSV:

1. У IBKR перейдіть до **Performance & Reports** > **Statements** > **Activity Statement**.
2. **Period**: Виберіть **Custom Date Range** і вкажіть увесь період від моменту відкриття рахунку.
3. Натисніть **Download CSV**.
4. Цей файл можна використати з командою [import ↗](usage.md#import).
