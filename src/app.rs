mod app;
mod static_app;

pub use static_app::StaticApp;
pub fn new() -> StaticApp {
    StaticApp::new()
}
