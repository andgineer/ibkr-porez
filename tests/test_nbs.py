import allure
import pytest
from unittest.mock import MagicMock, patch, call
from datetime import date
from decimal import Decimal
import requests
from tenacity import RetryError

from ibkr_porez.nbs import NBSClient
from ibkr_porez.models import Currency, ExchangeRate
from ibkr_porez.storage import Storage


@pytest.fixture
def mock_storage():
    s = MagicMock(spec=Storage)
    s.get_exchange_rate.return_value = None
    return s


@pytest.fixture
def nbs_client(mock_storage):
    return NBSClient(mock_storage)


@allure.epic("Rates")
class TestNBSClient:
    def test_rsd_rate(self, nbs_client):
        """RSD should always be 1."""
        rate = nbs_client.get_rate(date(2023, 1, 1), Currency.RSD)
        assert rate == Decimal(1)
        # Should not query storage or API
        nbs_client.storage.get_exchange_rate.assert_not_called()

    def test_cache_hit(self, nbs_client):
        """If rate is in storage, use it."""
        target_date = date(2023, 1, 4)  # Wednesday
        nbs_client.storage.get_exchange_rate.return_value = ExchangeRate(
            date=target_date, currency=Currency.USD, rate=Decimal("110.0")
        )

        with patch.object(nbs_client, "_fetch_rate") as mock_fetch:
            rate = nbs_client.get_rate(target_date, Currency.USD)

        assert rate == Decimal("110.0")
        mock_fetch.assert_not_called()

    def test_offline_skip_weekend(self, nbs_client):
        """Should skip weekend days directly without API call."""
        # 2023-01-07 (Sat), 2023-01-08 (Sun). Orthodox Christmas on 7th too.
        # Let's pick a regular Sat: 2023-01-14.
        orig_date = date(2023, 1, 14)  # Saturday
        prev_friday = date(2023, 1, 13)

        # Mock storage: miss on Sat, miss on Fri (so it triggers fetch on Fri)
        nbs_client.storage.get_exchange_rate.return_value = None

        with patch.object(nbs_client, "_fetch_rate") as mock_fetch:
            mock_fetch.return_value = Decimal("108.5")

            rate = nbs_client.get_rate(orig_date, Currency.USD)

            assert rate == Decimal("108.5")

            # Should have called fetch ONLY for Friday (13th)
            # Not Saturday (14th)
            mock_fetch.assert_called_once_with(prev_friday, Currency.USD)

            # Verify saves:
            # 1. Save for Fri (actual rate date)
            # 2. Save for Sat (original request date)
            assert nbs_client.storage.save_exchange_rate.call_count == 2

    def test_offline_skip_holiday(self, nbs_client):
        """Should skip holidays using holidays lib."""
        # 2023-02-15 is Statehood Day in Serbia (Wednesday).
        orig_date = date(2023, 2, 15)
        prev_day = date(2023, 2, 14)  # Tuesday

        nbs_client.storage.get_exchange_rate.return_value = None

        with patch.object(nbs_client, "_fetch_rate") as mock_fetch:
            mock_fetch.return_value = Decimal("109.0")

            rate = nbs_client.get_rate(orig_date, Currency.USD)

            assert rate == Decimal("109.0")
            mock_fetch.assert_called_once_with(prev_day, Currency.USD)

    @patch("ibkr_porez.nbs.requests.get")
    def test_fetch_rate_api_success(self, mock_get, nbs_client):
        """Test _fetch_rate logic regarding API response."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "code": "USD",
            "date": "2023-01-13",
            "exchange_middle": 108.5555,
        }
        mock_get.return_value = mock_response

        rate = nbs_client._fetch_rate(date(2023, 1, 13), Currency.USD)

        assert rate == Decimal("108.5555")
        mock_get.assert_called_once()
        assert "currencies/usd/rates/2023-01-13" in mock_get.call_args[0][0]

    @patch("ibkr_porez.nbs.requests.get")
    @patch("time.sleep")  # Mock sleep to speed up retry test
    def test_fetch_rate_api_retry(self, mock_sleep, mock_get, nbs_client):
        """Test basic retry logic (simulated)."""
        # Note: mocking tenacity is hard in unit tests if not careful.
        # But we can verify it retries on exception.

        mock_response = MagicMock()
        mock_response.raise_for_status.side_effect = requests.RequestException("Fail")
        mock_get.return_value = mock_response

        # We expect RetryError after attempts
        with pytest.raises(RetryError):
            nbs_client._fetch_rate(date(2023, 1, 1), Currency.USD)

        assert mock_get.call_count >= 3

    def test_fallback_search(self, nbs_client):
        """
        Scenario: Working day fetch fails (e.g. API error or missing data).
        Should continue looking back.
        """
        # Pick safe dates without holidays (e.g. June)
        target_date = date(2023, 6, 14)  # Wed
        # Mock:
        # Jun 14 (Wed): Fetch Fail (Exception)
        # Jun 13 (Tue): Fetch Success

        nbs_client.storage.get_exchange_rate.return_value = None

        with patch.object(nbs_client, "_fetch_rate") as mock_fetch:
            # Side effect: First call raises, Second returns
            mock_fetch.side_effect = [Exception("API Error"), Decimal("110.0")]

            rate = nbs_client.get_rate(target_date, Currency.USD)

            assert rate == Decimal("110.0")

            assert mock_fetch.call_count == 2
            mock_fetch.assert_has_calls(
                [call(date(2023, 6, 14), Currency.USD), call(date(2023, 6, 13), Currency.USD)]
            )

    def test_give_up_after_limit(self, nbs_client):
        """Should give up after 10 days."""
        target_date = date(2023, 1, 20)

        nbs_client.storage.get_exchange_rate.return_value = None

        with patch.object(nbs_client, "_fetch_rate") as mock_fetch:
            mock_fetch.side_effect = Exception("No")

            rate = nbs_client.get_rate(target_date, Currency.USD)

            assert rate is None
            # Loop runs 10 times, but skips weekends.
            # 10 days back from Jan 20 involves some weekends (14, 15).
            # Expected calls: 20(F), 19(Th), 18(W), 17(Tu), 16(M), [Skip 15,14], 13(F), 12(Th), 11(W), 10(Tu).
            # Total ~8 calls?
            # Actually loop `range(10)` limits iterations, not days.
            # Wait, code: `for _ in range(10): ... target_date -= 1`.
            # So it checks exactly 10 chronological dates.
            # If weekend, it skips fetch but consumes iteration?
            # Code:
            # for _ in range(10):
            #   if weekend/holiday: date -= 1; continue
            #   fetch...
            #   if fail: date -= 1
            #
            # So yes, it scans 10 calendar days back.
            assert rate is None
