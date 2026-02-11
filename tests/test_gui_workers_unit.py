from __future__ import annotations

from pathlib import Path

import ibkr_porez.gui.export_worker as export_worker_module
import ibkr_porez.gui.import_worker as import_worker_module
import ibkr_porez.gui.sync_worker as sync_worker_module
from ibkr_porez.models import UserConfig
from ibkr_porez.operation_import import ImportType


def test_sync_worker_emits_error_when_ibkr_config_missing(monkeypatch) -> None:
    worker = sync_worker_module.SyncWorker()
    failed_messages: list[str] = []
    finished_calls: list[tuple[int, str]] = []
    worker.failed.connect(failed_messages.append)
    worker.finished.connect(lambda count, folder: finished_calls.append((count, folder)))

    monkeypatch.setattr(
        sync_worker_module.config_manager,
        "load_config",
        lambda: UserConfig(full_name="Test", address="Address"),
    )

    worker.run()

    assert finished_calls == []
    assert failed_messages == [
        "Missing IBKR configuration. Open Config and set Flex Token and Flex Query ID.",
    ]


def test_sync_worker_emits_finished_with_output_folder(monkeypatch) -> None:
    expected_output_folder = str(Path("/tmp/gui-output"))

    class FakeSyncOperation:
        def __init__(self, cfg: UserConfig) -> None:
            self.cfg = cfg

        @staticmethod
        def execute() -> list[str]:
            return ["decl-1", "decl-2"]

        @staticmethod
        def get_output_folder() -> Path:
            return Path(expected_output_folder)

    worker = sync_worker_module.SyncWorker()
    finished_calls: list[tuple[int, str]] = []
    failed_messages: list[str] = []
    worker.finished.connect(lambda count, folder: finished_calls.append((count, folder)))
    worker.failed.connect(failed_messages.append)

    monkeypatch.setattr(
        sync_worker_module.config_manager,
        "load_config",
        lambda: UserConfig(
            full_name="Test",
            address="Address",
            ibkr_token="token",
            ibkr_query_id="query-id",
        ),
    )
    monkeypatch.setattr(sync_worker_module, "SyncOperation", FakeSyncOperation)

    worker.run()

    assert failed_messages == []
    assert finished_calls == [(2, expected_output_folder)]


def test_sync_worker_emits_finished_with_empty_folder_when_nothing_created(monkeypatch) -> None:
    class FakeSyncOperation:
        def __init__(self, cfg: UserConfig) -> None:
            self.cfg = cfg

        @staticmethod
        def execute() -> list[str]:
            return []

        @staticmethod
        def get_output_folder() -> Path:
            return Path("/tmp/should-not-be-used")

    worker = sync_worker_module.SyncWorker()
    finished_calls: list[tuple[int, str]] = []
    failed_messages: list[str] = []
    worker.finished.connect(lambda count, folder: finished_calls.append((count, folder)))
    worker.failed.connect(failed_messages.append)

    monkeypatch.setattr(
        sync_worker_module.config_manager,
        "load_config",
        lambda: UserConfig(
            full_name="Test",
            address="Address",
            ibkr_token="token",
            ibkr_query_id="query-id",
        ),
    )
    monkeypatch.setattr(sync_worker_module, "SyncOperation", FakeSyncOperation)

    worker.run()

    assert failed_messages == []
    assert finished_calls == [(0, "")]


def test_sync_worker_emits_friendly_error_on_exception(monkeypatch) -> None:
    class FakeSyncOperation:
        def __init__(self, cfg: UserConfig) -> None:
            self.cfg = cfg

        @staticmethod
        def execute() -> list[str]:
            raise RuntimeError("sync failed")

    worker = sync_worker_module.SyncWorker()
    failed_messages: list[str] = []
    worker.failed.connect(failed_messages.append)

    monkeypatch.setattr(
        sync_worker_module.config_manager,
        "load_config",
        lambda: UserConfig(
            full_name="Test",
            address="Address",
            ibkr_token="token",
            ibkr_query_id="query-id",
        ),
    )
    monkeypatch.setattr(sync_worker_module, "SyncOperation", FakeSyncOperation)
    monkeypatch.setattr(
        sync_worker_module,
        "get_user_friendly_error_message",
        lambda _error: "Friendly sync error",
    )

    worker.run()

    assert failed_messages == ["Friendly sync error"]


def test_import_worker_emits_finished_on_success(monkeypatch, tmp_path: Path) -> None:
    expected_file = tmp_path / "input.csv"
    expected_file.write_text("id,date\n", encoding="utf-8")

    class FakeImportOperation:
        def execute(self, file_path: Path, import_type: ImportType):
            assert file_path == expected_file
            assert import_type == ImportType.CSV
            return ["t1", "t2", "t3"], 2, 1

    finished_calls: list[tuple[int, int, int]] = []
    failed_messages: list[str] = []
    worker = import_worker_module.ImportWorker(expected_file, ImportType.CSV)
    worker.finished.connect(
        lambda total, inserted, updated: finished_calls.append((total, inserted, updated))
    )
    worker.failed.connect(failed_messages.append)

    monkeypatch.setattr(import_worker_module, "ImportOperation", FakeImportOperation)

    worker.run()

    assert failed_messages == []
    assert finished_calls == [(3, 2, 1)]


def test_import_worker_emits_friendly_error_on_exception(monkeypatch, tmp_path: Path) -> None:
    expected_file = tmp_path / "input.csv"
    expected_file.write_text("id,date\n", encoding="utf-8")

    class FakeImportOperation:
        @staticmethod
        def execute(_file_path: Path, _import_type: ImportType):
            raise ValueError("bad file")

    failed_messages: list[str] = []
    worker = import_worker_module.ImportWorker(expected_file, ImportType.CSV)
    worker.failed.connect(failed_messages.append)

    monkeypatch.setattr(import_worker_module, "ImportOperation", FakeImportOperation)
    monkeypatch.setattr(
        import_worker_module,
        "get_user_friendly_error_message",
        lambda _error: "Friendly import error",
    )

    worker.run()

    assert failed_messages == ["Friendly import error"]


def test_export_worker_emits_finished_with_parent_directory(monkeypatch) -> None:
    expected_parent_dir = str(Path("/tmp/exported"))

    class FakeDeclarationManager:
        @staticmethod
        def export(declaration_id: str, output_dir):
            assert declaration_id == "decl-id"
            assert output_dir is None
            return Path(expected_parent_dir) / "decl-id.xml", []

    finished_paths: list[str] = []
    failed_messages: list[str] = []
    monkeypatch.setattr(export_worker_module, "DeclarationManager", FakeDeclarationManager)

    worker = export_worker_module.ExportWorker("decl-id")
    worker.finished.connect(finished_paths.append)
    worker.failed.connect(failed_messages.append)
    worker.run()

    assert failed_messages == []
    assert finished_paths == [expected_parent_dir]


def test_export_worker_emits_friendly_error_on_exception(monkeypatch) -> None:
    class FakeDeclarationManager:
        @staticmethod
        def export(_declaration_id: str, output_dir):
            assert output_dir is None
            raise RuntimeError("export failed")

    failed_messages: list[str] = []
    monkeypatch.setattr(export_worker_module, "DeclarationManager", FakeDeclarationManager)
    monkeypatch.setattr(
        export_worker_module,
        "get_user_friendly_error_message",
        lambda _error: "Friendly export error",
    )

    worker = export_worker_module.ExportWorker("decl-id")
    worker.failed.connect(failed_messages.append)
    worker.run()

    assert failed_messages == ["Friendly export error"]
