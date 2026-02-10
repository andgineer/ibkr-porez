from __future__ import annotations

import sys

from PySide6.QtWidgets import QApplication

from ibkr_porez.gui.app_icon import create_app_icon
from ibkr_porez.gui.launcher import prepare_gui_process_identity
from ibkr_porez.gui.main_window import MainWindow


def run() -> int:
    prepare_gui_process_identity()
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
