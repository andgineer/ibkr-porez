from __future__ import annotations

from PySide6.QtCore import QObject, Signal, Slot

from ibkr_porez.config import config_manager
from ibkr_porez.error_handling import get_user_friendly_error_message
from ibkr_porez.operation_sync import SyncOperation


class SyncWorker(QObject):
    finished = Signal(int, str)
    failed = Signal(str)

    @Slot()
    def run(self) -> None:
        try:
            cfg = config_manager.load_config()
            if not cfg.ibkr_token or not cfg.ibkr_query_id:
                self.failed.emit(
                    "Missing IBKR configuration. Open Config and set Flex Token and Flex Query ID.",
                )
                return
            operation = SyncOperation(cfg)
            created = operation.execute()
            output_folder = str(operation.get_output_folder()) if created else ""
            self.finished.emit(len(created), output_folder)
        except Exception as e:  # noqa: BLE001
            self.failed.emit(get_user_friendly_error_message(e))
