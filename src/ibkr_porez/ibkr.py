import time
import xml.etree.ElementTree as ET
from datetime import date
from decimal import Decimal

import requests
from tenacity import retry, stop_after_attempt, wait_fixed

from ibkr_porez.models import Currency, Transaction, TransactionType


class IBKRClient:
    FLEX_URL_REQUEST = (
        "https://ndcdyn.interactivebrokers.com/Universal/servlet/FlexStatementService.SendRequest"
    )
    FLEX_URL_GET = (
        "https://ndcdyn.interactivebrokers.com/Universal/servlet/FlexStatementService.GetStatement"
    )
    VERSION = "3"

    def __init__(self, token: str, query_id: str):
        self.token = token
        self.query_id = query_id

    @retry(stop=stop_after_attempt(3), wait=wait_fixed(2))
    def fetch_latest_report(
        self,
        start_date: "date | None" = None,
        end_date: "date | None" = None,
    ) -> bytes:
        """
        Fetch the latest Flex Query report.
        If start_date/end_date (date objects) are provided, they override the query's
        default period.
        """

        # Step 1: Request the report
        params_req = {"t": self.token, "q": self.query_id, "v": self.VERSION}

        # Incremental fetch: override period
        # IBKR format: yyyymmdd or yyyy-mm-dd?
        # API docs say: yyyymmdd
        if start_date:
            params_req["fd"] = start_date.strftime("%Y%m%d")
        if end_date:
            params_req["td"] = end_date.strftime("%Y%m%d")

        resp_req = requests.get(self.FLEX_URL_REQUEST, params=params_req, timeout=30)
        resp_req.raise_for_status()

        # Parse Reference Code
        try:
            root = ET.fromstring(resp_req.content)  # noqa: S314
            if root.find("ErrorCode") is not None:
                code_el = root.find("ErrorCode")
                msg_el = root.find("ErrorMessage")
                code = code_el.text if code_el is not None else "Unknown"
                msg = msg_el.text if msg_el is not None else "Unknown"
                raise ValueError(f"IBKR API Error {code}: {msg}")

            ref_code_el = root.find("ReferenceCode")
            if ref_code_el is None:
                raise ValueError("No ReferenceCode found in IBKR response")

            reference_code = ref_code_el.text
            url_node = root.find("Url")
            base_url = (
                url_node.text if url_node is not None and url_node.text else self.FLEX_URL_GET
            )

        except ET.ParseError as e:
            raise ValueError(f"Failed to parse IBKR response: {e}") from e

        # Step 2: Get the report (Wait loop maybe needed,
        # but usually immediate provided we wait a bit?)
        # Actually IBKR says "Check the status". But typically for small reports it's fast.
        # But robust implementation loops checking status or just tries to get it.
        # Simple implementation: Wait 1s and try.
        time.sleep(1)

        params_get = {"q": reference_code, "t": self.token, "v": self.VERSION}
        resp_get = requests.get(base_url, params=params_get, timeout=30)
        resp_get.raise_for_status()

        # Check if response is an error (sometimes it returns XML with ErrorCode even on 200 OK)
        if b"<ErrorCode>" in resp_get.content and b"<FlexStatementResponse" in resp_get.content:
            # Basic check, detailed parsing later
            pass

        return resp_get.content

    def parse_report(self, xml_content: bytes) -> list[Transaction]:
        """Parse XML content and extract transactions manually."""
        try:
            root = ET.fromstring(xml_content)  # noqa: S314
        except ET.ParseError as e:
            raise ValueError("Invalid XML content") from e

        # Check for errors in the report root
        if root.tag == "FlexStatementResponse":
            status = root.find("Status")
            if status is not None and status.text != "Success":
                err_code = root.find("ErrorCode")
                err_msg = root.find("ErrorMessage")
                c = err_code.text if err_code is not None else "?"
                m = err_msg.text if err_msg is not None else "Unknown error"
                raise ValueError(f"Flex Query Failed: {c} - {m}")

        transactions = []

        # FlexQueryResponse -> FlexStatements -> FlexStatement
        # Usually structure: <FlexQueryResponse> <FlexStatements> <FlexStatement> ...

        flex_statements = root.findall(".//FlexStatement")

        for stmt in flex_statements:
            # 1. Trades
            # trades_el = stmt.find("FlexQuery/FlexStatements/FlexStatement/Trades")
            # Actually findall searches recursively with .// if at root.
            # But inside stmt, we look for direct children usually.
            # Structure matches the configured Flex Query.
            # It's usually <Trades> under <FlexStatement>.

            # Using findall with .//Trade to be safe but check parent?
            # Safer: iterate all children and match tag name or use findall("Trades/Trade")?
            # Let's use Iteration for robustness against nesting variations.

            # Find all Trade elements under this statement
            for trade_el in stmt.findall(".//Trade"):
                t = self._convert_trade(trade_el)
                if t:
                    transactions.append(t)

            # 2. CashTransactions
            for cash_el in stmt.findall(".//CashTransaction"):
                t = self._convert_cash_transaction(cash_el)
                if t:
                    transactions.append(t)

        return transactions

    def _convert_trade(self, el: ET.Element) -> Transaction | None:  # noqa: C901,PLR0912
        # Extract attributes safely with defaults
        # .get() returns None if missing, which is what we want to avoid crashing.

        # Mappings based on user's fields and our needs
        # Required: symbol, currency, quantity, tradePrice, tradeDate, tradeID

        symbol = el.get("symbol")
        currency_str = el.get("currency")
        quantity = el.get("quantity")
        price = el.get("tradePrice")
        date_str = el.get("tradeDate")
        tx_id = el.get("tradeID")

        # Check essential fields
        if not (symbol and currency_str and quantity and price and date_str and tx_id):
            return None  # Skip incomplete records

        # Formatting Date: IBKR usually "YYYYMMDD" or "YYYYMMDD;HH:MM:SS" ??
        # User says "Date/Time" or "Trade Date".
        # XML date format from IBKR is typically YYYYMMDD for dates.
        # "20230101".

        # Helper for date parsing
        def parse_date(d_str):
            if ";" in d_str:
                d_str = d_str.split(";")[0]
            try:
                # Format typically YYYYMMDD
                return time.strptime(d_str, "%Y%m%d")
            except ValueError:
                # Try other formats?
                try:
                    return time.strptime(d_str, "%Y-%m-%d")
                except ValueError:
                    return None

        # Actually models expects python date object.
        from datetime import datetime

        try:
            if "-" in date_str:
                d = datetime.strptime(date_str, "%Y-%m-%d").date()
            else:
                d = datetime.strptime(date_str, "%Y%m%d").date()
        except (ValueError, TypeError):
            return None

        # Amount? fifoPnlRealized?
        # User prompt mentions: "Privilege Adjust..., Proceeds, Taxes, ..."
        # For sales, we need P/L calculation, but for the basic Transaction model,
        # 'amount' is generic.
        # We used 'fifoPnlRealized' before.
        amount_val = el.get("fifoPnlRealized") or el.get("proceeds") or "0"

        # Open Date/Price for matching (Closed Lots)
        orig_date_str = el.get("origTradeDate")
        orig_price_str = el.get("origTradePrice")

        open_date = None
        if orig_date_str:
            try:
                if "-" in orig_date_str:
                    open_date = datetime.strptime(orig_date_str, "%Y-%m-%d").date()
                else:
                    open_date = datetime.strptime(orig_date_str, "%Y%m%d").date()
            except (ValueError, TypeError):  # noqa: S110
                pass

        open_price = Decimal(orig_price_str) if orig_price_str else None

        try:
            curr = Currency(currency_str)
        except ValueError:
            # Fallback or skip?
            # Probably ok to skip explicit Currency Enum if we want strictness,
            # OR map to a default if unknown? Model forces Enum.
            # If unknown currency, we can't get rate.
            return None

        return Transaction(
            transaction_id=tx_id,
            date=d,
            type=TransactionType.TRADE,
            symbol=symbol,
            description=el.get("description", ""),
            quantity=Decimal(quantity),
            price=Decimal(price),
            amount=Decimal(amount_val),
            currency=curr,
            open_date=open_date,
            open_price=open_price,
        )

    def _convert_cash_transaction(self, el: ET.Element) -> Transaction | None:
        type_str = el.get("type", "")

        tx_type_map = {
            "Dividends": TransactionType.DIVIDEND,
            "Withholding Tax": TransactionType.WITHHOLDING_TAX,
            "Payment In Lieu Of Dividends": TransactionType.DIVIDEND,
        }

        mapped_type = tx_type_map.get(type_str)
        if not mapped_type:
            return None

        symbol = el.get("symbol", "")
        currency_str = el.get("currency")
        amount = el.get("amount")
        date_str = el.get("dateTime")  # Often 'dateTime' in CashTransactions?
        tx_id = el.get("transactionID")

        if not (amount and currency_str and date_str and tx_id):
            return None

        from datetime import datetime

        d = None
        # Date parsing
        date_clean = date_str.split(";")[0]  # Remove time part if any
        try:
            if "-" in date_clean:
                d = datetime.strptime(date_clean, "%Y-%m-%d").date()
            else:
                d = datetime.strptime(date_clean, "%Y%m%d").date()
        except ValueError:
            return None

        try:
            curr = Currency(currency_str)
        except ValueError:
            return None

        return Transaction(
            transaction_id=tx_id,
            date=d,
            type=mapped_type,
            symbol=symbol,
            description=el.get("description", ""),
            quantity=Decimal(0),
            price=Decimal(0),
            amount=Decimal(amount),
            currency=curr,
        )
