"""Logging configuration for ibkr-porez."""

import logging
from logging.handlers import TimedRotatingFileHandler
from pathlib import Path

from platformdirs import user_data_dir


def setup_logger() -> logging.Logger:
    """
    Setup file logger for error logging with rotation.

    Uses TimedRotatingFileHandler with daily rotation.
    Keeps last 90 days (approximately 3 months) of logs.
    Standard Python logging mechanism automatically deletes old files.

    Returns:
        Configured logger instance
    """
    log_dir = Path(user_data_dir("ibkr-porez"))
    log_dir.mkdir(parents=True, exist_ok=True)
    error_log_file = log_dir / "error.log"

    file_handler = TimedRotatingFileHandler(
        filename=str(error_log_file),
        when="midnight",
        interval=1,  # Rotate daily
        backupCount=90,  # Keep 90 days (approximately 3 months)
        encoding="utf-8",
        delay=False,
    )
    file_handler.setLevel(logging.ERROR)
    file_handler.setFormatter(
        logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s"),
    )

    logger = logging.getLogger("ibkr_porez")
    logger.addHandler(file_handler)
    logger.setLevel(logging.ERROR)

    return logger


def get_error_log_path() -> Path:
    """
    Get path to error log file.

    Returns:
        Path to error.log file
    """
    log_dir = Path(user_data_dir("ibkr-porez"))
    return log_dir / "error.log"
