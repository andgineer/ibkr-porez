# Настройка Interactive Brokers (IBKR)

Чтобы использовать `ibkr-porez`, необходимо настроить **Flex Query** в вашем аккаунте Interactive Brokers. Это позволит инструменту автоматически загружать историю ваших транзакций.

## 1. Включите Flex Web Service

1.  Войдите в **Interactive Brokers** (Управление счетом).
2.  Перейдите в **Performance & Reports** > **Flex Queries** (Эффективность и отчеты -> Flex-запросы).
3.  Нажмите на значок **Настроек** (шестеренка) (или найдите "Flex Web Service").
4.  Включите **Flex Web Service**.
5.  Сгенерируйте **Токен** (Generate Token).
    *   **Важно**: Скопируйте этот токен сразу. Вы не сможете увидеть его полностью снова.
    *   Установите срок действия (например, 1 год).

## 2. Создайте Flex Query (Flex-запрос)

1.  В **Performance & Reports** > **Flex Queries**, нажмите **+**, чтобы создать новый **Trade Confirmation Flex Query** (или "Activity Flex Query", но нам нужны конкретные разделы, см. ниже).
    *   *Примечание*: Обычно "Activity Flex Query" предпочтительнее для более полных данных, но проверьте разделы ниже.
2.  **Name** (Имя): например, `ibkr-porez-data`.
3.  **Delivery Configuration** (внизу страницы):
    *   **Period**: Выберите **Last 365 Calendar Days**.
    *   *Совет*: `ibkr-porez` загружает то, что доступно. Установка "Last 365 Calendar Days" стандартна для автоматизации.
4.  **Format** (Формат): **XML**.

### Разделы для включения (Sections):

Включите следующие разделы и отметьте **Select All** (Выбрать все) для колонок, чтобы обеспечить совместимость (или как минимум выберите обязательные поля, перечисленные ниже).

#### A. Trades (Executions) - Сделки
*   **Выберите**: `Trades` (находится в разделе Trade Confirmations или Activity).
*   **Обязательные колонки**:
    *   `Symbol`
    *   `Description`
    *   `Currency`
    *   `Quantity`
    *   `TradePrice` (или Price)
    *   `TradeDate` (Date)
    *   `TradeID`
    *   `OrigTradeDate` (Необходимо для расчета P/L)
    *   `OrigTradePrice` (Необходимо для расчета P/L)
    *   `AssetClass` (рекомендуется)
    *   `Buy/Sell` (рекомендуется)

#### B. Cash Transactions - Денежные транзакции
*   **Выберите**: `Cash Transactions`.
*   **Обязательные колонки**:
    *   `Type` (например, Dividend, Withholding Tax)
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## 3. Сохраните и получите Query ID

1.  Сохраните запрос.
2.  Запишите **Query ID** (число, которое обычно отображается рядом с именем запроса в списке).

Вам понадобятся **Token** и **Query ID** для настройки `ibkr-porez`.
