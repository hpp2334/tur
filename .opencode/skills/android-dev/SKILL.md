---
name: android-dev
description: Use when building, signing, installing, or debugging the tur Android app on a physical device or emulator. Covers `tur-android` / `demo/compose`, the `cargo ndk` + `gradlew assembleRelease` build, the unsigned-APK debug-sign flow (`apksigner`, `INSTALL_PARSE_FAILED_NO_CERTIFICATES`), and the macOS Sequoia `adb` local-network block workaround. Triggers: Android device, Mi 11 / M2011K2G, `adb connect`, `adb pair`, wireless debugging, `device offline`, `No route to host`, `libtur_android`.
---

# tur Android on-device debug

Everything needed to get the tur playground running on an Android device and to
keep `adb` talking to it from macOS. The native side is `libs/tur-android`
(cdylib `libtur_android`); the shell is `demo/compose` (Kotlin/Compose); the
Compose integration is `integrations/compose`.

Environment: Android cmdline-tools at `/usr/local/share/android-commandlinetools`,
NDK `27.0.12077973`, JDK 17 at `/usr/local/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home`
(set `JAVA_HOME`). adb at `…/platform-tools/adb`. `cargo-ndk` must be installed.

## Build

Build the Rust cdylib (per ABI) + the demo APK:

```sh
cargo ndk -t arm64-v8a build --release -p tur-android   # also: -t x86_64 for emulator
cd demo/compose && JAVA_HOME=/usr/local/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
    ANDROID_HOME=/usr/local/share/android-commandlinetools \
    ANDROID_NDK_HOME=/usr/local/share/android-commandlinetools/ndk/27.0.12077973 \
    ANDROID_NDK_ROOT=$ANDROID_NDK_HOME ./gradlew assembleRelease
# → demo/compose/build/outputs/apk/release/tur-android-demo-release-unsigned.apk
```

## Sign the release APK (it ships unsigned)

The release APK is **unsigned** — modern Android rejects it
(`INSTALL_PARSE_FAILED_NO_CERTIFICATES`). Sign it with the debug keystore
before installing (`apksigner` needs `JAVA_HOME`):

```sh
BT=/usr/local/share/android-commandlinetools/build-tools/35.0.0
SRC=demo/compose/build/outputs/apk/release/tur-android-demo-release-unsigned.apk
$BT/zipalign -p -f 4 "$SRC" /tmp/tur-signed.apk
JAVA_HOME=/usr/local/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home $BT/apksigner sign \
    --ks ~/.android/debug.keystore --ks-pass pass:android \
    --ks-key-alias androiddebugkey --key-pass pass:android /tmp/tur-signed.apk
adb -s <device> install -r -t /tmp/tur-signed.apk
adb -s <device> shell monkey -p ai.tur.demo -c android.intent.category.LAUNCHER 1   # launch
```

## adb over wireless + the macOS Sequoia local-network block

On Android 11+ the device's **Wireless debugging** exposes a *connect* port
(shown on the main wireless-debugging screen) plus a separate short-lived
*pairing* port (shown under "Pair device with pairing code"). Pair once with
`adb pair <host> <pair_port> <code>`, then `adb connect <host> <connect_port>`.

**macOS Sequoia silently blocks the `adb` binary from the local network.**
Symptom: `nc` (`/usr/bin/nc`) and `python3` (`/usr/bin/python3`) — both
Apple-signed — connect to the device fine, but `adb connect` fails instantly
with `No route to host` (`EHOSTUNREACH`). This is *not* a firewall (built-in
firewall off, no Little Snitch/LuLu) and *not* a TCC entry you can reset — the
Local Network DB is empty and `tccutil reset LocalNetwork` errors. Cause: adb
makes the LAN connection from a **background daemon** (the adb server), and
Sequoia suppresses the permission prompt for daemons, silently denying
third-party (non-Apple-signed) binaries. Apple-signed binaries are
pre-approved, so they reach the LAN.

### Workaround: bridge adb through a localhost proxy run by Apple-signed `python3`

Loopback has no LAN gate; `python3` does the actual LAN hop. Save as
`/tmp/adb_proxy.py` — the proper `SHUT_WR` half-close + thread join are
essential. A naive proxy forces `SHUT_RDWR`, which drops adb's persistent
transport and marks the device `offline`:

```python
import socket, threading, sys
local_port, target_host, target_port = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])
def forward(src, dst):
    try:
        while True:
            d = src.recv(65536)
            if not d: break
            dst.sendall(d)
    except Exception: pass
    finally:
        try: dst.shutdown(socket.SHUT_WR)
        except Exception: pass
def handle(client):
    try: up = socket.create_connection((target_host, target_port), timeout=5)
    except Exception as e: print(f"upstream fail: {e}"); client.close(); return
    t = threading.Thread(target=forward, args=(client, up), daemon=True); t.start()
    forward(up, client); t.join(timeout=3)
    for s in (client, up):
        try: s.close()
        except Exception: pass
srv = socket.socket(); srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", local_port)); srv.listen(16)
print(f"proxy 127.0.0.1:{local_port} -> {target_host}:{target_port}", flush=True)
while True:
    c, _ = srv.accept(); threading.Thread(target=handle, args=(c,), daemon=True).start()
```

Then:

```sh
nohup /usr/bin/python3 /tmp/adb_proxy.py 7777 192.168.124.36 40011 >/tmp/adb_proxy.log 2>&1 &
adb connect 127.0.0.1:7777          # adb talks to loopback; python3 does the LAN hop
adb -s 127.0.0.1:7777 devices -l     # Xiaomi Mi 11 (venus / M2011K2G) shows as a real device
```

- The proxy must keep running for the adb session. The app itself runs
  on-device regardless, so a dropped adb session does not affect a running app.
- If `adb` reports `device offline` (transport hiccup):
  `adb disconnect 127.0.0.1:7777 && adb connect 127.0.0.1:7777`.
- If the device's connect port changed (wireless debugging was toggled),
  re-check the port on the device screen and point the proxy at the new one.

## Reading engine state on-device

The engine exposes a dev-tool bridge reachable via JS evaluation / the host.
From the device, the element tree is queryable through `turDevTool` (mirrors
the playground's web path) — useful for confirming hit-test rects and whether
`onPointerDown` fired without needing the gesture subsystem's `tracing::info!`
logs (which may not be wired to logcat). Grab a logcat dump with
`adb -s <device> logcat -d -v time --pid=$(adb -s <device> shell pidof ai.tur.demo)`.
