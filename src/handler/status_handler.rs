use crate::{Context, DynTargetHandler, State, Status, TargetHandler};

pub type DynStatusHandler<S> = DynTargetHandler<S, Status>;

pub trait StatusHandler<S: State>: TargetHandler<S, Status> {}

impl<S, T> StatusHandler<S> for T
where
    S: State,
    T: TargetHandler<S, Status>,
{
}

pub async fn default_status_handler<S: State>(
    mut context: Context<S>,
    status: Status,
) -> Result<(), Status> {
    if !status.success() {
        log::error!("{}", &status);
    }
    context.response.status(status.status_code);
    if status.expose {
        context.response.write_str(&status.message);
    }
    if status.need_throw() {
        Err(status)
    } else {
        Ok(())
    }
}
