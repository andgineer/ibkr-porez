
## Instalacija
Instalirajte koristeći [`pipx`](https://pypa.github.io/pipx/) za izolovana okruženja, što sprečava ometanje
Python paketa vašeg sistema:

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

**Završni korak**: Kada je `pipx` podešen, instalirajte `ibkr-porez`:

```bash
pipx install ibkr-porez
```
