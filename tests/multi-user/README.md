# `tests/multi-user`

End-to-end tests for the viewer's OS-user access control
(`viewer.same_user_policy`, the peer-UID check) that require a **second real OS
user**. The in-crate integration tests (`cargo test`, in
`tpnote/src/viewer/sse_server.rs`) can only drive a same-process loopback client,
which always resolves to the *same* user. Proving that a **foreign** user is
refused needs a genuinely different account, which cannot be arranged inside a
single process or in ordinary CI — hence this separate suite.

All scripts here are [Nushell](https://www.nushell.sh/) scripts.

## What it checks

Viewer started as the current user; a client connects as the current user
(control) and as the foreign user over SSH:

| `same_user_policy` | current user | foreign user |
| --- | --- | --- |
| `"Reject"` (default) | `200` | **`403`** + peer-user-unknown page (offering the `Off` remedy) |
| `"Off"` | `200` | `200` (check disabled) |

On Linux a foreign peer resolves to **`Unknown`** (a non-root viewer cannot read
another user's `/proc/<pid>/fd`), so `Reject` refuses it via the fail-closed
path. This is the live counterpart of that finding. (There is no `"Warn"`: it was
removed because it fail-opens on `Unknown` and therefore serves foreign users.)

## Prerequisites

1. **A second OS account** to act as the foreign user — any name will do. The
   scripts default to `tpnote-peer-test`; override with
   `$env.TPNOTE_TEST_FOREIGN_USER`. (Examples below use `tpnote-peer-test`;
   substitute your account.)
2. **Passwordless SSH** from the current user to that account on `localhost`:
   ```nu
   ssh-copy-id -i ~/.ssh/id_ed25519.pub tpnote-peer-test@localhost   # once; asks that account's password
   ssh -o BatchMode=yes -l tpnote-peer-test localhost id             # must print uid=... with no prompt
   ```
3. `tpnote` built (`cargo build -p tpnote`); the driver builds it if missing.

No HTTP client needs installing: requests use Nushell's built-in `http get`.
The foreign-user request runs `http get` on the far side of `ssh` using the
**same Nushell binary** (`$nu.current-exe`), which the second user can execute
because it is the same machine — so nothing extra is required for that account.

If the account is not reachable the suite **skips cleanly** (prints `SKIP`,
exits `0`) — it never fails for a missing prerequisite.

## Running

```nu
./run-tests
```

Environment overrides:

- `TPNOTE_TEST_FOREIGN_USER` — the foreign account (default `tpnote-peer-test`).
- `TPNOTE_TEST_PORT` — base TCP port (default `28390`; case 2 uses `+1`).

Exit status: `0` all-passed or skipped, `1` a check failed.

## Encoded gotchas

These were discovered while validating the feature by hand and are baked into the
scripts:

- **The viewer binds to one loopback family** — IPv6 `[::1]` on some hosts,
  IPv4 `127.0.0.1` on others (whichever `"localhost"` resolves to first).
  Nushell's `http get` cannot use the IPv6 *literal* `http://[::1]:PORT/` (it
  treats `[::1]` as a hostname), so the driver connects via `localhost` for the
  IPv6 case and by literal for the IPv4 case. It detects the family by probing
  the IPv4 literal (`127.0.0.1`) first — if that answers the viewer is IPv4,
  otherwise it is IPv6 — and prints which. All requests carry `--max-time`.
- **Keeping the viewer alive headless:** `--view` (viewer, no editor) plus
  `TPNOTE_BROWSER=browser-stub.nu`, a script that just sleeps. Tp-Note blocks
  until that "browser" exits, so killing the stub makes the viewer exit cleanly —
  `run-tests` stops each case by killing the stub (via `pgrep -f browser-stub.nu`),
  which avoids orphaned servers.
- **Policy selection:** a one-line `TPNOTE_CONFIG` file (`[viewer]`
  `same_user_policy = "…"`); Tp-Note merges it over the built-in defaults. That
  config also sets `session_binding_cookie = false` so this suite isolates the
  peer-UID check — otherwise the `wait-listening` probe would bind the session
  and a later cookie-less request would be refused for the wrong reason. (The
  cookie binding is covered by the in-crate integration tests.)
- **Tp-Note renames the note** to match its title, so each viewer gets a fresh
  temp note.

## Not covered here

- The proven-**foreign** (`Other`) branch and its `peer_user_mismatch_page`:
  unreachable on Linux (it needs a root-capable resolver, but Tp-Note refuses to
  run the viewer as root). Exercise on macOS/Windows instead.
- The session-cookie binding / `Host` checks: covered by the in-crate integration
  tests (`cargo test`).

## Files

- `run-tests` — the Nushell driver (executable).
- `browser-stub.nu` — the sleeping stand-in browser used via `TPNOTE_BROWSER`.
