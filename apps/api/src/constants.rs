// Constants for the Diraigent API.

use uuid::Uuid;

/// The default tenant UUID, used when a project does not specify a tenant.
///
/// This must match the seed row in the `tenant` table (migration 016).
pub const DEFAULT_TENANT_ID: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
]);
