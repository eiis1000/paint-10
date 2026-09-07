//! Paint 10's document model, raster operations, file formats, and desktop services.
//!
//! The native interface lives in the binary crate. Keeping the document engine
//! here allows its rendering, history, serialization, and layout behavior to be
//! tested without opening a window.

pub mod document;
pub mod integration;
pub mod metadata;
pub mod preferences;
pub mod printing;
pub mod project;
pub mod raster_io;
pub mod text;
