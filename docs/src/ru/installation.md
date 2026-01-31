## Установка
Установите с помощью [`pipx`](https://pypa.github.io/pipx/), что предотвращает конфликты с системными пакетами Python:

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

После настройки `pipx` установите `ibkr-porez`:

```bash
pipx install ibkr-porez
```
