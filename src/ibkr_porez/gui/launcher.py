"""GUI process launcher helpers."""

from __future__ import annotations

import ctypes
import plistlib
import subprocess
import sys
import tempfile
import time
from importlib.util import find_spec
from pathlib import Path
from typing import TypedDict

from platformdirs import user_data_dir
from rich.console import Console

_MIN_STATUS_VISIBLE_SECONDS = 0.8
_WINDOWS_APP_ID = "engineer.sorokin.ibkr-porez"
_PROCESS_NAME = b"ibkr-porez"


class _GuiPopenKwargs(TypedDict, total=False):
    stdin: int
    stdout: int
    stderr: int
    close_fds: bool
    creationflags: int
    start_new_session: bool


def prepare_gui_process_identity() -> None:
    """Apply platform-specific process identity settings for GUI process."""
    if sys.platform == "darwin":
        _set_macos_process_name()
    elif sys.platform == "win32":
        _set_windows_app_id()


def launch_gui_process(console: Console, app_version: str) -> None:
    """Launch GUI in a detached process and return immediately."""
    if find_spec("ibkr_porez.gui.main") is None:
        raise RuntimeError(
            "GUI module is not available. Reinstall package so GUI package is included.",
        )

    started_at = time.monotonic()
    with console.status("[bold green]Starting GUI...[/bold green]"):
        if sys.platform == "darwin":
            bundle_path = _ensure_macos_gui_bundle(app_version)
            command = ["open", "-na", str(bundle_path)]
        elif sys.platform == "win32":
            pythonw_executable = Path(sys.executable).with_name("pythonw.exe")
            gui_interpreter = (
                str(pythonw_executable) if pythonw_executable.exists() else sys.executable
            )
            command = [gui_interpreter, "-m", "ibkr_porez.gui.main"]
        else:
            command = [sys.executable, "-m", "ibkr_porez.gui.main"]
        popen_kwargs: _GuiPopenKwargs = {
            "stdin": subprocess.DEVNULL,
            "stdout": subprocess.DEVNULL,
            "stderr": subprocess.DEVNULL,
            "close_fds": True,
        }
        if sys.platform == "win32":
            popen_kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            popen_kwargs["start_new_session"] = True

        process = subprocess.Popen(command, **popen_kwargs)  # noqa: S603

        deadline = time.monotonic() + 1.4
        while time.monotonic() < deadline:
            return_code = process.poll()
            if return_code is not None:
                if return_code != 0:
                    raise RuntimeError("GUI process exited immediately.")
                break
            time.sleep(0.14)
        elapsed = time.monotonic() - started_at
        if elapsed < _MIN_STATUS_VISIBLE_SECONDS:
            time.sleep(_MIN_STATUS_VISIBLE_SECONDS - elapsed)


def _set_macos_process_name() -> None:
    try:
        libc = ctypes.CDLL(None)
        setprogname = libc.setprogname
        setprogname.argtypes = [ctypes.c_char_p]
        setprogname.restype = None
        setprogname(_PROCESS_NAME)
    except Exception:  # noqa: BLE001
        return


def _set_windows_app_id() -> None:
    try:
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(  # type: ignore[attr-defined]
            _WINDOWS_APP_ID,
        )
    except Exception:  # noqa: BLE001
        return


def _ensure_macos_gui_bundle(app_version: str) -> Path:
    """Create/reuse a minimal .app bundle and return its path."""
    candidates = [
        Path(user_data_dir("ibkr-porez")),
        Path(tempfile.gettempdir()) / "ibkr-porez",
    ]
    errors: list[str] = []

    for base_dir in candidates:
        try:
            return _build_macos_gui_bundle(base_dir, app_version)
        except OSError as e:
            errors.append(f"{base_dir}: {e}")

    joined_errors = "; ".join(errors)
    raise RuntimeError(
        f"Unable to prepare macOS app bundle for GUI launch. Tried: {joined_errors}",
    )


def _build_macos_gui_bundle(base_dir: Path, app_version: str) -> Path:
    """Build macOS bundle files in the provided base directory."""
    bundle_path = base_dir / "ibkr-porez.app"
    contents_dir = bundle_path / "Contents"
    macos_dir = contents_dir / "MacOS"
    contents_dir.mkdir(parents=True, exist_ok=True)
    macos_dir.mkdir(parents=True, exist_ok=True)

    info_plist = contents_dir / "Info.plist"
    plist_data = {
        "CFBundleName": "ibkr-porez",
        "CFBundleDisplayName": "ibkr-porez",
        "CFBundleIdentifier": "engineer.sorokin.ibkr-porez",
        "CFBundleVersion": app_version,
        "CFBundleShortVersionString": app_version,
        "CFBundlePackageType": "APPL",
        "CFBundleExecutable": "ibkr-porez",
        "NSHighResolutionCapable": True,
    }
    with open(info_plist, "wb") as f:
        plistlib.dump(plist_data, f, sort_keys=False)

    launcher = macos_dir / "ibkr-porez"
    launcher_contents = f'#!/bin/sh\nexec "{sys.executable}" -m ibkr_porez.gui.main "$@"\n'
    launcher.write_text(launcher_contents, encoding="utf-8")
    launcher.chmod(0o755)

    return bundle_path
