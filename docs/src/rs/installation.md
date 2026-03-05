# Instalacija

## Grafički instalater (samo GUI, bez CLI)

Ako vam je potrebna samo grafička aplikacija i ne treba vam komandna linija, preuzmite gotov instalater sa stranice izdanja:

**[https://github.com/andgineer/ibkr-porez/releases](https://github.com/andgineer/ibkr-porez/releases)**

=== "macOS"
    Preuzmite najnoviji `.dmg` fajl.
    Pošto aplikacija nije potpisana Apple sertifikatom, macOS će pri prvom pokretanju prikazati poruku:

    > _"IBKR Porez" je oštećen i ne može da se otvori. Treba ga premestiti u smeće._

    **Ne premeštajte u smeće.** Umesto toga:

    1. Otvorite **System Settings -> Privacy & Security**
    2. Pri dnu odeljka Security pojaviće se poruka o blokiranoj aplikaciji — kliknite **Open Anyway**
    3. U sledećem dijalogu potvrdite otvaranje

    Nakon toga aplikacija će se pokretati bez upozorenja.

=== "Windows"
    Preuzmite najnoviji `.msi` fajl.

---

## Instalacija Python package (CLI + GUI)

Ako je CLI vaš maternji jezik (AI agenti i hrabri ljudi), instalirajte Python package.

Instalirajte Astral `uv tool`: [Instalacija `uv`](https://docs.astral.sh/uv/getting-started/installation/)

### Instalacija aplikacije

```bash
uv tool install ibkr-porez --python 3.12
```
