from __future__ import annotations

import allure

import ibkr_porez.gui.main as gui_main_module


@allure.epic("GUI")
@allure.feature("Startup")
class TestGuiMainUnit:
    def test_run_configures_application_and_returns_exec_code(self, monkeypatch) -> None:
        calls: dict[str, object] = {}

        class FakeApplication:
            def __init__(self, argv):
                calls["argv"] = argv
                calls["app"] = self
                self.icon = None

            def setApplicationName(self, value: str) -> None:  # noqa: N802
                calls["app_name"] = value

            def setApplicationDisplayName(self, value: str) -> None:  # noqa: N802
                calls["app_display_name"] = value

            def setDesktopFileName(self, value: str) -> None:  # noqa: N802
                calls["desktop_file_name"] = value

            def setStyle(self, value: str) -> None:  # noqa: N802
                calls["style"] = value

            def setWindowIcon(self, icon) -> None:  # noqa: N802
                calls["app_icon"] = icon
                self.icon = icon

            def exec(self) -> int:
                return 321

        class FakeWindow:
            def __init__(self) -> None:
                calls["window"] = self
                self.icon = None
                self.shown = False

            def setWindowIcon(self, icon) -> None:  # noqa: N802
                calls["window_icon"] = icon
                self.icon = icon

            def show(self) -> None:
                calls["window_show_called"] = True
                self.shown = True

        identity_called = {"value": False}

        monkeypatch.setattr(gui_main_module, "QApplication", FakeApplication)
        monkeypatch.setattr(gui_main_module, "MainWindow", FakeWindow)
        monkeypatch.setattr(gui_main_module, "create_app_icon", lambda: "ICON")
        monkeypatch.setattr(
            gui_main_module,
            "prepare_gui_process_identity",
            lambda: identity_called.__setitem__("value", True),
        )
        monkeypatch.setattr(gui_main_module.sys, "argv", ["prog", "--verbose"])

        result = gui_main_module.run()

        assert result == 321
        assert identity_called["value"] is True
        assert calls["argv"] == ["ibkr-porez", "--verbose"]
        assert calls["app_name"] == "ibkr-porez"
        assert calls["app_display_name"] == "ibkr-porez"
        assert calls["desktop_file_name"] == "ibkr-porez"
        assert calls["style"] == "Fusion"
        assert calls["app_icon"] == "ICON"
        assert calls["window_icon"] == "ICON"
        assert calls["window_show_called"] is True

    def test_main_shows_dialog_and_returns_non_zero_on_startup_error(
        self,
        monkeypatch,
    ) -> None:
        captured: dict[str, object] = {}

        def fake_run() -> int:
            raise RuntimeError("boom")

        def fake_show(error: Exception) -> None:
            captured["error"] = error

        monkeypatch.setattr(gui_main_module, "run", fake_run)
        monkeypatch.setattr(gui_main_module, "_show_startup_error_dialog", fake_show)

        result = gui_main_module.main()

        assert result == 1
        assert isinstance(captured["error"], RuntimeError)
        assert str(captured["error"]) == "boom"
