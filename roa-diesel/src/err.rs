use diesel::r2d2::PoolError;
use diesel::result::Error as DieselError;
use roa_core::http::StatusCode;
use roa_core::Error;
use std::fmt::{self, Display, Formatter};

pub type Result<T> = std::result::Result<T, WrapError>;

#[derive(Debug)]
pub enum WrapError {
    Diesel(DieselError),
    Pool(PoolError),
}

impl Display for WrapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use WrapError::*;
        match self {
            Diesel(err) => f.write_fmt(format_args!("Diesel error:\n{}", err)),
            Pool(err) => f.write_fmt(format_args!("Pool error:\n{}", err)),
        }
    }
}

impl From<DieselError> for WrapError {
    fn from(err: DieselError) -> Self {
        WrapError::Diesel(err)
    }
}

impl From<PoolError> for WrapError {
    fn from(err: PoolError) -> Self {
        WrapError::Pool(err)
    }
}

impl From<WrapError> for Error {
    fn from(err: WrapError) -> Self {
        Error::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
    }
}

impl std::error::Error for WrapError {}
