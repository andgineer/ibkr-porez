"""Table rendering for tax reports."""

from rich.table import Table

from ibkr_porez.models import TaxReportEntry


def render_declaration_table(entries: list[TaxReportEntry]) -> Table:
    """
    Render declaration data table (Part 4 of PPDG-3R).

    This table shows the tabular part of the declaration with all entries.

    Args:
        entries: List of tax report entries.

    Returns:
        Table: Rich table with declaration data.
    """
    table = Table(title="Declaration Data (Part 4)", box=None)

    table.add_column("No.", justify="right")
    table.add_column("Ticker (Naziv)", justify="left")
    table.add_column("Sale Date (4.3)", justify="left")
    table.add_column("Qty (4.5/4.9)", justify="right")
    table.add_column("Sale Price RSD (4.6)", justify="right")  # Prodajna Cena
    table.add_column("Buy Date (4.7)", justify="left")
    table.add_column("Buy Price RSD (4.10)", justify="right")  # Nabavna Cena
    table.add_column("Gain RSD", justify="right")
    table.add_column("Loss RSD", justify="right")

    i = 1
    for e in entries:
        gain = e.capital_gain_rsd
        g_str = f"{gain:.2f}" if gain >= 0 else "0.00"
        l_str = f"{abs(gain):.2f}" if gain < 0 else "0.00"

        table.add_row(
            str(i),
            e.ticker,
            e.sale_date.strftime("%Y-%m-%d"),
            f"{e.quantity:.2f}",
            f"{e.sale_value_rsd:.2f}",
            e.purchase_date.strftime("%Y-%m-%d"),
            f"{e.purchase_value_rsd:.2f}",
            g_str,
            l_str,
        )
        i += 1

    return table
