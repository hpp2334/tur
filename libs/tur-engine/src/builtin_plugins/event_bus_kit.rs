//! The event bus's kit prelude — the authored face over the event bus rows
//! (`event_on_raw` / `event_send_raw`, the mechanism in
//! [`core::event_bus`](crate::core::event_bus)). Owned by the standard
//! bundle (the layering law applied to the kit): `TurStdPlugin` pushes it
//! as a compile-session prelude beside the rows, so `use
//! tur_event_kit::{ event_on, event_send };` resolves in every module the
//! standard plugin set compiles.

/// The event kit's rut source.
pub const EVENT_KIT_RUT: &str = include_str!("../../../../rut/tur_event_kit/mod.rut");

/// The kit as a compile pkg (spec `tur_event_kit`).
pub fn event_kit_pkg() -> rut_driver::Mounted {
    rut_driver::Mounted {
        spec: "tur_event_kit".to_string(),
        namespace: Some("tur_event_kit".to_string()),
        manifest: rut_driver::Manifest::lib("tur_event_kit", &["tur_host", "tur_kit"]),
        body: rut_driver::MountedBody::Source {
            text: EVENT_KIT_RUT.to_string(),
            is_decl: false,
        },
        ..Default::default()
    }
}
