use chrono::{Local, NaiveDate};
use quick_xml::Writer;
use quick_xml::events::{BytesCData, BytesEnd, BytesStart, BytesText, Event};
use rust_decimal::Decimal;

use crate::due_date::next_working_day;
use crate::holidays::HolidayCalendar;
use crate::models::{TaxReportEntry, UserConfig};

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_gains_xml(
    entries: &[TaxReportEntry],
    config: &UserConfig,
    period_end: NaiveDate,
    holidays: &HolidayCalendar,
) -> String {
    let due_date = next_working_day(period_end, holidays);
    let today = Local::now().date_naive();

    let total_gain: Decimal = entries
        .iter()
        .map(|e| e.capital_gain_rsd.max(Decimal::ZERO))
        .sum();
    let total_loss: Decimal = entries
        .iter()
        .map(|e| e.capital_gain_rsd.min(Decimal::ZERO).abs())
        .sum();
    let osnovica = (total_gain - total_loss).max(Decimal::ZERO);
    let porez = (osnovica * Decimal::new(15, 2)).round_dp(2);

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

    write_section_podaci_o_prijavi(&mut w, period_end, due_date, today);
    write_section_poreski_obveznik(&mut w, config);
    write_section_deklarisano(&mut w, entries);
    write_section_utvrdjivanje(&mut w, total_gain, total_loss, osnovica, porez);
    write_section_prilozi(&mut w);

    w.write_event(Event::End(BytesEnd::new("ns1:PodaciPoreskeDeklaracije")))
        .unwrap();

    String::from_utf8(buf).unwrap()
}

fn write_section_podaci_o_prijavi(
    w: &mut Writer<&mut Vec<u8>>,
    period_end: NaiveDate,
    due_date: NaiveDate,
    today: NaiveDate,
) {
    start(w, "ns1:PodaciOPrijavi");
    text_elem(w, "ns1:VrstaPrijave", "1");
    text_elem(w, "ns1:OsnovZaPrijavu", "4");
    text_elem(
        w,
        "ns1:DatumOstvarenjaPrihodaDelaPrihoda",
        &fmt_date(period_end),
    );
    text_elem(w, "ns1:IsplataUDelovima", "0");
    text_elem(
        w,
        "ns1:DatumDospelostiZaPodnosenjePoreskePrijave",
        &fmt_date(due_date),
    );

    start(w, "ns1:DatumINacinPodnosenjaPrijave");
    text_elem(w, "ns1:DatumPodnosenjaPrijave", &fmt_date(today));
    text_elem(w, "ns1:NacinPodnosenjaPrijave", "E");
    end(w, "ns1:DatumINacinPodnosenjaPrijave");

    end(w, "ns1:PodaciOPrijavi");
}

fn write_section_poreski_obveznik(w: &mut Writer<&mut Vec<u8>>, config: &UserConfig) {
    start(w, "ns1:PodaciOPoreskomObvezniku");
    text_elem(w, "ns1:TipPoreskogObveznika", "1");
    text_elem(w, "ns1:PoreskiIdentifikacioniBroj", &config.personal_id);
    cdata_elem(
        w,
        "ns1:ImeIPrezimePoreskogObveznika",
        &config.full_name.to_uppercase(),
    );
    text_elem(
        w,
        "ns1:PrebivalisteBoravistePoreskogObveznika",
        &config.city_code,
    );
    cdata_elem(
        w,
        "ns1:AdresaPoreskogObveznika",
        &config.address.to_uppercase(),
    );
    text_elem(w, "ns1:TelefonKontaktOsobe", &config.phone);
    cdata_elem(w, "ns1:ElektronskaPosta", &config.email);
    text_elem(w, "ns1:JMBGPodnosiocaPrijave", &config.personal_id);
    end(w, "ns1:PodaciOPoreskomObvezniku");
}

fn write_section_deklarisano(w: &mut Writer<&mut Vec<u8>>, entries: &[TaxReportEntry]) {
    start(w, "ns1:DeklarisanoPrenosHOVInvesticionihJed");
    for (i, e) in entries.iter().enumerate() {
        let idx = i + 1;
        let mut el = BytesStart::new("ns1:PodaciOPrenosuHOVInvesticionihJed");
        el.push_attribute(("id", idx.to_string().as_str()));
        w.write_event(Event::Start(el)).unwrap();

        text_elem(w, "ns1:RedniBroj", &idx.to_string());
        cdata_elem(w, "ns1:NazivEmitenta", &e.ticker);
        text_elem(w, "ns1:DatumPrenosaHOV", &fmt_date(e.sale_date));
        text_elem(w, "ns1:BrojDokumentaOPrenosu", "1");
        text_elem(w, "ns1:BrojPrenetihHOVInvesticionihJed", &fmt2(e.quantity));
        text_elem(w, "ns1:ProdajnaCena", &fmt2(e.sale_value_rsd));

        start(w, "ns1:Sticanje");
        text_elem(w, "ns1:DatumSticanja", &fmt_date(e.purchase_date));
        text_elem(w, "ns1:NabavnaCena", &fmt2(e.purchase_value_rsd));
        text_elem(w, "ns1:BrojDokumentaOSticanju", "1");
        text_elem(w, "ns1:BrojStecenihHOVInvesticionihJed", &fmt2(e.quantity));
        end(w, "ns1:Sticanje");

        let gain = e.capital_gain_rsd;
        text_elem(w, "ns1:KapitalniDobitak", &fmt2(gain.max(Decimal::ZERO)));
        text_elem(
            w,
            "ns1:KapitalniGubitak",
            &fmt2(gain.min(Decimal::ZERO).abs()),
        );
        if e.is_tax_exempt {
            text_elem(w, "ns1:PoreskoOslobodjenje", "DA");
        }

        end(w, "ns1:PodaciOPrenosuHOVInvesticionihJed");
    }
    end(w, "ns1:DeklarisanoPrenosHOVInvesticionihJed");
}

fn write_section_utvrdjivanje(
    w: &mut Writer<&mut Vec<u8>>,
    total_gain: Decimal,
    total_loss: Decimal,
    osnovica: Decimal,
    porez: Decimal,
) {
    start(w, "ns1:PodaciOUtvrđivanju");
    text_elem(w, "ns1:UkupanKapitalniDobitak", &fmt2(total_gain));
    text_elem(w, "ns1:UkupanKapitalniGubitak", &fmt2(total_loss));
    text_elem(w, "ns1:Osnovica", &fmt2(osnovica));
    text_elem(w, "ns1:PorezZaUplatu", &fmt2(porez));
    end(w, "ns1:PodaciOUtvrđivanju");
}

fn write_section_prilozi(w: &mut Writer<&mut Vec<u8>>) {
    start(w, "ns1:DeklarisanoPriloziUzPrijavu");
    start(w, "ns1:PriloziUzPrijavu");
    text_elem(w, "ns1:RedniBroj", "1");
    text_elem(w, "ns1:fileName", "IBKR_REPORT_MANUAL_UPLOAD.pdf");
    text_elem(w, "ns1:fileUrl", "http://localhost/placeholder.pdf");
    end(w, "ns1:PriloziUzPrijavu");
    end(w, "ns1:DeklarisanoPriloziUzPrijavu");
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
