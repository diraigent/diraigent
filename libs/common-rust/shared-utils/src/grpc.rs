#[cfg(feature = "grpc")]
use anyhow::Context;
#[cfg(feature = "grpc")]
use std::{net::SocketAddr, sync::Arc};
#[cfg(feature = "grpc")]
use tonic::{Status, transport::Server as TonicServer};

pub trait FromProto<T> {
    fn from_proto(proto: T) -> Self
    where
        Self: Sized;
}

pub trait IntoProto<T> {
    fn into_proto(self) -> T;
}

#[cfg(feature = "grpc")]
pub trait IntoGrpcResult<T> {
    fn into_grpc(self) -> Result<T, Status>;
    fn into_grpc_invalid_argument(self) -> Result<T, Status>;
}

#[cfg(feature = "grpc")]
impl<T, E: std::fmt::Display> IntoGrpcResult<T> for Result<T, E> {
    fn into_grpc(self) -> Result<T, Status> {
        self.map_err(|e| Status::internal(format!("{}", e)))
    }

    fn into_grpc_invalid_argument(self) -> Result<T, Status> {
        self.map_err(|e| Status::invalid_argument(format!("{}", e)))
    }
}

#[cfg(feature = "grpc")]
pub type GrpcResult<T> = Result<T, Status>;

#[cfg(feature = "grpc")]
pub fn parse_uuid(s: &str) -> GrpcResult<uuid::Uuid> {
    s.parse().into_grpc_invalid_argument()
}

#[cfg(feature = "grpc")]
#[derive(Clone)]
pub struct GrpcRepository<T: ?Sized> {
    inner: Arc<T>,
}

#[cfg(feature = "grpc")]
impl<T: ?Sized> GrpcRepository<T> {
    pub fn new(inner: Arc<T>) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &Arc<T> {
        &self.inner
    }
}

#[cfg(feature = "grpc")]
pub struct GrpcServerConfig {
    pub addr: String,
    pub descriptor_bytes: Option<&'static [u8]>,
}

#[cfg(feature = "grpc")]
impl GrpcServerConfig {
    pub fn new() -> Self {
        Self {
            addr: "0.0.0.0:50051".to_string(),
            descriptor_bytes: None,
        }
    }

    pub fn with_addr(mut self, addr: impl Into<String>) -> Self {
        self.addr = addr.into();
        self
    }

    pub fn with_reflection(mut self, descriptor_bytes: &'static [u8]) -> Self {
        self.descriptor_bytes = Some(descriptor_bytes);
        self
    }
}

#[cfg(feature = "grpc")]
impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "grpc")]
pub fn spawn_grpc_server<F>(
    config: GrpcServerConfig,
    service_builder: F,
) -> tokio::task::JoinHandle<()>
where
    F: FnOnce(tonic::transport::Server) -> tonic::transport::server::Router + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = run_grpc_server(config, service_builder).await {
            tracing::error!(error = %e, "gRPC server failed");
        }
    })
}

#[cfg(feature = "grpc")]
async fn run_grpc_server<F>(config: GrpcServerConfig, service_builder: F) -> anyhow::Result<()>
where
    F: FnOnce(tonic::transport::Server) -> tonic::transport::server::Router,
{
    let addr: SocketAddr = config
        .addr
        .parse()
        .with_context(|| format!("Invalid gRPC bind address: {}", config.addr))?;

    tracing::info!(addr = %addr, "Starting gRPC server");

    let router_with_user_services = service_builder(TonicServer::builder());

    let final_router = if let Some(descriptor_bytes) = config.descriptor_bytes {
        let reflection_service = build_reflection_service(descriptor_bytes)?;
        tracing::info!("gRPC reflection enabled");
        router_with_user_services.add_service(reflection_service)
    } else {
        router_with_user_services
    };

    final_router
        .serve(addr)
        .await
        .context("gRPC server failed to serve")
}

#[cfg(feature = "grpc-reflection")]
fn build_reflection_service(
    descriptor_bytes: &'static [u8],
) -> anyhow::Result<
    tonic_reflection::server::v1::ServerReflectionServer<
        impl tonic_reflection::server::v1::ServerReflection,
    >,
> {
    use tonic_reflection::server::Builder as ReflectionBuilder;

    ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(descriptor_bytes)
        .build_v1()
        .context("Failed to build reflection service")
}

#[cfg(all(feature = "grpc", not(feature = "grpc-reflection")))]
fn build_reflection_service<T>(_descriptor_bytes: &'static [u8]) -> anyhow::Result<T> {
    anyhow::bail!("gRPC reflection support not enabled. Enable the 'grpc-reflection' feature.")
}
