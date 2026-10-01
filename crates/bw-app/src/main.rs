// Pas de console en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clock;
mod controller;
mod demo;
mod geometry;
mod platform;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let Some(_instance) = platform::single_instance() else {
        log::info!("BoringWindows est déjà lancé");
        return Ok(());
    };

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_window_attributes_hook(platform::window_attributes)
        .select()?;

    controller::run()
}
