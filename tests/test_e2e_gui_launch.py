from __future__ import annotations

import allure
import pytest
from click.testing import CliRunner

import ibkr_porez.gui.launcher as gui_launcher
import ibkr_porez.main as main_module
from ibkr_porez.main import ibkr_porez


@allure.epic("End-to-end")
@allure.feature("gui")
def test_root_command_without_subcommand_starts_gui(monkeypatch):
    called = {"value": False}

    def fake_launch(*args, **kwargs) -> None:  # noqa: ARG001
        called["value"] = True

    monkeypatch.setattr("ibkr_porez.main.launch_gui_process", fake_launch)
    monkeypatch.setattr("ibkr_porez.main.sys.argv", ["ibkr-porez"])

    runner = CliRunner()
    result = runner.invoke(ibkr_porez, [])

    assert result.exit_code == 0
    assert called["value"] is True


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_shows_status_and_starts_gui_process(monkeypatch):
    status_messages: list[str] = []

    class DummyStatus:
        def __enter__(self):
            return self

        def __exit__(self, exc_type, exc, tb):  # noqa: ARG002
            return False

    class DummyConsole:
        def status(self, message: str):
            status_messages.append(message)
            return DummyStatus()

    class DummyProcess:
        pid = 12345

        def poll(self):
            return None

    monkeypatch.setattr(gui_launcher, "find_spec", lambda _name: object())
    monkeypatch.setattr(
        gui_launcher.subprocess,
        "Popen",
        lambda *args, **kwargs: DummyProcess(),
    )
    monotonic_state = {"value": 0.0}

    def fake_monotonic() -> float:
        monotonic_state["value"] += 0.5
        return monotonic_state["value"]

    monkeypatch.setattr(gui_launcher.time, "monotonic", fake_monotonic)
    monkeypatch.setattr(gui_launcher.time, "sleep", lambda _seconds: None)
    gui_launcher.launch_gui_process(console=DummyConsole(), app_version="0.0.0")

    assert status_messages == ["[bold green]Starting GUI...[/bold green]"]


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_raises_click_exception_on_missing_gui(monkeypatch):
    monkeypatch.setattr(gui_launcher, "find_spec", lambda _name: None)

    with pytest.raises(RuntimeError, match="GUI module is not available"):
        gui_launcher.launch_gui_process(console=main_module.console, app_version="0.0.0")


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_raises_when_child_exits_early(monkeypatch):
    class DummyProcess:
        pid = 777

        def poll(self):
            return 1

    monkeypatch.setattr(gui_launcher, "find_spec", lambda _name: object())
    monkeypatch.setattr(
        gui_launcher.subprocess,
        "Popen",
        lambda *args, **kwargs: DummyProcess(),
    )
    monotonic_state = {"value": 0.0}

    def fake_monotonic() -> float:
        monotonic_state["value"] += 0.1
        return monotonic_state["value"]

    monkeypatch.setattr(gui_launcher.time, "monotonic", fake_monotonic)
    monkeypatch.setattr(gui_launcher.time, "sleep", lambda _seconds: None)

    with pytest.raises(RuntimeError, match="exited immediately"):
        gui_launcher.launch_gui_process(console=main_module.console, app_version="0.0.0")


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_keeps_status_visible_for_short_launch(monkeypatch):
    sleep_calls: list[float] = []

    class DummyStatus:
        def __enter__(self):
            return self

        def __exit__(self, exc_type, exc, tb):  # noqa: ARG002
            return False

    class DummyConsole:
        def status(self, _message: str):
            return DummyStatus()

    class DummyProcess:
        pid = 2026

        def poll(self):
            return 0

    monotonic_values = iter([0.0, 0.1, 0.2, 0.3])
    monkeypatch.setattr(gui_launcher, "find_spec", lambda _name: object())
    monkeypatch.setattr(
        gui_launcher.subprocess,
        "Popen",
        lambda *args, **kwargs: DummyProcess(),
    )
    monkeypatch.setattr(gui_launcher.time, "monotonic", lambda: next(monotonic_values))
    monkeypatch.setattr(gui_launcher.time, "sleep", lambda seconds: sleep_calls.append(seconds))

    gui_launcher.launch_gui_process(console=DummyConsole(), app_version="0.0.0")

    assert sleep_calls[-1] == pytest.approx(0.5, abs=1e-9)
