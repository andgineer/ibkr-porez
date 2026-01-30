from datetime import date
from decimal import Decimal
from ibkr_porez.report import XMLGenerator
from ibkr_porez.models import TaxReportEntry, UserConfig


def test_xml_generator_structure():
    # Setup
    config = UserConfig(
        ibkr_token="dummy",
        ibkr_query_id="dummy",
        personal_id="1234567890123",
        full_name="Petar Petrovic",
        address="Bulevar Oslobodjenja 1",
        city_code="223",
        phone="0631234567",
        email="pp@example.com",
    )
    xml_gen = XMLGenerator(config)

    entries = [
        TaxReportEntry(
            ticker="AAPL",
            quantity=Decimal("10"),
            sale_date=date(2023, 6, 15),
            sale_price=Decimal("150.00"),
            sale_exchange_rate=Decimal("117.0000"),
            sale_value_rsd=Decimal("175500.00"),
            purchase_date=date(2023, 1, 15),
            purchase_price=Decimal("100.00"),
            purchase_exchange_rate=Decimal("117.0000"),
            purchase_value_rsd=Decimal("117000.00"),
            capital_gain_rsd=Decimal("58500.00"),
            is_tax_exempt=False,
        )
    ]

    start_date = date(2023, 1, 1)
    end_date = date(2023, 6, 30)

    # Execute
    xml_out = xml_gen.generate_xml(entries, start_date, end_date)

    # Verify namespaces and root
    assert 'xmlns:ns1="http://pid.purs.gov.rs"' in xml_out
    assert "ns1:PodaciPoreskeDeklaracije" in xml_out

    # Verify proper nesting
    assert "<ns1:PodaciOPrijavi>" in xml_out
    assert "<ns1:PodaciOPoreskomObvezniku>" in xml_out
    assert "<ns1:DeklarisanoPrenosHOVInvesticionihJed>" in xml_out

    # Verify CDATA and fields
    assert "PETAR PETROVIC" in xml_out  # ImeIPrezime
    assert "<![CDATA[PETAR PETROVIC]]>" in xml_out or "PETAR PETROVIC" in xml_out

    # Verify numeric
    assert "175500.00" in xml_out

    # Verify new mandatory fields
    assert "2023-06-30" in xml_out  # Ostvarenje Prihoda (End)
    assert "2023-07-31" in xml_out  # Due Date (H1 -> July 31)

    # Verify Config Fields in XML
    assert "0631234567" in xml_out
    assert "pp@example.com" in xml_out
    assert (
        "<ns1:PrebivalisteBoravistePoreskogObveznika>223</ns1:PrebivalisteBoravistePoreskogObveznika>"
        in xml_out
    )

    # Verify Osnov 4
    assert "<ns1:OsnovZaPrijavu>4</ns1:OsnovZaPrijavu>" in xml_out

    # Verify Part 3 REMOVED (should NOT be present)
    assert "<ns1:PodaciZaUtvrdjivanjePorezaKodPrenosaPravaIliUdela" not in xml_out

    # Verify Child Tag Name Fixed and Specific Fields
    assert '<ns1:PodaciOPrenosuHOVInvesticionihJed id="1">' in xml_out
    assert "<ns1:NazivEmitenta><![CDATA[AAPL]]></ns1:NazivEmitenta>" in xml_out
    assert "<ns1:DatumPrenosaHOV>2023-06-15</ns1:DatumPrenosaHOV>" in xml_out

    # Verify Part 8 (Attachments)
    assert "<ns1:DeklarisanoPriloziUzPrijavu>" in xml_out
    assert "<ns1:fileName>IBKR_REPORT_MANUAL_UPLOAD.pdf</ns1:fileName>" in xml_out

    # Verify summary
    assert "8775.00" in xml_out
