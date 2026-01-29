import xml.etree.ElementTree as ET
from datetime import date

from ibkr_porez.models import TaxReportEntry, UserConfig


class XMLGenerator:
    def __init__(self, config: UserConfig):
        self.config = config

    def generate_xml(
        self,
        entries: list[TaxReportEntry],
        period_start: date,
        period_end: date,
    ) -> str:
        """Generate PPDG-3R XML content."""

        # Root Element
        # Note: Namespace 'ns1' is common in Serbian tax XMLs.
        # We'll use a generic structure.
        # Assumption: Root is <PPDG3R> or similar.
        # Based on search: <PPDG3R> might be the root.

        root = ET.Element("PPDG3R")

        # 1. Header / General Info
        # <PodaciOPrijavi>
        #   <VrstaPrijave> ...
        #   <ObracunskiPeriod> ...
        # </PodaciOPrijavi>

        # We will create a simplified flat structure if exact nesting is unknown,
        # or a logical one. Let's try logical.

        head = ET.SubElement(root, "PodaciOPoreskomObvezniku")
        ET.SubElement(head, "JMBG").text = self.config.personal_id
        ET.SubElement(head, "ImePrezime").text = self.config.full_name
        ET.SubElement(head, "Adresa").text = self.config.address

        # Period
        period = ET.SubElement(root, "PodaciOPrijavi")
        ET.SubElement(period, "PeriodOd").text = period_start.isoformat()
        ET.SubElement(period, "PeriodDo").text = period_end.isoformat()

        # Capital Gains Section
        # Tag from search with Namespace 'ns1'
        # <ns1:DeklarisanoPrenosHOVInvesticionihJed>
        # But ElementTree handles namespaces differently.
        # Let's just use the tag name for now.

        gains_section = ET.SubElement(root, "DeklarisanoPrenosHOVInvesticionihJed")

        i = 1
        for entry in entries:
            # Each row
            row = ET.SubElement(gains_section, "RedniBroj")
            row.set("id", str(i))

            # Asset Info
            ET.SubElement(row, "VrstaImovine").text = "1"  # 1 = Shares? Assumption.
            ET.SubElement(row, "NazivVrsteImovine").text = entry.ticker
            ET.SubElement(row, "Kolicina").text = f"{entry.quantity:.4f}"

            # Sale
            ET.SubElement(row, "DatumProdaje").text = entry.sale_date.isoformat()
            ET.SubElement(row, "CenaProdaje").text = f"{entry.sale_price:.4f}"
            ET.SubElement(row, "VrednostProdaje").text = f"{entry.sale_value_rsd:.2f}"

            # Purchase
            ET.SubElement(row, "DatumSticanja").text = entry.purchase_date.isoformat()
            ET.SubElement(row, "CenaSticanja").text = f"{entry.purchase_price:.4f}"
            ET.SubElement(row, "NabavnaVrednost").text = f"{entry.purchase_value_rsd:.2f}"

            # Result
            if entry.capital_gain_rsd > 0:
                ET.SubElement(row, "KapitalniDobitak").text = f"{entry.capital_gain_rsd:.2f}"
                ET.SubElement(row, "KapitalniGubitak").text = "0.00"
            else:
                ET.SubElement(row, "KapitalniDobitak").text = "0.00"
                ET.SubElement(row, "KapitalniGubitak").text = f"{abs(entry.capital_gain_rsd):.2f}"

            # 10 Year Exempt?
            if entry.is_tax_exempt:
                # Is there a flag? Or just exclude?
                # Usually included but marked as exempt.
                ET.SubElement(row, "PoreskoOslobodjenje").text = "DA"

            i += 1

        # PPDG-3R might require summary totals too.
        # But let's stick to itemized list.

        # Pretty print
        return self._prettify(root)

    def _prettify(self, elem) -> str:
        """Return a pretty-printed XML string for the Element."""
        from xml.dom import minidom

        rough_string = ET.tostring(elem, "utf-8")
        reparsed = minidom.parseString(rough_string)
        return reparsed.toprettyxml(indent="  ")
