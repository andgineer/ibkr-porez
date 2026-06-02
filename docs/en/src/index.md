> **English** | [Русский](../ru/) | [Українська](../uk/) | [Srpski](../rs/) | [Српски](../rs-cyr/)

Automated generation of PPDG-3R (Capital Gains Tax) and PP OPO (Capital Income Tax) declarations
for Interactive Brokers users in Serbia.

The app downloads your trades from Interactive Brokers and creates a ready-to-upload XML file for ePorezi.
It tracks the full chain of purchases and sales for each security, calculates profit and loss,
and converts all amounts to dinars at the official NBS exchange rate on the date of each transaction —
exactly as required by the declaration.

## Installation

> ⚠️ **Windows and macOS will block the download or launch** — the app is distributed for free,
> and paying ~€100/year for a developer certificate is not an option.
> The installation guide explains how to work around this —
> **read it before downloading.**

[Installation guide ↗](installation.md)

## How to use

1. Open the app — it will launch with a graphical interface.
2. Click **Config** and enter your Interactive Brokers credentials.
3. Click **Sync** — the app will download your latest trades and create declarations.
4. Upload the generated XML file to the **ePorezi** portal (PPDG-3R section).

![PPDG-3R](images/ppdg-3r.png)

> ℹ️ If you have **more than one year of trade history** in Interactive Brokers —
> you need to load the older data manually before the first Sync.
> [How to do this ↗](ibkr.md#export-full-history-for-import-command)

---

Full documentation for the command line and other features — see [Usage ↗](usage.md).
