# Audio pipeline

Tier A only. If `produces_pcm` is false, this graph does not run. See [providers](providers.md).

Threading rules are in [ARCHITECTURE.md](../ARCHITECTURE.md). The callback does not allocate, lock, or do I/O.

## Formats

Internal format: interleaved stereo `f32`, nominal full scale ±1. Values outside ±1 may exist before the limiter. Channels other than stereo are downmixed or upmixed to stereo before resample. Mono is duplicated. More than two channels are folded to stereo with equal power on the extra channels and a documented gain trim so a 5.1 file is not quieter by accident and not clipped by summing. The trim constants are fixed in a phase 1 test, not guessed in a comment later.

Sample-rate conversion happens once, from the decoded rate to the **device** rate, on the decode thread, with `rubato` 5. There is no second resampler in the callback. If the device rate changes, the stream is rebuilt, the resampler is created at the new ratio, and the ring is drained of stale-rate frames (those frames are dropped, an underrun may be counted once). We do not play 44.1 kHz frames into a 48 kHz stream.

| Codec | Decoder | Gapless in 1.0 |
| --- | --- | --- |
| MP3 | `symphonia` 0.6, feature `mp3` | Only if the phase 1 investigation shows LAME/Xing encoder delay and padding are surfaced. Otherwise no. Do not guess the delay. |
| AAC-LC | `symphonia`, feature `aac` | No. Status is “Great”, not gapless. Do not promise it. |
| HE-AAC | macOS AudioToolbox decoder plugin, not linked into the portable core | Best effort. Not a core promise. |
| FLAC | `symphonia` | Yes |
| ALAC | `symphonia`, feature `alac` | Yes |
| WAV, AIFF | `symphonia` | Yes (PCM) |
| Vorbis | `symphonia` | Yes |
| Opus | `symphonia-adapter-libopus` (`libopus`, BSD-3) | Yes, pre-skip from the container |

Native Opus in `symphonia` is not production-ready (SILK-only work in progress as of the 0.6 line). Do not flip the feature on and call it done.

`fdk-aac` is not a dependency. FFmpeg is not a dependency. See [LEGAL.md](../LEGAL.md).

Tracker modules (MOD, S3M, XM, IT) are not in this graph. An optional `libopenmpt` plugin is post-1.0.

## Buffers

Sizes are in stereo frames unless noted.

| Buffer | Capacity | Producer | Consumer |
| --- | --- | --- | --- |
| Audio ring (`rtrb`) | 1 second at the device rate, allocated before the stream starts | Decode thread | Audio callback |
| Analysis ring (`rtrb`) | 4 FFT hops, allocated before the stream starts | Audio callback, `try_push` only | Analysis thread |
| Device period | Whatever CoreAudio requests. We do not demand a period below 128 or above 4096 frames. If the device offers only outside that, we accept it and record it. | CoreAudio | Our callback |

The callback processes exactly the frame count it was given. It does not grow a buffer to “catch up.” An empty ring writes silence and increments an atomic underrun count.

Decode reads from the source in chunks of at most 64 KiB of compressed bytes per wake, into a buffer allocated when the track opens, reused until the track closes. That bound is so a corrupt length cannot ask for a gigabyte. It is not a realtime-thread rule; decode may allocate when a track opens.

## Graph

In order, in the callback, in place on the popped frames:

1. ReplayGain gain (scalar already computed).
2. User preamp (scalar, slewed).
3. 10-band biquad cascade.
4. First-party DSP chain, each plugin in declared order.
5. Limiter.
6. TPDF dither, only if the device format is integer. Float devices, including the normal CoreAudio path, are not dithered.

Several bands at +12 dB may exceed ±1 after the preamp and the cascade. Boost is allowed to exceed ±1 there. The limiter, not the preamp, brings the peak to −1.0 dBFS.

DSP plugins that are not first-party native do not run here in 1.0. See [ADR 004](../adr/004-plugin-runtime.md).

### EQ

Bands, center frequencies, range ±12 dB: 60, 170, 310, 600, 1000, 3000, 6000, 12000, 14000, 16000 Hz.

Each band is a peaking biquad, RBJ cookbook, sample rate equal to the device rate. Q is proportional to the spacing to the neighboring centers so the bands overlap instead of poking holes. The numeric Q for each band is fixed in a phase 1 test vector (impulse in, magnitude at the center within 0.5 dB of the slider). Do not copy coefficients from another player.

On and off: when off, the cascade is bypassed (coefficients unused, signal copied). Bypass is not “all sliders at 0” left in the signal path, because a 0 dB biquad is still a filter.

**AUTO.** Published notes disagree enough that this spec will not assign a behavior. Phase 4 investigates from published Winamp behavior descriptions, not from source. Until that note lands in `docs/investigations/eq-auto.md`, the AUTO button is visible, does not change audio, and its accessibility label says the action is not available yet. Shipping a guessed auto-preamp is worse than a disabled button.

### Zipper noise

Slider moves do not write coefficients into the live filter from the UI thread.

The UI stores a target gain per band (atomic `f32` bits). The callback slews the applied gain toward the target with a one-pole coefficient whose time constant is about 20 ms. Coefficient redesign runs on the callback only when the slewed gain has moved by more than 0.01 dB, using stack memory. Two states are not allocated.

A preset load may jump the targets. The slew still applies. A preset load must not click.

### Limiter

A feed-forward limiter after DSP, lookahead of 64 samples held in a preallocated delay line in the callback’s stack or in a buffer allocated when the stream starts. Ceiling −1.0 dBFS. Release 50 ms. This is protection, not a loudness maximizer. If the lookahead cannot be done without allocating, the fallback is a hard clip at ±1 after a 5 ms gain reduction, and a phase 1 test fails if we ship the fallback without writing it down here. Prefer the lookahead. Implement it with the buffer allocated at stream start.

### ReplayGain 2.0

Apply tags the file already has. Do not scan a library to compute ReplayGain in 1.0.

- If both v1 and v2 gain tags exist, use v2.
- Album gain if this track and the next queued track share an album id and both have album gain. Otherwise track gain.
- Peak tag, if present, caps the applied gain so peak after gain does not exceed −1.0 dBFS, before the limiter. The limiter remains.
- Missing tags: gain is 0 dB. The UI does not pretend ReplayGain ran.
- User preamp (the EQ window’s preamp, and a separate “ReplayGain preamp” in preferences) are two scalars. Both slew. Default ReplayGain preamp is 0 dB.

`lofty` 0.25 field names for v2 tags are an investigation in phase 1. The trait is the seam. `lofty` 0.25.2 is the planning pin as of 2026-09-13.

### Gapless

Required for FLAC, Vorbis, ALAC, and Opus when the container provides delay and padding. MP3 is required only if the phase 1 investigation shows `symphonia` surfaces LAME/Xing encoder delay and padding; if it does not, MP3 is documented as not gapless and drops off the list. The decode thread trims those frames before the ring. The resampler is reset at a gapless boundary with its leftover samples drained into the next track’s start, not discarded, if `rubato` exposes a way to do that without allocating. If it does not, drain into a buffer allocated when the stream starts. A phase 1 test plays two tone files with known delay and checks the join is within one sample of the trimmed length.

AAC-LC is excluded. Do not trim AAC by guesswork.

### Crossfade

Off by default. Duration 0 to 10 seconds, preferences. When on, the decode thread mixes the end of track A and the start of track B into the audio ring using equal-power fades. The mix uses a buffer allocated when crossfade is enabled, not on the callback. Gapless trim still applies to each side before the fade. Crossfade and a one-sample gapless join are different features. Crossfade wins if the user turned it on.

## FFT tap

The analysis thread, not the callback:

- Window: 1024 samples, Hann.
- Hop: 512 (50% overlap).
- Magnitude in dB, referenced to a full-scale sine in one bin.
- One snapshot write per hop into the seqlock.

The 76×16 analyzer maps those bins to bars. Bar count and bar width are fixture-locked by a golden image in phase 2–3. Do not hard-code 19 bars in advance. Colors are `viscolor.txt` indices 2–17 by height and 23 for the peak-hold dot. Peak-hold decay is a constant chosen to match that golden image, not a constant copied from Winamp source.

The oscilloscope draws the latest PCM window into 76×16 using indices 18–22, background 0, dots 1.

The full-window visualizer reads the same snapshot. It does not install a second tap. Packet layout is in [visualizer](visualizer.md).

## Output

See [ADR 006](../adr/006-audio-output.md).

- macOS: CoreAudio via `cpal` 0.18, shared float, no hog mode. Hot-swap on `DeviceChanged` / stream invalidation: tear down, open the new default, reset the resampler, count one underrun if the ring was the wrong rate.
- Windows (phase 13): WASAPI shared by default, exclusive opt-in, exclusive releases the device after a pause of five seconds.
- Linux (phase 14): PipeWire, ALSA fallback.

Underrun recovery: silence for that callback, atomic counter, session may show a discreet status in the gen line if more than three underruns happen in ten seconds. Do not pop a modal.

Device enumeration is a snapshot the UI reads from the session. Opening a device happens on the library or a dedicated output thread, never on the callback, never on the UI thread if it can block. The UI thread sends “use device id” and waits for the snapshot to change.

## CLI

`llamp play <path>` opens the default output and runs this graph. `llamp decode <path> <out.wav>` writes float or 16-bit WAV without a device, for tests. Both exist in phase 1, before any window.
