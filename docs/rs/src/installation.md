> [English](../en/installation.html) | [Русский](../ru/installation.html) | [Українська](../uk/installation.html) | **Srpski** | [Српски](../rs-cyr/installation.html)

# Instalacija

## Grafički instalater (samo GUI, bez CLI)

Ako vam je potrebna samo grafička aplikacija i ne treba vam komandna linija, preuzmite gotov instalater sa stranice izdanja:

**[https://github.com/andgineer/ibkr-porez-rs/releases](https://github.com/andgineer/ibkr-porez-rs/releases)**

### macOS

Preuzmite najnoviji `.dmg` fajl.
Pošto aplikacija nije potpisana Apple sertifikatom, macOS može da je blokira pri prvom otvaranju:

> _"IBKR Porez" je oštećen i ne može da se otvori. Treba ga premestiti u smeće._

**Ne premeštajte u smeće.** Umesto toga:

1. Otvorite **System Settings -> Privacy & Security**
2. Pri dnu odeljka Security pojaviće se poruka o blokiranoj aplikaciji — kliknite **Open Anyway**
3. U sledećem dijalogu potvrdite otvaranje

Možda će biti potrebno da ove korake ponovite **dva puta**:
- prvo pri otvaranju preuzetog instalatera (`.dmg`)
- a zatim pri prvom pokretanju instalirane aplikacije iz `/Applications`

Nakon toga aplikacija bi trebalo da se pokreće bez upozorenja.

### Windows

Preuzmite najnoviji `.msi` fajl.
Pošto instalater nije digitalno potpisan, Windows može prikazati bezbednosna upozorenja.

Ako pregledač blokira preuzimanje (na primer u Microsoft Edge):
1. Otvorite panel preuzimanja u pregledaču (`Ctrl+J`)
2. Pronađite blokirano `.msi` preuzimanje
3. Kliknite **Keep** -> **Show more** -> **Keep anyway**

Pri pokretanju instalatera, Windows može prikazati poruku **Windows protected your PC**:
1. Kliknite **More info**
2. Kliknite **Run anyway**

Može se pojaviti i User Account Control dijalog sa porukom **Unknown publisher**.
Ako je fajl preuzet sa zvanične stranice izdanja, kliknite **Yes** za nastavak.

Nakon instalacije, aplikacija bi trebalo da se pokreće normalno.

---

## Preuzimanje gotovog binarnog fajla (CLI)

Preuzmite binarni fajl za vašu platformu sa stranice izdanja:

**[https://github.com/andgineer/ibkr-porez-rs/releases](https://github.com/andgineer/ibkr-porez-rs/releases)**

Raspakujte arhivu i stavite `ibkr-porez` binarni fajl negde u vaš `PATH`.

## Instalacija iz izvornog koda (CLI + GUI)

Ako imate instaliran Rust:

```bash
cargo install ibkr-porez
```
