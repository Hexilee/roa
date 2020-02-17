use diesel::result::Error as DieselError;
use roa_core::{Error, StatusCode};
use std::fmt::{self, Display, Formatter};

pub type Result<T> = std::result::Result<T, WrapError>;

#[derive(Debug)]
pub struct WrapError(DieselError);

impl Display for WrapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Diesel error:\n{}", self.0))
    }
}

impl From<DieselError> for WrapError {
    fn from(err: DieselError) -> Self {
        Self(err)
    }
}

impl From<WrapError> for Error {
    fn from(err: WrapError) -> Self {
        Error::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
    }
}

impl std::error::Error for WrapError {}
