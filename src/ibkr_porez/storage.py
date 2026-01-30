import json
from datetime import date
from decimal import Decimal
from pathlib import Path

import pandas as pd
from platformdirs import user_data_dir

from ibkr_porez.models import Currency, ExchangeRate, Transaction


class Storage:
    APP_NAME = "ibkr-porez"
    RATES_FILENAME = "rates.json"
    RAW_DIR = "raw_reports"
    PARTITION_DIR = "partitions"

    def __init__(self):
        self._data_dir = Path(user_data_dir(self.APP_NAME))
        self._partition_dir = self._data_dir / self.PARTITION_DIR
        self._rates_file = self._data_dir / self.RATES_FILENAME
        self._raw_dir = self._data_dir / self.RAW_DIR
        self._ensure_dirs()

    def _ensure_dirs(self):
        self._data_dir.mkdir(parents=True, exist_ok=True)
        self._partition_dir.mkdir(parents=True, exist_ok=True)
        self._raw_dir.mkdir(parents=True, exist_ok=True)

    def save_raw_report(self, content: str | bytes, filename: str):
        path = self._raw_dir / filename
        mode = "wb" if isinstance(content, bytes) else "w"
        with open(path, mode) as f:
            f.write(content)

    def save_transactions(self, transactions: list[Transaction]) -> int:
        if not transactions:
            return 0

        # Convert to DataFrame
        data = [t.model_dump(mode="json") for t in transactions]
        df_new = pd.DataFrame(data)

        # Ensure date columns are datetime
        df_new["date"] = pd.to_datetime(df_new["date"]).dt.date

        # Calculate partition key (Year-H1/H2)
        df_new["_year"] = pd.to_datetime(df_new["date"]).dt.year
        df_new["_half"] = pd.to_datetime(df_new["date"]).dt.month.apply(
            lambda x: 1 if x <= 6 else 2,  # noqa: PLR2004
        )

        total_new_saved = 0
        # Group by partition to save
        for (year, half), group_df in df_new.groupby(["_year", "_half"]):
            saved_count = self._save_partition(int(year), int(half), group_df)
            total_new_saved += saved_count

        return total_new_saved

    def _save_partition(self, year: int, half: int, new_df: pd.DataFrame) -> int:
        file_path = self._partition_dir / f"transactions_{year}_H{half}.json"

        combined_df = new_df
        new_items_count = len(new_df)

        if file_path.exists():
            try:
                # Load existing
                existing_df = pd.read_json(file_path, orient="records")

                # Check duplicates based on transaction_id
                existing_ids = set(existing_df["transaction_id"])
                current_new_ids = set(new_df["transaction_id"])

                # Calculate how many are ACTUALLY new (not present before)
                # (Note: we still Upsert/Overwrite if present, but for User Report we care
                # about *new* IDs usually)
                # Or maybe "Modified or New"?
                # Let's report "New" as "Not in existing".
                truly_new = current_new_ids - existing_ids
                new_items_count = len(truly_new)

                # Upsert Logic:
                # Filter out existing IDs that are in new_df (to be replaced)
                existing_df = existing_df[~existing_df["transaction_id"].isin(current_new_ids)]
                combined_df = pd.concat([existing_df, new_df], ignore_index=True)
            except ValueError:
                # Malformed file? Overwrite with new.
                pass

        # Save
        cols_to_save = [c for c in combined_df.columns if not c.startswith("_")]
        combined_df[cols_to_save].to_json(file_path, orient="records", date_format="iso", indent=4)

        return new_items_count

    def get_transactions(  # noqa: C901,PLR0912
        self,
        start_date: date | None = None,
        end_date: date | None = None,
    ) -> pd.DataFrame:
        """Load transactions into a DataFrame."""

        # 1. Identify partitions to load
        # If no dates, load ALL.
        # If range, calculate years pairs.

        files_to_load = []
        if start_date is None and end_date is None:
            files_to_load = list(self._partition_dir.glob("transactions_*.json"))
        else:
            # Smart loading based on range
            # Range might span multiple years/halves
            # Simplest logic: Load partitions that *might* overlap.
            # But "transactions_YYYY_HX.json"
            # Let's just iterate all files and filter by name if we want optimization,
            # Or simpler: load all and filter in memory (given 20k rows total).
            # For 20k rows, full scan is fastest to implement vs regex parsing filename.
            # But user asked for "transparent sharding... load needed halfyears".

            # Let's parse filenames to filter.
            all_files = list(self._partition_dir.glob("transactions_*_H*.json"))
            files_to_load = []

            s_year = start_date.year if start_date else 0
            e_year = end_date.year if end_date else 9999

            for f in all_files:
                # transactions_2023_H1.json
                try:
                    parts = f.stem.split("_")  # ['transactions', '2023', 'H1']
                    f_year = int(parts[1])
                    if s_year <= f_year <= e_year:
                        files_to_load.append(f)
                except (IndexError, ValueError):
                    continue

        if not files_to_load:
            return pd.DataFrame()  # Empty

        dfs = []
        for f in files_to_load:
            try:
                df = pd.read_json(f, orient="records")
                dfs.append(df)
            except ValueError:
                continue

        if not dfs:
            return pd.DataFrame()

        full_df = pd.concat(dfs, ignore_index=True)

        # Convert date column back to date object (or timestamp) for filtering
        if "date" in full_df.columns:
            full_df["date"] = pd.to_datetime(full_df["date"]).dt.date

            if start_date:
                full_df = full_df[full_df["date"] >= start_date]
            if end_date:
                full_df = full_df[full_df["date"] <= end_date]

        # Convert open_date to date object if present
        if "open_date" in full_df.columns:
            # Errors='coerce' handles NaT/None safely
            full_df["open_date"] = pd.to_datetime(full_df["open_date"], errors="coerce").dt.date

        return full_df

    def get_last_transaction_date(self) -> date | None:
        """Find the date of the latest transaction across all partitions."""
        all_files = list(self._partition_dir.glob("transactions_*.json"))
        if not all_files:
            return None

        max_date = None

        # Optimization: Check file names first?
        # filenames usually transactions_YYYY_HX.json
        # We can sort files by year/half descending, then check content.

        def parse_file_key(f):
            try:
                parts = f.stem.split("_")
                return (int(parts[1]), int(parts[2][1:]))  # (Year, Half)
            except (ValueError, IndexError):
                return (0, 0)

        sorted_files = sorted(all_files, key=parse_file_key, reverse=True)

        for f in sorted_files:
            try:
                df = pd.read_json(f, orient="records")
                if not df.empty and "date" in df.columns:
                    # Calculate max in this file
                    # Ensure date is date object
                    df["date"] = pd.to_datetime(df["date"]).dt.date
                    local_max = df["date"].max()
                    if local_max and (max_date is None or local_max > max_date):
                        # Since we iterate descending, the first valid max found in
                        # the newest partition MIGHT be the global max, provided partitions
                        # are strictly time-ordered. But a transaction from Jan could be in H2
                        # file if mis-saved? No, we shard by date.
                        # So yes, the max date in the newest non-empty partition is the global max.
                        return local_max
            except ValueError:
                continue

        return max_date

    # --- Exchange Rates (Simple JSON) ---

    def save_exchange_rate(self, rate: ExchangeRate):
        rates = self._load_rates()
        key = f"{rate.date.isoformat()}_{rate.currency.value}"
        rates[key] = str(rate.rate)

        with open(self._rates_file, "w") as f:
            json.dump(rates, f, indent=4)

    def get_exchange_rate(self, date_obj: date, currency: Currency) -> ExchangeRate | None:
        rates = self._load_rates()
        key = f"{date_obj.isoformat()}_{currency.value}"
        val = rates.get(key)

        if val:
            return ExchangeRate(date=date_obj, currency=currency, rate=Decimal(val))
        return None

    def _load_rates(self) -> dict:
        if not self._rates_file.exists():
            return {}
        try:
            with open(self._rates_file) as f:
                return json.load(f)
        except (json.JSONDecodeError, OSError):
            return {}
