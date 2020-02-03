#[doc(inline)]
pub use roa_core::*;

mod logger;

mod forward;

mod query;

#[doc(inline)]
pub use logger::logger;

#[doc(inline)]
pub use query::{query_parser, Query};

#[doc(inline)]
pub use forward::Forward;

#[cfg(feature = "cookies")]
mod cookie;

#[cfg(feature = "cookies")]
#[doc(inline)]
pub use crate::cookie::{cookie_parser, Cookier};

#[cfg(feature = "jwt")]
mod jwt;

#[cfg(feature = "jwt")]
#[doc(inline)]
pub use crate::jwt::{jwt_verify, JwtVerifier};

#[cfg(feature = "body")]
mod body;

#[cfg(feature = "body")]
#[doc(inline)]
pub use crate::body::PowerBody;

#[cfg(feature = "router")]
mod router;

#[cfg(feature = "router")]
#[doc(inline)]
pub use crate::router::{
    Conflict as RouterConflict, Endpoint, Error as RouterError, Router, RouterParam,
};

#[cfg(feature = "compress")]
mod compress;