import json
from pathlib import Path

from platformdirs import user_config_dir, user_data_dir

from ibkr_porez.models import UserConfig

DATA_SUBDIR = "ibkr-porez-data"


class ConfigManager:
    APP_NAME = "ibkr-porez"
    CONFIG_FILENAME = "config.json"

    def __init__(self):
        self._config_dir = Path(user_config_dir(self.APP_NAME))
        self._config_file = self._config_dir / self.CONFIG_FILENAME
        self._ensure_config_dir()

    def _ensure_config_dir(self):
        self._config_dir.mkdir(parents=True, exist_ok=True)

    def load_config(self) -> UserConfig:
        if not self._config_file.exists():
            return UserConfig(full_name="", address="")

        try:
            with open(self._config_file) as f:
                data = json.load(f)
                return UserConfig(**data)
        except (json.JSONDecodeError, OSError):
            return UserConfig(full_name="", address="")

    def save_config(self, config: UserConfig):
        with open(self._config_file, "w") as f:
            json.dump(config.model_dump(), f, indent=4)

    @property
    def config_path(self) -> Path:
        return self._config_file


config_manager = ConfigManager()


def get_default_data_dir_path() -> Path:
    """Return default data directory path."""
    return Path(user_data_dir(ConfigManager.APP_NAME)) / DATA_SUBDIR


def get_effective_data_dir_path(config: UserConfig) -> Path:
    """Resolve data dir with fallback to default directory."""
    if config.data_dir:
        return Path(config.data_dir).expanduser().resolve()
    return get_default_data_dir_path().expanduser().resolve()


def get_default_output_dir_path() -> Path:
    """Return default output directory path."""
    return Path.home() / "Downloads"


def get_effective_output_dir_path(config: UserConfig | None = None) -> Path:
    """Resolve output directory with fallback to default directory."""
    resolved_config = config or config_manager.load_config()
    if resolved_config.output_folder:
        return Path(resolved_config.output_folder)
    return get_default_output_dir_path()


def get_data_dir_change_warning(old_config: UserConfig, new_config: UserConfig) -> str | None:
    """Return warning message if effective data directory changed."""
    old_path = get_effective_data_dir_path(old_config)
    new_path = get_effective_data_dir_path(new_config)
    if old_path == new_path:
        return None
    return (
        "Data directory changed. Move existing database files manually "
        f"from {old_path} to {new_path}."
    )
