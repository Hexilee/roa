#![cfg_attr(feature = "docs", feature(external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

mod async_ext;
mod pool;

#[doc(inline)]
pub use async_ext::SqlQuery;

#[doc(inline)]
pub use pool::{builder, make_pool, AsyncPool, Pool, WrapConnection};

#[doc(inline)]
pub use diesel::r2d2::ConnectionManager;
