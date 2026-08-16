//! Proves the section 15-17 contract — "SHER-Input provides normalized events,
//! SHER-Display provides focus and routing semantics" — end to end, using a small
//! stand-in for SHER-Display's router built only on `sher_input_core`'s public API.
//! This intentionally does not depend on the real `SHER-Display` crate: SHER-Input's
//! own test suite must prove its contract without reaching into a sibling repository.

use sher_input_core::{
    CaptureKind, InputConfig, InputDeviceId, InputEvent, InputEventPayload, InputService,
    KeyAction, LogicalKey, PhysicalKey, PointerEvent,
};
use sher_input_test::SimulatedController;
use std::collections::HashMap;

/// A deliberately tiny stand-in for SHER-Display's real input router: it owns focus
/// and capture-awareness, and decides routing — the same split of responsibility the
/// architecture requires (section 15). SHER-Input never appears on the left-hand side
/// of an `if` about *which surface* gets an event.
#[derive(Default)]
struct MockDisplayRouter {
    keyboard_focus: Option<&'static str>,
    pointer_over: Option<&'static str>,
    global_shortcuts: HashMap<(bool, bool, PhysicalKey), &'static str>,
    delivered: Vec<Delivery>,
}

#[derive(Debug, PartialEq)]
enum Delivery {
    GlobalShortcut(&'static str),
    ToSurface(&'static str, String),
    Dropped,
}

impl MockDisplayRouter {
    fn route(&mut self, event: &InputEvent, captured_pointer_owner: Option<&'static str>) {
        match &event.payload {
            InputEventPayload::Keyboard(k) if k.action == KeyAction::Down => {
                let key = (event.modifiers.ctrl, event.modifiers.alt, k.physical_key);
                if let Some(action) = self.global_shortcuts.get(&key) {
                    self.delivered.push(Delivery::GlobalShortcut(action));
                    return;
                }
                match self.keyboard_focus {
                    Some(surface) => self
                        .delivered
                        .push(Delivery::ToSurface(surface, format!("{:?}", k.logical_key))),
                    None => self.delivered.push(Delivery::Dropped),
                }
            }
            InputEventPayload::Pointer(PointerEvent::MotionRelative { .. }) => {
                // While a drag/resize/pointer-lock capture is active, motion goes to
                // the captured owner regardless of what geometry says is underneath
                // the cursor (section 16) — SHER-Input doesn't know or care that this
                // is happening, it just kept streaming normalized motion.
                let target = captured_pointer_owner.or(self.pointer_over);
                match target {
                    Some(surface) => self
                        .delivered
                        .push(Delivery::ToSurface(surface, "motion".to_string())),
                    None => self.delivered.push(Delivery::Dropped),
                }
            }
            _ => {}
        }
    }
}

async fn next_payload_event(
    events: &mut tokio::sync::broadcast::Receiver<InputEvent>,
) -> InputEvent {
    loop {
        let event = events.recv().await.unwrap();
        if !matches!(
            event.payload,
            InputEventPayload::DeviceAdded(_) | InputEventPayload::DeviceRemoved(_)
        ) {
            return event;
        }
    }
}

#[tokio::test]
async fn unfocused_key_events_are_dropped_not_broadcast_to_every_surface() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let controller = SimulatedController::new(service.sink());
    let kb = controller.add_keyboard("Test Keyboard");

    let mut router = MockDisplayRouter::default();
    controller.tap_key(kb, PhysicalKey::KeyA);

    let event = next_payload_event(&mut events).await;
    router.route(&event, None);

    assert_eq!(router.delivered, vec![Delivery::Dropped]);
}

#[tokio::test]
async fn focused_surface_receives_its_own_keys_only() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let controller = SimulatedController::new(service.sink());
    let kb = controller.add_keyboard("Test Keyboard");

    let mut router = MockDisplayRouter {
        keyboard_focus: Some("surface-42"),
        ..Default::default()
    };
    controller.tap_key(kb, PhysicalKey::KeyA);

    let event = next_payload_event(&mut events).await;
    router.route(&event, None);

    assert_eq!(
        router.delivered,
        vec![Delivery::ToSurface(
            "surface-42",
            format!("{:?}", LogicalKey::Character('a'))
        )]
    );
}

#[tokio::test]
async fn global_shortcut_intercepts_before_focused_surface() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let controller = SimulatedController::new(service.sink());
    let kb = controller.add_keyboard("Test Keyboard");

    let mut router = MockDisplayRouter {
        keyboard_focus: Some("some-app"),
        ..Default::default()
    };
    router
        .global_shortcuts
        .insert((true, false, PhysicalKey::KeyT), "aurora.launcher.open");

    controller.press_key(kb, PhysicalKey::ControlLeft);
    controller.tap_key(kb, PhysicalKey::KeyT);

    let ctrl_down = next_payload_event(&mut events).await;
    router.route(&ctrl_down, None); // Ctrl alone is not a shortcut; falls to focus.

    let t_down = next_payload_event(&mut events).await;
    router.route(&t_down, None);

    assert_eq!(
        router.delivered[1],
        Delivery::GlobalShortcut("aurora.launcher.open")
    );
}

#[tokio::test]
async fn pointer_capture_overrides_hit_test_target_and_is_revocable() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let controller = SimulatedController::new(service.sink());
    let mouse = controller.add_mouse("Test Mouse");

    let mut router = MockDisplayRouter {
        pointer_over: Some("window-under-cursor"),
        ..Default::default()
    };

    // SHER-Display grabs the pointer for a drag — the interface SHER-Input exposes
    // for section 16, acquired here exactly as SHER-Display would.
    let guard = service
        .request_capture(
            CaptureKind::Pointer,
            "window-being-dragged",
            "titlebar drag",
            None,
        )
        .unwrap();

    controller.move_relative(mouse, 5.0, 5.0);
    let motion = next_payload_event(&mut events).await;
    let owner = service.captures().owner_of(CaptureKind::Pointer);
    router.route(&motion, owner.as_deref().map(|_| "window-being-dragged"));

    assert_eq!(
        router.delivered,
        vec![Delivery::ToSurface(
            "window-being-dragged",
            "motion".to_string()
        )]
    );

    drop(guard);
    assert!(!service.captures().is_captured(CaptureKind::Pointer));

    controller.move_relative(mouse, 1.0, 1.0);
    let motion_after_release = next_payload_event(&mut events).await;
    router.route(&motion_after_release, None);

    assert_eq!(
        router.delivered[1],
        Delivery::ToSurface("window-under-cursor", "motion".to_string())
    );
}

#[tokio::test]
async fn unplugging_the_keyboard_mid_session_is_reported_not_fatal() {
    let service = InputService::new(InputConfig::default());
    let mut events = service.subscribe();
    let controller = SimulatedController::new(service.sink());
    let kb = controller.add_keyboard("Test Keyboard");

    let _added: InputDeviceId = match events.recv().await.unwrap().payload {
        InputEventPayload::DeviceAdded(d) => d.id,
        other => panic!("expected DeviceAdded, got {other:?}"),
    };

    controller.press_key(kb, PhysicalKey::ControlLeft);
    let _ctrl_down = events.recv().await.unwrap();

    controller.unplug(kb);
    let removed = events.recv().await.unwrap();
    assert!(matches!(removed.payload, InputEventPayload::DeviceRemoved(id) if id == kb));

    // Section 9: existing state is cleaned up, not left dangling for a device that no
    // longer exists — the registry (and via it, SHER-Display) must not still see it.
    assert!(service.registry().get(kb).is_none());
}
