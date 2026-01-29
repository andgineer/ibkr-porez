import json
from pathlib import Path

from platformdirs import user_config_dir

from ibkr_porez.models import UserConfig


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
            return UserConfig()

        try:
            with open(self._config_file) as f:
                data = json.load(f)
                return UserConfig(**data)
        except (json.JSONDecodeError, OSError):
            return UserConfig()

    def save_config(self, config: UserConfig):
        with open(self._config_file, "w") as f:
            json.dump(config.model_dump(), f, indent=4)

    @property
    def config_path(self) -> Path:
        return self._config_file


config_manager = ConfigManager()
