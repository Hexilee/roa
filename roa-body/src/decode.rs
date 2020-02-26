use encoding::label::encoding_from_whatwg_label;
use roa_core::http::StatusCode;
use roa_core::{throw, Error, Result};

pub fn decode(raw_data: &[u8], encoding: &str) -> Result<String> {
    match encoding_from_whatwg_label(encoding) {
        Some(encoder) => encoder
            .decode(raw_data, encoding::DecoderTrap::Strict)
            .map_err(|err| {
                Error::new(
                    StatusCode::BAD_REQUEST,
                    format!("{}\nbody cannot be decoded by `{}`", err, encoding),
                    true,
                )
            }),
        None => throw!(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("encoding(`{}`) is unsupported", encoding)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::decode;
    use http::StatusCode;

    #[test]
    fn decode_fails() {
        let ret = decode([255].as_ref(), "utf-8");
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status
            .message
            .ends_with("body cannot be decoded by `utf-8`"));
    }

    #[test]
    fn unsupported_encoding() {
        let ret = decode(b"", "rust");
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::UNSUPPORTED_MEDIA_TYPE, status.status_code);
        assert!(status.message.ends_with("encoding(`rust`) is unsupported"));
    }
}
