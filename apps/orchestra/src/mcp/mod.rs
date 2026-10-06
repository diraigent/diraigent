//! Deny-by-default task-local MCP broker. No listener or upstream secrets are
//! exposed to providers; an adapter must retain the opaque task capability.
mod broker;
mod transport;

pub use broker::*;
