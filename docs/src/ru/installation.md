# Установка
Рекомендуется для установки использовать [`pipx`](https://pipx.pypa.io), чтобы предотвратить конфликты с системными пакетами Python:

### Установка `pipx`

=== "MacOS"
    ```bash
    brew install pipx
    pipx ensurepath
    ```

=== "Linux"
    ```bash
    python3 -m pip install --user pipx
    python3 -m pipx ensurepath
    ```

=== "Windows"
    ```bash
    # Если вы установили python через app-store, замените `python` на `python3` в следующей строке.
    python -m pip install --user pipx
    ```

### Установка приложения

```bash
pipx install ibkr-porez
```
