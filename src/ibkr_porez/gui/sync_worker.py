from __future__ import annotations

from PySide6.QtCore import QObject, Signal, Slot

from ibkr_porez.config import config_manager
from ibkr_porez.error_handling import get_user_friendly_error_message
from ibkr_porez.operation_sync import SyncOperation


class SyncWorker(QObject):
    finished = Signal(int, str)
    failed = Signal(str)

    def __init__(self, forced: bool = False) -> None:
        super().__init__()
        self.forced = forced

    @Slot()
    def run(self) -> None:
        try:
            cfg = config_manager.load_config()
            forced_lookback_days = (
                SyncOperation.DEFAULT_FIRST_SYNC_LOOKBACK_DAYS if self.forced else None
            )
            operation = SyncOperation(cfg, forced_lookback_days=forced_lookback_days)
            created = operation.execute()
            output_folder = str(operation.get_output_folder()) if created else ""
            self.finished.emit(len(created), output_folder)
        except Exception as e:  # noqa: BLE001
            self.failed.emit(get_user_friendly_error_message(e))
