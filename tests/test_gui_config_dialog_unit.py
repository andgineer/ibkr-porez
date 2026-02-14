from __future__ import annotations

from pathlib import Path

import allure
import pytest
from PySide6.QtWidgets import QApplication, QLabel

import ibkr_porez.gui.config_dialog as config_dialog_module
from ibkr_porez.gui.config_dialog import FLEX_DOCS_URL, ConfigDialog
from ibkr_porez.models import UserConfig


@pytest.fixture(scope="module")
def qapp() -> QApplication:
    app = QApplication.instance()
    if app is None:
        app = QApplication(["pytest"])
    return app


def test_config_dialog_get_config_normalizes_and_applies_defaults(qapp: QApplication) -> None:  # noqa: ARG001
    base_config = UserConfig(
        full_name="Base Name",
        address="Base Address",
        ibkr_token="base-token",
        ibkr_query_id="base-query",
        personal_id="base-personal-id",
        city_code="101",
        phone="111",
        email="base@example.com",
        data_dir="/old/data",
        output_folder="/old/output",
    )

    dialog = ConfigDialog(base_config)
    try:
        dialog.ibkr_token.setText("  new-token  ")
        dialog.ibkr_query_id.setText(" new-query ")
        dialog.personal_id.setText(" 123456 ")
        dialog.full_name.setText(" New User ")
        dialog.address.setText(" New Address ")
        dialog.city_code.setText("   ")
        dialog.phone.setText("  ")
        dialog.email.setText("   ")
        dialog.data_dir.setText("  ")
        dialog.output_folder.setText(" /tmp/output ")

        config = dialog.get_config()
    finally:
        dialog.close()

    assert config.ibkr_token == "new-token"
    assert config.ibkr_query_id == "new-query"
    assert config.personal_id == "123456"
    assert config.full_name == "New User"
    assert config.address == "New Address"
    assert config.city_code == "223"
    assert config.phone == "0600000000"
    assert config.email == "email@example.com"
    assert config.data_dir is None
    assert config.output_folder == "/tmp/output"


def test_config_dialog_choose_directory_updates_field_when_selected(
    qapp: QApplication,  # noqa: ARG001
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    dialog = ConfigDialog(UserConfig(full_name="User", address="Address"))
    chosen_dir = str(tmp_path / "selected-dir")

    monkeypatch.setattr(
        config_dialog_module.QFileDialog,
        "getExistingDirectory",
        lambda *_args, **_kwargs: chosen_dir,
    )

    try:
        dialog.data_dir.setText("")
        dialog._choose_directory(dialog.data_dir, "Select Data Directory")
        assert dialog.data_dir.text() == chosen_dir

        dialog.output_folder.setText("/keep/me")
        monkeypatch.setattr(
            config_dialog_module.QFileDialog,
            "getExistingDirectory",
            lambda *_args, **_kwargs: "",
        )
        dialog._choose_directory(dialog.output_folder, "Select Output Folder")
        assert dialog.output_folder.text() == "/keep/me"
    finally:
        dialog.close()


def test_config_dialog_has_human_friendly_flex_help_link(qapp: QApplication) -> None:  # noqa: ARG001
    dialog = ConfigDialog(UserConfig(full_name="User", address="Address"))
    try:
        link_labels = [
            label for label in dialog.findChildren(QLabel) if FLEX_DOCS_URL in label.text()
        ]
        assert len(link_labels) == 1
        assert "How to get Flex Token and Flex Query ID in IBKR" in link_labels[0].text()
    finally:
        dialog.close()


def test_config_dialog_open_data_dir_uses_field_value(
    qapp: QApplication,  # noqa: ARG001
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    dialog = ConfigDialog(UserConfig(full_name="User", address="Address"))
    opened_paths: list[str] = []
    custom_dir = tmp_path / "custom-data-dir"

    monkeypatch.setattr(
        config_dialog_module.QDesktopServices,
        "openUrl",
        lambda url: opened_paths.append(url.toLocalFile()) or True,
    )

    try:
        dialog.data_dir.setText(str(custom_dir))
        dialog._open_data_dir()
        assert custom_dir.exists()
        assert opened_paths == [str(custom_dir)]
    finally:
        dialog.close()


def test_config_dialog_open_output_dir_uses_default_when_field_empty(
    qapp: QApplication,  # noqa: ARG001
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    dialog = ConfigDialog(UserConfig(full_name="User", address="Address"))
    opened_paths: list[str] = []
    default_output = tmp_path / "default-output"

    monkeypatch.setattr(
        config_dialog_module,
        "get_default_output_dir_path",
        lambda: default_output,
    )
    monkeypatch.setattr(
        config_dialog_module.QDesktopServices,
        "openUrl",
        lambda url: opened_paths.append(url.toLocalFile()) or True,
    )

    try:
        dialog.output_folder.setText("")
        dialog._open_output_folder()
        assert default_output.exists()
        assert opened_paths == [str(default_output)]
    finally:
        dialog.close()


def test_config_dialog_open_directory_shows_warning_when_open_fails(
    qapp: QApplication,  # noqa: ARG001
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    dialog = ConfigDialog(UserConfig(full_name="User", address="Address"))
    warnings: list[tuple[str, str]] = []
    target_dir = tmp_path / "cannot-open-dir"

    monkeypatch.setattr(
        config_dialog_module.QDesktopServices,
        "openUrl",
        lambda _url: False,
    )
    monkeypatch.setattr(
        config_dialog_module.QMessageBox,
        "warning",
        lambda _parent, title, text: warnings.append((title, text)),
    )

    try:
        dialog._open_directory(str(target_dir), tmp_path / "unused")
        assert target_dir.exists()
        assert warnings == [("Open folder failed", f"Cannot open folder: {target_dir}")]
    finally:
        dialog.close()


def _apply_allure_labels() -> None:
    labels = (allure.epic("GUI"), allure.feature("Config Dialog"))
    for name, value in list(globals().items()):
        if name.startswith("test_") and callable(value):
            decorated = value
            for label in labels:
                decorated = label(decorated)
            globals()[name] = decorated


_apply_allure_labels()
