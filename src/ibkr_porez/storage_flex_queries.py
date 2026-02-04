"""Storage and restoration of flex query XML reports using delta compression."""

import difflib
import re
import zipfile
from datetime import date
from pathlib import Path

from ibkr_porez.storage import Storage

# Constants
SMALL_FILE_THRESHOLD_BYTES = 2048  # Files smaller than this use higher delta threshold


def _save_zipped_file(file_path: Path, content: str) -> None:
    """Save content to a zip file."""
    with zipfile.ZipFile(file_path, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(file_path.stem, content)


def _read_zipped_file(file_path: Path) -> str:
    """Read content from a zip file."""
    with zipfile.ZipFile(file_path, "r") as zf:
        # Get the first (and only) file in the zip
        member_name = zf.namelist()[0]
        return zf.read(member_name).decode("utf-8")


def save_raw_report_with_delta(
    storage: Storage,
    xml_content: str,
    report_date: date,
) -> None:
    """
    Save raw XML report using delta compression.

    Only one report per day is kept (latest replaces previous).
    Reports for different days use delta compression to save space.

    Args:
        storage: Storage instance
        xml_content: Full XML content from IBKR
        report_date: Date of the report
    """
    flex_queries_dir = storage.flex_queries_dir
    base_file = flex_queries_dir / f"base_{report_date.strftime('%Y%m%d')}.xml.zip"
    delta_file = flex_queries_dir / f"delta_{report_date.strftime('%Y%m%d')}.patch.zip"

    # Remove any existing files for this date (only one query per day)
    if base_file.exists():
        base_file.unlink()
    if delta_file.exists():
        delta_file.unlink()

    # Find previous report from a different day (for delta compression)
    result = _get_latest_report_content_any_date(flex_queries_dir)
    if result is None:
        # No previous report found, save as new base
        _save_zipped_file(base_file, xml_content)
        return

    previous_xml, actual_base_file = result

    # Use delta compression
    # (we already removed files for current date, so this is from different day)
    previous_lines = previous_xml.splitlines(keepends=True)
    current_lines = xml_content.splitlines(keepends=True)
    # Use the inner filename (without .zip) for the delta header
    inner_delta_name = f"delta_{report_date.strftime('%Y%m%d')}.patch"
    delta = list(
        difflib.unified_diff(
            previous_lines,
            current_lines,
            lineterm="\n",  # Use newline so each line in delta is separate
            fromfile=actual_base_file.stem,  # Use actual base file name (without .zip)
            tofile=inner_delta_name,
            n=0,  # No context - only changed lines
        ),
    )

    # Save delta
    delta_content = "".join(delta)
    _save_zipped_file(delta_file, delta_content)

    # If delta is too large (more than 30% of base for files > 2KB),
    # save as new base instead to prevent storing large deltas
    base_size = len(previous_xml)
    delta_size = len(delta_content)
    # For small files (< 2KB), delta overhead can be significant, so use higher threshold
    # For large files, 30% threshold ensures better space savings (prevents storing 30-50KB deltas)
    threshold = 0.95 if base_size < SMALL_FILE_THRESHOLD_BYTES else 0.3
    if delta_size > base_size * threshold:
        # Delta is too large, save as new base and remove delta
        delta_file.unlink()
        _save_zipped_file(base_file, xml_content)


def restore_report(storage: Storage, report_date: date) -> str | None:
    """
    Restore full XML report for a specific date.

    Applies deltas sequentially to base file to reconstruct the full XML.

    Args:
        storage: Storage instance
        report_date: Date of the report to restore

    Returns:
        Full XML content as string, or None if report not found
    """
    flex_queries_dir = storage.flex_queries_dir

    # Find base file (closest before or equal to report_date)
    base_file = _find_base_file(flex_queries_dir, report_date)
    if base_file is None:
        return None

    # Read base file
    base_content = _read_zipped_file(base_file)
    current_lines = base_content.splitlines(keepends=True)

    # Apply all deltas from base_date to report_date
    base_date = _parse_date_from_filename(base_file.name)
    if base_date is None:
        return None

    delta_files = _get_delta_files_between(flex_queries_dir, base_date, report_date)
    for delta_file in delta_files:
        current_lines = _apply_patch(current_lines, delta_file)

    return "".join(current_lines)


def _get_latest_report_content_any_date(flex_queries_dir: Path) -> tuple[str, Path] | None:
    """
    Get content of the latest report (regardless of date).

    Returns:
        tuple[str, Path] | None: (content, base_file_path) or None if no report found
    """
    # Find the most recent base file (by date in filename)
    base_files = sorted(flex_queries_dir.glob("base_*.xml.zip"), reverse=True)
    if not base_files:
        return None

    base_file = base_files[0]  # Most recent base file
    base_date = _parse_date_from_filename(base_file.name)
    if base_date is None:
        return None

    # Read base
    content = _read_zipped_file(base_file)

    # Apply ALL deltas after this base file (regardless of date)
    lines = content.splitlines(keepends=True)
    # Get all delta files sorted by date
    all_delta_files = sorted(flex_queries_dir.glob("delta_*.patch.zip"))
    for delta_file in all_delta_files:
        delta_date = _parse_date_from_filename(delta_file.name)
        if delta_date and delta_date >= base_date:
            lines = _apply_patch(lines, delta_file)

    return "".join(lines), base_file


def _get_latest_report_content(flex_queries_dir: Path, before_date: date) -> str | None:
    """Get content of the latest report before given date."""
    # Find base file
    base_file = _find_base_file(flex_queries_dir, before_date)
    if base_file is None:
        return None

    # Read base
    content = _read_zipped_file(base_file)

    # Apply all deltas up to before_date
    base_date = _parse_date_from_filename(base_file.name)
    if base_date is None:
        return content

    lines = content.splitlines(keepends=True)
    delta_files = _get_delta_files_between(flex_queries_dir, base_date, before_date)
    for delta_file in delta_files:
        lines = _apply_patch(lines, delta_file)

    return "".join(lines)


def _find_base_file(flex_queries_dir: Path, before_date: date) -> Path | None:
    """Find base file closest to (before or equal to) given date."""
    base_files = sorted(flex_queries_dir.glob("base_*.xml.zip"), reverse=True)
    for base_file in base_files:
        file_date = _parse_date_from_filename(base_file.name)
        if file_date and file_date <= before_date:
            return base_file
    return None


def _get_delta_files_between(
    flex_queries_dir: Path,
    start_date: date,
    end_date: date,
) -> list[Path]:
    """Get all delta files between start_date and end_date (inclusive)."""
    delta_files = []
    for delta_file in sorted(flex_queries_dir.glob("delta_*.patch.zip")):
        file_date = _parse_date_from_filename(delta_file.name)
        if file_date and start_date < file_date <= end_date:
            delta_files.append(delta_file)
    return delta_files


def _apply_patch(lines: list[str], patch_file: Path) -> list[str]:  # noqa: C901
    """
    Apply unified diff patch to lines.

    Parses hunk headers to get correct line positions.
    Unified diff format: @@ -old_start,old_count +new_start,new_count @@
    When old_count is omitted, it defaults to 1 (not 0).
    """
    patch_text = _read_zipped_file(patch_file)

    # Parse unified diff
    patch_lines = patch_text.splitlines(keepends=True)
    if not patch_lines:
        return lines

    result = lines.copy()
    i = 0
    line_idx = 0  # Initialize line index

    while i < len(patch_lines):
        line = patch_lines[i]

        # Skip diff headers
        if line.startswith(("---", "+++")):
            i += 1
            continue

        # Hunk header: @@ -old_start,old_count +new_start,new_count @@
        if line.startswith("@@"):
            match = re.match(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", line)
            if match:
                old_start = int(match.group(1))  # 1-based line number
                # When old_count is omitted, it defaults to 1 (not 0)
                # Convert to 0-based index
                # Start at old_start - 1 (0-based) for modifications
                line_idx = old_start - 1
            else:
                line_idx = 0
            i += 1
            continue

        # Skip empty lines
        if not line.strip():
            i += 1
            continue

        # Context line (unchanged) - advance line index
        if not line.startswith(("-", "+")):
            if line_idx < len(result):
                line_idx += 1
            i += 1
            continue

        # Remove line (starts with -)
        if line.startswith("-") and not line.startswith("---"):
            content = line[1:]
            # Only remove if content matches (safety check)
            if line_idx < len(result) and result[line_idx] == content:
                result.pop(line_idx)
            # Don't advance line_idx after removal
            i += 1
            continue

        # Add line (starts with +)
        if line.startswith("+") and not line.startswith("+++"):
            content = line[1:]
            result.insert(line_idx, content)
            line_idx += 1  # Advance after insertion
            i += 1
            continue

        i += 1

    return result


def _parse_date_from_filename(filename: str) -> date | None:
    """Parse date from filename like 'base_20260129.xml.zip' or 'delta_20260130.patch.zip'."""
    try:
        # Remove .zip extension if present
        name_without_zip = filename.removesuffix(".zip")
        # Extract date part (YYYYMMDD)
        if name_without_zip.startswith("base_"):
            date_str = name_without_zip[5:13]  # "base_YYYYMMDD.xml"
        elif name_without_zip.startswith("delta_"):
            date_str = name_without_zip[6:14]  # "delta_YYYYMMDD.patch"
        else:
            return None

        return date(int(date_str[:4]), int(date_str[4:6]), int(date_str[6:8]))
    except (ValueError, IndexError):
        return None


def _cleanup_old_base(flex_queries_dir: Path, keep_date: date) -> None:
    """Remove old base files, keeping only the one for keep_date."""
    for base_file in flex_queries_dir.glob("base_*.xml.zip"):
        file_date = _parse_date_from_filename(base_file.name)
        if file_date and file_date < keep_date:
            base_file.unlink()
