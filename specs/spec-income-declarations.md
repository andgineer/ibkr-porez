# Income Declarations (PP-OPO)

Dividends and broker interest received from abroad are declared to PURS on a
PP-OPO form, one form per income group, within 30 days of the income date. Tax
withheld abroad is credited against the 15% due in Serbia.

See `spec-transaction-sources.md` for where the underlying transactions come
from and why withholding tax keeps changing after the fact.

## Domain of definition

Every rule about changing a declaration is defined over declarations the
application produced under these rules. A declaration that carries no record of
its own source amounts is outside that domain: it is not compared, not amended,
not assessed, and its correctness is not a question this document answers.

Transactions are a different matter and are not covered by this boundary. Every
stored transaction is read as an input, whatever its age — declaring income the
taxpayer asks for explicitly is exactly that. What is never done is writing back:
no stored transaction is corrected or backfilled.

This is the boundary of the domain, not a compromise, not an accepted risk and not
a limitation. It has no justification that can be argued with, because there is
nothing to argue. Every guarantee below — including the one about erring toward
overpaying — is a guarantee about output produced under these rules and says
nothing about anything outside them. An observation of the form "but that
declaration is wrong" is outside the domain and is not a finding.

Nothing in the application reaches back across this boundary: no migration, no
retroactive pass, no reconstruction of missing records, no flag that would enable
one.

## What one declaration covers

An income group is all income of one security (for dividends) or one currency
(for interest) received on one date, of one income kind. One group is one
declaration. A group is identified by those three things and by nothing else —
not by the name of the file it produced, which is free to change.

A group holds one currency, the one its income is paid in. A single distribution
split across two currencies on one date is not something the broker produces, and
these rules assume it does not.

## A withholding tax belongs to the distribution it names

Tax is attributed to income by identifying the distribution both belong to, never
by how close their dates are. Dates take no part in it: a reversal arriving months
after its dividend still finds it.

The distribution is identified, in order:

1. by the identifier the broker assigns to the corporate action (`actionID` in the
   Flex report), which a dividend and every withholding row belonging to it —
   reversals included — share;
2. failing that, by the distribution's own description, which the broker builds
   the tax description from;
3. for broker interest, which is not a corporate action and never carries such an
   identifier, by the month it is paid for **together with its currency**. The
   currency is essential: the tax description does not name it, and without it a
   month's tax on one currency's interest would be credited to another's.

The second rule is permanent, not a transitional fallback, and carries a large
share of the stored transactions on its own. The identifier is recorded when a
transaction first arrives and is never added to a transaction already stored —
that would write outside the domain of definition. It could not be done completely
in any case: a CSV activity statement has no such field at all, and a Flex Query
does not reach far enough back to re-supply the earliest
years. Matching by description is what serves every row without an identifier, and
it is equally what covers anything the broker emits without one: the field is not
listed in the published Flex Query reference and carries no stability promise.

Within one distribution the rules do not mix. Income and withholding that both
lack the identifier match by description on both sides; a distribution that
carries it carries it on both sides.

A tax belongs to one distribution and no more. Where an identification would fit
two distributions at once — a rate per share is not an identifier, so two payments
of one security at the same rate are indistinguishable by it — the tax is credited
to neither. Both then declare no credit and both overpay, which is the direction
every failure here takes; crediting each of them the whole amount would hand out
the same relief twice and understate the tax due. Two distributions are recognised
as competing only when both are read in the same run — see *Known problems*.

Amounts within one distribution are summed **with their sign**, so a reversal
cancels what it reverses. This holds on the income side as well as the tax side: a
reversed dividend cancels its original rather than counting twice.

Neither sum may turn a declaration against the taxpayer's own figures. A credit is
never negative: should reversals ever exceed the withholding of one distribution,
the credit is zero, not a negative amount that would push the tax due above the
15% actually owed. And income that nets to zero or below is not income — a fully
reversed dividend produces no declaration, because a form declaring nothing states
nothing.

## The income's own currency, and the one rate that converts it

A declaration is computed in the currency the income was paid in and converted to
dinars once, at the exchange rate of the day the income was received. That one
rate converts the gross income and the foreign tax credit alike.

The credit is not converted row by row at the rate of the day each withholding was
posted. Amounts within a distribution net with their sign, and a pair that cancels
in dollars has to cancel in dinars too; converted at two different rates it would
leave a residue, and the credit would depend on when the broker happened to post a
correction rather than on what was withheld. It also means a withholding row
posted long after its income needs no rate of its own, so no declaration waits on
an exchange rate for any date but the income's.

A declaration records the amounts in the income's own currency beside the dinar
figures, and the rate between them. That serves two purposes at once: those
amounts are what a later comparison is made against, and together with the rate
they are what makes a declaration checkable against the broker, who shows the
taxpayer dollars where the form shows dinars.

## When a declaration is created

Every sync looks for undeclared income within a recent window and declares it. A
group that already has a declaration is never declared again. There is no stored
high-water mark: income that reaches storage late is picked up by the next sync
while it is still inside the window, whatever made it late — a failed fetch, a
missing exchange rate, or ordinary reporting lag by the broker.

Income older than that window is not declared on the app's own initiative. Its
filing deadline has passed, so filing it is a deliberate act by the taxpayer, who
asks for it explicitly by widening the window.

A declaration deleted by the user is rebuilt by the next sync while its group is
still inside the window. Beyond it, rebuilding also takes an explicit request.

Whether a group already has a declaration is decided from what the declaration
records about the group itself, never from the name of the file it produced: a
file name is free to change, and a declaration need not have one at all. A
declaration whose record no longer identifies its group cannot be matched to it,
and nothing is reconstructed from what remains — the group reads as undeclared and
is declared again. The application reports such a declaration rather than working
around it silently.

## When a declaration must wait

A group for which no withholding tax was found at all is held for a bounded wait,
then declared without a foreign-tax credit — the full 15% is declared as due,
which is fiscally safe. The wait exists because tax usually posts within days of
the income, and the deadline leaves ample room for it.

A group whose withholding was found and nets to zero does **not** wait. Zero is
its answer, not a gap: the tax was withheld and reversed. This is the normal shape
of an interest-related dividend under IRC §871(k), and treating it as "not arrived
yet" would delay the most common case for nothing.

The wait runs from the income date, not from the day the row reached the
application, so income imported long after the fact is never held. There is no way
to shorten it: a declaration carrying a credit the broker has not finished
computing is precisely what it exists to prevent.

A group missing an exchange rate is reported and retried on the next sync.

## When a declaration is amended

When the amounts underlying an already-declared group change, an amendment
(измењена пријава) is generated. It is an ordinary declaration in every respect —
same list, same output directory, same submission and payment handling — marked as
amending the original.

The comparison is made in the income's own currency, never in dinars: a moved
exchange rate must not look like a change in income.

An amendment amends the **most recent** declaration of its group, not the first.
A group's withholding can change more than once, and each change produces exactly
one amendment referencing the one before it. Once an amendment exists it is what
subsequent comparisons are made against, so a settled group produces nothing
further.

The taxpayer records the PURS number of a declaration when submitting it, and an
amendment carries that number so the authority can tell which return it replaces.
The number is optional in the generated document — without it the amendment is
still valid to produce, and the taxpayer supplies it when filing.

How far back the app looks for such changes is set by how long withholding can
still change, not by the window that governs creation. Since a distribution of one
tax year can be corrected until the following March, the app examines income from
the start of the previous calendar year — or from the start of an explicitly
widened creation window, whichever reaches further back. A shorter horizon would
miss the annual reclassification entirely, which is the very event amendments
exist for.

Every declared group inside that horizon is recomputed and compared on every sync.
The comparison is deliberately not driven by which rows arrived in the latest
fetch: a row already stored is never new again, so a failed fetch, a run
interrupted between import and generation, or an import from a locally downloaded
report would lose the change for good — the same failure a stored high-water mark
causes on the creation side.

A declaration carrying no record of its source amounts is outside the domain of
definition and is never amended. Its recorded amounts are not reconstructed from
anything else it carries.

## Generating documents without recording them

The same rules also produce PP-OPO documents on demand, for a period the taxpayer
names. That path records no declaration and consults none: nothing there is
skipped as already declared, and nothing is ever amended. The period asked for is
the window, so nothing inside it is left out as too old, and the wait for missing
withholding tax applies unchanged.

## A declaration states whether it is on time

The PP-OPO form distinguishes a return filed on or before its deadline from one
filed after it, and the two carry different codes. A generated declaration states
which it is, judged by the deadline — income date plus 30 days, moved past
weekends and holidays — against the day it is generated.

The app never claims a return is timely when it knows the deadline has passed.
Every amendment is late by nature, as is every declaration produced for income
old enough to need an explicitly widened window.

Timeliness and amendment are two independent facts recorded in two independent
fields, each with its own codebook, and neither constrains the other. An amendment
filed after the deadline records both: late, and amending. There is no combined
code and no case where one field changes what the other may hold.

A late return also owes interest. The app does not compute it and does not claim
to: the interest fields are left empty for ePorezi and the taxpayer to complete,
as with the declaration number.

## Known problems

Unlike the accepted limitations below, these are defects. They are recorded here
because they are worth fixing, not because they have been accepted.

### A tax can be credited to the wrong distribution at the edge of the period read

Where one identification fits two distributions — a rate per share is not an
identifier, so two payments of one security at the same rate are indistinguishable
by it — the tax is credited to neither and both overpay. That rule is applied
among the distributions read in the same run, and the two sides are not read over
the same span: withholding tax is read with no upper bound on its date, because a
correction arrives long after the income it belongs to, while income is read
within a period.

A withholding row can therefore belong to a distribution lying outside that period
while a distribution inside it carries the same identification. Nothing recognises
the two as competing, and the whole of the tax is credited to the one inside: the
credit is overstated and the tax due understated — the one direction every other
rule here avoids. Closing this means recognising a competing distribution whatever
its date, not only when it falls inside the period being declared.

Reaching it takes a distribution the broker gave no identifier, since two
identifiers never collide, and a second distribution of the same security paying
the identical rate per share. Identification by description is permanent and
serves every row without an identifier, so this is not a case that ages out.

## Accepted limitations

These are limitations of behaviour inside the domain of definition. The boundary
of that domain is not one of them and does not belong on this list.

- An amendment is generated, not filed. The taxpayer submits it, and records the
  original's PURS number so the amendment can reference it.
- A withholding reversal joins nothing when the income it belongs to carries no
  distribution identifier and the reversal does. Backfilling the identifier onto
  stored income to close this is not done: it would rewrite data outside the
  domain of definition.
- Income imported from CSV is never declared. See `spec-transaction-sources.md`.
- A group whose exchange rate never resolves leaves the creation window and is
  forgotten. Every currency the application handles is published daily by the
  national bank, and a rate is looked for over several preceding days, so this
  takes an outage lasting the whole window.
- If distribution matching ever breaks entirely, groups see no tax and are
  declared with no credit — the taxpayer overpays. This is deliberate: it errs
  toward overpaying rather than underpaying, and it announces itself, because
  declarations that were always zero suddenly demand payment. The app carries no
  machinery to detect a broken match.
- A group that nets to zero **after** it was declared is left alone. Withdrawing a
  filed return takes a cancellation, a different document than an amendment, and
  the app produces only amendments. The standing declaration then overstates
  income, which overpays — the direction every other failure here takes.
- The app never emits a one-time signal: no migration notice, no pass over past
  declarations announcing that they may be wrong, nothing shown once at first run
  after an upgrade. Every notice is recomputed from current state on every sync
  and disappears when its cause does. A one-time signal has no state that keeps it
  true, cannot be re-derived once dismissed, and asks to be acknowledged rather
  than fixed.

## Sources

The form's structure and its field codes come from three places, which agree on
every code the application writes:

- the official schema and sample document,
  [`PPOPO-Prijava.xsd`](https://www.purs.gov.rs/upload/media/2025/2/4/609118/PPOPO-Prijava.xsd)
  and [`PPOPO.XML`](https://www.purs.gov.rs/upload/media/2025/2/4/609119/PPOPO.XML),
  both linked from [PURS, Упутства и обрасци](https://www.purs.gov.rs/sr/e-porezi/Uputstva.html).
  The schema fixes element order and the accepted codes, and validates the
  uploaded document;
- the [rulebook on self-assessed
  tax](https://www.paragraf.rs/propisi/pravilnik-o-poreskoj-prijavi-o-obracunatom-porezu-samooporezivanjem-i-pripadajucim-doprinosima-na-zaradu.html),
  which gives each code its meaning, form position by form position;
- the ePorezi portal's own entry form, which is what a taxpayer sees.

Relevant meanings, position by position: **1.1 Врста пријаве** — `1` општа
пријава, filed on or before the due date; `3` пријава по члану 182б ЗПППА, filed
after it; `4` по налогу контроле and `5` по одлуци суда, neither of which the
application produces. **1.5 Измена пријаве** — `1` измена по члану 40. ЗПППА, the
voluntary correction, which is what an amendment carries; `2` по налогу контроле
and `9` сторно, neither of which the application produces. **1.5a
Идентификациони број пријаве** — the PURS number of the declaration being changed.

The three sources diverge on codes the application never writes: the portal offers
a code under 1.1 that the schema rejects, and the schema accepts codes that neither
the rulebook nor the portal lists. This divergence is known and does not affect the
application, whose entire output uses codes confirmed by all three.

**1.1 and 1.5 are independent, and this is settled — do not re-derive it.** Each
has its own codebook; neither codebook references the other; no value in one
restricts, defaults or excludes any value in the other. An amendment filed after
the deadline carries `3` under 1.1 and `1` under 1.5 at the same time. In
particular, position 1.4a lists situations for deciding whether to fill 1.4a
itself — general returns, returns under article 182б, amendments under article 40,
and others. That list is not a taxonomy of 1.1 values and implies no relationship
between the two fields. Reading one into it is a mistake that has been made and
corrected.
