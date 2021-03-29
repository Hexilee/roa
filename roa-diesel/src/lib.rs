#![cfg_attr(feature = "docs", feature(external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

mod async_ext;
mod pool;

#[doc(inline)]
pub use diesel::r2d2::ConnectionManager;
#[doc(inline)]
pub use pool::{builder, make_pool, Pool, WrapConnection};

/// preload ext traits.
pub mod preload {
    #[doc(inline)]
    pub use crate::async_ext::SqlQuery;
    #[doc(inline)]
    pub use crate::pool::AsyncPool;
}
