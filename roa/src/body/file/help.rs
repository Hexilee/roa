use crate::http::StatusCode;
use crate::Status;

const BUG_HELP: &str =
    r"This is a bug, please report it to https://github.com/Hexilee/roa.";

#[inline]
pub fn bug_report(message: impl ToString) -> Status {
    Status::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("{}\n{}", message.to_string(), BUG_HELP),
        false,
    )
}
