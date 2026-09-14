#!/bin/sh
# Phase 3 budgets. Bundle size is a hard CI gate. Launch-to-window is
# printed always. 400 ms is a hard gate on developer hardware only. CI
# records launch_ms and painted; it does not fail on them. Do not raise 400.
# Idle CPU and RSS are the same script, same process, after warmup.
# Referenced tracks are library rows with tracks.storage = 'referenced'.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
export MACOSX_DEPLOYMENT_TARGET=26.0
"$root/shells/macos/build-xcframework.sh"
swift build --package-path "$root/shells/macos" -c release --product LLaMP
bin="$root/shells/macos/.build/release/LLaMP"
app="$root/shells/macos/.build/release/LLaMP.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$bin" "$app/Contents/MacOS/LLaMP"
# Product bundle id is unset until the naming gate. This directory is the size
# artifact, not a notarized bundle.
bytes=$(du -sk "$app" | awk '{print $1 * 1024}')
echo "bundle_bytes $bytes"
if [ "$bytes" -gt $((30 * 1024 * 1024)) ]; then
  echo "bundle size exceeds 30 MiB" >&2
  exit 1
fi
python3 - "$bin" << 'PY'
import os, subprocess, sys, time
bin = sys.argv[1]
start = time.perf_counter()
proc = subprocess.run([bin, "--paint-and-exit"], capture_output=True, text=True, timeout=10)
elapsed_ms = (time.perf_counter() - start) * 1000
print(proc.stdout.strip())
print(f"launch_ms {elapsed_ms:.1f}")
if proc.returncode != 0:
    print(proc.stderr, file=sys.stderr)
    raise SystemExit(proc.returncode)
if "painted" not in proc.stdout:
    raise SystemExit("window did not report painted")
# 400 ms is developer hardware. Do not raise it.
# macos-26 wall times for the same --paint-and-exit path, same day:
# 444.2 / 764.5 / 1567.2. The last two shared bundle_bytes 6152192.
# The runner is not the 400 ms clock. CI records; it does not gate.
if os.environ.get("GITHUB_ACTIONS") == "true":
    print("launch_ms_ci_gate none")
    print("macos-26 samples 444.2 764.5 1567.2 bundle_bytes 6152192")
else:
    if elapsed_ms > 400:
        raise SystemExit(f"launch-to-window {elapsed_ms:.1f} ms exceeds 400")
PY
scene=$(mktemp -d)
trap 'rm -rf "$scene"' EXIT
python3 - "$scene/tone.wav" "$scene/refs" << 'PY'
import math, os, struct, sys, wave
wav_path, refs = sys.argv[1], sys.argv[2]
os.makedirs(refs)
rate = 44100
n = rate * 2
with wave.open(wav_path, "w") as handle:
    handle.setnchannels(2)
    handle.setsampwidth(2)
    handle.setframerate(rate)
    frames = bytearray()
    for i in range(n):
        sample = int(16000 * math.sin(2 * math.pi * 440 * i / rate))
        frames += struct.pack("<hh", sample, sample)
    handle.writeframes(frames)
for i in range(1000):
    open(os.path.join(refs, f"track-{i:04d}"), "wb").close()
PY
wsz="$scene/fixture.wsz"
cargo run --quiet --manifest-path "$root/xtask/Cargo.toml" -- emit "$wsz"
python3 - "$bin" "$wsz" "$scene/tone.wav" "$scene/refs" << 'PY'
import select, subprocess, sys, time
bin, wsz, track, refs = sys.argv[1:5]
proc = subprocess.Popen([bin, "--budget-scene", wsz, track, refs], stdout=subprocess.PIPE, text=True)
deadline = time.perf_counter() + 20
ready = ""
while time.perf_counter() < deadline:
    readable, _, _ = select.select([proc.stdout], [], [], 0.2)
    if not readable:
        if proc.poll() is not None:
            raise SystemExit(f"budget process exited {proc.returncode}")
        continue
    line = proc.stdout.readline()
    if line.startswith("budget-ready"):
        ready = line.strip()
        break
if "refs=" not in ready:
    proc.kill()
    raise SystemExit(f"budget scene did not report references: {ready!r}")
count = int(ready.split("refs=", 1)[1])
if count < 1000:
    proc.kill()
    raise SystemExit(f"referenced tracks {count} < 1000")
# Spec says average after warmup and does not name the warmup. Five seconds
# lets the stream and the first blits settle. It is not a player constant.
time.sleep(5)
samples = []
for _ in range(30):
    out = subprocess.check_output(["ps", "-o", "rss=,%cpu=", "-p", str(proc.pid)], text=True).split()
    samples.append((int(out[0]), float(out[1])))
    time.sleep(1)
proc.kill()
rss = [s[0] for s in samples]
cpu = [s[1] for s in samples]
rss_max = max(rss)
cpu_mean = sum(cpu) / len(cpu)
print(f"idle_rss_kib_max {rss_max}")
print(f"idle_cpu_percent_mean {cpu_mean:.1f}")
print(f"idle_cpu_percent_samples {' '.join(f'{c:.1f}' for c in cpu)}")
print("scene window-visible track-playing oscilloscope-on other-windows-closed tracks.storage=referenced n=1000")
# 80 MiB. ps RSS is KiB. Do not raise this.
if rss_max > 80 * 1024:
    raise SystemExit(f"idle RSS {rss_max} KiB exceeds 80 MiB")
# 5% of one P-core. ps %cpu is percent of one core. Do not raise this.
if cpu_mean > 5.0:
    raise SystemExit(f"idle CPU {cpu_mean:.1f}% exceeds 5% of one P-core")
PY
