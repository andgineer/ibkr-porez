from collections import deque
from datetime import date
from decimal import Decimal

import pandas as pd

from ibkr_porez.models import Currency, TaxReportEntry
from ibkr_porez.nbs import NBSClient


class TaxCalculator:
    def __init__(self, nbs_client: NBSClient):
        self.nbs = nbs_client

    def process_trades(self, df: pd.DataFrame) -> list[TaxReportEntry]:  # noqa: C901
        """
        Process trades using FIFO matching to calculate capital gains.

        Args:
            df: DataFrame containing all transactions.

        Returns:
            List of TaxReportEntry.
        """
        if df.empty:
            return []

        # Filter for TRADES only
        trades_df = df[df["type"] == "TRADE"].copy()

        if trades_df.empty:
            return []

        # Ensure date sorting (Oldest first) for FIFO
        trades_df = trades_df.sort_values(by="date", ascending=True)

        # Group by Symbol
        grouped = trades_df.groupby("symbol")

        report_entries = []

        for symbol, group in grouped:
            # Inventory for this symbol: List of (Date, Price, QuantityRemaining, Currency)
            # Using simple list as queue
            inventory: deque[dict] = deque()

            for _, row in group.iterrows():
                qty = Decimal(str(row["quantity"]))
                price = Decimal(str(row["price"]))
                trade_date = row["date"]  # date object
                currency_str = row["currency"]

                # Ensure currency enum
                try:
                    currency = Currency(currency_str)
                except ValueError:
                    # Skip unknown currencies
                    continue

                if qty > 0:
                    # BUY: Add to inventory
                    inventory.append(
                        {
                            "date": trade_date,
                            "price": price,
                            "quantity": qty,
                            "currency": currency,
                        },
                    )
                elif qty < 0:
                    # SELL: Consume from inventory FIFO
                    sale_qty_remaining = abs(qty)
                    sale_price = price
                    sale_date = trade_date
                    sale_currency = currency

                    # Match against inventory
                    while sale_qty_remaining > 0:
                        if not inventory:
                            # Error: Selling more than we have?
                            # Or maybe position opened before our data history.
                            # In this case, we assume 0 cost basis (100% gain) as fallback?
                            # Or log a warning and use sale date as open date with 0 price?
                            # Let's use 0 price and sale date as purchase date to avoid crashing.
                            # Taxman will take 15% of full amount.
                            matched_qty = sale_qty_remaining
                            purchase_date = sale_date  # No history
                            purchase_price = Decimal(0)
                            # purchase_currency = sale_currency  # Unused

                            self._create_entry(
                                report_entries,
                                str(symbol),
                                Decimal(matched_qty),
                                sale_date,
                                sale_price,
                                sale_currency,
                                purchase_date,
                                purchase_price,
                            )
                            sale_qty_remaining = Decimal(0)
                            break

                        # Peek at oldest lot
                        lot = inventory[0]

                        if lot["quantity"] <= sale_qty_remaining:
                            # Consume entire lot
                            matched_qty = lot["quantity"]
                            inventory.popleft()  # Remove empty lot
                            sale_qty_remaining -= matched_qty
                        else:
                            # Partial lot consumption
                            matched_qty = sale_qty_remaining
                            lot["quantity"] -= matched_qty
                            sale_qty_remaining = Decimal(0)

                        # Create Report Entry for this segment
                        self._create_entry(
                            report_entries,
                            str(symbol),
                            Decimal(matched_qty),
                            sale_date,
                            sale_price,
                            sale_currency,
                            lot["date"],
                            lot["price"],
                        )

        return report_entries

    def _create_entry(  # noqa: PLR0913
        self,
        entries_list: list[TaxReportEntry],
        ticker: str,
        quantity: Decimal,
        sale_date: date,
        sale_price: Decimal,
        sale_currency: Currency,
        purchase_date: date,
        purchase_price: Decimal,
    ):
        # 1. Get Exchange Rates
        rate_sale = self.nbs.get_rate(sale_date, sale_currency)
        rate_purchase = self.nbs.get_rate(
            purchase_date,
            sale_currency,
        )  # Assuming buy/sell same currency for now

        if rate_sale is None:
            print(f"DEBUG: Missing Sale Rate for {sale_date}")
            rate_sale = Decimal(0)

        if rate_purchase is None:
            print(f"DEBUG: Missing Purchase Rate for {purchase_date}")
            rate_purchase = Decimal(0)

        # 2. Calculate Values in RSD
        sale_value_rsd = quantity * sale_price * rate_sale
        purchase_value_rsd = quantity * purchase_price * rate_purchase

        capital_gain = sale_value_rsd - purchase_value_rsd

        # 3. Check Tax Exemption (10 years)
        # 10 years = 365 * 10? Or Date comparison.
        # Logic: If sale_year - purchase_year > 10... specific rules.
        # Generally 10 full years.

        from dateutil.relativedelta import relativedelta

        ten_years_ago = sale_date - relativedelta(years=10)
        is_exempt = purchase_date <= ten_years_ago

        # Tax can't be negative for the report sum usually, but Capital Gain can be negative (Loss).

        entry = TaxReportEntry(
            ticker=ticker,
            quantity=quantity,
            sale_date=sale_date,
            sale_price=sale_price,
            sale_exchange_rate=round(rate_sale, 4),
            sale_value_rsd=round(sale_value_rsd, 2),
            purchase_date=purchase_date,
            purchase_price=purchase_price,
            purchase_exchange_rate=round(rate_purchase, 4),
            purchase_value_rsd=round(purchase_value_rsd, 2),
            capital_gain_rsd=round(capital_gain, 2),
            is_tax_exempt=is_exempt,
        )
        entries_list.append(entry)
