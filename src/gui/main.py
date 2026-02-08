from __future__ import annotations

from PySide6.QtWidgets import QApplication

from gui.main_window import MainWindow


def run() -> int:
    app = QApplication([])
    app.setStyle("Fusion")
    window = MainWindow()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(run())
