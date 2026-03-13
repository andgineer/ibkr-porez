use chrono::NaiveDate;
use ibkr_porez::declaration_gains_xml::generate_gains_xml;
use ibkr_porez::declaration_income_xml::generate_income_xml;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::{IncomeDeclarationEntry, TaxReportEntry, UserConfig};
use quick_xml::Reader;
use quick_xml::events::Event;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn golden_config() -> UserConfig {
    UserConfig {
        ibkr_token: "tok".into(),
        ibkr_query_id: "qid".into(),
        personal_id: "1234567890123".into(),
        full_name: "Test User".into(),
        address: "Test City Test Street 001".into(),
        city_code: "223".into(),
        phone: "0601234567".into(),
        email: "test@example.com".into(),
        data_dir: None,
        output_folder: None,
    }
}

fn holidays() -> HolidayCalendar {
    let mut cal = HolidayCalendar::empty();
    cal.set_fallback(true);
    cal
}

fn extract_leaf_elements(xml: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut elements = Vec::new();
    let mut current_tag: Option<String> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                current_tag = Some(String::from_utf8_lossy(e.name().as_ref()).to_string());
                current_text.clear();
            }
            Ok(Event::Text(e)) => {
                current_text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::CData(e)) => {
                current_text.push_str(&String::from_utf8_lossy(&e.into_inner()));
            }
            Ok(Event::End(_)) => {
                if let Some(tag) = current_tag.take()
                    && !current_text.is_empty()
                {
                    elements.push((tag, current_text.clone()));
                }
                current_text.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }
    elements
}

fn elements_to_map(elements: &[(String, String)]) -> HashMap<&str, &str> {
    let mut map = HashMap::new();
    for (k, v) in elements {
        map.insert(k.as_str(), v.as_str());
    }
    map
}

fn extract_top_level_children(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut children = Vec::new();
    let mut depth = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                if depth == 2 {
                    children.push(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 1 {
                    children.push(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }
    children
}

// ========================================================================
// PP-OPO (income) golden-file tests
// ========================================================================

#[test]
fn test_ppopo_xml_structure_matches_golden() {
    let golden = include_str!("resources/golden-004-ppopo-voo-2025-1224.xml");
    let golden_children = extract_top_level_children(golden);

    let entry = IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2025, 12, 24).unwrap(),
        symbol_or_currency: Some("VOO".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(2113.28),
        osnovica_za_porez: dec!(2113.28),
        obracunati_porez: dec!(316.99),
        porez_placen_drugoj_drzavi: dec!(634.48),
        porez_za_uplatu: dec!(0.00),
    };

    let rust_xml = generate_income_xml(&entry, &golden_config(), &holidays());
    let rust_children = extract_top_level_children(&rust_xml);

    assert_eq!(
        golden_children, rust_children,
        "top-level XML sections must match between Python and Rust"
    );
}

#[test]
fn test_ppopo_xml_values_match_golden() {
    let golden = include_str!("resources/golden-004-ppopo-voo-2025-1224.xml");
    let golden_elems = extract_leaf_elements(golden);
    let golden_map = elements_to_map(&golden_elems);

    let entry = IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2025, 12, 24).unwrap(),
        symbol_or_currency: Some("VOO".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(2113.28),
        osnovica_za_porez: dec!(2113.28),
        obracunati_porez: dec!(316.99),
        porez_placen_drugoj_drzavi: dec!(634.48),
        porez_za_uplatu: dec!(0.00),
    };

    let rust_xml = generate_income_xml(&entry, &golden_config(), &holidays());
    let rust_elems = extract_leaf_elements(&rust_xml);
    let rust_map = elements_to_map(&rust_elems);

    let check_fields = [
        "ns1:VrstaPrijave",
        "ns1:ObracunskiPeriod",
        "ns1:DatumOstvarivanjaPrihoda",
        "ns1:Rok",
        "ns1:DatumDospelostiObaveze",
        "ns1:PoreskiIdentifikacioniBroj",
        "ns1:ImePrezimeObveznika",
        "ns1:UlicaBrojPoreskogObveznika",
        "ns1:PrebivalisteOpstina",
        "ns1:JMBGPodnosiocaPrijave",
        "ns1:TelefonKontaktOsobe",
        "ns1:ElektronskaPosta",
        "ns1:NacinIsplate",
        "ns1:Ostalo",
        "ns1:SifraVrstePrihoda",
        "ns1:BrutoPrihod",
        "ns1:OsnovicaZaPorez",
        "ns1:ObracunatiPorez",
        "ns1:PorezPlacenDrugojDrzavi",
    ];

    for field in &check_fields {
        let golden_val = golden_map.get(field);
        let rust_val = rust_map.get(field);
        assert_eq!(
            golden_val, rust_val,
            "mismatch for {field}: golden={golden_val:?}, rust={rust_val:?}"
        );
    }
}

#[test]
fn test_ppopo_xml_ixus_values_match_golden() {
    let golden = include_str!("resources/golden-002-ppopo-ixus-2025-1219.xml");
    let golden_elems = extract_leaf_elements(golden);
    let golden_map = elements_to_map(&golden_elems);

    let entry = IncomeDeclarationEntry {
        date: NaiveDate::from_ymd_opt(2025, 12, 19).unwrap(),
        symbol_or_currency: Some("IXUS".into()),
        sifra_vrste_prihoda: "111402000".into(),
        bruto_prihod: dec!(4087.08),
        osnovica_za_porez: dec!(4087.08),
        obracunati_porez: dec!(613.06),
        porez_placen_drugoj_drzavi: dec!(3677.57),
        porez_za_uplatu: dec!(0.00),
    };

    let rust_xml = generate_income_xml(&entry, &golden_config(), &holidays());
    let rust_elems = extract_leaf_elements(&rust_xml);
    let rust_map = elements_to_map(&rust_elems);

    for field in [
        "ns1:DatumOstvarivanjaPrihoda",
        "ns1:DatumDospelostiObaveze",
        "ns1:BrutoPrihod",
        "ns1:ObracunatiPorez",
        "ns1:PorezPlacenDrugojDrzavi",
        "ns1:PorezZaUplatu",
    ] {
        assert_eq!(
            golden_map.get(field),
            rust_map.get(field),
            "mismatch for {field}"
        );
    }
}

// ========================================================================
// PPDG-3R (capital gains) golden-file tests
// ========================================================================

fn extract_gains_entries(xml: &str) -> Vec<HashMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut entries = Vec::new();
    let mut in_entry = false;
    let mut in_sticanje = false;
    let mut current: HashMap<String, String> = HashMap::new();
    let mut current_tag: Option<String> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "ns1:PodaciOPrenosuHOVInvesticionihJed" {
                    in_entry = true;
                    current.clear();
                } else if name == "ns1:Sticanje" {
                    in_sticanje = true;
                }
                current_tag = Some(name);
                current_text.clear();
            }
            Ok(Event::Text(e)) => {
                current_text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::CData(e)) => {
                current_text.push_str(&String::from_utf8_lossy(&e.into_inner()));
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "ns1:PodaciOPrenosuHOVInvesticionihJed" {
                    in_entry = false;
                    entries.push(current.clone());
                } else if name == "ns1:Sticanje" {
                    in_sticanje = false;
                } else if in_entry && let Some(tag) = current_tag.take() {
                    let key = if in_sticanje {
                        format!("Sticanje.{tag}")
                    } else {
                        tag
                    };
                    if !current_text.is_empty() {
                        current.insert(key, current_text.clone());
                    }
                }
                current_text.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }
    entries
}

#[test]
fn test_ppdg3r_xml_structure_matches_golden() {
    let golden = include_str!("resources/golden-001-ppdg3r-2025-H2.xml");
    let golden_children = extract_top_level_children(golden);

    let expected_sections = vec![
        "ns1:PodaciOPrijavi",
        "ns1:PodaciOPoreskomObvezniku",
        "ns1:DeklarisanoPrenosHOVInvesticionihJed",
        "ns1:PodaciOUtvrđivanju",
        "ns1:DeklarisanoPriloziUzPrijavu",
    ];
    assert_eq!(
        golden_children
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        expected_sections,
        "golden file should have expected top-level sections"
    );

    let entries = extract_gains_entries(golden);
    assert_eq!(entries.len(), 14, "golden PPDG-3R has 14 entries");

    assert_eq!(entries[0]["ns1:NazivEmitenta"], "IJH");
    assert_eq!(entries[0]["ns1:BrojPrenetihHOVInvesticionihJed"], "77.00");
    assert_eq!(entries[0]["ns1:ProdajnaCena"], "516180.13");
    assert_eq!(entries[0]["ns1:KapitalniDobitak"], "0.00");
    assert_eq!(entries[0]["ns1:KapitalniGubitak"], "146.56");
    assert_eq!(entries[0]["Sticanje.ns1:NabavnaCena"], "516326.69");
}

#[test]
fn test_ppdg3r_xml_totals_match_golden() {
    let golden = include_str!("resources/golden-001-ppdg3r-2025-H2.xml");
    let golden_elems = extract_leaf_elements(golden);
    let golden_map = elements_to_map(&golden_elems);

    assert_eq!(golden_map["ns1:UkupanKapitalniDobitak"], "696.39");
    assert_eq!(golden_map["ns1:UkupanKapitalniGubitak"], "23271.56");
    assert_eq!(golden_map["ns1:Osnovica"], "0.00");
    assert_eq!(golden_map["ns1:PorezZaUplatu"], "0.00");
}

#[test]
fn test_ppdg3r_rust_xml_matches_golden_structure() {
    let golden = include_str!("resources/golden-001-ppdg3r-2025-H2.xml");
    let golden_children = extract_top_level_children(golden);

    let entries = vec![TaxReportEntry {
        ticker: "IJH".into(),
        quantity: dec!(77),
        sale_date: NaiveDate::from_ymd_opt(2025, 12, 26).unwrap(),
        sale_price: dec!(67.32),
        sale_exchange_rate: dec!(99.5787),
        sale_value_rsd: dec!(516180.13),
        purchase_date: NaiveDate::from_ymd_opt(2025, 12, 23).unwrap(),
        purchase_price: dec!(67.275),
        purchase_exchange_rate: dec!(99.6736),
        purchase_value_rsd: dec!(516326.69),
        capital_gain_rsd: dec!(-146.56),
        is_tax_exempt: false,
    }];

    let period_end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
    let rust_xml = generate_gains_xml(&entries, &golden_config(), period_end, &holidays());
    let rust_children = extract_top_level_children(&rust_xml);

    assert_eq!(
        golden_children, rust_children,
        "Rust PPDG-3R XML must have same top-level sections as Python golden"
    );
}

#[test]
fn test_ppdg3r_rust_entry_values_match_golden_first_entry() {
    let golden = include_str!("resources/golden-001-ppdg3r-2025-H2.xml");
    let golden_entries = extract_gains_entries(golden);
    let first_golden = &golden_entries[0];

    let entries = vec![TaxReportEntry {
        ticker: "IJH".into(),
        quantity: dec!(77),
        sale_date: NaiveDate::from_ymd_opt(2025, 12, 26).unwrap(),
        sale_price: dec!(67.32),
        sale_exchange_rate: dec!(99.5787),
        sale_value_rsd: dec!(516180.13),
        purchase_date: NaiveDate::from_ymd_opt(2025, 12, 23).unwrap(),
        purchase_price: dec!(67.275),
        purchase_exchange_rate: dec!(99.6736),
        purchase_value_rsd: dec!(516326.69),
        capital_gain_rsd: dec!(-146.56),
        is_tax_exempt: false,
    }];

    let period_end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
    let rust_xml = generate_gains_xml(&entries, &golden_config(), period_end, &holidays());
    let rust_entries = extract_gains_entries(&rust_xml);
    let first_rust = &rust_entries[0];

    for field in [
        "ns1:NazivEmitenta",
        "ns1:DatumPrenosaHOV",
        "ns1:BrojPrenetihHOVInvesticionihJed",
        "ns1:ProdajnaCena",
        "ns1:KapitalniDobitak",
        "ns1:KapitalniGubitak",
        "Sticanje.ns1:DatumSticanja",
        "Sticanje.ns1:NabavnaCena",
        "Sticanje.ns1:BrojStecenihHOVInvesticionihJed",
    ] {
        assert_eq!(
            first_golden.get(field),
            first_rust.get(field),
            "entry field mismatch for {field}"
        );
    }
}

#[test]
fn test_ppopo_golden_value_formats_are_two_decimal_places() {
    let golden = include_str!("resources/golden-004-ppopo-voo-2025-1224.xml");
    let elems = extract_leaf_elements(golden);

    // Only PodaciOVrstamaPrihoda / Ukupno use 2-decimal format;
    // Kamata section uses "0" (no decimals) — matching Python behavior.
    let two_dp_fields = [
        "ns1:BrutoPrihod",
        "ns1:OsnovicaZaPorez",
        "ns1:ObracunatiPorez",
        "ns1:PorezPlacenDrugojDrzavi",
        "ns1:FondSati",
    ];

    for (tag, val) in &elems {
        if two_dp_fields.contains(&tag.as_str()) {
            assert!(
                val.contains('.'),
                "golden file field {tag}={val} should have decimal point"
            );
            let parts: Vec<&str> = val.split('.').collect();
            assert_eq!(
                parts[1].len(),
                2,
                "golden file field {tag}={val} should have exactly 2 decimal places"
            );
        }
    }
}

#[test]
fn test_ppdg3r_golden_gains_quantity_format() {
    let golden = include_str!("resources/golden-001-ppdg3r-2025-H2.xml");
    let entries = extract_gains_entries(golden);

    for (i, entry) in entries.iter().enumerate() {
        let qty = &entry["ns1:BrojPrenetihHOVInvesticionihJed"];
        assert!(
            qty.ends_with(".00"),
            "entry {i}: quantity {qty} should end with .00"
        );
    }
}

#[test]
fn test_all_ppopo_golden_files_have_consistent_structure() {
    let golden_files = [
        include_str!("resources/golden-002-ppopo-ixus-2025-1219.xml"),
        include_str!("resources/golden-003-ppopo-sgov-2025-1224.xml"),
        include_str!("resources/golden-004-ppopo-voo-2025-1224.xml"),
        include_str!("resources/golden-005-ppopo-sgov-2026-0205.xml"),
        include_str!("resources/golden-006-ppopo-sgov-2026-0305.xml"),
    ];

    let expected_sections = vec![
        "ns1:PodaciOPrijavi",
        "ns1:PodaciOPoreskomObvezniku",
        "ns1:PodaciONacinuOstvarivanjaPrihoda",
        "ns1:DeklarisaniPodaciOVrstamaPrihoda",
        "ns1:Ukupno",
        "ns1:Kamata",
        "ns1:PodaciODodatnojKamati",
    ];

    for (i, xml) in golden_files.iter().enumerate() {
        let children = extract_top_level_children(xml);
        assert_eq!(
            children.iter().map(String::as_str).collect::<Vec<_>>(),
            expected_sections,
            "golden PP-OPO file {i} should have expected sections"
        );
    }
}
