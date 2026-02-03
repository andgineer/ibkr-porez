"""Base class for report generators."""

from abc import ABC, abstractmethod
from collections.abc import Generator
from datetime import date
from typing import Any

from ibkr_porez.config import config_manager
from ibkr_porez.nbs import NBSClient
from ibkr_porez.storage import Storage


class ReportGeneratorBase(ABC):
    """Base class for report generators."""

    def __init__(self):
        """Initialize common dependencies."""
        self.cfg = config_manager.load_config()
        self.storage = Storage()
        self.nbs = NBSClient(self.storage)

    @abstractmethod
    def filename(self, *args: Any, **kwargs: Any) -> str:  # type: ignore[override]
        """
        Generate filename for the report.

        Args:
            *args: Positional arguments (type-specific).
            **kwargs: Keyword arguments (type-specific).

        Returns:
            Filename string.
        """
        raise NotImplementedError

    @abstractmethod
    def generate(
        self,
        start_date: date,
        end_date: date,
        force: bool = False,
    ) -> Generator[tuple[str, str, list[Any]], None, None]:
        """
        Generate report declarations.

        Args:
            start_date: Start date for the report period.
            end_date: End date for the report period.
            force: Force generation even if validation fails (for income reports).

        Yields:
            tuple[str, str, list]: (filename, xml_content, entries) tuples.
        """
        raise NotImplementedError
