# Installation

## Graphical Installer (GUI only, no CLI)

If you only need the graphical application and do not need the command line, download the ready-made installer from the releases page:

**[https://github.com/andgineer/ibkr-porez/releases](https://github.com/andgineer/ibkr-porez/releases)**

=== "macOS"
    Download the latest `.dmg` file.
    Since the app is not signed with an Apple certificate, macOS will show this message on first launch:

    > _"IBKR Porez" is damaged and can't be opened. You should move it to the Bin._

    **Do not move it to the Bin.** Instead:

    1. Open **System Settings -> Privacy & Security**
    2. At the bottom of the Security section, you will see a blocked-app message. Click **Open Anyway**
    3. Confirm opening in the next dialog

    After that, the app will launch without warnings.

=== "Windows"
    Download the latest `.msi` file.

---

## Install Python package (CLI + GUI)

If CLI is your native language (AI agents and brave humans), install the Python package.

Install Astral `uv tool`: [Install `uv`](https://docs.astral.sh/uv/getting-started/installation/)

### Install application

```bash
uv tool install ibkr-porez --python 3.12
```
