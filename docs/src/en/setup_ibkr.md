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
