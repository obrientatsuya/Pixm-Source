use godot::prelude::*;

pub mod core;
pub mod sim;
pub mod input;
pub mod bridge;

struct PixmExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PixmExtension {}
