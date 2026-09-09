//! Paint 10's document model, raster operations, file formats, and desktop services.
//!
//! The native interface lives in the binary crate; the browser compiles those
//! same UI modules into this library. Keeping the document engine independent
//! allows rendering, history, serialization, and layout to be tested without
//! opening a window.

pub mod color;
pub mod document;
pub mod integration;
pub mod metadata;
pub mod preferences;
pub mod printing;
pub mod project;
pub mod raster_io;
pub mod text;

// Both entry points compile the same UI modules and drawing engine.
#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod icons;
#[cfg(target_arch = "wasm32")]
mod print_preview;
#[cfg(target_arch = "wasm32")]
pub mod web;
