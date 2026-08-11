use evdev::{Device, Key, RelativeAxisType};
use sher_input_core::{BackendMetadata, ConnectionState, InputDevice, InputDeviceCapabilities, InputDeviceClass, InputDeviceId};
use std::path::Path;

/// Classifies a raw evdev node the same way the kernel's own `libinput` roughly does:
/// presence of `EV_KEY` letter keys implies a keyboard, `BTN_LEFT` plus relative axes
/// implies a mouse, `BTN_TOUCH` plus absolute axes implies a touch device. A device can
/// legitimately match more than one (many laptops' built-in keyboards also expose
/// media keys as part of the same node) — `class` records the primary role for
/// diagnostics, `capabilities` is what consumers actually branch on (section 3, 10).
pub fn describe(path: &Path, device: &Device) -> InputDevice {
    let keys = device.supported_keys();
    let rel_axes = device.supported_relative_axes();
    let abs_axes = device.supported_absolute_axes();

    let has_letter_keys = keys.is_some_and(|k| k.contains(Key::KEY_A) || k.contains(Key::KEY_SPACE));
    let has_pointer_buttons = keys.is_some_and(|k| k.contains(Key::BTN_LEFT));
    let has_relative_motion = rel_axes.is_some_and(|a| a.contains(RelativeAxisType::REL_X));
    let has_touch = keys.is_some_and(|k| k.contains(Key::BTN_TOUCH));
    let has_absolute_motion = abs_axes.is_some_and(|a| a.contains(evdev::AbsoluteAxisType::ABS_X));

    let class = if has_letter_keys {
        InputDeviceClass::Keyboard
    } else if has_touch {
        InputDeviceClass::Touchscreen
    } else if has_pointer_buttons || has_relative_motion {
        InputDeviceClass::Pointer
    } else {
        InputDeviceClass::Unknown
    };

    let capabilities = InputDeviceCapabilities {
        keys: has_letter_keys,
        pointer_relative: has_relative_motion,
        pointer_absolute: has_absolute_motion && !has_touch,
        scroll: rel_axes.is_some_and(|a| a.contains(RelativeAxisType::REL_WHEEL) || a.contains(RelativeAxisType::REL_HWHEEL)),
        scroll_high_resolution: rel_axes
            .is_some_and(|a| a.contains(RelativeAxisType::REL_WHEEL_HI_RES) || a.contains(RelativeAxisType::REL_HWHEEL_HI_RES)),
        touch: has_touch,
        max_touch_contacts: if has_touch { 10 } else { 0 },
        tablet: false,
        gamepad: false,
    };

    let input_id = device.input_id();
    InputDevice {
        id: InputDeviceId::new(),
        name: device.name().unwrap_or("unknown input device").to_string(),
        vendor: Some(input_id.vendor()),
        product: Some(input_id.product()),
        class,
        capabilities,
        connection_state: ConnectionState::Connected,
        physical_path: Some(path.display().to_string()),
        backend_metadata: BackendMetadata::default()
            .insert("backend", "linux-evdev")
            .insert("bus_type", format!("{:?}", input_id.bus_type()))
            .insert("phys", device.physical_path().unwrap_or("unknown").to_string()),
    }
}
