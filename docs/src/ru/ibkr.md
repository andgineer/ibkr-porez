# Interactive Brokers (IBKR)

## Flex Web Service

1. **Performance & Reports** > **Flex Queries**.
2. Нажмите на значок **Настроек** (шестеренка) в "Flex Web Service Configuration".
3. Включите **Flex Web Service**.
4. Сгенерируйте **Токен** (Generate Token).
    *   **Важно**: Скопируйте этот токен сразу. Вы не сможете увидеть его полностью снова.
    *   Установите срок действия (рекомендуется максимум - 1 год).

## Flex Query

1. **Performance & Reports** > **Flex Queries**.
2. Нажмите **+**, чтобы создать новый **Activity Flex Query**.
3. **Name**: например, `ibkr-porez-data`.
4.  **Delivery Configuration** (внизу страницы):
    *   **Period**: Выберите **Last 365 Calendar Days**.
5.  **Format**: **XML**.

### Разделы для включения (Sections):

Включите следующие разделы и отметьте **Select All** (Выбрать все) для колонок.

Если вы никому не доверяете 8-) вместо **Select All** выберите как минимум поля, перечисленные в `Обязательные колонки`.

### Trades - Сделки
Находится в разделе Trade Confirmations или Activity.

??? Info "Обязательные колонки"
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

### Cash Transactions - Денежные транзакции
??? Info "Обязательные колонки"
    *   `Type`
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## Сохраните и получите Query ID

Запишите **Query ID** (число, которое обычно отображается рядом с именем запроса в списке).

Вам понадобятся **Token** и **Query ID** для настройки `ibkr-porez`.

## Документ подтверждения

Для **Пункта 8 (Докази уз пријаву)** налоговой декларации ППДГ-3Р вам потребуется PDF-отчет от брокера.
Его надо прикрепить вручную на портале ePorezi после импорта XML.

Как скачать подходящий отчет:

1.  В IBKR перейдите в **Performance & Reports** > **Statements** > **Activity**.
2.  **Period**: Выберите **Custom Date Range**.
3.  Укажите даты, соответствующие вашему налоговому периоду (например, `01-01-2024` по `30-06-2024` для первого полугодия).
4.  **Format**: **PDF**.
5.  Нажмите **Run**.
6.  Скачайте полученный PDF.
7.  На портале ePorezi, в разделе **8. Докази уз пријаву** загрузите этот файл.

## Экспорт всей истории (для команды import)

Если вам нужно загрузить историю транзакций за период более 1 года (что недоступно через Flex Web Service),
экспортируйте даные в CSV:

1.  В IBKR перейдите в **Performance & Reports** > **Statements** > **Activity Statement**.
2.  **Period**: Выберите **Custom Date Range** и укажите весь период с момента открытия счета.
3.  Нажмите **Download CSV**.
4.  Этот файл можно использовать с командой [import](usage.md#import).
