from datetime import date
from decimal import Decimal
from enum import Enum

from pydantic import BaseModel, Field


class TransactionType(str, Enum):
    TRADE = "TRADE"
    DIVIDEND = "DIVIDEND"
    TAX = "TAX"
    WITHHOLDING_TAX = "WITHHOLDING_TAX"
    INTEREST = "INTEREST"


class AssetClass(str, Enum):
    STOCK = "STK"
    OPTION = "OPT"
    CFD = "CFD"
    BOND = "BOND"
    CASH = "CASH"


class Currency(str, Enum):
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
    amount: Decimal  # В исходной валюте
    currency: Currency
    amount_rsd: Decimal  # В RSD
    exchange_rate: Decimal
    income_type: str  # "dividend" или "coupon"
    description: str
    withholding_tax_usd: Decimal = Decimal("0.00")  # Удержанный налог в USD
    withholding_tax_rsd: Decimal = Decimal("0.00")  # Удержанный налог в RSD


class IncomeDeclarationEntry(BaseModel):
    """Single row in PP OPO declaration with calculated tax fields."""

    date: date
    sifra_vrste_prihoda: str  # "111402000" for dividend, "111403000" for coupon
    bruto_prihod: Decimal  # BrutoPrihod (gross income in RSD)
    osnovica_za_porez: Decimal  # OsnovicaZaPorez (tax base)
    obracunati_porez: Decimal  # ObracunatiPorez (calculated tax)
    porez_placen_drugoj_drzavi: Decimal  # PorezPlacenDrugojDrzavi (foreign tax paid)
    porez_za_uplatu: Decimal  # PorezZaUplatu (tax to pay)


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
