pub mod config;
pub mod country;
pub mod data_utils;
pub mod grpc;
#[cfg(feature = "http-client")]
pub mod http_client;
pub mod language;
pub mod logging;
pub mod macros;
pub mod persistence;
#[cfg(feature = "rest")]
pub mod server;
pub mod structs;
pub mod uuid_adapter;

pub mod state {
    pub use crate::macros::AppState;
}

pub mod db {
    pub use crate::persistence::{DbPool, create_pool};
}

#[cfg(feature = "grpc")]
pub mod grpc_errors {
    pub use crate::grpc::{FromProto, GrpcResult, IntoGrpcResult, IntoProto, parse_uuid};
}

#[cfg(feature = "grpc")]
pub use grpc::{FromProto, IntoGrpcResult, IntoProto};
pub use structs::{ApiResponse, Validate};
