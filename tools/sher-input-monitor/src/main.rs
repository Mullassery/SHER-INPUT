//! `sher-input-monitor` (section 28): shows the normalized SHER-Input event stream
//! live, using only the public `sher_input_core` API — nothing here prints an evdev
//! path, a Linux keycode, or any other backend-specific detail, on purpose. That's the
//! whole point of the tool: if something is wrong upstream of normalization, this is
//! where it becomes visible without reaching into backend internals.

use sher_input_core::{
    BackendEvent, InputConfig, InputDeviceId, InputEvent, InputEventPayload, InputService, KeyAction, PhysicalKey,
    PointerButton, PointerEvent, Timestamp, TouchPhase,
};
use sher_input_test::{keyboard_device, mouse_device, ScriptedBackend};
use std::time::Duration;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    let simulate = std::env::args().any(|a| a == "--simulate") || cfg!(not(target_os = "linux"));

    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();

    let _backend_handle = if simulate {
        println!("sher-input-monitor: no Linux backend on this platform (or --simulate passed) — running a scripted demo device");
        service
            .start_backend(Box::new(ScriptedBackend::new(demo_script())))
            .expect("scripted backend should always start")
    } else {
        match service.start_backend(Box::new(sher_input_linux::LinuxBackend::new())) {
            Ok(handle) => handle,
            Err(err) => {
                eprintln!("sher-input-monitor: failed to start Linux backend: {err}");
                eprintln!("(needs read access to /dev/input/event*; try --simulate, or run as a user in the `input` group)");
                std::process::exit(1);
            }
        }
    };

    println!("sher-input-monitor: listening for normalized SHER-Input events (Ctrl+C to exit)\n");

    let mut count: u64 = 0;
    loop {
        match events.recv().await {
            Ok(event) => {
                count += 1;
                print_event(count, &event);
            }
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                eprintln!("[monitor] dropped {missed} events — consumer fell behind");
            }
            Err(broadcast::error::RecvError::Closed) => {
                println!("sher-input-monitor: event stream closed");
                break;
            }
        }
    }
}

fn print_event(count: u64, event: &InputEvent) {
    let latency = latency_since(event.timestamp);
    let dev = short_id(event.device_id);
    let src = match &event.source {
        sher_input_core::InputSource::Physical => "physical".to_string(),
        sher_input_core::InputSource::Synthetic(origin) => format!("synthetic:{origin:?}"),
    };

    match &event.payload {
        InputEventPayload::DeviceAdded(device) => {
            println!(
                "#{count:<6} [{src}] device-added   {dev}  \"{}\"  class={:?} caps(keys={} ptr_rel={} ptr_abs={} scroll={} touch={})",
                device.name,
                device.class,
                device.capabilities.keys,
                device.capabilities.pointer_relative,
                device.capabilities.pointer_absolute,
                device.capabilities.scroll,
                device.capabilities.touch,
            );
        }
        InputEventPayload::DeviceRemoved(id) => {
            println!("#{count:<6} [{src}] device-removed {}", short_id(*id));
        }
        InputEventPayload::Keyboard(k) => {
            let action = match k.action {
                KeyAction::Down => "key-down",
                KeyAction::Up => "key-up  ",
                KeyAction::Repeat => "key-rpt ",
            };
            println!(
                "#{count:<6} [{src}] {action} {dev}  physical={:?} logical={:?} text={:?}  mods={}  (+{latency}us)",
                k.physical_key,
                k.logical_key,
                k.text,
                fmt_modifiers(event),
            );
        }
        InputEventPayload::Pointer(p) => match p {
            PointerEvent::MotionRelative { dx, dy } => {
                println!("#{count:<6} [{src}] motion-rel     {dev}  dx={dx:.1} dy={dy:.1}  (+{latency}us)")
            }
            PointerEvent::MotionAbsolute { position } => {
                println!("#{count:<6} [{src}] motion-abs     {dev}  x={:.1} y={:.1}  (+{latency}us)", position.x, position.y)
            }
            PointerEvent::Button { button, pressed } => {
                let label = if *pressed { "button-down" } else { "button-up  " };
                println!("#{count:<6} [{src}] {label}    {dev}  {}", fmt_button(*button))
            }
            PointerEvent::Scroll { axis, delta, high_resolution } => {
                println!("#{count:<6} [{src}] scroll         {dev}  axis={axis:?} delta={delta:.2} hi_res={high_resolution}")
            }
            PointerEvent::Enter => println!("#{count:<6} [{src}] pointer-enter  {dev}"),
            PointerEvent::Leave => println!("#{count:<6} [{src}] pointer-leave  {dev}"),
        },
        InputEventPayload::Touch(t) => {
            let phase = match t.phase {
                TouchPhase::Down => "touch-down",
                TouchPhase::Move => "touch-move",
                TouchPhase::Up => "touch-up  ",
                TouchPhase::Cancel => "touch-cncl",
            };
            println!(
                "#{count:<6} [{src}] {phase} {dev}  contact={} x={:.1} y={:.1}",
                t.contact_id.0, t.position.x, t.position.y
            );
        }
        InputEventPayload::Tablet(_) => println!("#{count:<6} [{src}] tablet         {dev}  (Phase 4)"),
        InputEventPayload::Gamepad(_) => println!("#{count:<6} [{src}] gamepad        {dev}  (Phase 4)"),
    }
}

fn fmt_button(button: PointerButton) -> String {
    format!("{button:?}")
}

fn fmt_modifiers(event: &InputEvent) -> String {
    let m = event.modifiers;
    let mut parts = Vec::new();
    if m.ctrl {
        parts.push("ctrl");
    }
    if m.alt {
        parts.push("alt");
    }
    if m.shift {
        parts.push("shift");
    }
    if m.logo {
        parts.push("super");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join("+")
    }
}

fn short_id(id: InputDeviceId) -> String {
    let full = id.to_string();
    full.chars().take(8).collect()
}

fn latency_since(timestamp: Timestamp) -> u64 {
    Timestamp::now().as_nanos().saturating_sub(timestamp.as_nanos()) / 1_000
}

fn demo_script() -> Vec<(Duration, BackendEvent)> {
    let keyboard = keyboard_device("Demo Keyboard");
    let mouse = mouse_device("Demo Mouse");
    let kb_id = keyboard.id;
    let mouse_id = mouse.id;

    let mut script = vec![
        (Duration::from_millis(200), BackendEvent::DeviceAdded(keyboard)),
        (Duration::from_millis(200), BackendEvent::DeviceAdded(mouse)),
    ];

    for key in [PhysicalKey::KeyH, PhysicalKey::KeyI] {
        script.push((Duration::from_millis(400), BackendEvent::Key { device_id: kb_id, physical_key: key, pressed: true }));
        script.push((Duration::from_millis(80), BackendEvent::Key { device_id: kb_id, physical_key: key, pressed: false }));
    }

    script.push((Duration::from_millis(600), BackendEvent::MotionRelative { device_id: mouse_id, dx: 12.0, dy: -4.0 }));
    script.push((
        Duration::from_millis(150),
        BackendEvent::PointerButton { device_id: mouse_id, button: PointerButton::Left, pressed: true },
    ));
    script.push((
        Duration::from_millis(80),
        BackendEvent::PointerButton { device_id: mouse_id, button: PointerButton::Left, pressed: false },
    ));

    script
}
