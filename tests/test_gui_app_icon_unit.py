from __future__ import annotations

import allure
import pytest

import ibkr_porez.gui.app_icon as app_icon_module


@allure.epic("GUI")
@allure.feature("App Icon")
class TestGuiAppIconUnit:
    def test_create_app_icon_uses_all_configured_sizes(
        self,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        rendered: list[int] = []

        class FakeIcon:
            def __init__(self) -> None:
                self.pixmaps: list[str] = []

            def addPixmap(self, pixmap: str) -> None:  # noqa: N802
                self.pixmaps.append(pixmap)

        monkeypatch.setattr(app_icon_module, "QIcon", FakeIcon)
        monkeypatch.setattr(
            app_icon_module,
            "_draw_icon_pixmap",
            lambda size: rendered.append(size) or f"pix-{size}",
        )

        icon = app_icon_module.create_app_icon()

        assert rendered == list(app_icon_module._ICON_SIZES)
        assert isinstance(icon, FakeIcon)
        assert icon.pixmaps == [f"pix-{size}" for size in app_icon_module._ICON_SIZES]

    def test_panel_rect_scales_with_icon_size(self) -> None:
        rect = app_icon_module._panel_rect(100, 6.0)

        assert rect.left() == pytest.approx(17.0)
        assert rect.top() == pytest.approx(26.0)
        assert rect.width() == pytest.approx(78.0)
        assert rect.height() == pytest.approx(63.0)

    def test_to_y_maps_higher_value_upwards(self) -> None:
        panel = app_icon_module._panel_rect(100, 6.0)

        y_low = app_icon_module._to_y(panel, 0.1)
        y_high = app_icon_module._to_y(panel, 0.9)

        assert y_high < y_low
