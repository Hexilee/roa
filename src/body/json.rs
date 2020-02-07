use crate::{Error, Result};
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn from_bytes<B: DeserializeOwned>(data: &[u8]) -> Result<B> {
    serde_json::from_slice(data).map_err(|err| {
        Error::new(
            StatusCode::BAD_REQUEST,
            format!("{}\ninvalid body", err),
            true,
        )
    })
}

pub fn from_str<B: DeserializeOwned>(data: &str) -> Result<B> {
    serde_json::from_str(data).map_err(|err| {
        Error::new(
            StatusCode::BAD_REQUEST,
            format!("{}\ninvalid body", err),
            true,
        )
    })
}

pub fn to_bytes<B: Serialize>(object: &B) -> Result<Vec<u8>> {
    serde_json::to_vec(object).map_err(|err| {
        Error::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{}\nobject cannot be serialized to json", err),
            false,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;

    #[test]
    fn from_str_fails() {
        let ret = from_str::<i32>("");
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status.message.ends_with("invalid body"));
    }

    #[test]
    fn from_bytes_fails() {
        let ret = from_bytes::<i32>(b"");
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status.message.ends_with("invalid body"));
    }
}
