from datetime import date
from decimal import Decimal

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
        from xml.dom import minidom
        from datetime import timedelta
        
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
            # Ensure CDATA is only created for non-empty string? 
            # Empty string in CDATA is <![CDATA[]]> which is fine.
            # Explicitly cast to str
            cdata = doc.createCDATASection(str(value) if value else "")
            el.appendChild(cdata)
            parent.appendChild(el)

        # 1. PodaciOPrijavi
        p_prijavi = doc.createElement("ns1:PodaciOPrijavi")
        root.appendChild(p_prijavi)
        
        # 1.1 VrstaPrijave: 1 (Utvrđivanje)
        create_text(p_prijavi, "VrstaPrijave", "1")
        
        # 1.1a OsnovZaPrijavu: 4 (Transfer of Shares/Securities)
        create_text(p_prijavi, "OsnovZaPrijavu", "4") 
        
        # 1.2 DatumOstvarenjaPrihoda: End of period usually, or last sale. 
        create_text(p_prijavi, "DatumOstvarenjaPrihodaDelaPrihoda", period_end.strftime("%Y-%m-%d"))
        
        create_text(p_prijavi, "IsplataUDelovima", "0")
        
        # 1.3 DatumDospelosti: Jan 31 or Jul 31.
        if period_end.month == 6:
            due_date = date(period_end.year, 7, 31)
        else:
            due_date = date(period_end.year + 1, 1, 31)
        create_text(p_prijavi, "DatumDospelostiZaPodnosenjePoreskePrijave", due_date.strftime("%Y-%m-%d"))
        
        d_nacin = doc.createElement("ns1:DatumINacinPodnosenjaPrijave")
        p_prijavi.appendChild(d_nacin)
        create_text(d_nacin, "DatumPodnosenjaPrijave", date.today().strftime("%Y-%m-%d"))
        create_text(d_nacin, "NacinPodnosenjaPrijave", "E")

        # 2. PodaciOPoreskomObvezniku
        p_obveznik = doc.createElement("ns1:PodaciOPoreskomObvezniku")
        root.appendChild(p_obveznik)
        
        create_text(p_obveznik, "TipPoreskogObveznika", "1")
        create_text(p_obveznik, "PoreskiIdentifikacioniBroj", self.config.personal_id)
        
        create_cdata(p_obveznik, "ImeIPrezimePoreskogObveznika", self.config.full_name.upper())
        # Use city_code from config (default '223' or real code like '1')
        create_text(p_obveznik, "PrebivalisteBoravistePoreskogObveznika", self.config.city_code) 
        create_cdata(p_obveznik, "AdresaPoreskogObveznika", self.config.address.upper())
        
        # 2.6 Telefon: Mandatory
        create_text(p_obveznik, "TelefonKontaktOsobe", self.config.phone) 
        # 2.7 Email: Mandatory
        create_cdata(p_obveznik, "ElektronskaPosta", self.config.email) 
        
        create_text(p_obveznik, "JMBGPodnosiocaPrijave", self.config.personal_id)

        # 3. PodaciZaUtvrdjivanjePorezaKodPrenosaPravaIliUdela
        # VALID FILE DOES NOT HAVE Part 3 when Osnov=4. 
        # Removing Part 3 completely.
        
        # 4. DeklarisanoPrenosHOVInvesticionihJed (Our Data)
        gains_section = doc.createElement("ns1:DeklarisanoPrenosHOVInvesticionihJed")
        root.appendChild(gains_section)
        
        i = 1
        i = 1
        for entry in entries:
            row = doc.createElement("ns1:PodaciOPrenosuHOVInvesticionihJed")
            row.setAttribute("id", str(i))
            gains_section.appendChild(row)
            
            create_text(row, "RedniBroj", str(i))
            # Part 4 Specific Tags found in user reports/schema inference
            create_cdata(row, "NazivEmitenta", entry.ticker)
            create_text(row, "DatumPrenosaHOV", entry.sale_date.strftime("%Y-%m-%d"))
            
            # Dummy/Default values for mandatory structure
            create_text(row, "BrojDokumentaOPrenosu", "1") 
            create_text(row, "BrojPrenetihHOVInvesticionihJed", f"{entry.quantity:.2f}")
            
            create_text(row, "ProdajnaCena", f"{entry.sale_value_rsd:.2f}")
            
            # Nested Purchase Info
            sticanje = doc.createElement("ns1:Sticanje")
            row.appendChild(sticanje)
            
            # Note: 4.7 is "Datum Sticanja" on form. XML likely "DatumSticanja" inside Sticanje or "DatumSticanjaHOV"
            # Based on Part 3 valid file (Osnov 1), it uses "DatumSticanja". 
            # We try generic "DatumSticanja" first as it worked for Part 3.
            create_text(sticanje, "DatumSticanja", entry.purchase_date.strftime("%Y-%m-%d"))
            create_text(sticanje, "NabavnaCena", f"{entry.purchase_value_rsd:.2f}")
            # Also need "BrojDokumenta" and "BrojStecenih"?
            create_text(sticanje, "BrojDokumentaOSticanju", "1")
            create_text(sticanje, "BrojStecenihHOVInvesticionihJed", f"{entry.quantity:.2f}")

            gain = entry.capital_gain_rsd
            if gain >= 0:
                create_text(row, "KapitalniDobitak", f"{gain:.2f}")
                create_text(row, "KapitalniGubitak", "0.00")
            else:
                create_text(row, "KapitalniDobitak", "0.00")
                create_text(row, "KapitalniGubitak", f"{abs(gain):.2f}")
            
            if entry.is_tax_exempt:
                create_text(row, "PoreskoOslobodjenje", "DA")
            
            i += 1

        # 7/Summary
        summary = doc.createElement("ns1:PodaciOUtvrđivanju")
        root.appendChild(summary)
        
        total_gain = sum(e.capital_gain_rsd for e in entries if e.capital_gain_rsd > 0)
        total_loss = sum(abs(e.capital_gain_rsd) for e in entries if e.capital_gain_rsd < 0)
        net_gain = max(Decimal(0), total_gain - total_loss)
        tax = net_gain * Decimal("0.15")

        create_text(summary, "UkupanKapitalniDobitak", f"{total_gain:.2f}")
        create_text(summary, "UkupanKapitalniGubitak", f"{total_loss:.2f}")
        create_text(summary, "Osnovica", f"{net_gain:.2f}")
        create_text(summary, "PorezZaUplatu", f"{tax:.2f}")
        
        # 8. DokaziUzPrijavu
        # Validator demands 'Part 8', and previous 'DokaziUzPrijavu' with Opis failed.
        # Valid files use 'DeklarisanoPriloziUzPrijavu' with file details.
        # We fill this with a placeholder so the XML structure is complete, 
        # and the user can delete/modify on the portal.
        part8_decl = doc.createElement("ns1:DeklarisanoPriloziUzPrijavu")
        root.appendChild(part8_decl)
        
        part8_item = doc.createElement("ns1:PriloziUzPrijavu")
        part8_decl.appendChild(part8_item)
        
        create_text(part8_item, "RedniBroj", "1")
        create_text(part8_item, "fileName", "IBKR_REPORT_MANUAL_UPLOAD.pdf")
        # Dummy URL mandatory for schema?
        create_text(part8_item, "fileUrl", "http://localhost/placeholder.pdf")
        
        return doc.toprettyxml(indent="  ")
