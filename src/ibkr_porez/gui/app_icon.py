from __future__ import annotations

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import (
    QColor,
    QIcon,
    QLinearGradient,
    QPainter,
    QPainterPath,
    QPen,
    QPixmap,
)

_ICON_SIZES = (16, 24, 32, 48, 64, 128, 256)


def create_app_icon() -> QIcon:
    icon = QIcon()
    for size in _ICON_SIZES:
        icon.addPixmap(_draw_icon_pixmap(size))
    return icon


def _draw_icon_pixmap(size: int) -> QPixmap:
    canvas = QPixmap(size, size)
    canvas.fill(Qt.GlobalColor.transparent)

    p = QPainter(canvas)
    p.setRenderHint(QPainter.RenderHint.Antialiasing, True)

    pad = max(1.0, size * 0.06)
    _draw_background(p, size, pad)
    panel = _panel_rect(size, pad)
    p.fillRect(panel, QColor(255, 255, 255, 30))
    _draw_chart_axes(p, panel, size)
    _draw_candles_and_trend(p, panel, size)
    _draw_document_badge(p, size, pad)

    p.end()
    return canvas


def _draw_background(painter: QPainter, size: int, pad: float) -> None:
    radius = size * 0.2
    base_rect = _rounded_rect_path(pad, pad, size - 2 * pad, size - 2 * pad, radius)
    gradient = QLinearGradient(pad, pad, size - pad, size - pad)
    gradient.setColorAt(0.0, QColor("#0d2238"))
    gradient.setColorAt(1.0, QColor("#113658"))
    painter.fillPath(base_rect, gradient)


def _panel_rect(size: int, pad: float) -> QRectF:
    return QRectF(
        pad + size * 0.11,
        pad + size * 0.2,
        size * 0.78,
        size * 0.63,
    )


def _draw_chart_axes(painter: QPainter, panel: QRectF, size: int) -> None:
    grid_pen = QPen(QColor(255, 255, 255, 40), max(1.0, size * 0.008))
    painter.setPen(grid_pen)
    for row in range(1, 4):
        y = panel.top() + panel.height() * (row / 4)
        painter.drawLine(QPointF(panel.left(), y), QPointF(panel.right(), y))

    axis_pen = QPen(QColor(255, 255, 255, 110), max(1.1, size * 0.012))
    painter.setPen(axis_pen)
    painter.drawLine(
        QPointF(panel.left(), panel.bottom()),
        QPointF(panel.right(), panel.bottom()),
    )
    painter.drawLine(
        QPointF(panel.left(), panel.top()),
        QPointF(panel.left(), panel.bottom()),
    )


def _to_y(panel: QRectF, value: float) -> float:
    return panel.bottom() - panel.height() * (0.1 + value * 0.82)


def _draw_candles_and_trend(painter: QPainter, panel: QRectF, size: int) -> None:
    candles = [
        (0.22, 0.58, 0.33, 0.5),
        (0.31, 0.72, 0.61, 0.44),
        (0.4, 0.86, 0.52, 0.76),
        (0.49, 0.92, 0.67, 0.84),
    ]
    bar_width = panel.width() * 0.1
    wick_pen = QPen(QColor("#e6edf7"), max(1.0, size * 0.008))

    for index, (low, high, open_v, close_v) in enumerate(candles):
        x = panel.left() + panel.width() * (0.2 + index * 0.18)
        painter.setPen(wick_pen)
        painter.drawLine(
            QPointF(x, _to_y(panel, high)),
            QPointF(x, _to_y(panel, low)),
        )

        body_top = min(_to_y(panel, open_v), _to_y(panel, close_v))
        body_height = max(size * 0.03, abs(_to_y(panel, open_v) - _to_y(panel, close_v)))
        body_rect = QRectF(x - bar_width / 2, body_top, bar_width, body_height)
        body_color = QColor("#2ecc71") if close_v >= open_v else QColor("#e74c3c")
        painter.fillRect(body_rect, body_color)

    trend_pen = QPen(QColor("#8dd7ff"), max(1.2, size * 0.015))
    trend_pen.setCapStyle(Qt.PenCapStyle.RoundCap)
    trend_pen.setJoinStyle(Qt.PenJoinStyle.RoundJoin)
    painter.setPen(trend_pen)
    trend = QPainterPath()
    trend.moveTo(panel.left() + panel.width() * 0.16, _to_y(panel, 0.28))
    trend.lineTo(panel.left() + panel.width() * 0.34, _to_y(panel, 0.43))
    trend.lineTo(panel.left() + panel.width() * 0.52, _to_y(panel, 0.39))
    trend.lineTo(panel.left() + panel.width() * 0.71, _to_y(panel, 0.62))
    trend.lineTo(panel.left() + panel.width() * 0.85, _to_y(panel, 0.76))
    painter.drawPath(trend)


def _draw_document_badge(painter: QPainter, size: int, pad: float) -> None:
    doc_rect = QRectF(
        pad + size * 0.61,
        pad + size * 0.06,
        size * 0.25,
        size * 0.28,
    )
    doc_path = _rounded_rect_path(
        doc_rect.left(),
        doc_rect.top(),
        doc_rect.width(),
        doc_rect.height(),
        size * 0.04,
    )
    painter.fillPath(doc_path, QColor("#f8fbff"))

    fold = QPainterPath()
    fold.moveTo(doc_rect.right() - doc_rect.width() * 0.3, doc_rect.top())
    fold.lineTo(doc_rect.right(), doc_rect.top())
    fold.lineTo(doc_rect.right(), doc_rect.top() + doc_rect.height() * 0.3)
    fold.closeSubpath()
    painter.fillPath(fold, QColor("#dce6f5"))

    check_pen = QPen(QColor("#1f8f5f"), max(1.2, size * 0.014))
    check_pen.setCapStyle(Qt.PenCapStyle.RoundCap)
    check_pen.setJoinStyle(Qt.PenJoinStyle.RoundJoin)
    painter.setPen(check_pen)
    check_left = doc_rect.left() + doc_rect.width() * 0.24
    check_mid_x = doc_rect.left() + doc_rect.width() * 0.42
    check_right = doc_rect.left() + doc_rect.width() * 0.74
    check_top = doc_rect.top() + doc_rect.height() * 0.36
    check_mid_y = doc_rect.top() + doc_rect.height() * 0.74
    check_low = doc_rect.top() + doc_rect.height() * 0.6
    painter.drawLine(QPointF(check_left, check_low), QPointF(check_mid_x, check_mid_y))
    painter.drawLine(QPointF(check_mid_x, check_mid_y), QPointF(check_right, check_top))


def _rounded_rect_path(x: float, y: float, w: float, h: float, r: float) -> QPainterPath:
    path = QPainterPath()
    path.addRoundedRect(x, y, w, h, r, r)
    return path
