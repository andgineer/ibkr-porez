"""Automated PPDG-3R tax reports for Interactive Brokers.

It automatically pulls your data and generates a ready-to-upload XML file
with all prices converted to RSD.

The file is mandatory for build system to find the package.
"""

from ibkr_porez.__about__ import __version__

__all__ = ["__version__"]
