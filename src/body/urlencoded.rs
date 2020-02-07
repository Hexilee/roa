use crate::{Error, Result};
use http::StatusCode;
use serde::de::DeserializeOwned;

pub fn from_bytes<B: DeserializeOwned>(data: &[u8]) -> Result<B> {
    serde_urlencoded::from_bytes(data).map_err(|err| {
        Error::new(
            StatusCode::BAD_REQUEST,
            format!("{}\ninvalid body", err),
            true,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;

    #[test]
    fn from_bytes_fails() {
        let ret = from_bytes::<i32>(b"");
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status.message.ends_with("invalid body"));
    }
}
