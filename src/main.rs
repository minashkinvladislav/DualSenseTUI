mod app;
mod config;
mod dualsense;
mod model;
mod ui;

fn main() -> anyhow::Result<()> {
    app::run()
}
