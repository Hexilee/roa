mod content_disposition;
mod help;
use std::convert::TryInto;

use async_std::fs::File;
pub use async_std::path::Path;
use content_disposition::ContentDisposition;
pub use content_disposition::DispositionType;

use crate::{http, Context, Result, State};

/// Write file to response body then set "Content-Type" and "Context-Disposition".
#[inline]
pub async fn write_file<S: State>(
    ctx: &mut Context<S>,
    path: impl AsRef<Path>,
    typ: DispositionType,
) -> Result {
    let path = path.as_ref();
    ctx.resp.write_reader(File::open(path).await?);

    if let Some(filename) = path.file_name() {
        ctx.resp.headers.insert(
            http::header::CONTENT_TYPE,
            mime_guess::from_path(&filename)
                .first_or_octet_stream()
                .as_ref()
                .parse()
                .map_err(help::bug_report)?,
        );

        let name = filename.to_string_lossy();
        let content_disposition = ContentDisposition::new(typ, Some(&name));
        ctx.resp.headers.insert(
            http::header::CONTENT_DISPOSITION,
            content_disposition.try_into()?,
        );
    }
    Ok(())
}
