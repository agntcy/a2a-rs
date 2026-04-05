// Copyright AGNTCY Contributors (https://github.com/agntcy)
// SPDX-License-Identifier: Apache-2.0
/// Generated protobuf types for the A2A v1 protocol.
pub mod proto {
    include!("gen/lf.a2a.v1.rs");
}

/// ProtoJSON-capable generated protobuf types for the A2A v1 protocol.
pub mod protojson {
    include!(concat!(env!("OUT_DIR"), "/lf.a2a.v1.rs"));
    include!(concat!(env!("OUT_DIR"), "/lf.a2a.v1.serde.rs"));
}

pub mod pbconv;
pub mod protojson_conv;

pub use proto::*;
