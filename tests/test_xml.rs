use chrono::{Duration, NaiveDate};
use ibkr_porez::declaration_gains_xml::generate_gains_xml;
use ibkr_porez::declaration_income_xml::generate_income_xml;
use ibkr_porez::declaration_manager::validate_purs_number;
use ibkr_porez::due_date::next_working_day;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::{IncomeDeclarationEntry, PriorRecognizedLoss, TaxReportEntry, UserConfig};
use ibkr_porez::report_income::Amends;
use rust_decimal_macros::dec;

fn test_config() -> UserConfig {
    UserConfig {
        ibkr_token: "tok".into(),
        ibkr_query_id: "qid".into(),
        personal_id: "1234567890123".into(),
        full_name: "Test User".into(),
        address: "Test Street 1".into(),
        city_code: "223".into(),
        phone: "0641234567".into(),
        email: "test@test.com".into(),
        data_dir: None,
        output_folder: None,
    }
}

#[test]
fn test_gains_xml_structure() {
    let cal = HolidayCalendar::load_embedded();
    let entries = vec![TaxReportEntry {
        ticker: "AAPL".into(),
        quantity: dec!(10),
        sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
        sale_price: dec!(170.0),
        sale_exchange_rate: dec!(108.0000),
        sale_value_rsd: dec!(183600.00),
        purchase_date: NaiveDate::from_ymd_opt(2023, 1, 15).unwrap(),
        purchase_price: dec!(150.0),
        purchase_exchange_rate: dec!(117.5000),
        purchase_value_rsd: dec!(176250.00),
        capital_gain_rsd: dec!(7350.00),
        is_tax_exempt: false,
    }];

    let xml = generate_gains_xml(
        &entries,
        &test_config(),
        NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        &cal,
        &[],
    );

    assert!(xml.contains("xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\""));
    assert!(xml.contains("xmlns:ns1=\"http://pid.purs.gov.rs\""));
    assert!(xml.contains("<ns1:TipPoreskogObveznika>1</ns1:TipPoreskogObveznika>"));
    assert!(xml.contains("<ns1:DatumPodnosenjaPrijave>"));
    assert!(xml.contains("<ns1:NacinPodnosenjaPrijave>E</ns1:NacinPodnosenjaPrijave>"));
    assert!(xml.contains("<ns1:DatumINacinPodnosenjaPrijave>"));
    assert!(xml.contains("<ns1:PodaciOUtvrđivanju>"));
    assert!(xml.contains("<ns1:VrstaPrijave>1</ns1:VrstaPrijave>"));
    assert!(xml.contains("<ns1:OsnovZaPrijavu>4</ns1:OsnovZaPrijavu>"));
    assert!(xml.contains("<ns1:IsplataUDelovima>0</ns1:IsplataUDelovima>"));
}

#[test]
fn test_gains_xml_uppercases_name_and_address() {
    let cal = HolidayCalendar::load_embedded();
    let entries = vec![TaxReportEntry {
        ticker: "X".into(),
        quantity: dec!(1),
        sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
        sale_price: dec!(10),
        sale_exchange_rate: dec!(100),
        sale_value_rsd: dec!(1000),
        purchase_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        purchase_price: dec!(5),
        purchase_exchange_rate: dec!(100),
        purchase_value_rsd: dec!(500),
        capital_gain_rsd: dec!(500),
        is_tax_exempt: false,
    }];

    let xml = generate_gains_xml(
        &entries,
        &test_config(),
        NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        &cal,
        &[],
    );
    assert!(xml.contains("TEST USER"));
    assert!(xml.contains("TEST STREET 1"));
}

#[test]
fn test_gains_xml_entry_with_id_attr() {
    let cal = HolidayCalendar::load_embedded();
    let entries = vec![
        TaxReportEntry {
            ticker: "A".into(),
            quantity: dec!(1),
            sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
            sale_price: dec!(10),
            sale_exchange_rate: dec!(100),
            sale_value_rsd: dec!(1000),
            purchase_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            purchase_price: dec!(5),
            purchase_exchange_rate: dec!(100),
            purchase_value_rsd: dec!(500),
            capital_gain_rsd: dec!(500),
            is_tax_exempt: false,
        },
        TaxReportEntry {
            ticker: "B".into(),
            quantity: dec!(2),
            sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
            sale_price: dec!(20),
            sale_exchange_rate: dec!(100),
            sale_value_rsd: dec!(4000),
            purchase_date: NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(),
            purchase_price: dec!(15),
            purchase_exchange_rate: dec!(100),
            purchase_value_rsd: dec!(3000),
            capital_gain_rsd: dec!(1000),
            is_tax_exempt: false,
        },
    ];

    let xml = generate_gains_xml(
        &entries,
        &test_config(),
        NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        &cal,
        &[],
    );
    assert!(xml.contains("id=\"1\""));
    assert!(xml.contains("id=\"2\""));
}

#[test]
fn test_gains_xml_tax_exemption_marker() {
    let cal = HolidayCalendar::load_embedded();
    let entries = vec![TaxReportEntry {
        ticker: "OLD".into(),
        quantity: dec!(1),
        sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
        sale_price: dec!(10),
        sale_exchange_rate: dec!(100),
        sale_value_rsd: dec!(1000),
        purchase_date: NaiveDate::from_ymd_opt(2010, 1, 1).unwrap(),
        purchase_price: dec!(5),
        purchase_exchange_rate: dec!(100),
        purchase_value_rsd: dec!(500),
        capital_gain_rsd: dec!(500),
        is_tax_exempt: true,
    }];

    let xml = generate_gains_xml(
        &entries,
        &test_config(),
        NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        &cal,
        &[],
    );
    assert!(xml.contains("<ns1:PoreskoOslobodjenje>DA</ns1:PoreskoOslobodjenje>"));
}

#[test]
fn test_gains_xml_gain_loss_split() {
    let cal = HolidayCalendar::load_embedded();
    let entries = vec![TaxReportEntry {
        ticker: "X".into(),
        quantity: dec!(1),
        sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
        sale_price: dec!(5),
        sale_exchange_rate: dec!(100),
        sale_value_rsd: dec!(500),
        purchase_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        purchase_price: dec!(10),
        purchase_exchange_rate: dec!(100),
        purchase_value_rsd: dec!(1000),
        capital_gain_rsd: dec!(-500),
        is_tax_exempt: false,
    }];

    let xml = generate_gains_xml(
        &entries,
        &test_config(),
        NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        &cal,
        &[],
    );
    assert!(xml.contains("<ns1:KapitalniDobitak>0.00</ns1:KapitalniDobitak>"));
    assert!(xml.contains("<ns1:KapitalniGubitak>500.00</ns1:KapitalniGubitak>"));
}

#[test]
fn test_gains_xml_prior_losses_section() {
    let cal = HolidayCalendar::load_embedded();
    let entries = vec![TaxReportEntry {
        ticker: "A".into(),
        quantity: dec!(1),
        sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
        sale_price: dec!(20),
        sale_exchange_rate: dec!(100),
        sale_value_rsd: dec!(2000),
        purchase_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        purchase_price: dec!(10),
        purchase_exchange_rate: dec!(100),
        purchase_value_rsd: dec!(1000),
        capital_gain_rsd: dec!(1000),
        is_tax_exempt: false,
    }];
    let prior_losses = vec![
        PriorRecognizedLoss {
            assessment_reference: Some("435-01234/2023".into()),
            assessment_date: NaiveDate::from_ymd_opt(2023, 3, 10),
            remaining_loss_rsd: dec!(600),
        },
        PriorRecognizedLoss {
            assessment_reference: None,
            assessment_date: None,
            remaining_loss_rsd: dec!(300),
        },
    ];

    let xml = generate_gains_xml(
        &entries,
        &test_config(),
        NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        &cal,
        &prior_losses,
    );

    assert!(xml.contains("<ns1:DeklarisanoKapitalniGubiciRanijihGodina>"));
    assert!(xml.contains(
        "<ns1:BrojResenjaOUtvrdjivanjuKapitalnogGubitka>435-01234/2023\
         </ns1:BrojResenjaOUtvrdjivanjuKapitalnogGubitka>"
    ));
    assert!(xml.contains(
        "<ns1:DatumDonosenjaResenjaOKapitalnomGubitku>2023-03-10\
         </ns1:DatumDonosenjaResenjaOKapitalnomGubitku>"
    ));
    assert!(xml.contains(
        "<ns1:IznosUtvrdjenogKapitalnogGubitka>600.00</ns1:IznosUtvrdjenogKapitalnogGubitka>"
    ));
    assert!(xml.contains(
        "<ns1:IznosUtvrdjenogKapitalnogGubitka>300.00</ns1:IznosUtvrdjenogKapitalnogGubitka>"
    ));
    assert!(xml.contains(
        "<ns1:KapitalniGubitakIzRanijihGodina>900.00</ns1:KapitalniGubitakIzRanijihGodina>"
    ));
    assert!(xml.contains("<ns1:Osnovica>100.00</ns1:Osnovica>"));
    assert!(xml.contains("<ns1:PorezZaUplatu>15.00</ns1:PorezZaUplatu>"));
}

#[test]
fn test_gains_xml_no_prior_losses_omits_section() {
    let cal = HolidayCalendar::load_embedded();
    let entries = vec![TaxReportEntry {
        ticker: "A".into(),
        quantity: dec!(1),
        sale_date: NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
        sale_price: dec!(20),
        sale_exchange_rate: dec!(100),
        sale_value_rsd: dec!(2000),
        purchase_date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        purchase_price: dec!(10),
        purchase_exchange_rate: dec!(100),
        purchase_value_rsd: dec!(1000),
        capital_gain_rsd: dec!(1000),
        is_tax_exempt: false,
    }];

    let xml = generate_gains_xml(
        &entries,
        &test_config(),
        NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        &cal,
        &[],
    );

    assert!(!xml.contains("DeklarisanoKapitalniGubici"));
    assert!(!xml.contains("KapitalniGubitakIzRanijihGodina"));
    assert!(xml.contains("<ns1:Osnovica>1000.00</ns1:Osnovica>"));
}

#[test]
fn test_income_xml_structure() {
    let cal = HolidayCalendar::load_embedded();
    let entry = IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        symbol_or_currency: Some("VOO".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(1000.00),
        osnovica_za_porez: dec!(1000.00),
        obracunati_porez: dec!(150.00),
        porez_placen_drugoj_drzavi: dec!(100.00),
        porez_za_uplatu: dec!(50.00),
    };

    let xml = generate_income_xml(
        &entry,
        &test_config(),
        &cal,
        NaiveDate::from_ymd_opt(2023, 8, 1).unwrap(),
        None,
    );

    assert!(xml.contains("xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\""));
    assert!(xml.contains("xmlns:ns1=\"http://pid.purs.gov.rs\""));
    assert!(!xml.contains("<ns1:TipPoreskogObveznika>"));
    assert!(!xml.contains("<ns1:DatumINacinPodnosenjaPrijave>"));
    assert!(xml.contains("<ns1:ObracunskiPeriod>2023-07</ns1:ObracunskiPeriod>"));
    assert!(xml.contains("<ns1:Rok>1</ns1:Rok>"));
    assert!(xml.contains("<ns1:NacinIsplate>3</ns1:NacinIsplate>"));
}

#[test]
fn test_income_xml_does_not_uppercase() {
    let cal = HolidayCalendar::load_embedded();
    let entry = IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        symbol_or_currency: Some("VOO".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(1000.00),
        osnovica_za_porez: dec!(1000.00),
        obracunati_porez: dec!(150.00),
        porez_placen_drugoj_drzavi: dec!(100.00),
        porez_za_uplatu: dec!(50.00),
    };

    let xml = generate_income_xml(
        &entry,
        &test_config(),
        &cal,
        NaiveDate::from_ymd_opt(2023, 8, 1).unwrap(),
        None,
    );
    assert!(xml.contains("Test User"));
    assert!(xml.contains("Test Street 1"));
    assert!(!xml.contains("TEST USER"));
}

#[test]
fn test_income_xml_kamata_values_are_zero_not_zero_point_zero() {
    let cal = HolidayCalendar::load_embedded();
    let entry = IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        symbol_or_currency: Some("VOO".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(1000.00),
        osnovica_za_porez: dec!(1000.00),
        obracunati_porez: dec!(150.00),
        porez_placen_drugoj_drzavi: dec!(100.00),
        porez_za_uplatu: dec!(50.00),
    };

    let xml = generate_income_xml(
        &entry,
        &test_config(),
        &cal,
        NaiveDate::from_ymd_opt(2023, 8, 1).unwrap(),
        None,
    );
    let kamata_start = xml.find("<ns1:Kamata>").unwrap();
    let kamata_end = xml.find("</ns1:Kamata>").unwrap();
    let kamata_section = &xml[kamata_start..kamata_end];

    assert!(kamata_section.contains(">0<"));
    assert!(!kamata_section.contains("0.00"));
}

#[test]
fn test_income_xml_element_names_differ_from_gains() {
    let cal = HolidayCalendar::load_embedded();
    let entry = IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        symbol_or_currency: Some("VOO".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(1000.00),
        osnovica_za_porez: dec!(1000.00),
        obracunati_porez: dec!(150.00),
        porez_placen_drugoj_drzavi: dec!(100.00),
        porez_za_uplatu: dec!(50.00),
    };

    let xml = generate_income_xml(
        &entry,
        &test_config(),
        &cal,
        NaiveDate::from_ymd_opt(2023, 8, 1).unwrap(),
        None,
    );
    assert!(xml.contains("<ns1:ImePrezimeObveznika>"));
    assert!(xml.contains("<ns1:UlicaBrojPoreskogObveznika>"));
    assert!(xml.contains("<ns1:PrebivalisteOpstina>"));
    assert!(!xml.contains("<ns1:ImeIPrezimePoreskogObveznika>"));
    assert!(!xml.contains("<ns1:PrebivalisteBoravistePoreskogObveznika>"));
}

// ---------------------------------------------------------------------------
// Timeliness and amendment: two independent fields, each with its own codebook
// ---------------------------------------------------------------------------

fn income_entry() -> IncomeDeclarationEntry {
    IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        symbol_or_currency: Some("VOO".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(1000.00),
        osnovica_za_porez: dec!(1000.00),
        obracunati_porez: dec!(150.00),
        porez_placen_drugoj_drzavi: dec!(100.00),
        porez_za_uplatu: dec!(50.00),
    }
}

#[test]
fn vrsta_prijave_is_1_before_the_due_date() {
    let cal = HolidayCalendar::load_embedded();
    let entry = income_entry();
    let due = next_working_day(entry.date, &cal);

    let xml = generate_income_xml(&entry, &test_config(), &cal, due, None);
    assert!(xml.contains("<ns1:VrstaPrijave>1</ns1:VrstaPrijave>"));
}

#[test]
fn vrsta_prijave_is_3_after_the_due_date() {
    let cal = HolidayCalendar::load_embedded();
    let entry = income_entry();
    let due = next_working_day(entry.date, &cal);

    let xml = generate_income_xml(&entry, &test_config(), &cal, due + Duration::days(1), None);
    assert!(xml.contains("<ns1:VrstaPrijave>3</ns1:VrstaPrijave>"));
}

#[test]
fn amendment_writes_izmena_elements() {
    let cal = HolidayCalendar::load_embedded();
    let entry = income_entry();
    let amends = Amends {
        declaration_id: "3".into(),
        purs_number: Some("1234567890".into()),
    };

    let xml = generate_income_xml(&entry, &test_config(), &cal, entry.date, Some(&amends));
    assert!(xml.contains("<ns1:VrstaIzmenePrijave>1</ns1:VrstaIzmenePrijave>"));
    assert!(xml.contains("<ns1:IdentifikatorPrijave>1234567890</ns1:IdentifikatorPrijave>"));
    // PodaciOPrijavi is an xs:sequence: both follow DatumDospelostiObaveze.
    let due = xml.find("DatumDospelostiObaveze").unwrap();
    let izmena = xml.find("VrstaIzmenePrijave").unwrap();
    let identifikator = xml.find("IdentifikatorPrijave").unwrap();
    assert!(due < izmena && izmena < identifikator);

    // Timeliness is a separate fact, still judged by the date alone.
    assert!(xml.contains("<ns1:VrstaPrijave>1</ns1:VrstaPrijave>"));

    let unknown_number = Amends {
        declaration_id: "3".into(),
        purs_number: None,
    };
    let xml = generate_income_xml(
        &entry,
        &test_config(),
        &cal,
        entry.date,
        Some(&unknown_number),
    );
    assert!(xml.contains("<ns1:VrstaIzmenePrijave>1</ns1:VrstaIzmenePrijave>"));
    assert!(
        !xml.contains("IdentifikatorPrijave"),
        "optional in the schema, so it is omitted rather than written empty"
    );
}

#[test]
fn a_declaration_is_not_an_amendment() {
    let cal = HolidayCalendar::load_embedded();
    let entry = income_entry();

    let xml = generate_income_xml(&entry, &test_config(), &cal, entry.date, None);
    assert!(!xml.contains("VrstaIzmenePrijave"));
    assert!(!xml.contains("IdentifikatorPrijave"));
}

#[test]
fn submit_number_rejects_non_numeric_and_over_19_digits() {
    // xs:unsignedLong, at most 19 digits.
    assert_eq!(
        validate_purs_number("1234567890123456789").unwrap(),
        "1234567890123456789"
    );
    assert_eq!(validate_purs_number(" 42 ").unwrap(), "42");

    assert!(validate_purs_number("12345678901234567890").is_err());
    assert!(validate_purs_number("12a45").is_err());
    assert!(validate_purs_number("-42").is_err());
    assert!(validate_purs_number("").is_err());
}
