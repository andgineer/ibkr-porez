# Installation
Recommended to use [`pipx`](https://pipx.pypa.io) for installation to prevent conflicts with system Python packages:

### Install `pipx`

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
    # If you installed python using the app-store, replace `python` with `python3` in the next line.
    python -m pip install --user pipx
    ```

### Install Application

```bash
pipx install ibkr-porez
```
