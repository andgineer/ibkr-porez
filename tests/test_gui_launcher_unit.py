from __future__ import annotations

import plistlib
from pathlib import Path

import pytest

import ibkr_porez.gui.launcher as launcher_module


def test_prepare_gui_process_identity_calls_macos_only(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[str] = []
    monkeypatch.setattr(
        launcher_module,
        "_set_macos_process_name",
        lambda: calls.append("mac"),
    )

    monkeypatch.setattr(launcher_module.sys, "platform", "linux")
    launcher_module.prepare_gui_process_identity()
    assert calls == []

    monkeypatch.setattr(launcher_module.sys, "platform", "darwin")
    launcher_module.prepare_gui_process_identity()
    assert calls == ["mac"]


def test_build_macos_gui_bundle_creates_plist_and_launcher(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    monkeypatch.setattr(launcher_module.sys, "executable", "/usr/bin/python3")

    bundle_path = launcher_module._build_macos_gui_bundle(tmp_path, "1.2.3")

    info_plist_path = bundle_path / "Contents" / "Info.plist"
    launcher_path = bundle_path / "Contents" / "MacOS" / "ibkr-porez"

    assert bundle_path == tmp_path / "ibkr-porez.app"
    assert info_plist_path.exists()
    assert launcher_path.exists()
    assert launcher_path.stat().st_mode & 0o111

    with open(info_plist_path, "rb") as info_file:
        plist_data = plistlib.load(info_file)

    assert plist_data["CFBundleName"] == "ibkr-porez"
    assert plist_data["CFBundleDisplayName"] == "ibkr-porez"
    assert plist_data["CFBundleIdentifier"] == "engineer.sorokin.ibkr-porez"
    assert plist_data["CFBundleVersion"] == "1.2.3"
    assert plist_data["CFBundleExecutable"] == "ibkr-porez"

    launcher_content = launcher_path.read_text(encoding="utf-8")
    assert launcher_content.startswith("#!/bin/sh\n")
    assert 'exec "/usr/bin/python3" -m ibkr_porez.gui.main "$@"\n' in launcher_content


def test_ensure_macos_gui_bundle_falls_back_to_temp_directory(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    first_base_dir = tmp_path / "first"
    second_tmp_dir = tmp_path / "tmp-root"
    expected_bundle = second_tmp_dir / "ibkr-porez" / "ibkr-porez.app"
    call_order: list[Path] = []

    monkeypatch.setattr(
        launcher_module,
        "user_data_dir",
        lambda _app_name: str(first_base_dir),
    )
    monkeypatch.setattr(
        launcher_module.tempfile,
        "gettempdir",
        lambda: str(second_tmp_dir),
    )

    def fake_build(base_dir: Path, _app_version: str) -> Path:
        call_order.append(base_dir)
        if base_dir == first_base_dir:
            raise OSError("first location unavailable")
        return expected_bundle

    monkeypatch.setattr(launcher_module, "_build_macos_gui_bundle", fake_build)

    result = launcher_module._ensure_macos_gui_bundle("0.0.1")

    assert result == expected_bundle
    assert call_order == [first_base_dir, second_tmp_dir / "ibkr-porez"]
