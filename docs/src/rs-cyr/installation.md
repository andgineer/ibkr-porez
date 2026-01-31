# Инсталација
Препоручује се употреба [`pipx`](https://pipx.pypa.io) за инсталацију како би се спречили конфликти са системским Python пакетима:

### Инсталација `pipx`

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
    # Ако сте инсталирали python преко app-store, замените `python` са `python3` у следећој линији.
    python -m pip install --user pipx
    ```

### Инсталација апликације

```bash
pipx install ibkr-porez
```
