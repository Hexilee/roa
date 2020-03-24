use crate::http::StatusCode;
use crate::Status;
use diesel::r2d2::PoolError;
use diesel::result::Error as DieselError;
use std::fmt::{self, Display, Formatter};

/// A wrapper for diesel error and r2d2 error.
#[derive(Debug)]
pub enum WrapError {
    /// Diesel error.
    Diesel(DieselError),

    /// R2D2 error.
    Pool(PoolError),
}

impl Display for WrapError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use WrapError::*;
        match self {
            Diesel(err) => f.write_fmt(format_args!("Diesel error:\n{}", err)),
            Pool(err) => f.write_fmt(format_args!("Pool error:\n{}", err)),
        }
    }
}

impl From<DieselError> for WrapError {
    #[inline]
    fn from(err: DieselError) -> Self {
        WrapError::Diesel(err)
    }
}

impl From<PoolError> for WrapError {
    #[inline]
    fn from(err: PoolError) -> Self {
        WrapError::Pool(err)
    }
}

impl From<WrapError> for Status {
    #[inline]
    fn from(err: WrapError) -> Self {
        Status::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
    }
}

impl std::error::Error for WrapError {}