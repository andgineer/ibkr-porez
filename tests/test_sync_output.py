"""Tests for sync command output and database persistence."""

import allure
import pytest
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from unittest.mock import MagicMock, patch

from click.testing import CliRunner

from ibkr_porez.config import UserConfig
from ibkr_porez.main import ibkr_porez
from ibkr_porez.models import (
    DeclarationType,
)
from ibkr_porez.storage import Storage


@pytest.fixture
def mock_user_data_dir(tmp_path):
    """Mock user data directory."""
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
        s = Storage()
        s._ensure_dirs()
        yield tmp_path


@pytest.fixture
def mock_config():
    """Mock user config."""
    return MagicMock(spec=UserConfig, ibkr_token="test_token", ibkr_query_id="test_query")


@allure.epic("End-to-end")
@allure.feature("Sync")
class TestSyncOutput:
    """Test sync command output and database persistence."""

    @patch("ibkr_porez.main.config_manager")
    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_shows_output_when_declarations_created(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config_manager,
        mock_config,
        mock_user_data_dir,
    ):
        """
        Test that sync shows output when declarations are created.

        This test would fail before the fix where return was outside if block.
        """
        # Mock config manager
        mock_config_manager.load_config.return_value = mock_config

        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator
        from ibkr_porez.models import TaxReportEntry as RealTaxReportEntry

        mock_gains_gen = MagicMock()
        mock_entry = RealTaxReportEntry(
            ticker="AAPL",
            sale_date=date(2023, 3, 15),
            quantity=Decimal("10"),
            sale_price=Decimal("120"),
            sale_exchange_rate=Decimal("117.0"),
            sale_value_rsd=Decimal("12000"),
            purchase_date=date(2023, 1, 10),
            purchase_price=Decimal("100"),
            purchase_exchange_rate=Decimal("117.0"),
            purchase_value_rsd=Decimal("10000"),
            capital_gain_rsd=Decimal("2000.00"),
        )
        mock_gains_gen.generate.return_value = [
            ("ppdg3r-2023-H1.xml", "<?xml version='1.0'?><test>gains</test>", [mock_entry])
        ]
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income)
        mock_income_gen = MagicMock()
        mock_income_gen.generate.return_value = []
        mock_income_gen_cls.return_value = mock_income_gen

        runner = CliRunner()

        # Mock current date to be in H2 2023 (so last complete is H1 2023)
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 8, 15)
            mock_dt.now.return_value = datetime(2023, 8, 15, 12, 0, 0)

            result = runner.invoke(ibkr_porez, ["sync"])

        # Verify output is shown
        assert result.exit_code == 0
        assert "Created" in result.output or "declaration" in result.output.lower()
        # Should show declaration ID
        assert "1" in result.output

    @patch("ibkr_porez.main.config_manager")
    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_shows_no_output_when_nothing_created(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config_manager,
        mock_config,
        mock_user_data_dir,
    ):
        """
        Test that sync shows "No new declarations created" when nothing is created.

        This test would fail before the fix where return was outside if block.
        """
        # Mock config manager
        mock_config_manager.load_config.return_value = mock_config

        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains)
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income)
        mock_income_gen = MagicMock()
        mock_income_gen.generate.side_effect = ValueError(
            "No income (dividends/coupons) found in this period.",
        )
        mock_income_gen_cls.return_value = mock_income_gen

        runner = CliRunner()

        # Mock current date
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 8, 15)
            mock_dt.now.return_value = datetime(2023, 8, 15, 12, 0, 0)

            result = runner.invoke(ibkr_porez, ["sync"])

        # Verify "No new declarations created" message is shown
        assert result.exit_code == 0
        assert "No new declarations created" in result.output

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_saves_all_declarations_to_database(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """
        Test that sync saves ALL declarations (PPDG-3R and PP OPO) to database.

        This test would fail before the fix where PPDG-3R declaration wasn't saved.
        """
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator
        from ibkr_porez.models import TaxReportEntry as RealTaxReportEntry

        mock_gains_gen = MagicMock()
        mock_gains_entry = RealTaxReportEntry(
            ticker="AAPL",
            sale_date=date(2023, 3, 15),
            quantity=Decimal("10"),
            sale_price=Decimal("120"),
            sale_exchange_rate=Decimal("117.0"),
            sale_value_rsd=Decimal("12000"),
            purchase_date=date(2023, 1, 10),
            purchase_price=Decimal("100"),
            purchase_exchange_rate=Decimal("117.0"),
            purchase_value_rsd=Decimal("10000"),
            capital_gain_rsd=Decimal("2000.00"),
        )
        mock_gains_gen.generate.return_value = [
            ("ppdg3r-2023-H1.xml", "<?xml version='1.0'?><test>gains</test>", [mock_gains_entry])
        ]
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator
        from ibkr_porez.models import IncomeDeclarationEntry as RealIncomeDeclarationEntry

        mock_income_gen = MagicMock()
        mock_income_entry = RealIncomeDeclarationEntry(
            date=date(2023, 7, 15),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("1000.00"),
            osnovica_za_porez=Decimal("1000.00"),
            obracunati_porez=Decimal("150.00"),
            porez_placen_drugoj_drzavi=Decimal("30.00"),
            porez_za_uplatu=Decimal("120.00"),
        )
        mock_income_gen.generate.return_value = [
            (
                "ppopo-voo-2023-0715.xml",
                "<?xml version='1.0'?><test>income</test>",
                [mock_income_entry],
            )
        ]
        mock_income_gen_cls.return_value = mock_income_gen

        from ibkr_porez.operation_sync import SyncOperation

        sync_op = SyncOperation(mock_config)

        # Mock current date to be in H2 2023
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 8, 15)
            mock_dt.now.return_value = datetime(2023, 8, 15, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify both declarations were created
        assert len(declarations) == 2

        # Verify PPDG-3R declaration is in the list
        gains_decl = next((d for d in declarations if d.type == DeclarationType.PPDG3R), None)
        assert gains_decl is not None, "PPDG-3R declaration should be created"
        assert gains_decl.declaration_id == "1"

        # Verify PP OPO declaration is in the list
        income_decl = next((d for d in declarations if d.type == DeclarationType.PPO), None)
        assert income_decl is not None, "PP OPO declaration should be created"
        assert income_decl.declaration_id == "2"  # Sequential global counter

        # Verify both declarations are saved to database
        storage = Storage()
        saved_declarations = storage.get_declarations()

        # Should have 2 declarations in database
        assert len(saved_declarations) == 2

        # Verify PPDG-3R is in database
        saved_gains = storage.get_declaration("1")
        assert saved_gains is not None
        assert saved_gains.type == DeclarationType.PPDG3R

        # Verify PP OPO is in database
        saved_income = storage.get_declaration("2")
        assert saved_income is not None
        assert saved_income.type == DeclarationType.PPO

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_saves_ppdg3r_when_both_types_created(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """
        Test that PPDG-3R declaration is saved when both types are created.

        This test specifically checks that PPDG-3R is not lost when processing multiple types.
        """
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator
        from ibkr_porez.models import TaxReportEntry as RealTaxReportEntry

        mock_gains_gen = MagicMock()
        mock_gains_entry = RealTaxReportEntry(
            ticker="MSFT",
            sale_date=date(2023, 9, 10),
            quantity=Decimal("5"),
            sale_price=Decimal("350"),
            sale_exchange_rate=Decimal("117.0"),
            sale_value_rsd=Decimal("204750"),
            purchase_date=date(2023, 7, 1),
            purchase_price=Decimal("300"),
            purchase_exchange_rate=Decimal("117.0"),
            purchase_value_rsd=Decimal("175500"),
            capital_gain_rsd=Decimal("29250.00"),
        )
        mock_gains_gen.generate.return_value = [
            ("ppdg3r-2023-H2.xml", "<?xml version='1.0'?><test>gains</test>", [mock_gains_entry])
        ]
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator
        from ibkr_porez.models import IncomeDeclarationEntry as RealIncomeDeclarationEntry

        mock_income_gen = MagicMock()
        mock_income_entry = RealIncomeDeclarationEntry(
            date=date(2023, 12, 24),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("5000.00"),
            osnovica_za_porez=Decimal("5000.00"),
            obracunati_porez=Decimal("750.00"),
            porez_placen_drugoj_drzavi=Decimal("750.00"),
            porez_za_uplatu=Decimal("0.00"),
        )
        mock_income_gen.generate.return_value = [
            (
                "ppopo-ko-2023-1224.xml",
                "<?xml version='1.0'?><test>income</test>",
                [mock_income_entry],
            )
        ]
        mock_income_gen_cls.return_value = mock_income_gen

        from ibkr_porez.operation_sync import SyncOperation

        sync_op = SyncOperation(mock_config)

        # Mock current date to be in 2024 (so last complete is H2 2023)
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2024, 1, 15)
            mock_dt.now.return_value = datetime(2024, 1, 15, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify both declarations were created
        assert len(declarations) == 2

        # Verify PPDG-3R declaration exists and is saved
        gains_decl = next((d for d in declarations if d.type == DeclarationType.PPDG3R), None)
        assert gains_decl is not None, "PPDG-3R declaration must be created"
        assert gains_decl.declaration_id == "1"

        # Verify in database
        storage = Storage()
        saved_gains = storage.get_declaration("1")
        assert saved_gains is not None, "PPDG-3R declaration must be saved to database"
        assert saved_gains.type == DeclarationType.PPDG3R

        # Verify file exists
        assert gains_decl.file_path is not None
        assert Path(gains_decl.file_path).exists(), "PPDG-3R file must exist"

        # Verify PP OPO is also saved
        saved_income_list = [d for d in storage.get_declarations() if d.type == DeclarationType.PPO]
        assert len(saved_income_list) == 1, "PP OPO declaration must be saved to database"
