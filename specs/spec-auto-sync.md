# Background Auto-Sync (GUI)

## Why

IBKR Flex Query frequently returns transient errors ("statement could not be
generated", "statement generation in progress"). Users who hit these don't
understand that the fix is simply "try again later," and may believe the app
is broken.

## Behavior

While the GUI is running, the app retries syncing automatically in the
background: once a day, after the US market close (when the prior trading
day's Flex Query report becomes available), it keeps retrying until it
succeeds (see Retry schedule). The user always sees, in a permanent status
line, when the last successful sync was and whether a later attempt ran into
an issue.

This is GUI-only; the CLI remains a one-shot command, unaffected.

A failed IBKR fetch never blocks declaration generation: `sync` still generates
declarations from the transactions already stored locally and records the fetch
failure as an issue, so the daily cycle keeps retrying for fresh data instead of
counting the run as a clean success. This resilience is part of every `sync`
(CLI and GUI); only the automatic retry loop is GUI-only.

## Trigger condition

The cycle starts once there is no successful sync recorded for the current
local day *and* the report for the previous US trading day can plausibly
exist.

Availability is anchored to New York rather than to the user's timezone: IBKR
processes the trading day overnight, and a Flex Query for it becomes
answerable in the small hours New York time. The cycle therefore makes no
attempt before 01:00 in New York on the current local date — in Belgrade that
falls mid-morning, hours after the local day has flipped. Attempting earlier
buys nothing: the answer is "not ready", and a run of those is actively
harmful (see Retry schedule).

## Retry schedule

A single persistent background timer ticks hourly while there is no
successful sync recorded for the current day.

- Transient errors (Flex Query not ready yet, network issues) are retried on
  every tick until they succeed.
- Errors that indicate a configuration problem (expired/invalid token,
  invalid query ID, and similar) are not retried hourly, since repeating the
  same request would just fail again. Such an error is retried at most once
  a day (the cycle self-heals automatically across midnight, even if the app
  stays open). The user can also force an immediate retry at any time with
  "Sync now".
- IBKR counts failed attempts and blocks further requests once they pile up,
  answering "too many failed attempts" regardless of whether the credentials
  are correct. This block is temporary and clears on its own, so it counts as
  transient and keeps the hourly cycle running — despite IBKR's own wording
  pointing at the configuration. Within a single attempt the app gives up
  immediately instead of re-polling, so it does not extend the block.

Because that counter is shared across everything using the same credentials
and network, nothing automated may exercise the live Flex endpoint — a test
suite that fetches with throwaway credentials will lock out the real user.

## Configuration gate

If the IBKR configuration is incomplete or invalid, the auto-cycle does not
run. A permanent status-line banner informs the user that configuration is
required, with a clickable link that opens the Config dialog directly. Saving
a valid configuration does not by itself trigger a sync —
the next hourly tick picks it up automatically (within an hour), or the user
can use "Sync now" for an immediate attempt. Triggering a sync manually with
an invalid configuration still shows the configuration-validation error as
before.

## Manual vs. automatic sync

There is no distinction between a manual and an automatic sync attempt — the
"Sync now" button simply forces an immediate attempt. Every result, whatever
triggered it, is handled identically: no modal error dialogs and no transient
"sync complete" messages, only the permanent status line and a dismissible
"new declarations" banner are updated.

Transient IBKR errors (the "statement generation in progress" family, the
"too many failed attempts" block) and network connectivity errors are shown
with friendly wording rather than the
raw error text. Errors that indicate a configuration problem (invalid token,
expired token, invalid query ID, and similar) are shown with their original
IBKR error text. This phrasing also holds up once the user has since changed
the configuration: the message still correctly describes a past attempt and
the action needed, rather than appearing to comment on the just-saved
configuration.

Every error is retried automatically at least once a day — there is no
"won't retry" outcome. The status line reflects this honestly:

- If the daily cycle hasn't succeeded yet today and the error is transient
  (Flex Query not ready, network issues), it is retried every hour, and the
  message says so ("retrying automatically").
- Otherwise — either the error is a configuration problem (retried only
  after local midnight, not hourly), or today's sync already succeeded and
  this was an extra manual attempt (the daily cycle is done for today) — the
  message names tomorrow as the next automatic attempt and points to "Sync
  now" for an immediate retry.

## Persistence

The timestamp of the last successful sync, the timestamp and message of the
most recent issue (covering both fetch failures and income notices from an
otherwise-successful sync), and the count of declarations created since the
user last dismissed the banner all survive app restarts.

Income notices reuse that same status line, counted rather than listed, and
are recomputed from scratch on every sync: a notice disappears as soon as its
cause does, and none of them is ever a one-time signal that has to be
dismissed. The detail behind the count is in the CLI's `sync` output.

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
