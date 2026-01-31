# Instalacija
Preporučuje se upotreba [`pipx`](https://pipx.pypa.io) za instalaciju kako bi se sprečili konflikti sa sistemskim Python paketima:

### Instalacija `pipx`

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
    # Ako ste instalirali python preko app-store, zamenite `python` sa `python3` u sledećoj liniji.
    python -m pip install --user pipx
    ```

### Instalacija aplikacije

```bash
pipx install ibkr-porez
```
