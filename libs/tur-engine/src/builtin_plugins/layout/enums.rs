//! Layout enum documentation — the enum TYPES (`Axis`, `MainAxisAlignment`,
//! `CrossAxisAlignment`, `MainAxisSize`, `FlexFit`, `HitTestBehavior`,
//! `BoxFit`, `BorderPosition`, `ClipBehavior`, `StackFit`, `Alignment`) live
//! in [`crate::core::layout`].
//!
//! The per-realm JS enum const-objects that used to mirror them for
//! `tur:std` died with the JS rail; native `Value` atoms carry the enum
//! variants directly (decoded via `FromValue` in `core::edgy::value`).
