from __future__ import annotations

import allure
import pytest
from click.testing import CliRunner

import ibkr_porez.gui_launcher as gui_launcher
import ibkr_porez.main as main_module
from ibkr_porez.main import ibkr_porez


@allure.epic("End-to-end")
@allure.feature("gui")
def test_root_command_without_subcommand_starts_gui(monkeypatch):
    called = {"value": False}

    def fake_launch() -> None:
        called["value"] = True

    monkeypatch.setattr("ibkr_porez.main._launch_gui_process", fake_launch)
    monkeypatch.setattr("ibkr_porez.main.sys.argv", ["ibkr-porez"])

    runner = CliRunner()
    result = runner.invoke(ibkr_porez, [])

    assert result.exit_code == 0
    assert called["value"] is True


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_shows_status_and_calls_gui_launcher(monkeypatch):
    status_messages: list[str] = []
    called = {"value": False}

    class DummyStatus:
        def __enter__(self):
            return self

        def __exit__(self, exc_type, exc, tb):  # noqa: ARG002
            return False

    class DummyConsole:
        def status(self, message: str):
            status_messages.append(message)
            return DummyStatus()

    def fake_launch() -> int:
        called["value"] = True
        return 12345

    monkeypatch.setattr(main_module, "console", DummyConsole())
    monkeypatch.setattr(main_module, "launch_gui_process", fake_launch)
    main_module._launch_gui_process()

    assert called["value"] is True
    assert status_messages == ["[bold green]Starting GUI...[/bold green]"]


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_raises_click_exception_on_error(monkeypatch):
    def fake_launch() -> int:
        raise RuntimeError("boom")

    monkeypatch.setattr(main_module, "launch_gui_process", fake_launch)

    with pytest.raises(main_module.click.ClickException, match="boom"):
        main_module._launch_gui_process()


@allure.epic("End-to-end")
@allure.feature("gui")
def test_gui_launcher_raises_when_pyside6_missing(monkeypatch):
    def fake_find_spec(module_name: str):
        if module_name == "gui.main":
            return object()
        if module_name == "PySide6":
            return None
        return object()

    monkeypatch.setattr(gui_launcher, "find_spec", fake_find_spec)

    with pytest.raises(RuntimeError, match="PySide6 is not installed"):
        gui_launcher.launch_gui_process()


@allure.epic("End-to-end")
@allure.feature("gui")
def test_gui_launcher_raises_when_child_exits_early(monkeypatch):
    class DummyProcess:
        pid = 777

        def poll(self):
            return 1

    def fake_find_spec(module_name: str):
        if module_name in {"gui.main", "PySide6"}:
            return object()
        return None

    monkeypatch.setattr(gui_launcher, "find_spec", fake_find_spec)
    monkeypatch.setattr(gui_launcher.subprocess, "Popen", lambda *args, **kwargs: DummyProcess())

    with pytest.raises(RuntimeError, match="exited immediately"):
        gui_launcher.launch_gui_process()
