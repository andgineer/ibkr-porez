import json
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path

import pandas as pd
from platformdirs import user_data_dir

from ibkr_porez.models import (
    Currency,
    Declaration,
    DeclarationStatus,
    DeclarationType,
    ExchangeRate,
    Transaction,
)


class Storage:
    APP_NAME = "ibkr-porez"
    RATES_FILENAME = "rates.json"
    DECLARATIONS_FILENAME = "declarations.json"
    RAW_DIR = "raw_reports"
    PARTITION_DIR = "partitions"
    LAST_DECLARATION_DATE_KEY = "last_declaration_date"

    def __init__(self):
        self._data_dir = Path(user_data_dir(self.APP_NAME))
        self._partition_dir = self._data_dir / self.PARTITION_DIR
        self._rates_file = self._data_dir / self.RATES_FILENAME
        self._declarations_file = self._data_dir / self.DECLARATIONS_FILENAME
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

    def save_transactions(self, transactions: list[Transaction]) -> tuple[int, int]:
        """
        Save transactions to storage.
        Returns:
            tuple[int, int]: (count_inserted, count_updated)
        """
        if not transactions:
            return 0, 0

        df = pd.DataFrame([t.__dict__ for t in transactions])

        # Ensure date format
        df["date"] = pd.to_datetime(df["date"]).dt.date

        # Partition data by year and half
        june_month = 6
        df["year"] = df["date"].apply(lambda d: d.year)
        df["half"] = df["date"].apply(lambda d: 1 if d.month <= june_month else 2)

        grouped = df.groupby(["year", "half"])

        total_inserted = 0
        total_updated = 0

        for (year, half), group in grouped:
            inserted, updated = self._save_partition(year, half, group)
            total_inserted += inserted
            total_updated += updated

        return total_inserted, total_updated

    def _save_partition(self, year: int, half: int, new_df: pd.DataFrame) -> tuple[int, int]:
        file_path = self._partition_dir / f"transactions_{year}_H{half}.json"

        # 1. Load Existing
        existing_df = pd.DataFrame()
        if file_path.exists():
            try:
                existing_df = pd.read_json(file_path, orient="records")
                if "transaction_id" in existing_df.columns:
                    existing_df["transaction_id"] = existing_df["transaction_id"].astype(str)
            except ValueError as e:
                print(f"[WARNING] Failed to load existing partition {file_path.name}: {e}")

        if existing_df.empty:
            count = self._write_df(new_df, file_path)
            return count, 0

        # 2. Smart Deduplication
        from collections import Counter

        # Build Counter of EXISTING transactions
        existing_keys = Counter()
        existing_id_map = {}

        for _, row in existing_df.iterrows():
            k = self._make_transaction_key(row)
            existing_keys[k] += 1
            if k not in existing_id_map:
                existing_id_map[k] = []
            existing_id_map[k].append(row["transaction_id"])

        # Process NEW transactions
        to_add, ids_to_remove, inserted, updated = self._identify_updates(
            new_df,
            existing_df,
            existing_keys,
            existing_id_map,
        )

        if not to_add and not ids_to_remove:
            return 0, 0

        # Construct Final DF
        final_df = self._construct_final_df(existing_df, pd.DataFrame(to_add), ids_to_remove)

        self._write_df(final_df, file_path)
        return inserted, updated

    def _make_transaction_key(self, row):
        try:
            raw_date = row.get("date")
            d = "" if pd.isna(raw_date) else str(pd.to_datetime(raw_date).date())

            s = str(row.get("symbol", ""))

            raw_type = row.get("type", "")
            t = str(raw_type.value) if hasattr(raw_type, "value") else str(raw_type)

            q = abs(float(row.get("quantity", 0)))
            p = round(float(row.get("price", 0)), 4)
            return (d, s, t, q, p)
        except (ValueError, TypeError):
            return ("ERR",)

    def _identify_updates(self, new_df, existing_df, existing_keys, existing_id_map):
        to_add = []
        ids_to_remove = set()
        existing_ids = set(existing_df["transaction_id"])

        # XML Supremacy: Dates that will be covered by new OFFICIAL records
        official_new_dates = self._get_official_dates(new_df)

        # 1. Identify Existing CSVs to remove due to XML Supremacy
        if official_new_dates:
            for _, row in existing_df.iterrows():
                if str(row["transaction_id"]).startswith("csv-"):
                    raw_date = row.get("date")
                    d_str = str(pd.to_datetime(raw_date).date()) if not pd.isna(raw_date) else ""
                    if d_str in official_new_dates:
                        ids_to_remove.add(row["transaction_id"])

        # 2. Process New Rows
        official_existing_dates = self._get_official_dates(existing_df)
        dedup_index = (existing_keys, existing_id_map)
        results = (to_add, ids_to_remove)

        updates_count = 0

        # Build index for fast lookup of existing rows to check for exact duplicates
        existing_df_indexed = existing_df.set_index("transaction_id")

        for _, row in new_df.iterrows():
            new_id = row["transaction_id"]
            if new_id in existing_ids:
                # Check for exact content match to avoid noisy "updates"
                try:
                    existing_row = existing_df_indexed.loc[new_id]
                    if self._is_identical_record(row, existing_row):
                        continue
                except (KeyError, ValueError, TypeError):
                    # In case of duplicate IDs index error or type mismatch, fall back to update
                    pass

                to_add.append(row)
                updates_count += 1
                continue

            k = self._make_transaction_key(row)
            if existing_keys[k] > 0:
                updates_count += self._handle_semantic_match(row, k, dedup_index, results)
            else:
                self._handle_no_match(row, official_existing_dates, to_add)

        # Determine "inserted" (truly new) count
        # Total added - updates (ID matches OR Semantic Upgrades)
        # Note: semantic match upgrades are technically new IDs, so they count as inserts here?
        # User wants to know if "Database grew".
        # If I replace CSV "csv-1" with XML "xml-1", that's 1 insert, 1 delete. Net 0.
        # But separating "New" (Inserts) from "Updated" (ID Match) is clear enough.
        # "68 new" vs "0 new, 68 updated".
        inserted_count = len(to_add) - updates_count

        return to_add, ids_to_remove, inserted_count, updates_count

    def _get_official_dates(self, df: pd.DataFrame) -> set[str]:
        """Return set of date strings that have official (XML) records."""
        dates = set()
        for _, row in df.iterrows():
            if not str(row["transaction_id"]).startswith("csv-"):
                raw_date = row.get("date")
                d_str = str(pd.to_datetime(raw_date).date()) if not pd.isna(raw_date) else ""
                if d_str:
                    dates.add(d_str)
        return dates

    def _handle_semantic_match(self, row, k, dedup_index, results) -> int:
        """
        Handle semantic match logic.
        Returns:
            int: 1 if this counts as an 'Update' (Upgrade), 0 otherwise.
        """
        existing_keys, existing_id_map = dedup_index
        to_add, ids_to_remove = results

        new_id = row["transaction_id"]
        is_new_csv = str(new_id).startswith("csv-")
        matched_existing_id = existing_id_map[k][0]
        is_old_csv = str(matched_existing_id).startswith("csv-")

        if not is_new_csv and is_old_csv:
            # Upgrade: XML replacing CSV
            ids_to_remove.add(matched_existing_id)
            to_add.append(row)
            self._consume_match(existing_keys, existing_id_map, k)
            return 1  # Count as UPDATE

        if is_new_csv and not is_old_csv:
            # Skip: New CSV trying to duplicate existing XML
            self._consume_match(existing_keys, existing_id_map, k)
            return 0

        if not is_new_csv and not is_old_csv:
            # XML vs XML with different IDs (Split Orders). Add, do not consume.
            to_add.append(row)
            return 0  # New split part -> Insert

        # CSV vs CSV. Assume duplicate.
        self._consume_match(existing_keys, existing_id_map, k)
        return 0

    def _handle_no_match(self, row, official_existing_dates, to_add):
        new_id = row["transaction_id"]
        is_new_csv = str(new_id).startswith("csv-")
        raw_date = row.get("date")
        d_str = str(pd.to_datetime(raw_date).date()) if not pd.isna(raw_date) else ""

        if is_new_csv and d_str in official_existing_dates:
            # Skip CSV because date is covered by XML (Source of Truth)
            pass
        else:
            to_add.append(row)

    def _is_identical_record(self, row_new, row_existing) -> bool:
        """
        Compare two rows to see if they are effectively identical.
        Handles type differences (Decimal vs float, str vs date, etc).
        Returns True if identical.
        """
        try:
            # 1. Date
            d1 = str(pd.to_datetime(row_new.get("date")).date())
            d2 = str(pd.to_datetime(row_existing.get("date")).date())
            if d1 != d2:
                return False

            # 2. Key Numeric Fields
            # Use rounding to ignore floating point noise
            fields = ["quantity", "price", "amount"]
            float_tolerance = 0.0001
            for f in fields:
                v1 = float(row_new.get(f, 0) or 0)
                v2 = float(row_existing.get(f, 0) or 0)
                if abs(v1 - v2) > float_tolerance:
                    return False

            # 3. Strings
            str_fields = ["symbol", "type", "currency", "description"]
            for f in str_fields:
                v1 = row_new.get(f, "")
                if hasattr(v1, "value"):
                    s1 = str(v1.value).strip().upper()
                else:
                    s1 = str(v1).strip().upper()

                s2 = str(row_existing.get(f, "")).strip().upper()

                if s1 != s2:
                    return False

            return True

        except (ValueError, TypeError):
            # If comparison fails due to weird data, assume different
            return False

    def _consume_match(self, keys, id_map, k):
        keys[k] -= 1
        id_map[k].pop(0)

    def _construct_final_df(self, existing_df, new_subset_df, ids_to_remove):
        if new_subset_df.empty:
            return existing_df[~existing_df["transaction_id"].isin(ids_to_remove)]

        existing_kept = existing_df[~existing_df["transaction_id"].isin(ids_to_remove)]
        new_ids = set(new_subset_df["transaction_id"])
        existing_kept = existing_kept[~existing_kept["transaction_id"].isin(new_ids)]

        # Align headers to silence FutureWarning and keep data clean
        new_subset_df = new_subset_df.dropna(axis=1, how="all")
        existing_kept = existing_kept.dropna(axis=1, how="all")
        return pd.concat([existing_kept, new_subset_df], ignore_index=True)

    def _write_df(self, df: pd.DataFrame, file_path: Path, return_count: int | None = None) -> int:
        cols_to_save = [c for c in df.columns if not c.startswith("_")]
        df[cols_to_save].to_json(
            file_path,
            orient="records",
            date_format="iso",
            indent=4,
        )
        return return_count if return_count is not None else len(df)

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

    # --- Declarations (JSON) ---

    def _load_declarations(self) -> dict:
        """Load declarations data from JSON file."""
        if not self._declarations_file.exists():
            return {"declarations": [], "last_declaration_date": None}
        try:
            with open(self._declarations_file, encoding="utf-8") as f:
                data = json.load(f)
                # Ensure required keys exist
                if "declarations" not in data:
                    data["declarations"] = []
                if "last_declaration_date" not in data:
                    data["last_declaration_date"] = None
                return data
        except (json.JSONDecodeError, OSError):
            return {"declarations": [], "last_declaration_date": None}

    def _save_declarations(self, data: dict) -> None:
        """Save declarations data to JSON file."""
        with open(self._declarations_file, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, default=str, ensure_ascii=False)

    def save_declaration(self, declaration: Declaration) -> None:
        """Save or update declaration."""
        data = self._load_declarations()
        declarations = [Declaration(**d) for d in data["declarations"]]

        # Find existing declaration by ID
        existing_idx = None
        for idx, decl in enumerate(declarations):
            if decl.declaration_id == declaration.declaration_id:
                existing_idx = idx
                break

        if existing_idx is not None:
            declarations[existing_idx] = declaration
        else:
            declarations.append(declaration)

        # Convert back to dicts
        data["declarations"] = [d.model_dump(mode="json") for d in declarations]
        self._save_declarations(data)

    def get_declarations(
        self,
        status: DeclarationStatus | None = None,
        declaration_type: DeclarationType | None = None,
    ) -> list[Declaration]:
        """Get declarations with optional filtering."""
        data = self._load_declarations()
        declarations = [Declaration(**d) for d in data["declarations"]]

        # Apply filters
        if status is not None:
            declarations = [d for d in declarations if d.status == status]
        if declaration_type is not None:
            declarations = [d for d in declarations if d.type == declaration_type]

        return declarations

    def get_declaration(self, declaration_id: str) -> Declaration | None:
        """Get specific declaration by ID."""
        data = self._load_declarations()
        declarations = [Declaration(**d) for d in data["declarations"]]

        for decl in declarations:
            if decl.declaration_id == declaration_id:
                return decl
        return None

    def declaration_exists(self, declaration_id: str) -> bool:
        """Check if declaration exists."""
        return self.get_declaration(declaration_id) is not None

    def update_declaration_status(
        self,
        declaration_id: str,
        status: DeclarationStatus,
        timestamp: datetime,
    ) -> None:
        """Update declaration status and timestamp."""
        decl = self.get_declaration(declaration_id)
        if not decl:
            raise ValueError(f"Declaration {declaration_id} not found")

        decl.status = status
        if status == DeclarationStatus.SUBMITTED:
            decl.submitted_at = timestamp
        elif status == DeclarationStatus.PAID:
            decl.paid_at = timestamp

        self.save_declaration(decl)

    def get_last_declaration_date(self) -> date | None:
        """Get last date for which declarations were created."""
        data = self._load_declarations()
        date_str = data.get("last_declaration_date")
        if date_str:
            return (
                datetime.fromisoformat(date_str).date() if isinstance(date_str, str) else date_str
            )
        return None

    def set_last_declaration_date(self, date_obj: date) -> None:
        """Set last date for which declarations were created."""
        data = self._load_declarations()
        data["last_declaration_date"] = date_obj.isoformat()
        self._save_declarations(data)
