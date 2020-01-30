use crate::{Context, DynTargetHandler, Model, Status, TargetHandler};

pub type DynStatusHandler<M> = DynTargetHandler<M, Status>;

pub trait StatusHandler<M: Model>: TargetHandler<M, Status> {}

impl<M, T> StatusHandler<M> for T
where
    M: Model,
    T: TargetHandler<M, Status>,
{
}

pub async fn default_status_handler<M: Model>(
    mut context: Context<M>,
    status: Status,
) -> Result<(), Status> {
    context.response.status = status.status_code;
    if status.expose {
        context.response.write_str(&status.message);
    }
    if status.need_throw() {
        Err(status)
    } else {
        Ok(())
    }
}
