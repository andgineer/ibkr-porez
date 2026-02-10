from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QObject, Signal, Slot

from ibkr_porez.error_handling import get_user_friendly_error_message
from ibkr_porez.operation_import import ImportOperation, ImportType


class ImportWorker(QObject):
    finished = Signal(int, int, int)
    failed = Signal(str)

    def __init__(self, file_path: Path, import_type: ImportType) -> None:
        super().__init__()
        self.file_path = file_path
        self.import_type = import_type

    @Slot()
    def run(self) -> None:
        try:
            operation = ImportOperation()
            transactions, count_inserted, count_updated = operation.execute(
                self.file_path,
                self.import_type,
            )
            self.finished.emit(len(transactions), count_inserted, count_updated)
        except Exception as e:  # noqa: BLE001
            self.failed.emit(get_user_friendly_error_message(e))
