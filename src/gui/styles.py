from __future__ import annotations

APP_STYLESHEET = """
QWidget#appRoot {
    background: #F4F7FB;
}
QPushButton#syncButton {
    background-color: #0F766E;
    color: #FFFFFF;
    border: none;
    border-radius: 12px;
    font-size: 20px;
    font-weight: 700;
    padding: 8px 14px;
}
QPushButton#syncButton:hover {
    background-color: #0D9488;
}
QPushButton#configButton {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #CBD5E1;
    border-radius: 8px;
    padding: 8px 16px;
    font-weight: 600;
}
QComboBox#filterCombo {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #CBD5E1;
    border-radius: 8px;
    padding: 4px 26px 4px 8px;
    min-width: 130px;
}
QComboBox#filterCombo::drop-down {
    subcontrol-origin: padding;
    subcontrol-position: top right;
    width: 24px;
    border-left: 1px solid #CBD5E1;
    background: #FFFFFF;
    border-top-right-radius: 8px;
    border-bottom-right-radius: 8px;
}
QComboBox#filterCombo QAbstractItemView {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #CBD5E1;
    selection-background-color: #E3F2FD;
    selection-color: #0F172A;
    outline: 0;
}
QPushButton#controlButton {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #CBD5E1;
    border-radius: 8px;
    padding: 6px 12px;
    font-weight: 600;
}
QPushButton#controlButton:hover {
    background: #F8FAFC;
}
QPushButton#primaryControlButton {
    background-color: #0F766E;
    color: #FFFFFF;
    border: none;
    border-radius: 8px;
    padding: 8px 14px;
    font-weight: 700;
}
QPushButton#primaryControlButton:disabled {
    background-color: #94A3B8;
    color: #E2E8F0;
}
QComboBox#controlCombo {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #CBD5E1;
    border-radius: 8px;
    padding: 4px 26px 4px 8px;
    min-width: 140px;
}
QComboBox#controlCombo::drop-down {
    subcontrol-origin: padding;
    subcontrol-position: top right;
    width: 24px;
    border-left: 1px solid #CBD5E1;
    background: #FFFFFF;
    border-top-right-radius: 8px;
    border-bottom-right-radius: 8px;
}
QComboBox#controlCombo QAbstractItemView {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #CBD5E1;
    selection-background-color: #E3F2FD;
    selection-color: #0F172A;
    outline: 0;
}
QLabel#selectionInfoLabel {
    color: #475569;
    font-weight: 600;
    padding-left: 6px;
}
QLabel#progressLabel {
    color: #334155;
    font-weight: 600;
}
QProgressBar#progressBar {
    border: 1px solid #CBD5E1;
    border-radius: 7px;
    background: #E2E8F0;
    text-align: center;
    min-height: 16px;
}
QProgressBar#progressBar::chunk {
    background: #14B8A6;
    border-radius: 6px;
}
QTableWidget {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #D8E3F0;
    border-radius: 10px;
    gridline-color: #EDF2F7;
    alternate-background-color: #F8FBFF;
    selection-background-color: #E3F2FD;
    selection-color: #0F172A;
}
QHeaderView::section {
    background: #E8F0FA;
    color: #334155;
    border: none;
    border-bottom: 1px solid #D8E3F0;
    padding: 8px;
    font-weight: 700;
}
QToolButton {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #CBD5E1;
    border-radius: 6px;
    padding: 3px 8px;
}
QToolButton:hover {
    background: #F1F5F9;
}
QToolButton#statusActionButton:disabled {
    background: #E2E8F0;
    color: #94A3B8;
    border: 1px dashed #94A3B8;
}
QToolButton#statusActionButton:disabled:hover {
    background: #E2E8F0;
    color: #94A3B8;
    border: 1px dashed #94A3B8;
}
QToolButton#exportActionButton {
    background: #FFFFFF;
    color: #0F172A;
    border: 1px solid #94A3B8;
}
QToolButton#exportActionButton:hover {
    background: #EEF2FF;
}
QToolButton#exportActionButton:checked {
    background: #0F766E;
    color: #FFFFFF;
    border: 1px solid #0F766E;
    font-weight: 700;
}
QToolButton#exportActionButton:disabled {
    background: #E2E8F0;
    color: #64748B;
    border: 1px solid #CBD5E1;
}
"""
