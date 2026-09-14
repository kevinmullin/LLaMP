//! ADR 006 spike: starved-ring underrun, then a scripted default-output change.
//!
//! The switch uses CoreAudio's default-output property (the event cpal listens
//! for). The original default is restored on the way out, including on panic.

use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use llamp_audio::cpal_output::CpalOutput;
use llamp_audio::graph::Callback;
use llamp_audio::output::{Output, OutputEvents, StreamRequest};
use rtrb::RingBuffer;

fn main() {
    starved_underrun();

    let setter = compile_setter();
    let original = command_output(&setter, &["--get"]);
    println!("original_default={original}");
    run_switch(&setter, &original);
    let after = command_output(&setter, &["--get"]);
    println!("default_after_exit={after}");
    println!("default_restored={}", after == original);
}

fn run_switch(setter: &std::path::Path, original: &str) {
    let restore = Restore {
        setter: setter.to_path_buf(),
        name: original.to_string(),
    };

    let devices = CpalOutput::new().enumerate().expect("enumerate");
    println!("enumerated_devices={}", devices.len());
    let alternate = devices
        .iter()
        .find(|device| device.id == "MacBook Pro Speakers" && device.id != original)
        .or_else(|| devices.iter().find(|device| device.id != original))
        .unwrap_or_else(|| panic!("need a second output to change the default"));
    println!("alternate_device={}", alternate.id);

    let events = OutputEvents::new();
    let host_rate = open_rate();
    let mut backend = CpalOutput::new();
    let (_producer, consumer) =
        RingBuffer::<f32>::new(llamp_audio::output::ring_capacity(host_rate, 2));
    let mut stream = backend
        .open(
            StreamRequest {
                device_id: None,
                sample_rate: host_rate,
                channels: 2,
            },
            consumer,
            events.clone(),
        )
        .expect("open default");
    let before = wait_callbacks(events.as_ref(), 1);
    println!("callbacks_before_switch={before}");
    println!("rate_before={host_rate}");

    let switched = command_output(&restore.setter, &[alternate.id.as_str()]);
    println!("switched_default={switched}");
    let (observed, during) = wait_event(events.as_ref(), Duration::from_secs(5));
    println!("event_after_switch={observed}");
    println!("callbacks_during_switch_window={during}");
    let new_rate = open_rate();
    println!("rate_after_switch={new_rate}");

    // Spec: tear down, open the new default, count one underrun if the rate changed.
    stream.stop();
    drop(stream);
    if observed == "none" {
        println!("recovered=false");
        println!("reason=no DeviceChanged or DeviceNotAvailable after default-output change");
        println!("adr_006=amend");
        return;
    }
    if new_rate != host_rate {
        events
            .underruns
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    let underruns_at_reopen = events.underruns.load(std::sync::atomic::Ordering::Relaxed);
    let (_producer, consumer) =
        RingBuffer::<f32>::new(llamp_audio::output::ring_capacity(new_rate, 2));
    let mut stream = backend
        .open(
            StreamRequest {
                device_id: None,
                sample_rate: new_rate,
                channels: 2,
            },
            consumer,
            events.clone(),
        )
        .expect("reopen new default");
    let after = wait_callbacks(events.as_ref(), 1);
    println!("callbacks_after_reopen={after}");
    println!("rate_change_underrun={}", new_rate != host_rate);
    println!("underruns_at_reopen={underruns_at_reopen}");
    if after == 0 {
        println!("recovered=false");
        println!("reason=reopened stream produced no callbacks");
        println!("adr_006=amend");
        return;
    }

    // Replug: put the original default back while this stream is still playing.
    let restored = command_output(&restore.setter, &[original]);
    println!("restored_default={restored}");
    let (back, back_callbacks) = wait_event(events.as_ref(), Duration::from_secs(5));
    println!("event_after_restore={back}");
    println!("callbacks_during_restore_window={back_callbacks}");
    let restored_rate = open_rate();
    println!("rate_after_restore={restored_rate}");
    stream.stop();
    drop(stream);
    if back == "none" {
        println!("recovered=false");
        println!("reason=no DeviceChanged or DeviceNotAvailable after restoring the default");
        println!("adr_006=amend");
        return;
    }
    if restored_rate != new_rate {
        events
            .underruns
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    let (_producer, consumer) =
        RingBuffer::<f32>::new(llamp_audio::output::ring_capacity(restored_rate, 2));
    let mut stream = backend
        .open(
            StreamRequest {
                device_id: None,
                sample_rate: restored_rate,
                channels: 2,
            },
            consumer,
            events.clone(),
        )
        .expect("reopen restored default");
    let after_restore = wait_callbacks(events.as_ref(), 1);
    println!("callbacks_after_restore_reopen={after_restore}");
    stream.stop();
    drop(stream);
    let recovered = after_restore > 0;
    println!("recovered={recovered}");
    if !recovered {
        println!("reason=reopened stream produced no callbacks after restore");
        println!("adr_006=amend");
        return;
    }
    println!("adr_006=unchanged");
}

fn starved_underrun() {
    let mut callback = Callback::new(256, 48_000);
    let mut out = [1.0f32; 512];
    callback.fill(&mut out);
    let silent = out.iter().all(|sample| *sample == 0.0);
    println!("underrun_count={}", callback.underruns());
    println!("starved_period_is_silence={silent}");
}

fn open_rate() -> u32 {
    let host = cpal::default_host();
    let device = cpal::traits::HostTrait::default_output_device(&host).expect("default output");
    let config = cpal::traits::DeviceTrait::default_output_config(&device).expect("default config");
    config.sample_rate()
}

fn wait_callbacks(events: &OutputEvents, minimum_new: u64) -> u64 {
    let start = events.underruns.load(std::sync::atomic::Ordering::Relaxed);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let now = events.underruns.load(std::sync::atomic::Ordering::Relaxed);
        if now.saturating_sub(start) >= minimum_new || Instant::now() >= deadline {
            return now.saturating_sub(start);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_event(events: &OutputEvents, budget: Duration) -> (String, u64) {
    let start = events.underruns.load(std::sync::atomic::Ordering::Relaxed);
    let deadline = Instant::now() + budget;
    let mut seen = None;
    let mut seen_at = None;
    loop {
        if seen.is_none() {
            if let Some(event) = events.take_loss() {
                seen = Some(format!("{event:?}"));
                seen_at = Some(Instant::now());
            }
        }
        let settled = seen_at.is_some_and(|at| at.elapsed() >= Duration::from_millis(400));
        if Instant::now() >= deadline || settled {
            let delta = events
                .underruns
                .load(std::sync::atomic::Ordering::Relaxed)
                .saturating_sub(start);
            return (seen.unwrap_or_else(|| "none".into()), delta);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn compile_setter() -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/set_default_output.swift");
    let out = std::env::temp_dir().join("llamp-set-default-output");
    let status = Command::new("swiftc")
        .arg("-O")
        .arg("-o")
        .arg(&out)
        .arg(&src)
        .status()
        .expect("swiftc");
    assert!(status.success(), "swiftc failed");
    out
}

fn command_output(bin: &PathBuf, args: &[&str]) -> String {
    let output = Command::new(bin).args(args).output().expect("setter");
    if !output.status.success() {
        panic!(
            "setter {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

struct Restore {
    setter: PathBuf,
    name: String,
}

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = Command::new(&self.setter)
            .arg(&self.name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}
