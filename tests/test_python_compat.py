# /// script
# requires-python = ">=3.12"
# dependencies = ["ibkr-porez @ git+https://github.com/andgineer/ibkr-porez.git"]
# ///
"""Cross-language compatibility test: verify Python can read Rust-written JSON files.

Invoked by `uv run` from a Rust integration test:
    uv run tests/test_python_compat.py <data_dir>

Expects transactions.json, rates.json, declarations.json, config.json in <data_dir>.
Uses mock.patch to bypass platformdirs -- no files outside the temp dir.
"""
import json
import sys
from datetime import date
from pathlib import Path
from unittest.mock import patch


def main():
    data_dir = Path(sys.argv[1])
    assert data_dir.exists(), f"data_dir does not exist: {data_dir}"

    from ibkr_porez.models import Currency, UserConfig
    from ibkr_porez.storage import Storage

    # 1. Config: verify Rust-written config.json is readable as UserConfig via Pydantic
    with open(data_dir / "config.json") as f:
        cfg = UserConfig(**json.load(f))
    assert cfg.full_name == "Test User"
    assert cfg.data_dir == str(data_dir)

    # 2. Storage: mock platformdirs so everything stays inside data_dir
    mock_config = UserConfig(
        full_name="Test User",
        address="Test Address",
        data_dir=str(data_dir),
    )
    with (
        patch("ibkr_porez.storage.config_manager") as mock_cm,
        patch("ibkr_porez.storage.user_data_dir", return_value=str(data_dir)),
    ):
        mock_cm.load_config.return_value = mock_config
        storage = Storage()

    # 3. Transactions: pd.read_json must succeed on Rust-written format
    df = storage.get_transactions()
    assert not df.empty, "get_transactions() returned empty DataFrame"
    row = df.iloc[0]
    assert str(row["transaction_id"]) == "TEST-001"
    assert str(row["symbol"]) == "AAPL"
    assert float(row["quantity"]) == 10.0
    assert float(row["amount"]) == -1500.0

    # 4. Exchange rates: json.load + Decimal parsing
    rate = storage.get_exchange_rate(date(2025, 6, 15), Currency.USD)
    assert rate is not None, "get_exchange_rate returned None"
    assert float(rate.rate) == 117.25

    # 5. Declarations: json.load + Declaration(**d) via Pydantic
    decls = storage.get_declarations()
    assert len(decls) >= 1, f"Expected at least 1 declaration, got {len(decls)}"
    decl = decls[0]
    assert decl.declaration_id == "DECL-001"
    assert str(decl.type.value) == "PPDG-3R"

    print("All Python compatibility checks passed.")


if __name__ == "__main__":
    main()
