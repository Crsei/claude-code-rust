//! Compatibility re-export for event classification and queue pressure.

pub use cc_ipc_transport::{
    classify_event, event_type, ClientEventQueue, EventClass, QueuePressureDiagnostic,
};
