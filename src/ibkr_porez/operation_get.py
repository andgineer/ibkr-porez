"""Operation for fetching data from IBKR and NBS."""

from datetime import date

from ibkr_porez.config import UserConfig
from ibkr_porez.ibkr_flex_query import IBKRClient
from ibkr_porez.models import Transaction
from ibkr_porez.nbs import NBSClient
from ibkr_porez.storage import Storage
from ibkr_porez.storage_flex_queries import save_raw_report_with_delta


class GetOperation:
    """Operation for fetching data from IBKR and NBS."""

    def __init__(self, config: UserConfig | None = None):
        self.config = config
        self.storage = Storage()
        if config:
            self.ibkr = IBKRClient(config.ibkr_token, config.ibkr_query_id)
        else:
            # For import-flex, we don't need real credentials
            self.ibkr = IBKRClient("dummy", "dummy")
        self.nbs = NBSClient(self.storage)

    def process_flex_query(
        self,
        xml_content: str | bytes,
        report_date: date,
    ) -> tuple[list[Transaction], int, int]:
        """
        Process flex query XML content (common logic for get and import-flex).

        Args:
            xml_content: XML content as string or bytes
            report_date: Date of the report

        Returns:
            tuple[list[Transaction], int, int]: (transactions, count_inserted, count_updated)
        """
        # Convert to bytes if string
        if isinstance(xml_content, str):
            xml_content_bytes = xml_content.encode("utf-8")
            xml_content_str = xml_content
        else:
            xml_content_bytes = xml_content
            xml_content_str = xml_content.decode("utf-8")

        save_raw_report_with_delta(self.storage, xml_content_str, report_date)
        transactions = self.ibkr.parse_report(xml_content_bytes)
        count_inserted, count_updated = self.storage.save_transactions(transactions)
        dates_to_fetch = set()
        for tx in transactions:
            dates_to_fetch.add((tx.date, tx.currency))
            if tx.open_date:
                dates_to_fetch.add((tx.open_date, tx.currency))

        for d, curr in dates_to_fetch:
            self.nbs.get_rate(d, curr)

        return transactions, count_inserted, count_updated

    def execute(self) -> tuple[list[Transaction], int, int]:
        """
        Execute get operation.

        Returns:
            tuple[list[Transaction], int, int]: (transactions, count_inserted, count_updated)
        """
        xml_content_bytes = self.ibkr.fetch_latest_report()
        report_date = date.today()
        return self.process_flex_query(xml_content_bytes, report_date)
