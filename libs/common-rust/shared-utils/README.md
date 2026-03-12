# Shared Utilities

Common utilities and patterns for Zivue microservices. This library dramatically reduces boilerplate code and ensures consistency across all services.

## Features

- 🚀 **Server Initialization** - One-line server setup with tracing, config, observability, and database
- 🔌 **gRPC Server Builder** - Automatic reflection support, standardized server spawning, and generic repository wrapper
- 🌐 **REST Utilities** - Pre-configured routers with health checks, CORS, tracing, and route composition helpers
- 🏗️ **Build Utilities** - Standardized proto compilation via `shared-build-utils` crate
- 📦 **State Macros** - Eliminate state boilerplate with `define_app_state!` macro
- ⚙️ **Configuration Management** - Environment-based config with validation
- 📊 **Observability** - OpenTelemetry metrics integration
- 🗄️ **Database** - Connection pooling with sqlx
- 🛑 **Graceful Shutdown** - Signal handling for clean shutdowns
- 🔧 **Workspace Dependencies** - Centralized dependency management via Cargo workspace

## Quick Example

Instead of 70+ lines of boilerplate:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize everything in one line!
    let (config, meter_provider, pool, start_time) = shared_utils::server::init_server().await?;

    let state = state::new(pool);
    let app = rest::routes::create_routes(state.clone());

    // gRPC with automatic reflection
    let repo_clone = state.repository.clone();
    let grpc_config = shared_utils::grpc::GrpcServerConfig::new()
        .with_addr("0.0.0.0:50051")
        .with_reflection(DESCRIPTOR_BYTES);
    
    let _grpc_handle = shared_utils::grpc::spawn_grpc_server(grpc_config, move |mut server| {
        let grpc_repo = GrpcRepository::new(repo_clone);
        let router = server.add_service(YourService::new(grpc_repo));
        router
    });

    // Start HTTP server
    let addr = config.bind_address().parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    shared_utils::server::log_startup_complete(&addr, start_time);

    axum::serve(listener, app)
        .with_graceful_shutdown(shared_utils::shutdown::shutdown_signal())
        .await?;

    drop(meter_provider);
    shared_utils::server::log_shutdown_complete();
    Ok(())
}
```

## Cargo Features

Enable only what you need:

```toml
[dependencies.shared-utils]
path = "../common/shared-utils"
features = ["grpc", "grpc-reflection", "rest"]
```

Available features:
- `grpc` - gRPC server utilities (requires tonic)
- `grpc-reflection` - Automatic gRPC reflection support
- `rest` - REST/HTTP utilities (requires axum)

## Module Documentation

### `server` - Server Initialization

```rust
// Complete initialization
let (config, meter_provider, pool, start_time) = shared_utils::server::init_server().await?;

// Or step-by-step
shared_utils::server::init_tracing();
let (config, meter_provider, start_time) = shared_utils::server::init_config_and_observability()?;
let pool = shared_utils::server::init_db_pool().await?;

// Logging helpers
shared_utils::server::log_startup_complete(&addr, start_time);
shared_utils::server::log_shutdown_complete();
```

### `grpc` - gRPC Server Builder

```rust
use shared_utils::grpc::GrpcServerConfig;

// Configure server
let grpc_config = GrpcServerConfig::new()
    .with_addr("0.0.0.0:50051")
    .with_reflection(DESCRIPTOR_BYTES);  // Enable reflection

// Spawn server
let handle = shared_utils::grpc::spawn_grpc_server(grpc_config, move |mut server| {
    server.add_service(YourService::new(repo))
});
```

**Reflection Support**: Automatically enables gRPC reflection when descriptor bytes are provided, allowing tools like `grpcurl` to introspect your service without proto files.

### `rest` - REST Utilities

```rust
use shared_utils::rest;

// Base router with health check, CORS, and tracing
let router = rest::base_router()
    .merge(Router::new()
        .route("/your-endpoint", get(handler))
    );

// Or add middleware to existing router
let router = Router::new()
    .route("/api", get(handler));
let router = rest::with_middleware(router);

// Health check endpoint (included in base_router)
// GET /health -> {"status":"ok","service":"your-service","version":"0.1.0"}
```

### `config` - Configuration Management

```rust
let config = shared_utils::config::config()?;

println!("Service: {}", config.service_name);
println!("Bind: {}", config.bind_address());
config.log_config();  // Logs all configuration
```

Environment variables:
- `HOST` - Bind host (default: 127.0.0.1)
- `PORT` - Bind port (default: 3000)
- `SERVICE_NAME` - Service name for logging/metrics (default: unknown_service)
- `OTLP_ENDPOINT` - OpenTelemetry endpoint
- `DATABASE_URL` - PostgreSQL connection string (required)

### `db` - Database Connection Pool

```rust
let pool = shared_utils::db::create_pool().await?;
type DbPool = shared_utils::db::DbPool;
```

### `observability` - OpenTelemetry

```rust
let meter_provider = shared_utils::observability::observability(&config)?;
// Use throughout application...
drop(meter_provider);  // Flush metrics on shutdown
```

### `shutdown` - Graceful Shutdown

```rust
axum::serve(listener, app)
    .with_graceful_shutdown(shared_utils::shutdown::shutdown_signal())
    .await?;
```

Handles SIGINT (Ctrl+C) and SIGTERM signals.

### `state` - Application State

```rust
use shared_utils::state::AppState;
use std::sync::Arc;

pub type RepositoryDyn = dyn RepositoryTrait + Send + Sync;
pub type AppState = shared_utils::state::AppState<RepositoryDyn>;

pub fn new(pool: DbPool) -> AppState {
    let repo = Repository::new(pool);
    AppState::new(Arc::new(repo))
}
```

### `structs` - Common Data Types

Shared proto message types:
- `Fid` - Foreign ID structure
- `Zuuid` - UUID wrapper
- `ListFids` - List of Fids
- `ListZuuids` - List of Zuuids

### `response` - HTTP Response Helpers

```rust
use shared_utils::ApiResponse;

async fn handler() -> Json<ApiResponse<Data>> {
    // Helper for consistent API responses
}
```

### `validation` - Input Validation

```rust
use shared_utils::validate_non_empty;

validate_non_empty(&value, "field_name")?;
```

### `country` & `language` - ISO Data

Provides ISO country and language code lookups from embedded CSV data.

### `grpc::GrpcRepository` - Generic Repository Wrapper (NEW)

Eliminates the need to create identical repository wrapper structs in each service:

```rust
use shared_utils::grpc::GrpcRepository;
use std::sync::Arc;

// Before: Every service had this boilerplate
// pub struct GrpcRepository { inner: Arc<dyn RepositoryTrait> }
// impl GrpcRepository { pub fn new(...) -> Self { ... } }

// After: Just use the generic one
let repo: Arc<dyn RepositoryTrait> = Arc::new(MyRepository::new(pool));
let grpc_repo = GrpcRepository::new(repo);
let service = MyServiceServer::new(grpc_repo);
```

### `macros::define_app_state!` - State Boilerplate Reduction (NEW)

Replaces 10+ identical lines in every service's state.rs:

```rust
// In your service's state.rs:
use shared_utils::define_app_state;

define_app_state!(
    crate::persistence::Repository,           // Your repository struct
    crate::persistence::traits::Repository    // The trait it implements
);

// This generates all the type aliases and constructor!
// - pub type RepositoryDyn = dyn Trait + Send + Sync;
// - pub type AppState = shared_utils::state::AppState<RepositoryDyn>;
// - pub fn new(pool: DbPool) -> AppState { ... }
```

### `rest::router_with_routes` - Simplified Router Building (NEW)

One-liner for creating routers with all middleware:

```rust
use shared_utils::rest::router_with_routes;

// Instead of manual CORS/tracing setup:
let router = router_with_routes(|r| {
    r.route("/api/users", get(list_users))
     .route("/api/items", get(list_items))
});

// Automatically includes: health check, CORS, tracing, and your routes!
```

## Build Utilities (`shared-build-utils`)

Separate crate for build-time proto compilation standardization:

```toml
# In your service's Cargo.toml
[build-dependencies]
shared-build-utils = { path = "../../../common/shared-build-utils" }
```

```rust
// In your service's build.rs - replaces 15+ lines!
fn main() -> Result<(), Box<dyn std::error::Error>> {
    shared_build_utils::compile_protos(
        &["fid/v1/service.proto", "common/v1/types.proto"],
        &["../../../common/proto"]
    )?;
    Ok(())
}
```

Benefits:
- Automatic descriptor file generation at `OUT_DIR/proto_descriptor.bin`
- Consistent build_server/build_client configuration
- Proper cargo:rerun-if-changed directives
- Standardized across all services

## Workspace Dependencies

Root-level `Cargo.toml` defines all common dependencies, eliminating version drift:

```toml
# Services inherit versions via workspace.dependencies
[workspace.dependencies]
tokio = { version = "1.45.1", features = ["full"] }
tonic = "0.14.1"
axum = "0.8.7"
# ... all shared deps
```

```toml
# In service Cargo.toml - just reference workspace versions
[dependencies]
tokio.workspace = true
tonic.workspace = true
axum.workspace = true
```

## Dependencies

Core dependencies (always included):
- `anyhow` - Error handling
- `tokio` - Async runtime
- `tracing` - Structured logging
- `tracing-subscriber` - Log output
- `sqlx` - Database (with postgres support)
- `serde` - Serialization
- `dotenv` - Environment variables
- `opentelemetry` - Observability

Optional (feature-gated):
- `tonic` - gRPC (`grpc` feature)
- `tonic-reflection` - Reflection (`grpc-reflection` feature)
- `axum` - HTTP framework (`rest` feature)
- `tower-http` - HTTP middleware (`rest` feature)

## Code Reduction Stats

Using shared-utils reduces typical service code by:
- **~40 lines** removed from main.rs
- **~30%** less total boilerplate
- **100%** consistent patterns across services
- **Zero** reflection configuration code

### Before (70+ lines)

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let start_time = std::time::Instant::now();
    
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = shared_utils::config::config()?;
    let meter_provider = shared_utils::observability::observability(&config)?;
    config.log_config();

    let pool = shared_utils::db::create_pool()
        .await
        .context("Failed to create database pool")?;

    let state = state::new(pool);
    let app = rest::routes::create_routes(state.clone());

    // 30+ lines of gRPC server setup with reflection...
    
    let addr = config.bind_address().parse::<SocketAddr>()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    let startup_duration = start_time.elapsed();
    tracing::info!(
        addr = %addr,
        startup_time_ms = startup_duration.as_millis(),
        "✅ Server ready"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shared_utils::shutdown::shutdown_signal())
        .await?;

    drop(meter_provider);
    tracing::info!("🛑 shutdown complete");
    Ok(())
}
```

### After (40 lines)

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (config, meter_provider, pool, start_time) = shared_utils::server::init_server().await?;
    
    let state = state::new(pool);
    let app = rest::routes::create_routes(state.clone());

    let repo_clone = state.repository.clone();
    let grpc_config = shared_utils::grpc::GrpcServerConfig::new()
        .with_addr("0.0.0.0:50051")
        .with_reflection(DESCRIPTOR_BYTES);
    
    let _grpc_handle = shared_utils::grpc::spawn_grpc_server(grpc_config, move |mut server| {
        let grpc_repo = GrpcRepository::new(repo_clone);
        let router = server.add_service(YourService::new(grpc_repo));
        router
    });

    let addr = config.bind_address().parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    shared_utils::server::log_startup_complete(&addr, start_time);

    axum::serve(listener, app)
        .with_graceful_shutdown(shared_utils::shutdown::shutdown_signal())
        .await?;

    drop(meter_provider);
    shared_utils::server::log_shutdown_complete();
    Ok(())
}
```

## Example Services

See these services for complete examples:
- `services/fid` - Full-featured service
- `services/search-proxy` - External API integration example

## Template

See [MICROSERVICE_TEMPLATE.md](../MICROSERVICE_TEMPLATE.md) for a complete guide to creating new services.

## Testing

```bash
cd common/shared-utils
cargo test
```

## Version

0.1.0 - Initial release with server, gRPC, REST, and utility modules.
