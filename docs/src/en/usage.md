# Usage Guide

## Installation

Prerequisites:
*   Python 3.10+
*   `uv` or `pip`

```bash
# Clone the repository
git clone https://github.com/andgineer/ibkr-porez.git
cd ibkr-porez

# Install
uv sync
# OR
pip install .
```

## Configuration

Before first use, run the config command to save your IBKR credentials and personal details.

```bash
ibkr-porez config
```

You will be prompted for:
*   **IBKR Flex Token**: (From IBKR Settings)
*   **IBKR Query ID**: (From your saved Flex Query)
*   **Personal ID**: (JMBG for tax forms)
*   **Full Name**: (For tax forms)
*   **Address**: (For tax forms)
*   **City Code**: (Municipality code, e.g. 223 or 1)
*   **Phone**: (Contact phone)
*   **Email**: (Contact email)

## 1. Fetch Data (`get`)

Downloads the latest data from IBKR and syncs exchange rates from NBS (National Bank of Serbia).

```bash
ibkr-porez get

# Force full update (ignore local history and fetch last 365 days)
ibkr-porez get --force
```

*   Downloads XML report using your Token/Query ID.
*   Parses new transactions (Trades, Dividends).
*   Saves them to local storage.
*   Fetches historical exchange rates for all transaction dates.
*   **Default**: Incrementally fetches only new data (since your last transaction).
*   **--force**: use this if you want to refresh older data.

## 2. Show Statistics (`show`)

Displays a summary of your portfolio activity grouped by month.

```bash
ibkr-porez show
```

Shows:
*   Dividends received (in RSD).
*   Number of sales (taxable events).
*   Realized P/L (Capital Gains) estimate (in RSD).

## 3. Generate Tax Report (`report`)

Generates the PPDG-3R XML file for the Serbian Tax Administration.

```bash
# Report for the first half of 2024 (Jan 1 - Jun 30)
ibkr-porez report --year 2024 --half 1

# Report for the second half of 2024 (Jul 1 - Dec 31)
ibkr-porez report --year 2024 --half 2
```

*   **Output**: `ppdg3r_2024_H1.xml`
*   Import this file into the Serbian Tax Administration portal (ePorezi).

## Troubleshooting

*   **Missing Data**: Ensure your Flex Query in IBKR is set to cover the relevant dates (e.g., "Last 365 Days").
*   **Exchange Rates**: If NBS is down, the tool might fail to convert currencies. Try again later.
