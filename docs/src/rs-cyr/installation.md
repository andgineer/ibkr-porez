
## Инсталација
Инсталирајте користећи [`pipx`](https://pypa.github.io/pipx/) за изолована окружења, што спречава ометање
Python пакета вашег система:

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

**Завршни корак**: Када је `pipx` подешен, инсталирајте `ibkr-porez`:

```bash
pipx install ibkr-porez
```
