use anyhow::Result;
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::debug;

const DEFAULT_BASE_URL: &str = "https://openholidaysapi.org";
const MAX_RETRIES: u32 = 2;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

pub struct OpenHolidayClient {
    http: reqwest::blocking::Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HolidayResponse {
    start_date: NaiveDate,
    end_date: NaiveDate,
}

impl Default for OpenHolidayClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenHolidayClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            http: build_http_client(),
            base_url: crate::config::base_url_override(crate::config::HOLIDAYS_URL_ENV)
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        }
    }

    #[must_use]
    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            http: build_http_client(),
            base_url: base_url.to_string(),
        }
    }

    /// Fetch Serbian public holidays for a range of years.
    ///
    /// The API limits queries to ~3-year spans, so wider ranges are
    /// automatically split into multiple requests.
    ///
    /// Returns holidays grouped by year. Each year maps to a sorted, deduplicated
    /// list of holiday dates. Multi-day holidays are expanded into individual dates.
    pub fn fetch_years(
        &self,
        from_year: i32,
        to_year: i32,
    ) -> Result<HashMap<i32, Vec<NaiveDate>>> {
        let mut all: HashMap<i32, Vec<NaiveDate>> = HashMap::new();

        let mut chunk_start = from_year;
        while chunk_start <= to_year {
            let chunk_end = (chunk_start + 2).min(to_year);
            let batch = self.fetch_chunk(chunk_start, chunk_end)?;
            for (year, mut dates) in batch {
                all.entry(year).or_default().append(&mut dates);
            }
            chunk_start = chunk_end + 1;
        }

        for dates in all.values_mut() {
            dates.sort();
            dates.dedup();
        }

        debug!(
            years = all.len(),
            total_dates = all.values().map(Vec::len).sum::<usize>(),
            "fetched holidays"
        );

        Ok(all)
    }

    fn fetch_chunk(&self, from_year: i32, to_year: i32) -> Result<HashMap<i32, Vec<NaiveDate>>> {
        let url = format!(
            "{}/PublicHolidays?countryIsoCode=RS&languageIsoCode=EN&validFrom={}-01-01&validTo={}-12-31",
            self.base_url, from_year, to_year,
        );

        debug!(from_year, to_year, "fetching holidays chunk");

        let holidays: Vec<HolidayResponse> = self.fetch_json(&url)?;

        let mut by_year: HashMap<i32, Vec<NaiveDate>> = HashMap::new();
        for h in &holidays {
            let mut d = h.start_date;
            while d <= h.end_date {
                by_year
                    .entry(chrono::Datelike::year(&d))
                    .or_default()
                    .push(d);
                d += chrono::Duration::days(1);
            }
        }

        Ok(by_year)
    }

    fn fetch_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut last_err = None;
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                std::thread::sleep(RETRY_DELAY);
            }
            match self.http.get(url).send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_client_error() {
                        return Err(anyhow::anyhow!("HTTP {status} from OpenHolidays API"));
                    }
                    if status.is_server_error() {
                        last_err = Some(anyhow::anyhow!("HTTP {status}"));
                        continue;
                    }
                    return Ok(resp.json()?);
                }
                Err(e) => {
                    last_err = Some(e.into());
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("OpenHolidays API request failed")))
    }
}

fn build_http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("TLS backend unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_response() -> &'static str {
        r#"[
            {"id":"a","startDate":"2025-01-01","endDate":"2025-01-02","type":"Public",
             "name":[{"language":"EN","text":"New Year"}],
             "regionalScope":"National","temporalScope":"FullDay","nationwide":true},
            {"id":"b","startDate":"2025-01-07","endDate":"2025-01-07","type":"Public",
             "name":[{"language":"EN","text":"Christmas"}],
             "regionalScope":"National","temporalScope":"FullDay","nationwide":true}
        ]"#
    }

    #[test]
    fn test_parse_and_expand_multi_day() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/PublicHolidays\?.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(sample_response())
            .create();

        let client = OpenHolidayClient::with_base_url(&server.url());
        let result = client.fetch_years(2025, 2025).unwrap();

        assert_eq!(result.len(), 1);
        let dates = &result[&2025];
        assert_eq!(
            dates,
            &vec![
                NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
                NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
            ]
        );
        mock.assert();
    }

    #[test]
    fn test_empty_response() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/PublicHolidays\?.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create();

        let client = OpenHolidayClient::with_base_url(&server.url());
        let result = client.fetch_years(2099, 2099).unwrap();
        assert!(result.is_empty());
        mock.assert();
    }

    #[test]
    fn test_client_error_fails_immediately() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/PublicHolidays\?.*".to_string()),
            )
            .with_status(400)
            .expect(1)
            .create();

        let client = OpenHolidayClient::with_base_url(&server.url());
        let result = client.fetch_years(2025, 2025);
        assert!(result.is_err());
        mock.assert();
    }

    #[test]
    fn test_server_error_retries() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/PublicHolidays\?.*".to_string()),
            )
            .with_status(500)
            .expect(2)
            .create();

        let client = OpenHolidayClient::with_base_url(&server.url());
        let result = client.fetch_years(2025, 2025);
        assert!(result.is_err());
        mock.assert();
    }

    #[test]
    fn test_multi_year_grouping() {
        let body = r#"[
            {"id":"a","startDate":"2025-12-31","endDate":"2026-01-02","type":"Public",
             "name":[{"language":"EN","text":"NYE"}],
             "regionalScope":"National","temporalScope":"FullDay","nationwide":true}
        ]"#;

        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/PublicHolidays\?.*".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();

        let client = OpenHolidayClient::with_base_url(&server.url());
        let result = client.fetch_years(2025, 2026).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[&2025],
            vec![NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()]
        );
        assert_eq!(
            result[&2026],
            vec![
                NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
            ]
        );
        mock.assert();
    }
}
