# Setting up Interactive Brokers (IBKR)

To use `ibkr-porez`, you need to configure a **Flex Query** in your Interactive Brokers account. This allows the tool to automatically fetch your transaction history.

## 1. Enable Flex Web Service

1.  Log in to **Interactive Brokers** (Account Management).
2.  Go to **Performance & Reports** > **Flex Queries**.
3.  Click the **Settings** icon (gear) (or look for "Flex Web Service").
4.  enable **Flex Web Service**.
5.  Generate a **Token**.
    *   **Important**: Copy this Token immediately. You will not be able to fully see it again.
    *   Set an expiration (e.g., 1 year).

## 2. Create a Flex Query

1.  In **Performance & Reports** > **Flex Queries**, click **+** to create a new **Trade Confirmation Flex Query** (or "Activity Flex Query", but strictly we need specific sections).
    *   *Note*: Usually "Activity Flex Query" is preferred for broader data, but check the sections below.
2.  **Name**: e.g., `ibkr-porez-data`.
3.  **Delivery Configuration** (at the bottom):
    *   **Period**: Select **Last 365 Calendar Days**.
    *   *Tip*: `ibkr-porez` fetches what is available. Setting "Last 365 Calendar Days" is standard for automation.
4.  **Format**: **XML**.

### Sections to Include:

Enable the following sections and check **Select All** columns to ensure compatibility (or at least the specific required columns listed).

#### A. Trades (Executions)
*   **Select**: `Trades` (under Trade Confirmations or similar in Activity).
*   **Required Columns**:
    *   `Symbol`
    *   `Description`
    *   `Currency`
    *   `Quantity`
    *   `TradePrice` (or Price)
    *   `TradeDate` (Date)
    *   `TradeID`
    *   `OrigTradeDate` (Required for P/L matching)
    *   `OrigTradePrice` (Required for P/L matching)
    *   `AssetClass` (recommended)
    *   `Buy/Sell` (recommended)

#### B. Cash Transactions
*   **Select**: `Cash Transactions`.
*   **Required Columns**:
    *   `Type` (e.g., Dividend, Withholding Tax)
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## 3. Save and Get Query ID

1.  Save the query.
2.  Note the **Query ID** (a number usually appearing next to the query name in the list).

You will use the **Token** and **Query ID** to configure `ibkr-porez`.

## 4. Confirmation Document (For Tax Filing)

For **Part 8 (Dokazi uz priјаvu / Attachments)** of the tax return, you need a PDF Activity Report from IBKR. `ibkr-porez` generates the XML with a file placeholder, but you must manually download the proof file and upload it to the ePorezi portal.

How to download the correct report:

1.  In IBKR go to **Performance & Reports** > **Statements** > **Activity**.
2.  **Period**: Select **Custom Date Range**.
3.  Specify dates matching your tax period (e.g., `01-01-2024` to `30-06-2024` for H1).
4.  **Format**: **PDF**.
5.  Click **Run**.
6.  Download the **PDF**.
7.  On the ePorezi portal, in section **8. Doкazi uz priјаvu**, delete the placeholder entry (if present) and upload this file.

## 5. Export Full History (for `import` command)

If you need to load transaction history older than 1 year (unavailable via regular Flex Web Service), use CSV export:

1.  In IBKR, go to **Performance & Reports** > **Statements** > **Activity**.
2.  **Period**: Select **Complete Date Range** or **Custom Date Range** (specify the entire period since account opening).
3.  **Format**: **CSV**.
4.  Click **Run**.
5.  Download the report file. This file can be used with the `ibkr-porez import` command.
