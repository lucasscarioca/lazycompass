mod app;
mod cli;
mod commands;
mod editor;
mod errors;
mod logging;
mod output;

fn main() {
    if let Err(error) = app::run() {
        errors::report_error(&error);
        std::process::exit(errors::exit_code(&error));
    }
}
