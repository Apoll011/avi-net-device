// Capability (Full partial) saved using the context system, Query from the context system,
// More robust context system
// Search Nodes based on the Type and capability

pub mod device;
mod capability;
mod query;

pub use capability::DeviceCapabilities;
pub use query::DeviceQuery;