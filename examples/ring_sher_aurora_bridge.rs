//! Example showing how SHER-Input ring buffer connects to SHER-Aurora dispatcher.
//! This is illustrative; in practice use the aurora-gtk input module with sher-input feature.

use sher_input_core::{InputRingBuffer, RawInputEvent};
use std::sync::Arc;

fn main() {
    let ring = Arc::new(InputRingBuffer::<RawInputEvent, 256>::new());
    // Producer (kernel/ISR side)
    let _ = ring.push(RawInputEvent {
        event_type: 1, // EV_KEY
        code: 30,      // KEY_A
        value: 1,      // press
        timestamp: 123456789,
    });
    // Consumer (compositor side)
    if let Some(ev) = ring.pop() {
        println!("event: type={} code={} val={}", ev.event_type, ev.code, ev.value);
    }
}
