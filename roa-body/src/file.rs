use crate::bug_report;
use crate::content_type::ContentType;
use actix_http::http::header::{
    Charset, ContentDisposition, DispositionParam, DispositionType, ExtendedValue,
    IntoHeaderValue,
};
use async_std::fs::File;
pub use async_std::path::Path;
use roa_core::{Context, Result, State};

pub async fn write_file<S: State, P: AsRef<Path> + Send>(
    ctx: &mut Context<S>,
    path: P,
) -> Result {
    let path = path.as_ref();
    ctx.resp_mut().write(File::open(path).await?);

    if let Some(filename) = path.file_name() {
        ctx.resp_mut().headers.insert(
            http::header::CONTENT_TYPE,
            ContentType(mime_guess::from_path(&filename).first_or_octet_stream())
                .to_value()?,
        );

        let content_disposition = ContentDisposition {
            disposition: DispositionType::Inline,
            parameters: vec![
                DispositionParam::FilenameExt(ExtendedValue {
                    charset: Charset::Ext(String::from("UTF-8")),
                    language_tag: None,
                    value: filename.to_string_lossy().as_bytes().to_vec(),
                }),
                // fallback for better compatibility
                DispositionParam::Filename(filename.to_string_lossy().to_string()),
            ],
        };
        ctx.resp_mut().headers.insert(
            http::header::CONTENT_DISPOSITION,
            IntoHeaderValue::try_into(content_disposition).map_err(bug_report)?,
        );
    }
    Ok(())
}
