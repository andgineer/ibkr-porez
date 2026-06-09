# Background Auto-Sync (GUI)

## Why

IBKR Flex Query frequently returns transient errors ("statement could not be
generated", "statement generation in progress"). Users who hit these don't
understand that the fix is simply "try again later," and may believe the app
is broken.

## Behavior

While the GUI is running, the app retries syncing automatically in the
background: once a day, after the US market close (when the prior trading
day's Flex Query report becomes available), it keeps trying — with backoff —
until it succeeds. The user always sees, in a permanent status line, when the
last successful sync was and whether a later attempt ran into an issue.

This is GUI-only; the CLI remains a one-shot command, unaffected.

## Trigger condition

The cycle starts once there is no successful sync recorded for the current
local day. Trading on the US exchange ends around 22:00–23:00 Belgrade time
depending on DST, so before local midnight a Flex Query for "yesterday" is
rarely ready — the cycle simply does not start before midnight, since that is
automatically true whenever today's date differs from the date of the last
successful sync.

## Retry schedule

On failure, the next attempt is scheduled one hour later, repeating hourly
until the sync succeeds.

## Configuration gate

If the IBKR configuration is incomplete or invalid, the auto-cycle does not
run at all — no attempts, no backoff, no status-line noise. Triggering a sync
manually still shows the configuration-validation dialog as before.

## Manual vs. automatic sync

There is no distinction between a manual and an automatic sync attempt — the
"Sync now" button simply forces an immediate attempt. Every result, whatever
triggered it, is handled identically: no modal error dialogs and no transient
"sync complete" messages, only the permanent status line and a dismissible
"new declarations" banner are updated.

Transient IBKR errors (the "statement generation in progress" family) are
shown with friendly wording rather than the raw IBKR error text, since the app
retries them automatically without the user needing to do anything.

## Persistence

The timestamp of the last successful sync, the timestamp and message of the
most recent issue (covering both fetch failures and tax-calculation issues
from an otherwise-successful sync), and the count of declarations created
since the user last dismissed the banner all survive app restarts.

## New-declarations banner and notification

When a sync creates new declarations, a dismissible banner shows the
accumulated count since the user last closed it; dismissing it resets the
count to zero, and it starts accumulating again from the next sync that
creates declarations. The banner is the reliable, persistent indicator —
it survives restarts and is always visible.

In addition, a best-effort OS desktop notification fires immediately when new
declarations are created, as a heads-up for when the app is in the
background. Notification failures (e.g. no notification daemon available) are
silently ignored — the banner remains the source of truth.

## Out of scope

- CLI daemon mode or any periodic behavior outside the GUI.
- Configurable schedule or backoff parameters — fixed values are enough for
  this use case; revisit only if real-world timing needs tuning.
