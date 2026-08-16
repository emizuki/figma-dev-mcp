//! Public MCP contracts. These wrappers deliberately do not expose wire enums.

mod common;
mod handoff;
mod navigation;
mod prototype;
mod visual;

pub use handoff::*;
pub use navigation::*;
pub use prototype::*;
pub(crate) use visual::public_asset;
pub use visual::*;
