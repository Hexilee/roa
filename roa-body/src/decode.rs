use encoding::label::encoding_from_whatwg_label;
use http::StatusCode;
use roa_core::{throw, Status};

pub fn decode(raw_data: &[u8], encoding: &str) -> Result<String, Status> {
    match encoding_from_whatwg_label(encoding) {
        Some(encoder) => encoder
            .decode(raw_data, encoding::DecoderTrap::Strict)
            .map_err(|err| {
                Status::new(
                    StatusCode::BAD_REQUEST,
                    format!("{}\nbody cannot be decoded by `{}`", err, encoding),
                    true,
                )
            }),
        None => throw(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("encoding(`{}`) is unsupported", encoding),
        ),
    }
}
