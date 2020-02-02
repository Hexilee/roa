use crate::Status;
use http::StatusCode;
use serde::de::DeserializeOwned;

pub fn from_bytes<B: DeserializeOwned>(data: &[u8]) -> Result<B, Status> {
    serde_urlencoded::from_bytes(data).map_err(|err| {
        Status::new(
            StatusCode::BAD_REQUEST,
            format!("{}\ninvalid body", err),
            true,
        )
    })
}
