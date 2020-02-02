use crate::Status;
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn from_bytes<B: DeserializeOwned>(data: &[u8]) -> Result<B, Status> {
    serde_json::from_slice(data).map_err(|err| {
        Status::new(
            StatusCode::BAD_REQUEST,
            format!("{}\ninvalid body", err),
            true,
        )
    })
}

pub fn from_str<B: DeserializeOwned>(data: &str) -> Result<B, Status> {
    serde_json::from_str(data).map_err(|err| {
        Status::new(
            StatusCode::BAD_REQUEST,
            format!("{}\ninvalid body", err),
            true,
        )
    })
}

pub fn to_bytes<B: Serialize>(object: &B) -> Result<Vec<u8>, Status> {
    serde_json::to_vec(object).map_err(|err| {
        Status::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{}\nobject cannot be serialized to json", err),
            false,
        )
    })
}
