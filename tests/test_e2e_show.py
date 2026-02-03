import allure
import pytest
from unittest.mock import patch
from click.testing import CliRunner
from ibkr_porez.main import ibkr_porez
from ibkr_porez.storage import Storage
from ibkr_porez.models import (
    Transaction,
    TransactionType,
    Currency,
    Declaration,
    DeclarationType,
    DeclarationStatus,
    TaxReportEntry,
    IncomeDeclarationEntry,
    INCOME_CODE_DIVIDEND,
)
from decimal import Decimal
from datetime import date, datetime


@pytest.fixture
def mock_user_data_dir(tmp_path):
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
        s = Storage()
        s._ensure_dirs()
        yield tmp_path


@pytest.fixture
def runner():
    return CliRunner()


@allure.epic("End-to-end")
@allure.feature("show")
class TestE2EShow:
    @pytest.fixture
    def setup_data(self, mock_user_data_dir):
        """Populate storage with some test data."""
        s = Storage()
        # AAPL: Buy 10 @ 100, Sell 5 @ 120 (Gain 20/share * 5 = 100 USD)
        # MSFT: Buy 10 @ 200, Sell 10 @ 220 (Gain 20/share * 10 = 200 USD)
        # Dividends: KO 50 USD
        txs = [
            Transaction(
                transaction_id="buy_aapl",
                date=date(2026, 1, 1),
                type=TransactionType.TRADE,
                symbol="AAPL",
                description="Buy",
                quantity=Decimal(10),
                price=Decimal(100),
                amount=Decimal("-1000"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="sell_aapl",
                date=date(2026, 1, 15),
                type=TransactionType.TRADE,
                symbol="AAPL",
                description="Sell",
                quantity=Decimal(-5),
                price=Decimal(120),
                amount=Decimal("600"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="buy_msft",
                date=date(2026, 2, 1),
                type=TransactionType.TRADE,
                symbol="MSFT",
                description="Buy",
                quantity=Decimal(10),
                price=Decimal(200),
                amount=Decimal("-2000"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="sell_msft",
                date=date(2026, 2, 10),
                type=TransactionType.TRADE,
                symbol="MSFT",
                description="Sell",
                quantity=Decimal(-10),
                price=Decimal(220),
                amount=Decimal("2200"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="div_ko",
                date=date(2026, 3, 15),
                type=TransactionType.DIVIDEND,
                symbol="KO",
                description="Dividend",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("50"),
                currency=Currency.USD,
            ),
        ]
        s.save_transactions(txs)
        return s

    @patch("ibkr_porez.operation_show.NBSClient")
    def test_show_default_summary(self, mock_nbs_cls, runner, mock_user_data_dir, setup_data):
        """
        Scenario: Run `show` without arguments.
        Expect: Monthly summary table.
        """
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")  # 1 USD = 100 RSD

        result = runner.invoke(ibkr_porez, ["show"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Monthly Report Breakdown" in result.output

        # Verify Rows
        # Jan 2023: AAPL, Sales 1, P/L 100 USD * 100 = 10,000 RSD
        assert "2026-01" in result.output
        assert "AAPL" in result.output
        assert "10,000.00" in result.output

        # Feb 2023: MSFT, Sales 1, P/L 200 USD * 100 = 20,000 RSD
        assert "2026-02" in result.output
        assert "MSFT" in result.output
        assert "20,000.00" in result.output

        # Mar 2023: KO, Divs 50 USD * 100 = 5,000 RSD
        assert "2026-03" in result.output
        assert "KO" in result.output
        assert "5,000.00" in result.output

    @patch("ibkr_porez.operation_show.NBSClient")
    def test_show_detailed_ticker(self, mock_nbs_cls, runner, mock_user_data_dir, setup_data):
        """
        Scenario: Run `show --ticker AAPL`.
        Expect: Detailed execution list for AAPL.
        """
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")

        result = runner.invoke(ibkr_porez, ["show", "--ticker", "AAPL"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Detailed Report: AAPL" in result.output

        # Should show sale date, quantity, prices
        # Sale Date 2023-01-15, Qty 5.00, Price 120.00
        # Date might be truncated in table output, so check for partial match
        assert "2026-01" in result.output or "2026-…" in result.output
        assert "5.00" in result.output
        assert "120.00" in result.output

        # Total P/L for this filter
        assert "Total P/L: 10,000.00 RSD" in result.output

        # Should NOT show MSFT
        assert "MSFT" not in result.output

    @patch("ibkr_porez.operation_show.NBSClient")
    def test_show_detailed_month(self, mock_nbs_cls, runner, mock_user_data_dir, setup_data):
        """
        Scenario: Run `show --month 2023-02`.
        Expect: Summary filtered by month (or detailed? logic says if ticker OR simple filter... wait.
        Let's check logic: if ticker IS passed -> Detailed. If ONLY month -> Summary filtered?
        Code: `if show_detailed_list: ...` where `show_detailed_list = True if ticker else False`.
        So `show -m 2023-02` shows SUMMARY filtered (NOT Detailed).
        Wait, I should verify what I implemented.
        """
        # Re-reading main.py from previous task (Step 3688):
        # show_detailed_list = False
        # if ticker: show_detailed_list = True
        # So providing only month keeps it as SUMMARY.

        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")

        result = runner.invoke(ibkr_porez, ["show", "--month", "2026-02"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Monthly Report Breakdown" in result.output

        # Should show Feb data (MSFT)
        assert "2026-02" in result.output
        assert "MSFT" in result.output

        # Should NOT show Jan data (AAPL)
        assert "2026-01" not in result.output
        assert "AAPL" not in result.output

    @patch("ibkr_porez.operation_show.NBSClient")
    def test_show_empty(self, mock_nbs_cls, runner, mock_user_data_dir):
        """Scenario: No transactions."""
        mock_nbs = mock_nbs_cls.return_value

        # Empty storage
        Storage()

        result = runner.invoke(ibkr_porez, ["show"])

        assert result.exit_code == 0
        assert "No transactions found" in result.output

    def test_show_declaration_gains(self, runner, mock_user_data_dir):
        """
        Scenario: Run `show 1` where declaration_id=1 is a PPDG-3R (gains) declaration.
        Expect: Declaration details with table showing entries.
        """
        storage = Storage()

        # Create a gains declaration
        gains_entry = TaxReportEntry(
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

        declaration = Declaration(
            declaration_id="1",
            type=DeclarationType.PPDG3R,
            status=DeclarationStatus.DRAFT,
            period_start=date(2023, 1, 1),
            period_end=date(2023, 6, 30),
            created_at=datetime(2023, 7, 1, 12, 0, 0),
            file_path=str(mock_user_data_dir / "001-ppdg3r-2023-H1.xml"),
            xml_content="<?xml version='1.0'?><test>gains</test>",
            report_data=[gains_entry],
            metadata={"total_gain": 2000.00, "entry_count": 1},
        )

        storage.save_declaration(declaration)

        result = runner.invoke(ibkr_porez, ["show", "1"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        # Check declaration header
        assert "Declaration ID: 1" in result.output
        assert "PPDG-3R" in result.output
        assert "draft" in result.output
        assert "2023-01-01 to 2023-06-30" in result.output

        # Check declaration data table
        assert "Declaration Data" in result.output
        assert "Declaration Data (Part 4)" in result.output
        assert "AAPL" in result.output
        assert "2023-0" in result.output  # Date may be truncated in table
        assert "10.00" in result.output
        assert "12000" in result.output
        assert "2000.00" in result.output  # Gain amount

        # Check metadata
        assert "Metadata" in result.output
        assert "total_gain" in result.output
        assert "2000.00" in result.output
        assert "entry_count" in result.output
        assert "1" in result.output

    def test_show_declaration_income(self, runner, mock_user_data_dir):
        """
        Scenario: Run `show 2` where declaration_id=2 is a PP OPO (income) declaration.
        Expect: Declaration details with income fields.
        """
        storage = Storage()

        # Create an income declaration
        income_entry = IncomeDeclarationEntry(
            date=date(2023, 12, 24),
            sifra_vrste_prihoda=INCOME_CODE_DIVIDEND,
            bruto_prihod=Decimal("8706.70"),
            osnovica_za_porez=Decimal("8706.70"),
            obracunati_porez=Decimal("1306.01"),
            porez_placen_drugoj_drzavi=Decimal("2612.51"),
            porez_za_uplatu=Decimal("0.00"),
        )

        declaration = Declaration(
            declaration_id="2",
            type=DeclarationType.PPO,
            status=DeclarationStatus.DRAFT,
            period_start=date(2023, 12, 24),
            period_end=date(2023, 12, 24),
            created_at=datetime(2023, 12, 25, 10, 0, 0),
            file_path=str(mock_user_data_dir / "002-ppopo-sgov-2023-1224.xml"),
            xml_content="<?xml version='1.0'?><test>income</test>",
            report_data=[income_entry],
            metadata={"symbol": "SGOV", "income_type": "dividend"},
        )

        storage.save_declaration(declaration)

        result = runner.invoke(ibkr_porez, ["show", "2"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        # Check declaration header
        assert "Declaration ID: 2" in result.output
        assert "PP OPO" in result.output
        assert "draft" in result.output
        assert "2023-12-24 to 2023-12-24" in result.output

        # Check declaration data fields
        assert "Declaration Data" in result.output
        assert "Date: 2023-12-24" in result.output
        assert f"SifraVrstePrihoda: {INCOME_CODE_DIVIDEND}" in result.output
        assert "BrutoPrihod: 8706.70 RSD" in result.output
        assert "OsnovicaZaPorez: 8706.70 RSD" in result.output
        assert "ObracunatiPorez: 1306.01 RSD" in result.output
        assert "PorezPlacenDrugojDrzavi: 2612.51 RSD" in result.output
        assert "PorezZaUplatu: 0.00 RSD" in result.output

        # Check metadata
        assert "Metadata" in result.output
        assert "symbol" in result.output
        assert "SGOV" in result.output
        assert "income_type" in result.output
        assert "dividend" in result.output

    def test_show_declaration_with_submitted_status(self, runner, mock_user_data_dir):
        """
        Scenario: Run `show 3` where declaration has been submitted.
        Expect: Declaration details including submitted timestamp.
        """
        storage = Storage()

        gains_entry = TaxReportEntry(
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

        declaration = Declaration(
            declaration_id="3",
            type=DeclarationType.PPDG3R,
            status=DeclarationStatus.SUBMITTED,
            period_start=date(2023, 7, 1),
            period_end=date(2023, 12, 31),
            created_at=datetime(2024, 1, 5, 10, 0, 0),
            submitted_at=datetime(2024, 1, 10, 14, 30, 0),
            file_path=str(mock_user_data_dir / "003-ppdg3r-2023-H2.xml"),
            xml_content="<?xml version='1.0'?><test>gains</test>",
            report_data=[gains_entry],
            metadata={"total_gain": 29250.00, "entry_count": 1},
        )

        storage.save_declaration(declaration)

        result = runner.invoke(ibkr_porez, ["show", "3"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Declaration ID: 3" in result.output
        assert "submitted" in result.output
        assert "Submitted:" in result.output
        assert "2024-01-10" in result.output

    def test_show_declaration_with_paid_status(self, runner, mock_user_data_dir):
        """
        Scenario: Run `show 4` where declaration has been paid.
        Expect: Declaration details including paid timestamp.
        """
        storage = Storage()

        income_entry = IncomeDeclarationEntry(
            date=date(2023, 6, 15),
            sifra_vrste_prihoda=INCOME_CODE_DIVIDEND,
            bruto_prihod=Decimal("5000.00"),
            osnovica_za_porez=Decimal("5000.00"),
            obracunati_porez=Decimal("750.00"),
            porez_placen_drugoj_drzavi=Decimal("750.00"),
            porez_za_uplatu=Decimal("0.00"),
        )

        declaration = Declaration(
            declaration_id="4",
            type=DeclarationType.PPO,
            status=DeclarationStatus.PAID,
            period_start=date(2023, 6, 15),
            period_end=date(2023, 6, 15),
            created_at=datetime(2023, 6, 16, 9, 0, 0),
            submitted_at=datetime(2023, 6, 20, 11, 0, 0),
            paid_at=datetime(2023, 6, 25, 15, 45, 0),
            file_path=str(mock_user_data_dir / "004-ppopo-ko-2023-0615.xml"),
            xml_content="<?xml version='1.0'?><test>income</test>",
            report_data=[income_entry],
            metadata={"symbol": "KO", "income_type": "dividend"},
        )

        storage.save_declaration(declaration)

        result = runner.invoke(ibkr_porez, ["show", "4"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Declaration ID: 4" in result.output
        assert "paid" in result.output
        assert "Submitted:" in result.output
        assert "Paid:" in result.output
        assert "2023-06-25" in result.output

    def test_show_declaration_not_found(self, runner, mock_user_data_dir):
        """
        Scenario: Run `show 999` where declaration_id=999 does not exist.
        Expect: Error message indicating declaration not found.
        """
        storage = Storage()
        # Don't create any declarations

        result = runner.invoke(ibkr_porez, ["show", "999"])

        assert result.exit_code == 0  # Command succeeds but shows error message
        assert "Declaration '999' not found" in result.output
