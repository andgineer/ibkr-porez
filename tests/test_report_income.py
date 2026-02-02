"""Tests for PP OPO (Capital Income) XML generation."""

import allure
import pytest
from datetime import date
from decimal import Decimal

from ibkr_porez.config import UserConfig
from ibkr_porez.declaration_income_xml import IncomeXMLGenerator
from ibkr_porez.models import Currency, IncomeEntry


@allure.epic("Tax")
@allure.feature("PP OPO (income)")
class TestIncomeXMLGenerator:
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
    def sample_entries(self):
        return [
            IncomeEntry(
                date=date(2025, 12, 24),
                symbol="VOO",
                amount=Decimal("21.25"),
                currency=Currency.USD,
                amount_rsd=Decimal("2113.28"),
                exchange_rate=Decimal("99.45"),
                income_type="dividend",
                description="VOO(US9229083632) CASH DIVIDEND USD 1.771 PER SHARE",
            ),
            IncomeEntry(
                date=date(2025, 12, 24),
                symbol="SGOV",
                amount=Decimal("87.55"),
                currency=Currency.USD,
                amount_rsd=Decimal("8706.70"),
                exchange_rate=Decimal("99.45"),
                income_type="dividend",
                description="SGOV(US46436E7186) CASH DIVIDEND USD 0.323046 PER SHARE",
            ),
        ]

    def test_xml_generator_structure(self, config, sample_entries):
        """Test that XML structure is correct."""
        generator = IncomeXMLGenerator(config)
        xml_out = generator.generate_xml(sample_entries, date(2025, 12, 24), "dividend")

        # Verify root element
        assert "ns1:PodaciPoreskeDeklaracije" in xml_out
        assert 'xmlns:ns1="http://pid.purs.gov.rs"' in xml_out

        # Verify PodaciOPrijavi
        assert "<ns1:VrstaPrijave>1</ns1:VrstaPrijave>" in xml_out
        assert "<ns1:ObracunskiPeriod>2025-12</ns1:ObracunskiPeriod>" in xml_out
        assert "<ns1:DatumOstvarivanjaPrihoda>2025-12-24</ns1:DatumOstvarivanjaPrihoda>" in xml_out
        assert "<ns1:Rok>1</ns1:Rok>" in xml_out
        assert "<ns1:DatumDospelostiObaveze>" in xml_out

        # Verify PodaciOPoreskomObvezniku
        assert "2403971060012" in xml_out
        assert "<![CDATA[Andrei Sorokin]]>" in xml_out or "Andrei Sorokin" in xml_out
        assert "223" in xml_out  # city_code
        assert "0637735698" in xml_out
        assert "andreyandex@gmail.com" in xml_out

        # Verify PodaciONacinuOstvarivanjaPrihoda
        assert "<ns1:NacinIsplate>3</ns1:NacinIsplate>" in xml_out
        assert "Isplata na brokerski racun" in xml_out

        # Verify DeklarisaniPodaciOVrstamaPrihoda
        assert "<ns1:DeklarisaniPodaciOVrstamaPrihoda>" in xml_out
        assert "<ns1:PodaciOVrstamaPrihoda>" in xml_out
        assert "<ns1:SifraVrstePrihoda>111402000</ns1:SifraVrstePrihoda>" in xml_out  # Dividends

        # Verify totals
        total_bruto = Decimal("2113.28") + Decimal("8706.70")
        assert f"{total_bruto:.2f}" in xml_out  # BrutoPrihod

        # Verify Ukupno section
        assert "<ns1:Ukupno>" in xml_out
        assert "<ns1:Kamata>" in xml_out

    def test_xml_generator_coupon_type(self, config):
        """Test XML generation for coupon (interest) type."""
        entries = [
            IncomeEntry(
                date=date(2025, 12, 24),
                symbol="BOND",
                amount=Decimal("100.00"),
                currency=Currency.USD,
                amount_rsd=Decimal("9945.00"),
                exchange_rate=Decimal("99.45"),
                income_type="coupon",
                description="Bond interest payment",
            ),
        ]

        generator = IncomeXMLGenerator(config)
        xml_out = generator.generate_xml(entries, date(2025, 12, 24), "coupon")

        # Verify coupon code
        assert "<ns1:SifraVrstePrihoda>111403000</ns1:SifraVrstePrihoda>" in xml_out  # Interest

    def test_xml_generator_tax_calculation(self, config, sample_entries):
        """Test that tax calculation is correct."""
        generator = IncomeXMLGenerator(config)
        xml_out = generator.generate_xml(sample_entries, date(2025, 12, 24), "dividend")

        # Total bruto = 2113.28 + 8706.70 = 10819.98
        total_bruto = Decimal("10819.98")
        # Tax rate = 15%
        expected_tax = total_bruto * Decimal("0.15")  # 1622.997

        # Verify tax calculation in XML
        assert f"{expected_tax:.2f}" in xml_out or str(int(expected_tax)) in xml_out

    def test_xml_generator_empty_entries(self, config):
        """Test that generator handles empty entries gracefully."""
        generator = IncomeXMLGenerator(config)

        # Should not crash with empty list
        xml_out = generator.generate_xml([], date(2025, 12, 24), "dividend")

        # Should still generate valid XML structure
        assert "ns1:PodaciPoreskeDeklaracije" in xml_out
        assert "<ns1:BrutoPrihod>0.00</ns1:BrutoPrihod>" in xml_out
