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
)
from ibkr_porez.operation_sync import SyncOperation
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
class TestSyncOperation:
    """Test sync operation for creating declarations."""

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.report_income.NBSClient")
    def test_sync_creates_gains_declaration(
        self,
        mock_nbs_income,
        mock_nbs_gains,
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

        # Mock GainsReportGenerator - create actual temp file
        temp_file = mock_user_data_dir / "ppdg3r_2023-01-01_2023-06-30.xml"
        temp_file.write_text("<?xml version='1.0'?><test>gains</test>", encoding="utf-8")

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
            capital_gain_rsd=Decimal("1000.00"),
        )
        mock_gains_gen.generate.return_value = [(str(temp_file), [mock_entry])]
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
        assert declarations[0].declaration_id == "ppdg3r_2023_H1"
        assert declarations[0].status == DeclarationStatus.DRAFT
        assert declarations[0].period_start == date(2023, 1, 1)
        assert declarations[0].period_end == date(2023, 6, 30)

        # Verify file was created
        assert declarations[0].file_path is not None
        assert Path(declarations[0].file_path).exists()

        # Verify declaration was saved
        storage = Storage()
        saved_decl = storage.get_declaration("ppdg3r_2023_H1")
        assert saved_decl is not None
        assert saved_decl.type == DeclarationType.PPDG3R

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_income.NBSClient")
    @patch("ibkr_porez.report_gains.NBSClient")
    def test_sync_creates_income_declaration(
        self,
        mock_nbs_gains,
        mock_nbs_income,
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

        # Mock IncomeReportGenerator - create actual temp file
        temp_file = mock_user_data_dir / "ppopo_2023-07-15_VOO_dividend.xml"
        temp_file.write_text("<?xml version='1.0'?><test>income</test>", encoding="utf-8")

        from ibkr_porez.models import IncomeDeclarationEntry as RealIncomeDeclarationEntry

        mock_income_gen = MagicMock()
        mock_entry = RealIncomeDeclarationEntry(
            date=date(2023, 7, 15),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("1000.00"),
            osnovica_za_porez=Decimal("1000.00"),
            obracunati_porez=Decimal("150.00"),
            porez_placen_drugoj_drzavi=Decimal("30.00"),
            porez_za_uplatu=Decimal("120.00"),
        )
        mock_income_gen.generate.return_value = [(str(temp_file), [mock_entry])]
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
        assert "ppopo_2023-07-15_VOO_dividend" in declarations[0].declaration_id

        # Verify file was created
        assert declarations[0].file_path is not None
        assert Path(declarations[0].file_path).exists()

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.report_income.NBSClient")
    def test_sync_skips_existing_declarations(
        self,
        mock_nbs_income,
        mock_nbs_gains,
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

        # Create existing declaration
        storage = Storage()
        existing_decl = Declaration(
            declaration_id="ppdg3r_2023_H1",
            type=DeclarationType.PPDG3R,
            status=DeclarationStatus.DRAFT,
            period_start=date(2023, 1, 1),
            period_end=date(2023, 6, 30),
            created_at=datetime.now(),
        )
        storage.save_declaration(existing_decl)

        # Mock GainsReportGenerator - create actual temp file
        temp_file = mock_user_data_dir / "ppdg3r_2023-01-01_2023-06-30.xml"
        temp_file.write_text("<?xml version='1.0'?><test>gains</test>", encoding="utf-8")

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
            capital_gain_rsd=Decimal("1000.00"),
        )
        mock_gains_gen.generate.return_value = [(str(temp_file), [mock_entry])]
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
    @patch("ibkr_porez.report_income.NBSClient")
    @patch("ibkr_porez.report_gains.NBSClient")
    def test_sync_updates_last_declaration_date(
        self,
        mock_nbs_gains,
        mock_nbs_income,
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

        # Mock IncomeReportGenerator - create actual temp file
        temp_file = mock_user_data_dir / "ppopo_2023-07-15_VOO_dividend.xml"
        temp_file.write_text("<?xml version='1.0'?><test>income</test>", encoding="utf-8")

        from ibkr_porez.models import IncomeDeclarationEntry as RealIncomeDeclarationEntry

        mock_income_gen = MagicMock()
        mock_entry = RealIncomeDeclarationEntry(
            date=date(2023, 7, 15),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("1000.00"),
            osnovica_za_porez=Decimal("1000.00"),
            obracunati_porez=Decimal("150.00"),
            porez_placen_drugoj_drzavi=Decimal("30.00"),
            porez_za_uplatu=Decimal("120.00"),
        )
        mock_income_gen.generate.return_value = [(str(temp_file), [mock_entry])]
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
    @patch("ibkr_porez.report_income.NBSClient")
    @patch("ibkr_porez.report_gains.NBSClient")
    def test_sync_handles_income_tax_not_found_error(
        self,
        mock_nbs_gains,
        mock_nbs_income,
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
    @patch("ibkr_porez.report_income.NBSClient")
    @patch("ibkr_porez.report_gains.NBSClient")
    def test_sync_first_run_sets_last_declaration_date_to_30_days_ago(
        self,
        mock_nbs_gains,
        mock_nbs_income,
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
    @patch("ibkr_porez.report_income.NBSClient")
    @patch("ibkr_porez.report_gains.NBSClient")
    def test_sync_creates_multiple_income_declarations(
        self,
        mock_nbs_gains,
        mock_nbs_income,
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

        # Mock IncomeReportGenerator (multiple declarations) - create actual temp files
        temp_file1 = mock_user_data_dir / "ppopo_2023-07-15_VOO_dividend.xml"
        temp_file1.write_text("<?xml version='1.0'?><test>income1</test>", encoding="utf-8")
        temp_file2 = mock_user_data_dir / "ppopo_2023-07-16_SGOV_dividend.xml"
        temp_file2.write_text("<?xml version='1.0'?><test>income2</test>", encoding="utf-8")

        from ibkr_porez.models import IncomeDeclarationEntry as RealIncomeDeclarationEntry

        mock_income_gen = MagicMock()
        mock_entry1 = RealIncomeDeclarationEntry(
            date=date(2023, 7, 15),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("1000.00"),
            osnovica_za_porez=Decimal("1000.00"),
            obracunati_porez=Decimal("150.00"),
            porez_placen_drugoj_drzavi=Decimal("30.00"),
            porez_za_uplatu=Decimal("120.00"),
        )
        mock_entry2 = RealIncomeDeclarationEntry(
            date=date(2023, 7, 16),
            sifra_vrste_prihoda="111402000",
            bruto_prihod=Decimal("2000.00"),
            osnovica_za_porez=Decimal("2000.00"),
            obracunati_porez=Decimal("300.00"),
            porez_placen_drugoj_drzavi=Decimal("60.00"),
            porez_za_uplatu=Decimal("240.00"),
        )
        mock_income_gen.generate.return_value = [
            (str(temp_file1), [mock_entry1]),
            (str(temp_file2), [mock_entry2]),
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
        assert "VOO" in declarations[0].declaration_id or "VOO" in declarations[1].declaration_id
        assert "SGOV" in declarations[0].declaration_id or "SGOV" in declarations[1].declaration_id

    @patch("ibkr_porez.operation_sync.GetOperation")
    @patch("ibkr_porez.operation_sync.GainsReportGenerator")
    @patch("ibkr_porez.operation_sync.IncomeReportGenerator")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.report_income.NBSClient")
    def test_sync_generates_proper_filename_format(
        self,
        mock_nbs_income,
        mock_nbs_gains,
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

        # Mock GainsReportGenerator - create actual temp file
        temp_file = mock_user_data_dir / "ppdg3r_2023-01-01_2023-06-30.xml"
        temp_file.write_text("<?xml version='1.0'?><test>gains</test>", encoding="utf-8")

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
            capital_gain_rsd=Decimal("1000.00"),
        )
        mock_gains_gen.generate.return_value = [(str(temp_file), [mock_entry])]
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
        filename = declarations[0].file_path
        assert filename.startswith("0001-ppdg3r-2023-H1.xml") or filename.startswith(
            "0001-ppdg3r-2023-H1"
        )
