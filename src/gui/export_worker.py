from __future__ import annotations

from PySide6.QtCore import QObject, Signal, Slot

from ibkr_porez.declaration_manager import DeclarationManager


class ExportWorker(QObject):
    finished = Signal(str)
    failed = Signal(str)

    def __init__(self, declaration_id: str) -> None:
        super().__init__()
        self.declaration_id = declaration_id
        self.declaration_manager = DeclarationManager()

    @Slot()
    def run(self) -> None:
        try:
            xml_path, _attached_paths = self.declaration_manager.export(
                self.declaration_id,
                output_dir=None,
            )
            self.finished.emit(str(xml_path.parent))
        except Exception as e:  # noqa: BLE001
            self.failed.emit(str(e))
