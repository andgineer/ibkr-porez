use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Result;
use chrono::NaiveDate;
use regex::Regex;

static HUNK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap()
});

const SMALL_FILE_THRESHOLD_BYTES: usize = 2048;

// ---------------------------------------------------------------------------
// Zip helpers
// ---------------------------------------------------------------------------

fn save_zipped_file(file_path: &Path, content: &str) -> Result<()> {
    let file = std::fs::File::create(file_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let inner_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("data");
    let options =
        zip::write::FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(inner_name, options)?;
    zip.write_all(content.as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn read_zipped_file(file_path: &Path) -> Result<String> {
    let file = std::fs::File::open(file_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive.by_index(0)?;
    let mut content = String::new();
    entry.read_to_string(&mut content)?;
    Ok(content)
}

// ---------------------------------------------------------------------------
// Date parsing from filenames
// ---------------------------------------------------------------------------

fn parse_date_from_filename(filename: &str) -> Option<NaiveDate> {
    let name = filename.strip_suffix(".zip").unwrap_or(filename);
    let date_str = if let Some(rest) = name.strip_prefix("base-") {
        rest.get(..8)?
    } else if let Some(rest) = name.strip_prefix("delta-") {
        rest.get(..8)?
    } else {
        return None;
    };
    NaiveDate::parse_from_str(date_str, "%Y%m%d").ok()
}

// ---------------------------------------------------------------------------
// File finders
// ---------------------------------------------------------------------------

fn find_base_file(dir: &Path, before_date: NaiveDate) -> Option<PathBuf> {
    let mut base_files: Vec<PathBuf> = glob_sorted(dir, "base-*.xml.zip");
    base_files.reverse();
    for f in base_files {
        if let Some(d) = parse_date_from_filename(f.file_name()?.to_str()?)
            && d <= before_date {
                return Some(f);
            }
    }
    None
}

fn get_delta_files_between(
    dir: &Path,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for f in glob_sorted(dir, "delta-*.patch.zip") {
        if let Some(name) = f.file_name().and_then(|n| n.to_str())
            && let Some(d) = parse_date_from_filename(name)
                && d > start_date && d <= end_date {
                    result.push(f);
                }
    }
    result
}

fn glob_sorted(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| glob_match(name, pattern))
        })
        .collect();
    files.sort();
    files
}

fn glob_match(name: &str, pattern: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() != 2 {
        return name.contains(pattern);
    }
    name.starts_with(parts[0]) && name.ends_with(parts[1])
}

// ---------------------------------------------------------------------------
// Patch application  (port of Python _apply_patch)
// ---------------------------------------------------------------------------

fn apply_patch(lines: &[String], patch_file: &Path) -> Result<Vec<String>> {
    let patch_text = read_zipped_file(patch_file)?;
    let patch_lines: Vec<&str> = patch_text.split_inclusive('\n').collect();
    if patch_lines.is_empty() {
        return Ok(lines.to_vec());
    }

    let hunk_re = &*HUNK_RE;
    let mut result: Vec<String> = lines.to_vec();
    let mut i = 0;
    let mut line_idx: usize = 0;

    while i < patch_lines.len() {
        let line = patch_lines[i];

        if line.starts_with("---") || line.starts_with("+++") {
            i += 1;
            continue;
        }

        if line.starts_with("@@") {
            if let Some(caps) = hunk_re.captures(line) {
                let old_start: usize = caps[1].parse().unwrap_or(0);
                line_idx = old_start.saturating_sub(1);
            } else {
                line_idx = 0;
            }
            i += 1;
            continue;
        }

        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        if !line.starts_with(['-', '+']) {
            if line_idx < result.len() {
                line_idx += 1;
            }
            i += 1;
            continue;
        }

        if line.starts_with('-') && !line.starts_with("---") {
            let content = &line[1..];
            if line_idx < result.len() && result[line_idx] == content {
                result.remove(line_idx);
            }
            i += 1;
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            let content = &line[1..];
            result.insert(line_idx, content.to_string());
            line_idx += 1;
            i += 1;
            continue;
        }

        i += 1;
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Unified diff generation  (matching Python difflib.unified_diff(n=0))
// ---------------------------------------------------------------------------

fn generate_unified_diff(
    old_lines: &[String],
    new_lines: &[String],
    from_file: &str,
    to_file: &str,
) -> String {
    use similar::TextDiff;

    let old_text = old_lines.join("");
    let new_text = new_lines.join("");

    let diff = TextDiff::from_lines(&old_text, &new_text);
    let mut result = diff
        .unified_diff()
        .context_radius(0)
        .header(from_file, to_file)
        .to_string();

    // similar adds a trailing "No newline at end of file" marker; strip it
    // if the Python output wouldn't have it.
    if result.ends_with("\n\\ No newline at end of file\n") {
        let marker = "\n\\ No newline at end of file\n";
        result.truncate(result.len() - marker.len());
        result.push('\n');
    }

    result
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

fn get_latest_report_content_any_date(dir: &Path) -> Option<(String, PathBuf)> {
    let mut base_files = glob_sorted(dir, "base-*.xml.zip");
    base_files.reverse();
    let base_file = base_files.into_iter().next()?;
    let base_date = parse_date_from_filename(base_file.file_name()?.to_str()?)?;

    let content = read_zipped_file(&base_file).ok()?;
    let mut lines: Vec<String> = content.split_inclusive('\n').map(String::from).collect();

    let all_deltas = glob_sorted(dir, "delta-*.patch.zip");
    for delta_file in all_deltas {
        if let Some(name) = delta_file.file_name().and_then(|n| n.to_str())
            && let Some(d) = parse_date_from_filename(name)
                && d >= base_date {
                    lines = apply_patch(&lines, &delta_file).unwrap_or(lines);
                }
    }

    Some((lines.join(""), base_file))
}

/// Save an XML report using delta compression.
pub fn save_raw_report_with_delta(
    flex_queries_dir: &Path,
    xml_content: &str,
    report_date: NaiveDate,
) -> Result<()> {
    let date_str = report_date.format("%Y%m%d").to_string();
    let base_file = flex_queries_dir.join(format!("base-{date_str}.xml.zip"));
    let delta_file = flex_queries_dir.join(format!("delta-{date_str}.patch.zip"));

    if base_file.exists() {
        std::fs::remove_file(&base_file)?;
    }
    if delta_file.exists() {
        std::fs::remove_file(&delta_file)?;
    }

    let result = get_latest_report_content_any_date(flex_queries_dir);
    let Some((previous_xml, actual_base_file)) = result else {
        save_zipped_file(&base_file, xml_content)?;
        return Ok(());
    };

    let previous_lines: Vec<String> = previous_xml
        .split_inclusive('\n')
        .map(String::from)
        .collect();
    let current_lines: Vec<String> = xml_content
        .split_inclusive('\n')
        .map(String::from)
        .collect();

    let from_name = actual_base_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("base");
    let to_name = format!("delta-{date_str}.patch");

    let delta_content = generate_unified_diff(&previous_lines, &current_lines, from_name, &to_name);

    save_zipped_file(&delta_file, &delta_content)?;

    let base_size = previous_xml.len();
    let delta_size = delta_content.len();
    let threshold = if base_size < SMALL_FILE_THRESHOLD_BYTES {
        0.95
    } else {
        0.3
    };
    #[allow(clippy::cast_precision_loss)]
    if delta_size as f64 > base_size as f64 * threshold {
        std::fs::remove_file(&delta_file)?;
        save_zipped_file(&base_file, xml_content)?;
    }

    Ok(())
}

/// Restore a full XML report for a specific date.
pub fn restore_report(flex_queries_dir: &Path, report_date: NaiveDate) -> Option<String> {
    let base_file = find_base_file(flex_queries_dir, report_date)?;
    let base_content = read_zipped_file(&base_file).ok()?;
    let mut lines: Vec<String> = base_content
        .split_inclusive('\n')
        .map(String::from)
        .collect();

    let base_date = parse_date_from_filename(base_file.file_name()?.to_str()?)?;
    let delta_files = get_delta_files_between(flex_queries_dir, base_date, report_date);

    for df in delta_files {
        lines = apply_patch(&lines, &df).unwrap_or(lines);
    }

    Some(lines.join(""))
}

/// Remove old base files, keeping only those on or after `keep_date`.
pub fn cleanup_old_base(flex_queries_dir: &Path, keep_date: NaiveDate) {
    for f in glob_sorted(flex_queries_dir, "base-*.xml.zip") {
        if let Some(name) = f.file_name().and_then(|n| n.to_str())
            && let Some(d) = parse_date_from_filename(name)
                && d < keep_date {
                    let _ = std::fs::remove_file(&f);
                }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_from_filename() {
        let d = parse_date_from_filename("base-20260129.xml.zip");
        assert_eq!(d, Some(NaiveDate::from_ymd_opt(2026, 1, 29).unwrap()));

        let d = parse_date_from_filename("delta-20260130.patch.zip");
        assert_eq!(d, Some(NaiveDate::from_ymd_opt(2026, 1, 30).unwrap()));

        assert_eq!(parse_date_from_filename("unknown.zip"), None);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("base-20260129.xml.zip", "base-*.xml.zip"));
        assert!(!glob_match("delta-20260129.patch.zip", "base-*.xml.zip"));
    }
}
