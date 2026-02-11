from __future__ import annotations

import os
import sys
from pathlib import Path

import allure
import pytest
from PySide6.QtWidgets import QApplication, QLabel

from ibkr_porez.gui.import_dialog import IMPORT_DOCS_URL, IMPORT_GUIDANCE_TEXT, ImportDialog

pytestmark = pytest.mark.skipif(
    sys.platform != "linux",
    reason="Qt UI tests run in CI only on Linux",
)


@pytest.fixture(scope="module")
def qapp() -> QApplication:
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    app = QApplication.instance()
    if app is None:
        app = QApplication(["pytest"])
    return app


@allure.epic("GUI")
@allure.feature("Import Dialog")
class TestImportDialogUnit:
    def test_import_dialog_shows_guidance_on_open(self, qapp: QApplication) -> None:  # noqa: ARG002
        dialog = ImportDialog()
        try:
            assert dialog.result_label.text() == IMPORT_GUIDANCE_TEXT
        finally:
            dialog.close()

    def test_import_dialog_has_human_friendly_import_help_link(
        self,
        qapp: QApplication,  # noqa: ARG002
    ) -> None:
        dialog = ImportDialog()
        try:
            link_labels = [
                label for label in dialog.findChildren(QLabel) if IMPORT_DOCS_URL in label.text()
            ]
            assert len(link_labels) == 1
            assert (
                "How to export CSV in Interactive Brokers for this import" in link_labels[0].text()
            )
        finally:
            dialog.close()

    def test_import_dialog_start_import_requires_file_selection(
        self,
        qapp: QApplication,  # noqa: ARG002
    ) -> None:
        dialog = ImportDialog()
        try:
            dialog.file_path_input.setText("   ")
            dialog.start_import()
            assert dialog.result_label.text() == ""
            assert dialog.error_label.text() == "Select a file first."
        finally:
            dialog.close()

    def test_import_dialog_start_import_rejects_missing_file(
        self,
        qapp: QApplication,  # noqa: ARG002
        tmp_path: Path,
    ) -> None:
        missing_file = tmp_path / "missing.csv"
        dialog = ImportDialog()
        try:
            dialog.file_path_input.setText(str(missing_file))
            dialog.start_import()
            assert dialog.result_label.text() == ""
            assert dialog.error_label.text() == f"File not found: {missing_file}"
        finally:
            dialog.close()

    def test_import_dialog_start_import_rejects_directory(
        self,
        qapp: QApplication,  # noqa: ARG002
        tmp_path: Path,
    ) -> None:
        directory = tmp_path / "as-directory"
        directory.mkdir()
        dialog = ImportDialog()
        try:
            dialog.file_path_input.setText(str(directory))
            dialog.start_import()
            assert dialog.result_label.text() == ""
            assert dialog.error_label.text() == "Selected path is a directory, not a file."
        finally:
            dialog.close()

    def test_import_dialog_running_state_toggles_controls(
        self,
        qapp: QApplication,  # noqa: ARG002
    ) -> None:
        dialog = ImportDialog()
        try:
            dialog._set_running_state(True)
            assert not dialog.progress_bar.isHidden()
            assert not dialog.import_button.isEnabled()
            assert not dialog.close_button.isEnabled()
            assert not dialog.browse_button.isEnabled()
            assert not dialog.file_path_input.isEnabled()
            assert not dialog.import_type_combo.isEnabled()

            dialog._set_running_state(False)
            assert dialog.progress_bar.isHidden()
            assert dialog.import_button.isEnabled()
            assert dialog.close_button.isEnabled()
            assert dialog.browse_button.isEnabled()
            assert dialog.file_path_input.isEnabled()
            assert dialog.import_type_combo.isEnabled()
        finally:
            dialog.close()
