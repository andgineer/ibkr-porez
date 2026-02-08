from __future__ import annotations

import allure
import pytest
from click.testing import CliRunner

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

    def fake_find_spec(module_name: str):
        if module_name == "gui.main":
            return object()
        return None

    monkeypatch.setattr(main_module, "console", DummyConsole())
    monkeypatch.setattr(main_module, "find_spec", fake_find_spec)
    monkeypatch.setattr(
        main_module.subprocess,
        "Popen",
        lambda *args, **kwargs: DummyProcess(),
    )
    monotonic_values = iter([0.0, 2.0])
    monkeypatch.setattr(main_module.time, "monotonic", lambda: next(monotonic_values))
    monkeypatch.setattr(main_module.time, "sleep", lambda _seconds: None)
    main_module._launch_gui_process()

    assert status_messages == ["[bold green]Starting GUI...[/bold green]"]


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_raises_click_exception_on_missing_gui(monkeypatch):
    monkeypatch.setattr(main_module, "find_spec", lambda _name: None)

    with pytest.raises(main_module.click.ClickException, match="GUI module is not available"):
        main_module._launch_gui_process()


@allure.epic("End-to-end")
@allure.feature("gui")
def test_main_launcher_raises_when_child_exits_early(monkeypatch):
    class DummyProcess:
        pid = 777

        def poll(self):
            return 1

    def fake_find_spec(module_name: str):
        if module_name == "gui.main":
            return object()
        return None

    monkeypatch.setattr(main_module, "find_spec", fake_find_spec)
    monkeypatch.setattr(
        main_module.subprocess,
        "Popen",
        lambda *args, **kwargs: DummyProcess(),
    )
    monotonic_values = iter([0.0, 0.1])
    monkeypatch.setattr(main_module.time, "monotonic", lambda: next(monotonic_values))
    monkeypatch.setattr(main_module.time, "sleep", lambda _seconds: None)

    with pytest.raises(main_module.click.ClickException, match="exited immediately"):
        main_module._launch_gui_process()
