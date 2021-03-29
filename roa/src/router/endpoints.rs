mod dispatcher;
mod guard;

use crate::http::{Method, StatusCode};
use crate::{throw, Result};

#[inline]
fn method_not_allowed(method: &Method) -> Result {
    throw!(
        StatusCode::METHOD_NOT_ALLOWED,
        format!("Method {} not allowed", method)
    )
}

pub use dispatcher::{connect, delete, get, head, options, patch, post, put, trace, Dispatcher};
pub use guard::{allow, deny, Guard};
