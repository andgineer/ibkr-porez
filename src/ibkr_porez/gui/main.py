from __future__ import annotations

import sys
import traceback

from PySide6.QtWidgets import QApplication, QMessageBox

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
    app_icon = create_app_icon()
    app.setWindowIcon(app_icon)
    window = MainWindow()
    window.setWindowIcon(app_icon)
    window.show()
    return app.exec()


def _show_startup_error_dialog(error: Exception) -> None:
    app = QApplication.instance()
    created_app = False
    if app is None:
        app = QApplication(["ibkr-porez"])
        app.setApplicationName("ibkr-porez")
        app.setApplicationDisplayName("ibkr-porez")
        created_app = True

    dialog = QMessageBox()
    dialog.setIcon(QMessageBox.Icon.Critical)
    dialog.setWindowTitle("ibkr-porez")
    dialog.setText("Failed to start GUI.")
    dialog.setInformativeText(str(error))
    dialog.setDetailedText("".join(traceback.format_exception(error)))
    dialog.exec()

    if created_app:
        app.quit()


def main() -> int:
    try:
        return run()
    except Exception as error:  # noqa: BLE001
        _show_startup_error_dialog(error)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
