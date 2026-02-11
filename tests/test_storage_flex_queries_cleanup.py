from __future__ import annotations

from datetime import date
from pathlib import Path

from ibkr_porez.storage_flex_queries import _cleanup_old_base


def _touch(path: Path) -> None:
    path.write_text("x", encoding="utf-8")


def test_cleanup_old_base_removes_only_base_files_older_than_keep_date(tmp_path: Path) -> None:
    old_base = tmp_path / "base-20260110.xml.zip"
    keep_base = tmp_path / "base-20260115.xml.zip"
    new_base = tmp_path / "base-20260120.xml.zip"
    delta_file = tmp_path / "delta-20260110.patch.zip"
    invalid_base_name = tmp_path / "base-invalid.xml.zip"

    for file_path in (old_base, keep_base, new_base, delta_file, invalid_base_name):
        _touch(file_path)

    _cleanup_old_base(tmp_path, keep_date=date(2026, 1, 15))

    assert not old_base.exists()
    assert keep_base.exists()
    assert new_base.exists()
    assert delta_file.exists()
    assert invalid_base_name.exists()
