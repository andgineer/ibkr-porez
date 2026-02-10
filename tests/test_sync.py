"""Tests for sync operation."""

import allure
import pytest
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from unittest.mock import MagicMock, patch

from ibkr_porez.config import UserConfig
from ibkr_porez.models import (
    Declaration,
    DeclarationStatus,
    DeclarationType,
    IncomeDeclarationEntry,
    TaxReportEntry,
)
from ibkr_porez.operation_sync import SyncOperation
from ibkr_porez.storage import Storage


@pytest.fixture
def mock_user_data_dir(tmp_path):
    """Mock user data directory."""
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
        mock_config = UserConfig(full_name="Test", address="Test", data_dir=None)
        with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
            s = Storage()
            s._ensure_dirs()
            yield tmp_path


@pytest.fixture
def mock_config():
    """Mock user config."""
    return UserConfig(
        ibkr_token="test_token",
        ibkr_query_id="test_query",
        personal_id="1234567890123",
        full_name="Test User",
        address="Test Address",
        city_code="223",
        phone="0601234567",
        email="test@example.com",
    )


@allure.epic("End-to-end")
@allure.feature("Sync")
class TestSyncOperation:
    """Test sync operation for creating declarations."""

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_creates_gains_declaration(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync creates gains declaration for last complete half-year."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator - no need for temp file anymore

        mock_gains_gen = MagicMock()
        mock_entry = TaxReportEntry(
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
            capital_gain_rsd=Decimal("1000.00"),
        )
        mock_gains_gen.generate.return_value = [
            ("ppdg3r-2023-H1.xml", "<?xml version='1.0'?><test>gains</test>", [mock_entry])
        ]
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income) - return empty list
        mock_income_gen = MagicMock()
        mock_income_gen.generate.return_value = []
        mock_income_gen_cls.return_value = mock_income_gen

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Mock current date to be in H2 2023 (so last complete is H1 2023)
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 8, 15)
            mock_dt.now.return_value = datetime(2023, 8, 15, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify
        assert len(declarations) == 1
        assert declarations[0].type == DeclarationType.PPDG3R
        assert declarations[0].declaration_id == "1"  # Sequential number
        assert declarations[0].status == DeclarationStatus.DRAFT
        assert declarations[0].period_start == date(2023, 1, 1)
        assert declarations[0].period_end == date(2023, 6, 30)
        assert declarations[0].metadata.get("period_start") == "2023-01-01"
        assert declarations[0].metadata.get("period_end") == "2023-06-30"
        assert declarations[0].metadata.get("gross_income_rsd") == Decimal("1000.00")
        assert declarations[0].metadata.get("tax_base_rsd") == Decimal("1000.00")
        assert declarations[0].metadata.get("calculated_tax_rsd") == Decimal("150.00")
        assert declarations[0].metadata.get("foreign_tax_paid_rsd") == Decimal("0.00")
        assert declarations[0].metadata.get("tax_due_rsd") == Decimal("150.00")

        # Verify file was created
        assert declarations[0].file_path is not None
        assert Path(declarations[0].file_path).exists()

        # Verify declaration was saved
        storage = Storage()
        saved_decl = storage.get_declaration("1")
        assert saved_decl is not None
        assert saved_decl.type == DeclarationType.PPDG3R

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_skips_income_when_no_income_found(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync skips income declarations when no income found (not an error)."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains) - should skip silently
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator - no income found (should skip, not error)
        mock_income_gen = MagicMock()
        mock_income_gen.generate.side_effect = ValueError(
            "No income (dividends/coupons) found in this period.",
        )
        mock_income_gen_cls.return_value = mock_income_gen

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Set last_declaration_date
        storage = Storage()
        storage.set_last_declaration_date(date(2023, 6, 30))

        # Mock current date to be in H2 2023
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 8, 15)
            mock_dt.now.return_value = datetime(2023, 8, 15, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify: no declarations created, but no error raised
        assert len(declarations) == 0

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_creates_income_declaration(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync creates income declarations for period."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains) - should skip silently
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator - no need for temp file anymore

        mock_income_gen = MagicMock()
        mock_entry = IncomeDeclarationEntry(
            date=date(2023, 7, 15),
            symbol_or_currency="VOO",
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("1000.00"),
            osnovica_za_porez=Decimal("1000.00"),
            obracunati_porez=Decimal("150.00"),
            porez_placen_drugoj_drzavi=Decimal("30.00"),
            porez_za_uplatu=Decimal("120.00"),
        )
        mock_income_gen.generate.return_value = [
            ("ppopo-voo-2023-0715.xml", "<?xml version='1.0'?><test>income</test>", [mock_entry])
        ]
        mock_income_gen_cls.return_value = mock_income_gen

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Set last_declaration_date
        storage = Storage()
        storage.set_last_declaration_date(date(2023, 7, 10))

        # Mock current date
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 7, 16)
            mock_dt.now.return_value = datetime(2023, 7, 16, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify
        assert len(declarations) == 1
        assert declarations[0].type == DeclarationType.PPO
        assert declarations[0].status == DeclarationStatus.DRAFT
        assert declarations[0].declaration_id == "1"  # Sequential number
        assert declarations[0].period_start == date(2023, 7, 15)
        assert declarations[0].period_end == date(2023, 7, 15)

        # Verify file was created
        assert declarations[0].file_path is not None
        assert Path(declarations[0].file_path).exists()
        assert declarations[0].metadata.get("symbol") == "VOO"

        saved_decl = storage.get_declaration("1")
        assert saved_decl is not None
        assert saved_decl.metadata.get("symbol") == "VOO"

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_skips_existing_declarations(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync skips declarations that already exist."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Create existing declaration with file_path matching the generator filename
        storage = Storage()
        existing_decl = Declaration(
            declaration_id="1",
            type=DeclarationType.PPDG3R,
            status=DeclarationStatus.DRAFT,
            period_start=date(2023, 1, 1),
            period_end=date(2023, 6, 30),
            created_at=datetime.now(),
            file_path=str(storage.data_dir / "001-ppdg3r-2023-H1.xml"),
        )
        storage.save_declaration(existing_decl)

        # Mock GainsReportGenerator - no need for temp file anymore

        mock_gains_gen = MagicMock()
        mock_entry = TaxReportEntry(
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
            capital_gain_rsd=Decimal("1000.00"),
        )
        mock_gains_gen.generate.return_value = [
            ("ppdg3r-2023-H1.xml", "<?xml version='1.0'?><test>gains</test>", [mock_entry])
        ]
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income) - return empty list
        mock_income_gen = MagicMock()
        mock_income_gen.generate.return_value = []
        mock_income_gen_cls.return_value = mock_income_gen

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Mock current date
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 8, 15)
            mock_dt.now.return_value = datetime(2023, 8, 15, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify no new declaration created
        assert len(declarations) == 0

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_updates_last_declaration_date(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync updates last_declaration_date after successful sync."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains) - should skip silently
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator - no need for temp file anymore

        mock_income_gen = MagicMock()
        mock_entry = IncomeDeclarationEntry(
            date=date(2023, 7, 15),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("1000.00"),
            osnovica_za_porez=Decimal("1000.00"),
            obracunati_porez=Decimal("150.00"),
            porez_placen_drugoj_drzavi=Decimal("30.00"),
            porez_za_uplatu=Decimal("120.00"),
        )
        mock_income_gen.generate.return_value = [
            ("ppopo-voo-2023-0715.xml", "<?xml version='1.0'?><test>income</test>", [mock_entry])
        ]
        mock_income_gen_cls.return_value = mock_income_gen

        # Set initial last_declaration_date
        storage = Storage()
        storage.set_last_declaration_date(date(2023, 7, 10))

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Mock current date
        today = date(2023, 7, 16)
        yesterday = date(2023, 7, 15)
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = today
            mock_dt.now.return_value = datetime(2023, 7, 16, 12, 0, 0)

            sync_op.execute()

        # Verify last_declaration_date was updated
        assert storage.get_last_declaration_date() == yesterday

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_handles_income_tax_not_found_error(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync raises error when income tax not found."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains) - should skip silently
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (tax not found)
        mock_income_gen = MagicMock()
        mock_income_gen.generate.side_effect = ValueError(
            "Withholding tax in IBKR for payment VOO dividend on 2023-07-15 not found"
        )
        mock_income_gen_cls.return_value = mock_income_gen

        # Set initial last_declaration_date
        storage = Storage()
        storage.set_last_declaration_date(date(2023, 7, 10))

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Mock current date
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 7, 16)
            mock_dt.now.return_value = datetime(2023, 7, 16, 12, 0, 0)

            # Should raise ValueError
            with pytest.raises(ValueError, match="Error creating PP OPO declarations"):
                sync_op.execute()

        # Verify last_declaration_date was NOT updated (error occurred)
        assert storage.get_last_declaration_date() == date(2023, 7, 10)

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_fails_when_personal_data_missing_and_keeps_last_declaration_date(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_user_data_dir,
    ):
        """Sync should fail early if personal config is incomplete."""
        incomplete_config = UserConfig(
            ibkr_token="test_token",
            ibkr_query_id="test_query",
            personal_id="",
            full_name="",
            address="",
            city_code="223",
            phone="0600000000",
            email="email@example.com",
        )

        storage = Storage()
        storage.set_last_declaration_date(date(2023, 7, 10))

        sync_op = SyncOperation(incomplete_config)

        with pytest.raises(ValueError, match="Missing personal data in configuration"):
            sync_op.execute()

        # Sync should fail before any fetching/generation and keep sync cursor unchanged.
        mock_get_op_cls.return_value.execute.assert_not_called()
        assert storage.get_last_declaration_date() == date(2023, 7, 10)

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_first_run_sets_last_declaration_date_to_30_days_ago(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that first sync sets last_declaration_date to 30 days ago."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains) - should skip silently
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income)
        mock_income_gen = MagicMock()
        mock_income_gen.generate.return_value = []
        mock_income_gen_cls.return_value = mock_income_gen

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Mock current date
        today = date(2023, 7, 16)
        yesterday = date(2023, 7, 15)
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = today
            mock_dt.now.return_value = datetime(2023, 7, 16, 12, 0, 0)

            sync_op.execute()

        # Verify last_declaration_date was set to yesterday
        # Even if no declarations created, it should update to yesterday
        storage = Storage()
        assert storage.get_last_declaration_date() == yesterday

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_first_run_uses_custom_lookback_days(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that custom first-run lookback overrides default income period start."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains)
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income)
        mock_income_gen = MagicMock()
        mock_income_gen.generate.return_value = []
        mock_income_gen_cls.return_value = mock_income_gen

        # First run with custom lookback
        sync_op = SyncOperation(mock_config, forced_lookback_days=10)

        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 7, 16)
            mock_dt.now.return_value = datetime(2023, 7, 16, 12, 0, 0)

            sync_op.execute()

        # last_declaration_date = 2023-07-06 => income start should be 2023-07-07
        mock_income_gen.generate.assert_called_once_with(
            start_date=date(2023, 7, 7),
            end_date=date(2023, 7, 15),
            force=False,
        )

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_lookback_override_ignores_saved_last_sync_date(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that lookback override is used even when last sync date exists."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains)
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income)
        mock_income_gen = MagicMock()
        mock_income_gen.generate.return_value = []
        mock_income_gen_cls.return_value = mock_income_gen

        # Persist a recent last_declaration_date that should be ignored
        storage = Storage()
        storage.set_last_declaration_date(date(2023, 7, 14))

        # Override lookback to 10 days from today
        sync_op = SyncOperation(mock_config, forced_lookback_days=10)

        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 7, 16)
            mock_dt.now.return_value = datetime(2023, 7, 16, 12, 0, 0)

            sync_op.execute()

        # Override yields last_declaration_date=2023-07-06 => start_date=2023-07-07
        mock_income_gen.generate.assert_called_once_with(
            start_date=date(2023, 7, 7),
            end_date=date(2023, 7, 15),
            force=False,
        )

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_creates_multiple_income_declarations(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync creates multiple income declarations for different days."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator (no gains) - should skip silently
        mock_gains_gen = MagicMock()
        mock_gains_gen.generate.side_effect = ValueError("No taxable sales found in this period.")
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (multiple declarations) - no need for temp files anymore

        mock_income_gen = MagicMock()
        mock_entry1 = IncomeDeclarationEntry(
            date=date(2023, 7, 15),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("1000.00"),
            osnovica_za_porez=Decimal("1000.00"),
            obracunati_porez=Decimal("150.00"),
            porez_placen_drugoj_drzavi=Decimal("30.00"),
            porez_za_uplatu=Decimal("120.00"),
        )
        mock_entry2 = IncomeDeclarationEntry(
            date=date(2023, 7, 16),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("2000.00"),
            osnovica_za_porez=Decimal("2000.00"),
            obracunati_porez=Decimal("300.00"),
            porez_placen_drugoj_drzavi=Decimal("60.00"),
            porez_za_uplatu=Decimal("240.00"),
        )
        mock_income_gen.generate.return_value = [
            ("ppopo-voo-2023-0715.xml", "<?xml version='1.0'?><test>income1</test>", [mock_entry1]),
            (
                "ppopo-sgov-2023-0716.xml",
                "<?xml version='1.0'?><test>income2</test>",
                [mock_entry2],
            ),
        ]
        mock_income_gen_cls.return_value = mock_income_gen

        # Set initial last_declaration_date
        storage = Storage()
        storage.set_last_declaration_date(date(2023, 7, 10))

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Mock current date
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 7, 17)
            mock_dt.now.return_value = datetime(2023, 7, 17, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify both declarations created
        assert len(declarations) == 2
        assert all(d.type == DeclarationType.PPO for d in declarations)
        periods = {(d.period_start, d.period_end) for d in declarations}
        assert periods == {
            (date(2023, 7, 15), date(2023, 7, 15)),
            (date(2023, 7, 16), date(2023, 7, 16)),
        }
        # Verify both declarations have sequential IDs
        declaration_ids = [d.declaration_id for d in declarations]
        assert len(declaration_ids) == 2
        assert all(did in ["1", "2"] for did in declaration_ids)
        # Verify filenames contain the expected generator filenames
        file_paths = [d.file_path for d in declarations if d.file_path]
        assert any("ppopo-voo-2023-0715" in fp for fp in file_paths)
        assert any("ppopo-sgov-2023-0716" in fp for fp in file_paths)

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_base.NBSClient")
    def test_sync_generates_proper_filename_format(
        self,
        mock_nbs_cls,
        mock_income_gen_cls,
        mock_gains_gen_cls,
        mock_get_op_cls,
        mock_config,
        mock_user_data_dir,
    ):
        """Test that sync generates filenames in proper format."""
        # Mock GetOperation
        mock_get_op = mock_get_op_cls.return_value
        mock_get_op.execute.return_value = ([], 0, 0)

        # Mock GainsReportGenerator - no need for temp file anymore

        mock_gains_gen = MagicMock()
        mock_entry = TaxReportEntry(
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
            capital_gain_rsd=Decimal("1000.00"),
        )
        mock_gains_gen.generate.return_value = [
            ("ppdg3r-2023-H1.xml", "<?xml version='1.0'?><test>gains</test>", [mock_entry])
        ]
        mock_gains_gen_cls.return_value = mock_gains_gen

        # Mock IncomeReportGenerator (no income) - return empty list
        mock_income_gen = MagicMock()
        mock_income_gen.generate.return_value = []
        mock_income_gen_cls.return_value = mock_income_gen

        # Create sync operation
        sync_op = SyncOperation(mock_config)

        # Mock current date
        with patch("ibkr_porez.operation_sync.datetime") as mock_dt:
            mock_dt.now.return_value.date.return_value = date(2023, 8, 15)
            mock_dt.now.return_value = datetime(2023, 8, 15, 12, 0, 0)

            declarations = sync_op.execute()

        # Verify filename format: nnnn-ppdg3r-yyyy-Hh.xml
        assert len(declarations) == 1
        file_path = declarations[0].file_path
        filename = Path(file_path).name
        assert filename == "001-ppdg3r-2023-H1.xml"
