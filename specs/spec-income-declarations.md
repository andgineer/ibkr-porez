# Income Declarations (PP-OPO)

Dividends and broker interest received from abroad are declared to PURS on a
PP-OPO form, one form per income group, within 30 days of the income date. Tax
withheld abroad is credited against the 15% due in Serbia.

See `spec-transaction-sources.md` for where the underlying transactions come
from and why withholding tax keeps changing after the fact.

## What one declaration covers

An income group is all income of one security (for dividends) or one currency
(for interest) received on one date, of one income kind. One group is one
declaration. A group is identified by those three things and by nothing else —
not by the name of the file it produced, which is free to change.

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

The second rule is not dead weight. The broker's identifier, though present on
every dividend and every dividend withholding row observed, is not listed in the
published Flex Query field reference and carries no stability promise. Should it
ever stop being emitted, matching by description keeps working silently and
correctly.

Amounts within one distribution are summed **with their sign**, so a reversal
cancels what it reverses. This holds on the income side as well as the tax side: a
reversed dividend cancels its original rather than counting twice.

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

## When a declaration must wait

A group for which no withholding tax was found at all is held for a bounded wait,
then declared without a foreign-tax credit — the full 15% is declared as due,
which is fiscally safe. The wait exists because tax usually posts within days of
the income, and the deadline leaves ample room for it.

A group whose withholding was found and nets to zero does **not** wait. Zero is
its answer, not a gap: the tax was withheld and reversed. This is the normal shape
of an interest-related dividend under IRC §871(k), and treating it as "not arrived
yet" would delay the most common case for nothing.

A group missing an exchange rate is reported and retried on the next sync.

## When a declaration is amended

When the amounts underlying an already-declared group change, an amendment
(измењена пријава) is generated. It is an ordinary declaration in every respect —
same list, same output directory, same submission and payment handling — marked as
amending the original.

The comparison is made in the income's own currency, never in dinars: a moved
exchange rate must not look like a change in income.

How far back the app looks for such changes is set by how long withholding can
still change, not by the window that governs creation. Since a distribution of one
tax year can be corrected until the following March, the app examines income from
the start of the previous calendar year — or from the start of an explicitly
widened creation window, whichever reaches further back. A shorter horizon would
miss the annual reclassification entirely, which is the very event amendments
exist for.

Declarations created before this rule existed carry no record of their source
amounts and are therefore never amended.

## Accepted limitations

- An amendment is generated, not filed. The taxpayer submits it, and records the
  original's PURS number so the amendment can reference it.
- A withholding reversal for income that predates this rule is ignored: the
  original income carries no distribution identifier while the reversal does, so
  the two do not meet. Income of that age was declared as it arrived, and its
  declaration stands.
- Income imported from CSV is never declared. See `spec-transaction-sources.md`.
- If distribution matching ever breaks entirely, groups see no tax and are
  declared with no credit — the taxpayer overpays. This is deliberate: it errs
  toward overpaying rather than underpaying, and it announces itself, because
  declarations that were always zero suddenly demand payment. The app carries no
  machinery to detect a broken match.
