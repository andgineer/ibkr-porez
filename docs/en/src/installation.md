> **English** | [Русский](../ru/installation.html) | [Українська](../uk/installation.html) | [Srpski](../rs/installation.html) | [Српски](../rs-cyr/installation.html)

# Installation

## Installer

Download the installer from the releases page:

**[https://github.com/andgineer/ibkr-porez/releases](https://github.com/andgineer/ibkr-porez/releases)**

### macOS

Download the latest `.pkg` file.
Because the installer is not signed with an Apple certificate, macOS will block it when you try to open it.
> _"IBKR Porez" is damaged and can't be opened. You should move it to the Bin._

**Do not move it to the Bin.** Instead:

1. Open **System Settings → Privacy & Security**
2. At the bottom of the Security section, you will see a blocked-app message. Click **Open Anyway**
3. Confirm opening in the next dialog

You may need to repeat the same steps **twice**:
- first when opening the downloaded installer (`.dmg`)
- and again when launching the installed app from `/Applications` for the first time

After that, the app should launch without warnings.

### Windows

Download the latest `.msi` file.
Because the installer is not code-signed, Windows may show security warnings.

If your browser blocks the download (for example in Microsoft Edge):
1. Open the browser downloads panel (`Ctrl+J`)
2. Find the blocked `.msi` download
3. Click **Keep** → **Show more** → **Keep anyway**

When launching the installer, Windows may show **Windows protected your PC**:
1. Click **More info**
2. Click **Run anyway**

You may also see a User Account Control dialog with **Unknown publisher**.
If the file came from the official releases page, click **Yes** to continue.

After installation, **IBKR Porez** will appear in the Start Menu.

---

## For Advanced Users

The installer also sets up the `ibkr-porez` command for the terminal
(you may need to restart the terminal after installation).

### Download Prebuilt Binary

You can also download binaries for your platform from the releases page:

**[https://github.com/andgineer/ibkr-porez/releases](https://github.com/andgineer/ibkr-porez/releases)**

The archive contains both binaries: `ibkr-porez` (CLI) and `ibkr-porez-gui` (GUI).
Extract the archive and place the files somewhere in your `PATH`.

### Install from Source

If you have the Rust toolchain installed:

```bash
cargo install ibkr-porez
```
