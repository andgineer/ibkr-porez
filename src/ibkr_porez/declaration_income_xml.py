"""Generator for PP OPO (Capital Income) XML declarations."""

from datetime import date, timedelta
from decimal import ROUND_HALF_UP, Decimal
from xml.dom import minidom

import holidays

from ibkr_porez.models import (
    INCOME_CODE_COUPON,
    INCOME_CODE_DIVIDEND,
    IncomeEntry,
    UserConfig,
)


class IncomeXMLGenerator:
    """Generator for PP OPO XML declarations."""

    # Tax declaration due date: 30 days after declaration date (legal requirement)
    TAX_DUE_DATE_DAYS = 30

    def __init__(self, config: UserConfig):
        self.config = config

    def generate_xml(  # noqa: PLR0915
        self,
        income_entries: list[IncomeEntry],
        declaration_date: date,
        income_type: str,  # "dividend" or "coupon"
        withholding_tax_rsd: Decimal = Decimal("0.00"),
    ) -> str:
        """
        Generate PP OPO XML for given income entries.

        Args:
            income_entries: List of income entries (all should be same date and type).
            declaration_date: Date of income realization (YYYY-MM-DD).
            income_type: Type of income ("dividend" or "coupon").

        Returns:
            str: XML content for PP OPO declaration.
        """
        doc = minidom.Document()

        # Root Element: ns1:PodaciPoreskeDeklaracije
        root = doc.createElement("ns1:PodaciPoreskeDeklaracije")
        root.setAttribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
        root.setAttribute("xmlns:ns1", "http://pid.purs.gov.rs")
        doc.appendChild(root)

        def create_text(parent, tag, value):
            el = doc.createElement(f"ns1:{tag}")
            if value is not None:
                el.appendChild(doc.createTextNode(str(value)))
            parent.appendChild(el)

        def create_cdata(parent, tag, value):
            el = doc.createElement(f"ns1:{tag}")
            cdata = doc.createCDATASection(str(value) if value else "")
            el.appendChild(cdata)
            parent.appendChild(el)

        p_prijavi = doc.createElement("ns1:PodaciOPrijavi")
        root.appendChild(p_prijavi)

        create_text(p_prijavi, "VrstaPrijave", "1")
        period_str = declaration_date.strftime("%Y-%m")
        create_text(p_prijavi, "ObracunskiPeriod", period_str)
        create_text(p_prijavi, "DatumOstvarivanjaPrihoda", declaration_date.strftime("%Y-%m-%d"))
        create_text(p_prijavi, "Rok", "1")
        # If weekend/holiday -> first next working day
        saturday = 5
        base_due = declaration_date + timedelta(days=self.TAX_DUE_DATE_DAYS)
        rs_holidays = holidays.country_holidays("RS")

        # Shift if weekend (5=Sat, 6=Sun) or Holiday
        while base_due.weekday() >= saturday or base_due in rs_holidays:
            base_due += timedelta(days=1)

        create_text(p_prijavi, "DatumDospelostiObaveze", base_due.strftime("%Y-%m-%d"))

        p_obveznik = doc.createElement("ns1:PodaciOPoreskomObvezniku")
        root.appendChild(p_obveznik)

        create_text(p_obveznik, "PoreskiIdentifikacioniBroj", self.config.personal_id)
        create_cdata(p_obveznik, "ImePrezimeObveznika", self.config.full_name)
        create_cdata(p_obveznik, "UlicaBrojPoreskogObveznika", self.config.address)
        create_text(p_obveznik, "PrebivalisteOpstina", self.config.city_code)
        create_text(p_obveznik, "JMBGPodnosiocaPrijave", self.config.personal_id)
        create_text(p_obveznik, "TelefonKontaktOsobe", self.config.phone)
        create_cdata(p_obveznik, "ElektronskaPosta", self.config.email)

        p_nacin = doc.createElement("ns1:PodaciONacinuOstvarivanjaPrihoda")
        root.appendChild(p_nacin)

        create_text(p_nacin, "NacinIsplate", "3")
        create_text(p_nacin, "Ostalo", "Isplata na brokerski racun")

        deklaracija = doc.createElement("ns1:DeklarisaniPodaciOVrstamaPrihoda")
        root.appendChild(deklaracija)

        # Calculate totals
        total_bruto = sum(entry.amount_rsd for entry in income_entries)
        # Round to 2 decimal places
        total_bruto = round(total_bruto, 2)

        # Tax rate: 15% for capital income
        tax_rate = Decimal("0.15")
        osnovica = total_bruto
        # Use quantize with ROUND_HALF_UP for taxes (standard rounding)

        obracunati_porez = (osnovica * tax_rate).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)

        # Calculate foreign tax paid (withholding tax)
        # This comes from WITHHOLDING_TAX transactions, passed as parameter
        porez_placen_drugoj_drzavi = withholding_tax_rsd

        # Round withholding tax
        porez_placen_drugoj_drzavi = round(porez_placen_drugoj_drzavi, 2)

        # PorezZaUplatu = ObracunatiPorez - PorezPlacenDrugojDrzavi
        porez_za_uplatu = max(Decimal("0.00"), obracunati_porez - porez_placen_drugoj_drzavi)
        porez_za_uplatu = round(porez_za_uplatu, 2)

        sifra_vrste_prihoda = (
            INCOME_CODE_DIVIDEND if income_type == "dividend" else INCOME_CODE_COUPON
        )

        podaci_vrsta = doc.createElement("ns1:PodaciOVrstamaPrihoda")
        deklaracija.appendChild(podaci_vrsta)

        create_text(podaci_vrsta, "RedniBroj", "1")
        create_text(podaci_vrsta, "SifraVrstePrihoda", sifra_vrste_prihoda)
        create_text(podaci_vrsta, "BrutoPrihod", f"{total_bruto:.2f}")
        create_text(podaci_vrsta, "OsnovicaZaPorez", f"{osnovica:.2f}")
        create_text(podaci_vrsta, "ObracunatiPorez", f"{obracunati_porez:.2f}")
        create_text(podaci_vrsta, "PorezPlacenDrugojDrzavi", f"{porez_placen_drugoj_drzavi:.2f}")
        create_text(podaci_vrsta, "PorezZaUplatu", f"{porez_za_uplatu:.2f}")

        ukupno = doc.createElement("ns1:Ukupno")
        root.appendChild(ukupno)

        create_text(ukupno, "FondSati", "0.00")
        create_text(ukupno, "BrutoPrihod", f"{total_bruto:.2f}")
        create_text(ukupno, "OsnovicaZaPorez", f"{osnovica:.2f}")
        create_text(ukupno, "ObracunatiPorez", f"{obracunati_porez:.2f}")
        create_text(ukupno, "PorezPlacenDrugojDrzavi", f"{porez_placen_drugoj_drzavi:.2f}")
        create_text(ukupno, "PorezZaUplatu", f"{porez_za_uplatu:.2f}")
        create_text(ukupno, "OsnovicaZaDoprinose", "0.00")
        create_text(ukupno, "PIO", "0.00")
        create_text(ukupno, "ZDRAVSTVO", "0.00")
        create_text(ukupno, "NEZAPOSLENOST", "0.00")

        kamata = doc.createElement("ns1:Kamata")
        root.appendChild(kamata)

        create_text(kamata, "PorezZaUplatu", "0")
        create_text(kamata, "OsnovicaZaDoprinose", "0")
        create_text(kamata, "PIO", "0")
        create_text(kamata, "ZDRAVSTVO", "0")
        create_text(kamata, "NEZAPOSLENOST", "0")

        dodatna_kamata = doc.createElement("ns1:PodaciODodatnojKamati")
        root.appendChild(dodatna_kamata)

        return doc.toprettyxml(indent="  ")
