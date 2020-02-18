use roa_core::{Error, StatusCode};

const BUG_HELP: &str =
    r"This is a bug, please report it to https://github.com/Hexilee/roa.";

pub fn bug_report(message: impl ToString) -> Error {
    Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("{}\n{}", message.to_string(), BUG_HELP),
        false,
    )
}
