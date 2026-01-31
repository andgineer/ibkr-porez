import logging
from datetime import date, timedelta
from decimal import Decimal

import holidays
import requests
from tenacity import retry, stop_after_attempt, wait_fixed

from ibkr_porez.models import Currency, ExchangeRate
from ibkr_porez.storage import Storage

logger = logging.getLogger(__name__)


class NBSClient:
    BASE_URL = "https://kurs.resenje.org/api/v1"

    def __init__(self, storage: Storage):
        self.storage = storage

    def get_rate(self, date_obj: date, currency: Currency) -> Decimal | None:
        """Get NBS middle rate for currency on date. Uses cache if available."""
        if currency == Currency.RSD:
            return Decimal(1)

        # Optimization: Find the nearest previous working day offline using holidays lib
        # This avoids 404s/Timeouts for known non-working days.

        rs_holidays = holidays.country_holidays("RS")

        target_date = date_obj
        # Look back up to 10 days to be safe (Orthodox Easter + May 1st overlap could be long)
        for _ in range(10):
            # Check cache first for this specific date (maybe we found it before)
            cached = self.storage.get_exchange_rate(target_date, currency)
            if cached:
                # If we found a cached rate for a past date, that IS the rate for our original date.
                # Save it for original date too to speed up next time.
                if target_date != date_obj:
                    self.storage.save_exchange_rate(
                        ExchangeRate(date=date_obj, currency=currency, rate=cached.rate),
                    )
                return cached.rate

            saturday = 5
            is_weekend = target_date.weekday() >= saturday
            is_holiday = target_date in rs_holidays

            if is_weekend or is_holiday:
                target_date -= timedelta(days=1)
                continue

            # If we are here, it SHOULD be a working day. Fetch it.
            try:
                rate_val = self._fetch_rate(target_date, currency)
                if rate_val:
                    # Save for target_date
                    self.storage.save_exchange_rate(
                        ExchangeRate(date=target_date, currency=currency, rate=rate_val),
                    )
                    # Save for original date (date_obj) if different
                    if target_date != date_obj:
                        self.storage.save_exchange_rate(
                            ExchangeRate(date=date_obj, currency=currency, rate=rate_val),
                        )
                    return rate_val
            except Exception as e:  # noqa: BLE001
                # Real error even on working day? Or maybe unexpected holiday?
                # Continue looking back just in case, but log debug.
                logger.debug(f"Failed to fetch working day {target_date}: {e}")

            # If failed, move back
            target_date -= timedelta(days=1)

        logger.warning(f"Could not find rate for {currency} on {date_obj} (checked 10 days back)")
        return None

    @retry(stop=stop_after_attempt(3), wait=wait_fixed(2))
    def _fetch_rate(self, date_obj: date, currency: Currency) -> Decimal | None:
        # URL logic: https://kurs.resenje.org/api/v1/currencies/{currency}/rates/{date}
        url = f"{self.BASE_URL}/currencies/{currency.value.lower()}/rates/{date_obj.isoformat()}"

        response = requests.get(url, timeout=10)
        response.raise_for_status()

        data = response.json()
        # Structure: {"code": "EUR", "date": "2023-01-01", "exchange_middle": 117.32}

        if "exchange_middle" in data:
            return Decimal(str(data["exchange_middle"]))

        return None
