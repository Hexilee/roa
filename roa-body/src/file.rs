use crate::bug_report;
use crate::content_type::ContentType;
use actix_http::http::header::{
    Charset, ContentDisposition, DispositionParam, DispositionType, ExtendedValue,
    IntoHeaderValue,
};
use async_std::fs::File;
pub use async_std::path::Path;
use roa_core::{async_trait, Context, Result, State};

#[async_trait]
pub trait WriteFile {
    async fn write_file<P: AsRef<Path> + Send>(&mut self, path: P) -> Result;
}

#[async_trait]
impl<S: State> WriteFile for Context<S> {
    async fn write_file<P: AsRef<Path> + Send>(&mut self, path: P) -> Result {
        let path = path.as_ref();
        self.resp_mut().await.write(File::open(path).await?);

        if let Some(filename) = path.file_name() {
            self.resp_mut().await.headers.insert(
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
            self.resp_mut().await.headers.insert(
                http::header::CONTENT_DISPOSITION,
                IntoHeaderValue::try_into(content_disposition).map_err(bug_report)?,
            );
        }
        Ok(())
    }
}
