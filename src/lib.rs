#![allow(clippy::missing_errors_doc)]

pub mod config;
pub mod declaration_gains_xml;
pub mod declaration_income_xml;
pub mod declaration_manager;
pub mod due_date;
pub mod fetch;
pub mod holidays;
pub mod holidays_fallback;
pub mod ibkr_csv;
pub mod ibkr_flex;
pub mod import;
pub mod list;
pub mod logging;
pub mod models;
pub mod nbs;
pub mod openholiday;
pub mod regenerate;
pub mod report_gains;
pub mod report_income;
pub mod stat;
pub mod storage;
pub mod storage_flex;
pub mod sync;
pub mod tax;

#[cfg(feature = "gui")]
#[allow(clippy::missing_panics_doc, clippy::must_use_candidate)]
pub mod gui;
