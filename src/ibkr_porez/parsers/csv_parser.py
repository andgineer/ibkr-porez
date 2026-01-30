import csv
from datetime import datetime
from decimal import Decimal
from typing import TextIO

from ibkr_porez.models import Currency, Transaction, TransactionType


class CSVParser:
    """Parses IBKR Activity Statement CSV files."""

    def parse(self, file_content: TextIO) -> list[Transaction]:  # noqa: C901
        """
        Parse CSV content and return a list of Transactions.
        Expects standard IBKR Activity Statement CSV export.
        """
        reader = csv.reader(file_content)
        transactions: list[Transaction] = []
        trades_header_map = {}
        divs_header_map = {}

        for row in reader:
            if not row:
                continue

            section = row[0]

            if section == "Trades" and row[1] == "Header":
                trades_header_map = {col: i for i, col in enumerate(row)}
                continue

            if section in {"Dividends", "Withholding Tax"} and row[1] == "Header":
                divs_header_map = {col: i for i, col in enumerate(row)}
                continue

            # Parse Data Rows
            if section == "Trades" and row[1] == "Data":
                if not trades_header_map:
                    continue  # No header seen yet?
                t = self._parse_trade_row(row, trades_header_map)
                if t:
                    transactions.append(t)

            elif section in {"Dividends", "Withholding Tax"} and row[1] == "Data":
                if not divs_header_map:
                    continue
                t = self._parse_dividend_row(row, divs_header_map, section)
                if t:
                    transactions.append(t)

        return transactions

    def _get_val(self, row, header_map, col_name, default=None):
        idx = header_map.get(col_name)
        if idx is not None and idx < len(row):
            return row[idx]
        return default

    def _parse_trade_row(self, row, header_map) -> Transaction | None:
        # Check Asset Category if needed (Stocks only?)
        asset_cat = self._get_val(row, header_map, "Asset Category")
        if asset_cat and asset_cat not in {"Stocks", "Equity"}:
            return None

        symbol = self._get_val(row, header_map, "Symbol")
        dt_str = self._get_val(row, header_map, "Date/Time")  # YYYY-MM-DD, HH:MM:SS
        qty_str = self._get_val(row, header_map, "Quantity")
        price_str = self._get_val(row, header_map, "T. Price")
        proceeds_str = self._get_val(row, header_map, "Proceeds")
        curr_str = self._get_val(row, header_map, "Currency")

        if not (symbol and dt_str and qty_str and price_str and curr_str):
            return None

        # Parse Date
        try:
            if "," in dt_str:
                d = datetime.strptime(dt_str, "%Y-%m-%d, %H:%M:%S").date()
            else:
                d = datetime.strptime(dt_str, "%Y-%m-%d").date()
        except ValueError:
            return None

        # Parse Numbers (remove commas)
        try:
            qty = Decimal(qty_str.replace(",", ""))
            price = Decimal(price_str.replace(",", ""))
            proceeds = Decimal(proceeds_str.replace(",", "")) if proceeds_str else Decimal(0)
        except (ValueError, TypeError):  # Fix broad exception
            return None

        # Currency
        try:
            c_enum = Currency(curr_str)
        except ValueError:
            return None

        # Transaction ID
        tx_id = self._get_val(row, header_map, "Transaction ID")
        if not tx_id:
            # Fallback synthesis
            tx_id = f"csv-{symbol}-{dt_str}-{qty_str}-{price_str}"

        return Transaction(
            transaction_id=tx_id,
            date=d,
            type=TransactionType.TRADE,
            symbol=symbol,
            description=f"Imported Trade {symbol}",
            quantity=qty,
            price=price,
            amount=proceeds,
            currency=c_enum,
        )

    def _parse_dividend_row(self, row, header_map, section) -> Transaction | None:
        desc = self._get_val(row, header_map, "Description")
        dt_str = self._get_val(row, header_map, "Date")
        curr_str = self._get_val(row, header_map, "Currency")
        amount_str = self._get_val(row, header_map, "Amount")

        symbol = self._get_val(row, header_map, "Symbol")
        if not symbol and desc:
            # Try Extract
            pass

        if not (dt_str and amount_str and curr_str):
            return None

        try:
            d = datetime.strptime(dt_str, "%Y-%m-%d").date()
        except ValueError:
            return None

        try:
            amt = Decimal(amount_str.replace(",", ""))
        except (ValueError, TypeError):  # Fix broad exception
            return None

        try:
            c_enum = Currency(curr_str)
        except ValueError:
            return None

        t_type = (
            TransactionType.DIVIDEND if section == "Dividends" else TransactionType.WITHHOLDING_TAX
        )

        # ID
        tx_id = self._get_val(row, header_map, "Transaction ID")
        if not tx_id:
            tx_id = f"csv-{section}-{dt_str}-{amount_str}-{curr_str}"

        return Transaction(
            transaction_id=tx_id,
            date=d,
            type=t_type,
            symbol=symbol or "UNKNOWN",
            description=desc or section,
            quantity=Decimal(0),
            price=Decimal(0),
            amount=amt,
            currency=c_enum,
        )
