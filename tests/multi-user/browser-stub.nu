#!/usr/bin/env nu
# tests/multi-user/browser-stub.nu
#
# A stand-in "web browser" for the multi-user viewer tests, wired in through the
# `TPNOTE_BROWSER` environment variable. Tp-Note's viewer blocks in
# `launch_web_browser()` until the browser it spawned exits, and then shuts the
# viewer down. This stub therefore just sleeps: it keeps the viewer alive and
# serving for the duration of a test, and when the driver kills this process
# Tp-Note's `launch_web_browser()` returns and the viewer exits cleanly on its
# own (no orphaned HTTP server left listening).
#
# Tp-Note appends the viewer URL as a final argument; we ignore it. The 5-minute
# sleep is only a safety ceiling — the driver kills this stub as soon as a test
# case finishes.

sleep 5min
