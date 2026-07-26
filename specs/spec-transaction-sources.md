# Transaction Sources

## Why

Transactions reach the application from two places, and they are not
interchangeable.

The IBKR Flex Query report is the live feed. It reaches back a limited period and
is complete: every field the application needs to compute a tax liability is
there.

A CSV activity statement is imported by hand to reach further back than a Flex
Query goes. A share sold today may have been bought years earlier, and the gain
cannot be computed without knowing what that purchase cost.

## Roles

The Flex report is the source of record. Every declaration the application
generates is computed from it.

A CSV import supplies purchase history and nothing else. It answers "what did
this position cost", so that a sale of it can be taxed correctly.

## A CSV import never produces an income declaration

Dividends, interest and withholding tax carried by a CSV import are ignored when
income declarations are generated. Two reasons, either sufficient on its own:

- the period a CSV import covers is older than any filing deadline, so a
  declaration built from it would be late by construction — filing it is a
  deliberate act by the taxpayer, not something the application should produce
  on its own;
- a CSV activity statement does not describe a distribution the way a Flex report
  does. It carries neither the identifier that ties a withholding tax to the
  dividend it was withheld from, nor a description in the shape that matching
  otherwise relies on. Any foreign-tax credit derived from it would be a guess,
  and a wrong credit understates the tax due.

Capital gains are unaffected. The purchase side of a sale is taken from CSV data
whenever that is where the purchase lives — which is the entire point of the
import.

## Withholding tax is corrected long after the fact

A withholding tax posted alongside a distribution is not final. The issuer reports
the final tax character of a year's distributions to the broker after that year
closes. Where the character differs from what was assumed when the distribution
was paid — it turns out to be an interest-related dividend under IRC §871(k), or
a return of capital — the broker reverses the withholding it took, and withholds
anew if the corrected character calls for it.

For the distributions of one tax year this happens in the following January to
March. The deadline is the filing of Form 1042-S in March: once it is filed the
withholding agent can no longer reclaim the tax from the IRS, so no further
correction is possible.

The gap between a distribution and the correction of its withholding therefore
runs from weeks to roughly fifteen months — a distribution paid in early January
can be corrected as late as March of the following year. Corrections outside this
annual cycle, such as a broker-interest withholding cancelled shortly after it was
posted, arrive sooner.

Two requirements follow, and neither may be bounded by a window measured from the
income date:

- a withholding row must be attributable to the distribution it belongs to
  however much time separates them;
- a declaration must stay open to correction for as long as its withholding can
  still change.

Source: [IBKR FAQ — dividend
reclassification](https://www.interactivebrokers.com/lib/cstools/faq/#/content/1128482286).

## Conflicts between the two sources

Where both sources describe the same day, the Flex report wins and the CSV
records for that day are discarded. A Flex report is authoritative and complete;
a CSV import is a manual act and may be partial or overlapping.

## Out of scope

- Declaring income from a CSV import, by any command or flag.
- Reconstructing distribution identifiers for CSV rows so that they could be
  declared.
