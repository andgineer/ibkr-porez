---
name: run-app
description: Build and drive the ibkr-porez CLI and GUI against a throwaway config/data dir, with IBKR stubbed. Use when asked to run, start, or screenshot the app, or to confirm a change works in the real app rather than only in tests.
---

# Running ibkr-porez

Two binaries come out of one build: `ibkr-porez` (CLI) and `ibkr-porez-gui`
(egui/eframe native window, behind the `gui` feature).

```bash
cargo build --release --features gui   # ~2 min cold; both binaries
```

Drop `--release` for a faster loop; the GUI is noticeably smoother when
optimized.

## Never run against the user's real state

Both binaries default to the real config dir, and the real config points at
the real declaration store. `IBKR_POREZ_CONFIG_DIR` moves config, data and
logs at once (logs live at `<config_dir>/logs`, data wherever `data_dir`
says), so set it and write a `config.json` whose `data_dir` also points into
the temp tree.

Three outbound services must be redirected too, or a run reaches them for
real:

| Variable | Service |
|---|---|
| `IBKR_POREZ_FLEX_URL` | IBKR Flex Query |
| `IBKR_POREZ_NBS_URL` | exchange rates |
| `IBKR_POREZ_HOLIDAYS_URL` | holiday calendar |

IBKR is the one that bites: it counts failed attempts and answers error 1025
— "too many failed attempts" — for the whole IP once they pile up, which then
breaks the user's real nightly sync. Point unused services at `http://127.0.0.1:1`
(connection refused is instant; a black-hole address would stall on timeouts).

## Setup

```bash
S=$(mktemp -d)
mkdir -p "$S/cfg" "$S/data"
cat > "$S/cfg/config.json" <<EOF
{
  "ibkr_token": "stub-token",
  "ibkr_query_id": "stub-query",
  "personal_id": "1234567890123",
  "full_name": "Probni Korisnik",
  "address": "Knez Mihailova 1",
  "city_code": "223",
  "phone": "0641234567",
  "email": "probni@example.com",
  "data_dir": "$S/data",
  "output_folder": null
}
EOF
export IBKR_POREZ_CONFIG_DIR=$S/cfg
export IBKR_POREZ_NBS_URL=http://127.0.0.1:1
export IBKR_POREZ_HOLIDAYS_URL=http://127.0.0.1:1
```

The config must be complete: an incomplete one makes the GUI show a
"Not configured" banner and skip the auto-sync cycle entirely, so nothing
interesting happens.

## Stubbing IBKR

To exercise a fetch, serve the XML yourself. Vary `ErrorCode` to pick the
outcome (1001 = not ready yet, 1025 = lockout, or a `<Status>Success</Status>`
body with a `ReferenceCode` to drive the happy path, which then needs a
`/GetStatement` route as well).

```python
# flex_stub.py — python3 flex_stub.py 8731
import http.server, socketserver, sys, datetime

BODY = (b"<FlexStatementResponse><Status>Fail</Status>"
        b"<ErrorCode>1025</ErrorCode>"
        b"<ErrorMessage>Too many failed attempts. Please review your configuration.</ErrorMessage>"
        b"</FlexStatementResponse>")

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(f"{datetime.datetime.now():%H:%M:%S} hit {self.path.split('?')[0]}", flush=True)
        self.send_response(200)
        self.send_header("Content-Type", "text/xml")
        self.send_header("Content-Length", str(len(BODY)))
        self.end_headers()
        self.wfile.write(BODY)
    def log_message(self, *a): pass

socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("127.0.0.1", int(sys.argv[1])), Handler) as httpd:
    print(f"stub listening on {sys.argv[1]}", flush=True)
    httpd.serve_forever()
```

Run it in the background, then `export IBKR_POREZ_FLEX_URL=http://127.0.0.1:8731`.
Its hit log doubles as an assertion: it shows how many requests one sync
actually made.

## Driving the CLI

`tests/resources/complex_activity.csv` is a real IBKR activity export — good
seed data.

```bash
B=target/release/ibkr-porez
$B import tests/resources/complex_activity.csv   # → "Parsed 4 transactions."
$B stat                                          # → monthly breakdown table
$B list
$B sync; echo "exit=$?"                          # → exercises the Flex fetch
```

A failed fetch is not an error exit: `sync` reports the failure, still
generates declarations from stored transactions, and exits 0.

## Driving the GUI

```bash
target/release/ibkr-porez-gui &   # run in background; it holds the terminal
```

The window opens without any bundling. On startup it fires one auto-sync tick
immediately, so a stub hit right after launch is the expected signal that the
sync path ran.

That tick only fires inside the daily window — no attempt before 01:00 in New
York on the current local date (mid-morning in Belgrade). Launching before
that, the GUI legitimately sits idle; use the "Sync now" button, which ignores
the window.

Screenshot it — a native window, so `screencapture` after raising it:

```bash
osascript -e 'tell application "System Events" to set frontmost of first process whose name contains "ibkr" to true'
sleep 2
screencapture -x -o "$S/gui.png"
```

`screencapture` grabs the whole screen. Crop to the app before reading it,
or the status line is unreadable at scale:

```bash
sips -c 120 1200 --cropOffset 200 420 "$S/gui.png" --out "$S/status.png"
```

Adjust the crop to the window; the sync status line sits just under the
toolbar. **Look at the image** — a blank frame means the window never
rendered.

Clean up with `pkill -f ibkr-porez-gui` and `pkill -f flex_stub.py`; both then
report exit 144 (SIGTERM), which is not a failure.

## Verifying isolation

After any run, confirm nothing escaped:

```bash
ls "$S/cfg/logs"                                              # entries here
ls -l ~/Library/"Application Support"/ibkr-porez/logs/        # unchanged
```

## Real credentials

When a run genuinely needs the live IBKR endpoint, copy the user's real
config but rewrite `data_dir` into the temp tree, and leave
`IBKR_POREZ_FLEX_URL` unset. That uses the real token while keeping the real
declaration store untouched. Do this sparingly — every failed attempt feeds
the 1025 counter that breaks the user's nightly sync.
