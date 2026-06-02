> [English](../en/) | [Русский](../ru/) | [Українська](../uk/) | **Srpski** | [Српски](../rs-cyr/)

Automatizovano generisanje poreskih prijava PPDG-3R (kapitalna dobit) i PP OPO (prihodi od kapitala)
za korisnike Interactive Brokers u Srbiji.

Aplikacija preuzima vaše transakcije sa Interactive Brokers i kreira gotov XML fajl za ePorezi.
Prati ceo lanac kupovina i prodaja za svaku hartiju od vrednosti, izračunava dobitke i gubitke,
i konvertuje sve iznose u dinare po zvaničnom kursu NBS na datum svake transakcije —
tačno onako kako zahteva prijava.

## Instalacija

> ⚠️ **Windows i macOS će blokirati preuzimanje ili pokretanje** — aplikacija se
> distribuira besplatno, a plaćanje ~100 evra godišnje za sertifikat programera nije opcija.
> Uputstvo za instalaciju objašnjava kako ovo zaobići —
> **pročitajte ga pre preuzimanja.**

[Uputstvo za instalaciju ↗](installation.md)

## Kako koristiti

1. Otvorite aplikaciju — pokrenuće se sa grafičkim interfejsom.
2. Kliknite **Config** i unesite podatke sa Interactive Brokers.
3. Kliknite **Sync** — aplikacija će preuzeti najnovije transakcije i kreirati prijave.
4. Otpremite kreirani XML fajl na portal **ePorezi** (sekcija PPDG-3R).

![PPDG-3R](images/ppdg-3r.png)

> ℹ️ Ako imate **više od godinu dana istorije transakcija** u Interactive Brokers —
> pre prvog Sync-a potrebno je ručno učitati starije podatke.
> [Kako to uraditi ↗](ibkr.md#izvoz-pune-istorije-za-import-komandu)

---

Detaljna dokumentacija za komandnu liniju i ostale mogućnosti — u sekciji [Upotreba ↗](usage.md).
