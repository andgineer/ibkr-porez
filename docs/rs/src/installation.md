> [English](../en/installation.html) | [Русский](../ru/installation.html) | [Українська](../uk/installation.html) | **Srpski** | [Српски](../rs-cyr/installation.html)

# Instalacija

## Instalater

Preuzmite gotov instalater sa stranice izdanja:

**[https://github.com/andgineer/ibkr-porez/releases](https://github.com/andgineer/ibkr-porez/releases)**

### macOS

Preuzmite najnoviji `.pkg` fajl.
Pošto instalater nije potpisan Apple sertifikatom, macOS će ga blokirati pri otvaranju.
> _"IBKR Porez" je oštećen i ne može da se otvori. Treba ga premestiti u smeće._

**Ne premeštajte u smeće.** Umesto toga:

1. Otvorite **System Settings → Privacy & Security**
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
3. Kliknite **Keep** → **Show more** → **Keep anyway**

Pri pokretanju instalatera, Windows može prikazati poruku **Windows protected your PC**:
1. Kliknite **More info**
2. Kliknite **Run anyway**

Može se pojaviti i User Account Control dijalog sa porukom **Unknown publisher**.
Ako je fajl preuzet sa zvanične stranice izdanja, kliknite **Yes** za nastavak.

Nakon instalacije, **IBKR Porez** će se pojaviti u Start meniju.

---

## Za napredne korisnike

Instalater takođe instalira komandu `ibkr-porez` za terminal
(možda će biti potrebno ponovo pokrenuti terminal nakon instalacije).

### Preuzimanje gotovog binarnog fajla

Takođe možete preuzeti binarne fajlove za vašu platformu sa stranice izdanja:

**[https://github.com/andgineer/ibkr-porez/releases](https://github.com/andgineer/ibkr-porez/releases)**

Arhiva sadrži oba binarna fajla: `ibkr-porez` (CLI) i `ibkr-porez-gui` (GUI).
Raspakujte arhivu i stavite fajlove negde u vaš `PATH`.

### Instalacija iz izvornog koda

Ako imate instaliran Rust:

```bash
cargo install ibkr-porez
```
