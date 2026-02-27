mod application;
mod backend;
mod config;
mod error;
mod models;
mod ui;

use gtk4::prelude::*;

fn main() {
    env_logger::init();

    let app = application::GrustyvmanApplication::new();
    app.run();
}
