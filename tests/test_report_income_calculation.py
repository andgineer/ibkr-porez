"""Tests for PP OPO calculation accuracy using real data."""

import allure
import json
import pytest
from datetime import date
from decimal import Decimal
from pathlib import Path
from unittest.mock import patch

from ibkr_porez.config import UserConfig
from ibkr_porez.declaration_income_xml import IncomeXMLGenerator
from ibkr_porez.models import Currency, IncomeEntry
from ibkr_porez.report_income import IncomeReportGenerator
from ibkr_porez.storage import Storage
from ibkr_porez.models import Transaction, TransactionType


@allure.epic("Tax")
@allure.feature("PP OPO (income)")
class TestIncomeCalculationAccuracy:
    """Test calculation accuracy against real data from 12.xml and 13.xml."""

    @pytest.fixture
    def config(self):
        return UserConfig(
            personal_id="2403971060012",
            full_name="Andrei Sorokin",
            address="Novi Sad, 21137, Нови Сад, Стевана Мокрањца, 4 / 6 / 27",
            city_code="223",
            phone="0637735698",
            email="andreyandex@gmail.com",
        )

    @pytest.fixture
    def rates_data(self):
        """Load rates from rates.json."""
        rates_file = Path(__file__).parent.parent / "rates.json"
        if rates_file.exists():
            with open(rates_file, encoding="utf-8") as f:
                return json.load(f)
        return {}

    def test_sgov_calculation_accuracy(self, config, rates_data):
        """
        Test SGOV calculation matches 12.xml:
        - BrutoPrihod: 8706.70
        - ObracunatiPorez: 1306.01
        - PorezPlacenDrugojDrzavi: 2612.51
        - PorezZaUplatu: 0.00
        """
        # SGOV dividend: 87.55 USD
        # Rate for 2025-12-24: 99.4483
        # Expected: 87.55 * 99.4483 = 8706.70 RSD
        rate = Decimal(str(rates_data.get("2025-12-24_USD", "99.4483")))
        dividend_usd = Decimal("87.55")
        expected_bruto = Decimal("8706.70")

        bruto_calculated = round(dividend_usd * rate, 2)
        assert bruto_calculated == expected_bruto, (
            f"Expected {expected_bruto}, got {bruto_calculated}"
        )

        # Tax calculation: 15% (use ROUND_HALF_UP for taxes)
        from decimal import ROUND_HALF_UP

        expected_tax = Decimal("1306.01")
        tax_calculated = (bruto_calculated * Decimal("0.15")).quantize(
            Decimal("0.01"), rounding=ROUND_HALF_UP
        )
        assert tax_calculated == expected_tax, f"Expected {expected_tax}, got {tax_calculated}"

        # Withholding tax: 26.27 USD
        withholding_usd = Decimal("26.27")
        withholding_rsd = round(withholding_usd * rate, 2)
        expected_withholding = Decimal("2612.51")
        assert withholding_rsd == expected_withholding, (
            f"Expected {expected_withholding}, got {withholding_rsd}"
        )

        # Generate XML and verify
        entries = [
            IncomeEntry(
                date=date(2025, 12, 24),
                symbol="SGOV",
                amount=dividend_usd,
                currency=Currency.USD,
                amount_rsd=bruto_calculated,
                exchange_rate=rate,
                income_type="dividend",
                description="SGOV CASH DIVIDEND",
            ),
        ]

        generator = IncomeXMLGenerator(config)
        xml_out = generator.generate_xml(entries, date(2025, 12, 24), "dividend", withholding_rsd)

        assert f"<ns1:BrutoPrihod>{expected_bruto:.2f}</ns1:BrutoPrihod>" in xml_out
        assert f"<ns1:ObracunatiPorez>{expected_tax:.2f}</ns1:ObracunatiPorez>" in xml_out
        assert (
            f"<ns1:PorezPlacenDrugojDrzavi>{expected_withholding:.2f}</ns1:PorezPlacenDrugojDrzavi>"
            in xml_out
        )
        assert "<ns1:PorezZaUplatu>0.00</ns1:PorezZaUplatu>" in xml_out

    def test_voo_calculation_accuracy(self, config, rates_data):
        """
        Test VOO calculation matches 13.xml:
        - BrutoPrihod: 2113.28
        - ObracunatiPorez: 316.99
        - PorezPlacenDrugojDrzavi: 634.48
        - PorezZaUplatu: 0.00
        """
        # VOO dividend: 21.25 USD
        # Rate for 2025-12-24: 99.4483
        # Expected: 21.25 * 99.4483 = 2113.28 RSD
        rate = Decimal(str(rates_data.get("2025-12-24_USD", "99.4483")))
        dividend_usd = Decimal("21.25")
        expected_bruto = Decimal("2113.28")

        bruto_calculated = round(dividend_usd * rate, 2)
        assert bruto_calculated == expected_bruto, (
            f"Expected {expected_bruto}, got {bruto_calculated}"
        )

        # Tax calculation: 15% (use ROUND_HALF_UP for taxes)
        from decimal import ROUND_HALF_UP

        expected_tax = Decimal("316.99")
        tax_calculated = (bruto_calculated * Decimal("0.15")).quantize(
            Decimal("0.01"), rounding=ROUND_HALF_UP
        )
        assert tax_calculated == expected_tax, f"Expected {expected_tax}, got {tax_calculated}"

        # Withholding tax: 6.38 USD
        withholding_usd = Decimal("6.38")
        withholding_rsd = round(withholding_usd * rate, 2)
        expected_withholding = Decimal("634.48")
        assert withholding_rsd == expected_withholding, (
            f"Expected {expected_withholding}, got {withholding_rsd}"
        )

        # Generate XML and verify
        entries = [
            IncomeEntry(
                date=date(2025, 12, 24),
                symbol="VOO",
                amount=dividend_usd,
                currency=Currency.USD,
                amount_rsd=bruto_calculated,
                exchange_rate=rate,
                income_type="dividend",
                description="VOO CASH DIVIDEND",
            ),
        ]

        generator = IncomeXMLGenerator(config)
        xml_out = generator.generate_xml(entries, date(2025, 12, 24), "dividend", withholding_rsd)

        assert f"<ns1:BrutoPrihod>{expected_bruto:.2f}</ns1:BrutoPrihod>" in xml_out
        assert f"<ns1:ObracunatiPorez>{expected_tax:.2f}</ns1:ObracunatiPorez>" in xml_out
        assert (
            f"<ns1:PorezPlacenDrugojDrzavi>{expected_withholding:.2f}</ns1:PorezPlacenDrugojDrzavi>"
            in xml_out
        )
        assert "<ns1:PorezZaUplatu>0.00</ns1:PorezZaUplatu>" in xml_out

    @patch("ibkr_porez.report_income.NBSClient")
    @patch("ibkr_porez.report_income.config_manager")
    def test_e2e_calculation_with_real_data(
        self,
        mock_config_manager,
        mock_nbs_cls,
        tmp_path,
        config,
        rates_data,
    ):
        """Test end-to-end calculation with real transaction data."""
        # Setup mocks
        mock_config_manager.load_config.return_value = config
        mock_nbs = mock_nbs_cls.return_value

        def get_rate_side_effect(date_obj, currency):
            if currency == Currency.USD:
                key = f"{date_obj.isoformat()}_USD"
                rate_str = rates_data.get(key, "99.4483")
                return Decimal(rate_str)
            return None

        mock_nbs.get_rate.side_effect = get_rate_side_effect

        # Setup storage with real data
        with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
            storage = Storage()
            storage._ensure_dirs()

            # Add transactions for 2025-12-24
            transactions = [
                Transaction(
                    transaction_id="div_sgov",
                    date=date(2025, 12, 24),
                    type=TransactionType.DIVIDEND,
                    symbol="SGOV",
                    description="SGOV CASH DIVIDEND",
                    quantity=Decimal(0),
                    price=Decimal(0),
                    amount=Decimal("87.55"),
                    currency=Currency.USD,
                ),
                Transaction(
                    transaction_id="tax_sgov",
                    date=date(2025, 12, 24),
                    type=TransactionType.WITHHOLDING_TAX,
                    symbol="SGOV",
                    description="SGOV US TAX",
                    quantity=Decimal(0),
                    price=Decimal(0),
                    amount=Decimal("-26.27"),  # Negative
                    currency=Currency.USD,
                ),
                Transaction(
                    transaction_id="div_voo",
                    date=date(2025, 12, 24),
                    type=TransactionType.DIVIDEND,
                    symbol="VOO",
                    description="VOO CASH DIVIDEND",
                    quantity=Decimal(0),
                    price=Decimal(0),
                    amount=Decimal("21.25"),
                    currency=Currency.USD,
                ),
                Transaction(
                    transaction_id="tax_voo",
                    date=date(2025, 12, 24),
                    type=TransactionType.WITHHOLDING_TAX,
                    symbol="VOO",
                    description="VOO US TAX",
                    quantity=Decimal(0),
                    price=Decimal(0),
                    amount=Decimal("-6.38"),  # Negative
                    currency=Currency.USD,
                ),
            ]

            storage.save_transactions(transactions)

            # Generate reports
            generator = IncomeReportGenerator()
            results = list(
                generator.generate(
                    start_date=date(2025, 12, 24),
                    end_date=date(2025, 12, 24),
                )
            )

            # Should create 2 separate declarations (one for SGOV, one for VOO)
            assert len(results) == 2

            # Find SGOV declaration (filename format: ppopo-sgov-yyyy-mmdd.xml)
            sgov_result = next((r for r in results if "sgov" in r[0].lower()), None)
            assert sgov_result is not None, "SGOV declaration not found"

            # Verify SGOV file exists and contains correct values
            sgov_file = sgov_result[0]
            assert Path(sgov_file).exists()

            with open(sgov_file, encoding="utf-8") as f:
                sgov_xml = f.read()

            assert "<ns1:BrutoPrihod>8706.70</ns1:BrutoPrihod>" in sgov_xml
            assert "<ns1:ObracunatiPorez>1306.01</ns1:ObracunatiPorez>" in sgov_xml
            assert "<ns1:PorezPlacenDrugojDrzavi>2612.51</ns1:PorezPlacenDrugojDrzavi>" in sgov_xml

            # Find VOO declaration (filename format: ppopo-voo-yyyy-mmdd.xml)
            voo_result = next((r for r in results if "voo" in r[0].lower()), None)
            assert voo_result is not None, "VOO declaration not found"

            # Verify VOO file exists and contains correct values
            voo_file = voo_result[0]
            assert Path(voo_file).exists()

            with open(voo_file, encoding="utf-8") as f:
                voo_xml = f.read()

            assert "<ns1:BrutoPrihod>2113.28</ns1:BrutoPrihod>" in voo_xml
            assert "<ns1:ObracunatiPorez>316.99</ns1:ObracunatiPorez>" in voo_xml
            assert "<ns1:PorezPlacenDrugojDrzavi>634.48</ns1:PorezPlacenDrugojDrzavi>" in voo_xml
