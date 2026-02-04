"""Operation for fetching data from IBKR and NBS."""

from datetime import date

from ibkr_porez.config import UserConfig
from ibkr_porez.ibkr_flex_query import IBKRClient
from ibkr_porez.models import Transaction
from ibkr_porez.nbs import NBSClient
from ibkr_porez.raw_reports import save_raw_report_with_delta
from ibkr_porez.storage import Storage


class GetOperation:
    """Operation for fetching data from IBKR and NBS."""

    def __init__(self, config: UserConfig):
        self.config = config
        self.storage = Storage()
        self.ibkr = IBKRClient(config.ibkr_token, config.ibkr_query_id)
        self.nbs = NBSClient(self.storage)

    def execute(self) -> tuple[list[Transaction], int, int]:
        """
        Execute get operation.

        Returns:
            tuple[list[Transaction], int, int]: (transactions, count_inserted, count_updated)
        """
        # 1. Fetch XML from IBKR
        xml_content_bytes = self.ibkr.fetch_latest_report()

        # 2. Save raw backup with delta compression
        report_date = date.today()
        xml_content_str = xml_content_bytes.decode("utf-8")
        save_raw_report_with_delta(self.storage, xml_content_str, report_date)

        # 3. Parse transactions
        transactions = self.ibkr.parse_report(xml_content_bytes)

        # 4. Save transactions
        count_inserted, count_updated = self.storage.save_transactions(transactions)

        # 5. Sync exchange rates
        dates_to_fetch = set()
        for tx in transactions:
            dates_to_fetch.add((tx.date, tx.currency))
            if tx.open_date:
                dates_to_fetch.add((tx.open_date, tx.currency))

        for d, curr in dates_to_fetch:
            self.nbs.get_rate(d, curr)

        return transactions, count_inserted, count_updated
