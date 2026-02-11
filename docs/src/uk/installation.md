# Встановлення
Рекомендується використовувати [`pipx`](https://pipx.pypa.io) для встановлення, щоб уникнути конфліктів із системними пакетами Python:

### Встановлення `pipx`

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
    # Якщо ви встановили python через app-store, замініть `python` на `python3` у наступному рядку.
    python -m pip install --user pipx
    ```

### Встановлення застосунку

```bash
pipx install ibkr-porez
```
