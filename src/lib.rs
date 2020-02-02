#[doc(inline)]
pub use roa_core::*;

mod query;

pub mod context {
    #[cfg(feature = "query")]
    #[doc(inline)]
    pub use crate::query::Query;

    // #[cfg(feature = "cookie")]
    // #[doc(inline)]
    // pub use roa_cookie::{Cookie, Cookier};

    // #[cfg(feature = "body")]
    // #[doc(inline)]
    // pub use roa_body::PowerBody;
}

pub mod middlewares {
    #[cfg(feature = "query")]
    #[doc(inline)]
    pub use crate::query::query_parser;
    // #[cfg(feature = "jwt")]
    // #[doc(inline)]
    // pub use roa_jwt::{jwt_verify, JwtVerifier, Validation};

    // #[cfg(feature = "router")]
    // #[doc(inline)]
    // pub use roa_router;
}
