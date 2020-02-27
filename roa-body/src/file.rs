mod content_disposition;
use crate::content_type::ContentType;
use async_std::fs::File;
pub use async_std::path::Path;
use content_disposition::ContentDisposition;
pub use content_disposition::DispositionType;
use roa_core::{http, Context, Result, State};

pub async fn write_file<S: State, P: AsRef<Path> + Send>(
    ctx: &mut Context<S>,
    path: P,
    typ: DispositionType,
) -> Result {
    let path = path.as_ref();
    ctx.resp_mut().write(File::open(path).await?);

    if let Some(filename) = path.file_name() {
        ctx.resp_mut().headers.insert(
            http::header::CONTENT_TYPE,
            ContentType(mime_guess::from_path(&filename).first_or_octet_stream())
                .to_value()?,
        );

        let name = filename.to_string_lossy();
        let content_disposition = ContentDisposition::new(typ, Some(&*name));
        ctx.resp_mut().headers.insert(
            http::header::CONTENT_DISPOSITION,
            content_disposition.value()?,
        );
    }
    Ok(())
}
