from datetime import date, datetime
from decimal import Decimal, InvalidOperation
from enum import StrEnum

from pydantic import BaseModel, Field


class TransactionType(StrEnum):
    TRADE = "TRADE"
    DIVIDEND = "DIVIDEND"
    TAX = "TAX"
    WITHHOLDING_TAX = "WITHHOLDING_TAX"
    INTEREST = "INTEREST"


class AssetClass(StrEnum):
    STOCK = "STK"
    OPTION = "OPT"
    CFD = "CFD"
    BOND = "BOND"
    CASH = "CASH"


class Currency(StrEnum):
    USD = "USD"
    EUR = "EUR"
    GBP = "GBP"
    RSD = "RSD"
    # Add others as needed


class Transaction(BaseModel):
    """Represents a single financial event (trade, dividend, tax)."""

    transaction_id: str = Field(..., description="Unique ID from IBKR (e.g. tradeID)")
    date: date
    type: TransactionType
    symbol: str
    description: str
    quantity: Decimal = Decimal(0)
    price: Decimal = Decimal(0)
    amount: Decimal = Field(..., description="Total amount in original currency")
    currency: Currency

    # Context for matching (only for Trades)
    open_date: date | None = None
    open_price: Decimal | None = None

    # RSD calculated values
    exchange_rate: Decimal | None = None
    amount_rsd: Decimal | None = None


class ExchangeRate(BaseModel):
    """NBS middle exchange rate for a specific date and currency."""

    date: date
    currency: Currency
    rate: Decimal


class TaxReportEntry(BaseModel):
    """Single row in the final tax report."""

    ticker: str
    quantity: Decimal

    sale_date: date
    sale_price: Decimal
    sale_exchange_rate: Decimal
    sale_value_rsd: Decimal

    purchase_date: date
    purchase_price: Decimal
    purchase_exchange_rate: Decimal
    purchase_value_rsd: Decimal

    capital_gain_rsd: Decimal  # Profit/Loss

    # Metadata
    is_tax_exempt: bool = False  # e.g. >10 years holding


class IncomeEntry(BaseModel):
    """Single income entry for PP OPO (Capital Income)."""

    date: date
    symbol: str
    amount: Decimal  # In original currency
    currency: Currency
    amount_rsd: Decimal  # In RSD
    exchange_rate: Decimal
    income_type: str  # "dividend" or "coupon"
    description: str
    withholding_tax_usd: Decimal = Decimal("0.00")  # Withholding tax in USD
    withholding_tax_rsd: Decimal = Decimal("0.00")  # Withholding tax in RSD


# Income type codes for PP OPO declarations
INCOME_CODE_DIVIDEND = "111402000"  # Dividends from shares
INCOME_CODE_COUPON = "111403000"  # Interest from bonds (coupons)


class IncomeDeclarationEntry(BaseModel):
    """Single row in PP OPO declaration with calculated tax fields."""

    date: date
    symbol_or_currency: str | None = None
    sifra_vrste_prihoda: str  # INCOME_CODE_DIVIDEND or INCOME_CODE_COUPON
    bruto_prihod: Decimal  # BrutoPrihod (gross income in RSD)
    osnovica_za_porez: Decimal  # OsnovicaZaPorez (tax base)
    obracunati_porez: Decimal  # ObracunatiPorez (calculated tax)
    porez_placen_drugoj_drzavi: Decimal  # PorezPlacenDrugojDrzavi (foreign tax paid)
    porez_za_uplatu: Decimal  # PorezZaUplatu (tax to pay)


class DeclarationType(StrEnum):
    """Type of tax declaration."""

    PPDG3R = "PPDG-3R"  # Capital Gains
    PPO = "PP OPO"  # Capital Income (Dividends/Coupons)


class DeclarationStatus(StrEnum):
    """Status of tax declaration."""

    DRAFT = "draft"  # Created but not submitted
    SUBMITTED = "submitted"  # Submitted to tax portal
    PENDING = "pending"  # Waiting tax authority assessment
    FINALIZED = "finalized"  # Finalized


class Declaration(BaseModel):
    """Tax declaration with lifecycle management."""

    declaration_id: str = Field(..., description="Unique declaration ID")
    type: DeclarationType
    status: DeclarationStatus = DeclarationStatus.DRAFT
    period_start: date
    period_end: date
    created_at: datetime
    submitted_at: datetime | None = None
    paid_at: datetime | None = None
    file_path: str | None = None  # Path to XML file
    xml_content: str | None = None  # XML content (for export)
    report_data: list[TaxReportEntry | IncomeDeclarationEntry] | None = (
        None  # Data for display (for export)
    )
    metadata: dict = Field(default_factory=dict)  # Additional data (count, sums, etc.)
    attached_files: dict[str, str] = Field(
        default_factory=dict,
        description="Attached files: {file_identifier: relative_path}",
    )

    def display_type(self) -> str:
        """Return user-facing declaration type label."""
        base = self.type.value
        if self.type != DeclarationType.PPO:
            return base

        symbol = str(self.metadata.get("symbol", "")).strip().upper()
        if not symbol or symbol == "UNKNOWN":
            return base
        return f"{base} ({symbol})"

    def display_period(self) -> str:
        """Return user-facing period label."""
        if self.period_start == self.period_end:
            return self.period_start.isoformat()
        return f"{self.period_start} to {self.period_end}"

    def display_tax(self) -> str:
        """Return user-facing tax amount."""
        for key in (
            "assessed_tax_due_rsd",
            "tax_due_rsd",
            "estimated_tax_rsd",
            "calculated_tax_rsd",
        ):
            value = self.metadata.get(key)
            if value is None:
                continue
            try:
                amount = Decimal(str(value)).quantize(Decimal("0.01"))
            except (InvalidOperation, TypeError, ValueError):
                continue
            return f"{amount:.2f}"
        return ""


class UserConfig(BaseModel):
    """User configuration stored on disk."""

    ibkr_token: str = ""
    ibkr_query_id: str = ""

    personal_id: str = ""  # JMBG
    full_name: str
    address: str
    city_code: str = "223"
    phone: str = "0600000000"
    email: str = "email@example.com"
    data_dir: str | None = None  # Absolute path to data directory (default: app data folder)
    output_folder: str | None = None  # Absolute path to output folder (default: Downloads)
