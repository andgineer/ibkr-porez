use chrono::NaiveDate;
use quick_xml::Writer;
use quick_xml::events::{BytesCData, BytesEnd, BytesStart, BytesText, Event};
use rust_decimal::Decimal;

use crate::due_date::next_working_day;
use crate::holidays::HolidayCalendar;
use crate::models::{IncomeDeclarationEntry, UserConfig};

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_income_xml(
    entry: &IncomeDeclarationEntry,
    config: &UserConfig,
    holidays: &HolidayCalendar,
) -> String {
    let due_date = next_working_day(entry.date, holidays);

    let mut buf = Vec::new();
    let mut w = Writer::new_with_indent(&mut buf, b' ', 2);

    w.write_event(Event::Decl(quick_xml::events::BytesDecl::new(
        "1.0", None, None,
    )))
    .unwrap();

    let mut root = BytesStart::new("ns1:PodaciPoreskeDeklaracije");
    root.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
    root.push_attribute(("xmlns:ns1", "http://pid.purs.gov.rs"));
    w.write_event(Event::Start(root)).unwrap();

    write_podaci_o_prijavi(&mut w, entry, due_date);
    write_poreski_obveznik(&mut w, config);
    write_nacin_ostvarivanja(&mut w);
    write_vrste_prihoda(&mut w, entry);
    write_ukupno(&mut w, entry);
    write_kamata(&mut w);
    write_dodatna_kamata(&mut w);

    w.write_event(Event::End(BytesEnd::new("ns1:PodaciPoreskeDeklaracije")))
        .unwrap();

    String::from_utf8(buf).unwrap()
}

fn write_podaci_o_prijavi(
    w: &mut Writer<&mut Vec<u8>>,
    entry: &IncomeDeclarationEntry,
    due_date: NaiveDate,
) {
    start(w, "ns1:PodaciOPrijavi");
    text_elem(w, "ns1:VrstaPrijave", "1");
    text_elem(
        w,
        "ns1:ObracunskiPeriod",
        &entry.date.format("%Y-%m").to_string(),
    );
    text_elem(w, "ns1:DatumOstvarivanjaPrihoda", &fmt_date(entry.date));
    text_elem(w, "ns1:Rok", "1");
    text_elem(w, "ns1:DatumDospelostiObaveze", &fmt_date(due_date));
    end(w, "ns1:PodaciOPrijavi");
}

fn write_poreski_obveznik(w: &mut Writer<&mut Vec<u8>>, config: &UserConfig) {
    start(w, "ns1:PodaciOPoreskomObvezniku");
    text_elem(w, "ns1:PoreskiIdentifikacioniBroj", &config.personal_id);
    cdata_elem(w, "ns1:ImePrezimeObveznika", &config.full_name);
    cdata_elem(w, "ns1:UlicaBrojPoreskogObveznika", &config.address);
    text_elem(w, "ns1:PrebivalisteOpstina", &config.city_code);
    text_elem(w, "ns1:JMBGPodnosiocaPrijave", &config.personal_id);
    text_elem(w, "ns1:TelefonKontaktOsobe", &config.phone);
    cdata_elem(w, "ns1:ElektronskaPosta", &config.email);
    end(w, "ns1:PodaciOPoreskomObvezniku");
}

fn write_nacin_ostvarivanja(w: &mut Writer<&mut Vec<u8>>) {
    start(w, "ns1:PodaciONacinuOstvarivanjaPrihoda");
    text_elem(w, "ns1:NacinIsplate", "3");
    text_elem(w, "ns1:Ostalo", "Isplata na brokerski racun");
    end(w, "ns1:PodaciONacinuOstvarivanjaPrihoda");
}

fn write_vrste_prihoda(w: &mut Writer<&mut Vec<u8>>, entry: &IncomeDeclarationEntry) {
    start(w, "ns1:DeklarisaniPodaciOVrstamaPrihoda");
    start(w, "ns1:PodaciOVrstamaPrihoda");
    text_elem(w, "ns1:RedniBroj", "1");
    text_elem(w, "ns1:SifraVrstePrihoda", &entry.sifra_vrste_prihoda);
    text_elem(w, "ns1:BrutoPrihod", &fmt2(entry.bruto_prihod));
    text_elem(w, "ns1:OsnovicaZaPorez", &fmt2(entry.osnovica_za_porez));
    text_elem(w, "ns1:ObracunatiPorez", &fmt2(entry.obracunati_porez));
    text_elem(
        w,
        "ns1:PorezPlacenDrugojDrzavi",
        &fmt2(entry.porez_placen_drugoj_drzavi),
    );
    text_elem(w, "ns1:PorezZaUplatu", &fmt2(entry.porez_za_uplatu));
    end(w, "ns1:PodaciOVrstamaPrihoda");
    end(w, "ns1:DeklarisaniPodaciOVrstamaPrihoda");
}

fn write_ukupno(w: &mut Writer<&mut Vec<u8>>, entry: &IncomeDeclarationEntry) {
    start(w, "ns1:Ukupno");
    text_elem(w, "ns1:FondSati", "0.00");
    text_elem(w, "ns1:BrutoPrihod", &fmt2(entry.bruto_prihod));
    text_elem(w, "ns1:OsnovicaZaPorez", &fmt2(entry.osnovica_za_porez));
    text_elem(w, "ns1:ObracunatiPorez", &fmt2(entry.obracunati_porez));
    text_elem(
        w,
        "ns1:PorezPlacenDrugojDrzavi",
        &fmt2(entry.porez_placen_drugoj_drzavi),
    );
    text_elem(w, "ns1:PorezZaUplatu", &fmt2(entry.porez_za_uplatu));
    text_elem(w, "ns1:OsnovicaZaDoprinose", "0.00");
    text_elem(w, "ns1:PIO", "0.00");
    text_elem(w, "ns1:ZDRAVSTVO", "0.00");
    text_elem(w, "ns1:NEZAPOSLENOST", "0.00");
    end(w, "ns1:Ukupno");
}

fn write_kamata(w: &mut Writer<&mut Vec<u8>>) {
    start(w, "ns1:Kamata");
    text_elem(w, "ns1:PorezZaUplatu", "0");
    text_elem(w, "ns1:OsnovicaZaDoprinose", "0");
    text_elem(w, "ns1:PIO", "0");
    text_elem(w, "ns1:ZDRAVSTVO", "0");
    text_elem(w, "ns1:NEZAPOSLENOST", "0");
    end(w, "ns1:Kamata");
}

fn write_dodatna_kamata(w: &mut Writer<&mut Vec<u8>>) {
    start(w, "ns1:PodaciODodatnojKamati");
    end(w, "ns1:PodaciODodatnojKamati");
}

fn start(w: &mut Writer<&mut Vec<u8>>, name: &str) {
    w.write_event(Event::Start(BytesStart::new(name))).unwrap();
}

fn end(w: &mut Writer<&mut Vec<u8>>, name: &str) {
    w.write_event(Event::End(BytesEnd::new(name))).unwrap();
}

fn text_elem(w: &mut Writer<&mut Vec<u8>>, name: &str, value: &str) {
    w.write_event(Event::Start(BytesStart::new(name))).unwrap();
    w.write_event(Event::Text(BytesText::new(value))).unwrap();
    w.write_event(Event::End(BytesEnd::new(name))).unwrap();
}

fn cdata_elem(w: &mut Writer<&mut Vec<u8>>, name: &str, value: &str) {
    w.write_event(Event::Start(BytesStart::new(name))).unwrap();
    w.write_event(Event::CData(BytesCData::new(value))).unwrap();
    w.write_event(Event::End(BytesEnd::new(name))).unwrap();
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn fmt2(d: Decimal) -> String {
    format!("{d:.2}")
}
