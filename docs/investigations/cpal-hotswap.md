# cpal hot-swap and underrun

Date: 2026-09-13. Backend: cpal 0.18. Script: `crates/llamp-audio/examples/cpal_hotswap.rs`.

## Underrun

Ran. An empty ring of 256 frames at 48 kHz wrote one period of silence and incremented the underrun count by one. That is the callback contract in `docs/spec/audio-pipeline.md`.

## Default-device change

Ran. The script opens a cpal stream on the system default (the path that installs cpal's `kAudioHardwarePropertyDefaultOutputDevice` listener), switches that default to MacBook Pro Speakers, then switches it back. The original default is restored on the way out, including if the run fails. This is a scripted default-output change, not a physical unplug. CoreAudio delivers the same property cpal listens for.

Both directions emitted `DeviceChanged`. Callbacks kept running through the change (starved periods still incremented the underrun counter). The product path then tore the stream down and opened the new default. The rate changed (44100 on PXC 550-II, 48000 on the speakers), so that reopen counted one underrun, which `docs/spec/audio-pipeline.md` specifies when the ring rate is wrong. Callbacks resumed after each reopen. `underruns_at_reopen` is the starved-period probe plus that one rate-change count, not that many dropouts of a filled buffer.

```
underrun_count=1
starved_period_is_silence=true
original_default=PXC 550-II
enumerated_devices=5
alternate_device=MacBook Pro Speakers
callbacks_before_switch=2
rate_before=44100
switched_default=MacBook Pro Speakers
event_after_switch=DeviceChanged
callbacks_during_switch_window=38
rate_after_switch=48000
callbacks_after_reopen=1
rate_change_underrun=true
underruns_at_reopen=47
restored_default=PXC 550-II
event_after_restore=DeviceChanged
callbacks_during_restore_window=36
rate_after_restore=44100
callbacks_after_restore_reopen=1
recovered=true
adr_006=unchanged
default_after_exit=PXC 550-II
default_restored=true
```

cpal recovered. ADR 006 is not amended. A CoreAudio backend was not written.
