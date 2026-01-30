# ibkr-porez

Автоматическая генерация налогового отчета ППДГ-3Р (Капитальный доход) для пользователей Interactive Brokers в Сербии.
Программа автоматически получает данные о транзакциях и создает готовый XML-файл для загрузки, конвертируя все цены в динары (RSD).

![ППДГ-3Р](images/ppdg-3r.png)

1. [Настройка](https://andgineer.github.io/ibkr-porez/ru/setup_ibkr/): Сохраните учетные данные Interactive Brokers Flex Query и данные налогоплательщика.
    ```bash
    ibkr-porez config
    ```

2. [Загрузка данных](https://andgineer.github.io/ibkr-porez/ru/usage/#1-fetch-data-get): Скачайте историю транзакций из Interactive Brokers и официальные курсы валют Народного банка Сербии.
    ```bash
    ibkr-porez get
    ```

3. [Создание отчета](https://andgineer.github.io/ibkr-porez/ru/usage/#3-generate-tax-report-report): Сгенерируйте XML-файл ППДГ-3Р.
    ```bash
    ibkr-porez report
    ```

> Просто загрузите созданный XML на портал **ePorezi** (раздел ППДГ-3Р).
