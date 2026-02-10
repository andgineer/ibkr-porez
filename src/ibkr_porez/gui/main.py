from __future__ import annotations

import ctypes
import sys

from PySide6.QtWidgets import QApplication

from ibkr_porez.gui.app_icon import create_app_icon
from ibkr_porez.gui.main_window import MainWindow


def _set_macos_process_name() -> None:
    if sys.platform != "darwin":
        return
    try:
        libc = ctypes.CDLL(None)
        setprogname = libc.setprogname
        setprogname.argtypes = [ctypes.c_char_p]
        setprogname.restype = None
        setprogname(b"ibkr-porez")
    except Exception:  # noqa: BLE001
        return


def run() -> int:
    _set_macos_process_name()
    app = QApplication(["ibkr-porez", *sys.argv[1:]])
    app.setApplicationName("ibkr-porez")
    app.setApplicationDisplayName("ibkr-porez")
    app.setDesktopFileName("ibkr-porez")
    app.setStyle("Fusion")
    app.setWindowIcon(create_app_icon())
    window = MainWindow()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(run())
