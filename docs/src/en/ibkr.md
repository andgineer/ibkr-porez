# Interactive Brokers (IBKR)

## Flex Web Service

1. **Performance & Reports** > **Flex Queries**.
2. Click the **Settings** icon (gear) in "Flex Web Service Configuration".
3. Enable **Flex Web Service**.
4. Generate **Token**.
    *   **Important**: Copy this token immediately. You will not be able to see it fully again.
    *   Set expiration (recommended max - 1 year).

## Flex Query

1. **Performance & Reports** > **Flex Queries**.
2. Click **+** to create a new **Activity Flex Query**.
3. **Name**: e.g., `ibkr-porez-data`.
4.  **Delivery Configuration** (at the bottom):
    *   **Period**: Select **Last 365 Calendar Days**.
5.  **Format**: **XML**.

### Sections to Include:

Enable the following sections and check **Select All** for columns.

If you don't trust anyone 8-) instead of **Select All**, select at least the fields listed in `Required Columns`.

### Trades
Located under Trade Confirmations or Activity.

??? Info "Required Columns"
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

### Cash Transactions
??? Info "Required Columns"
    *   `Type`
    *   `Amount`
    *   `Currency`
    *   `DateTime` / `Date`
    *   `Symbol`
    *   `Description`
    *   `TransactionID`

## Save and Get Query ID

Note the **Query ID** (number usually appearing next to the query name in the list).

You will need the **Token** and **Query ID** to configure `ibkr-porez`.

## Confirmation Document

For **Item 8 (Dokazi uz prijavu)** of the PPDG-3R tax return, you need a PDF report from the broker.
It must be attached manually on the ePorezi portal after importing the XML.

How to download the appropriate report:

1.  In IBKR go to **Performance & Reports** > **Statements** > **Activity Statement**.
2.  **Period**: Select **Custom Date Range**.
3.  Specify dates corresponding to your tax period (e.g., `01-01-2024` to `30-06-2024` for the first half).
4.  Click **Download PDF**.
5.  On the ePorezi portal, in section **8. Dokazi uz prijavu**, upload this file.

## Export Full History (for import command)

If you need to load transaction history for a period longer than 1 year (unavailable via Flex Web Service),
export data to CSV:

1.  In IBKR go to **Performance & Reports** > **Statements** > **Activity Statement**.
2.  **Period**: Select **Custom Date Range** and specify the entire period since account opening.
3.  Click **Download CSV**.
4.  This file can be used with the [import](usage.md#import-historical-data-import) command.
