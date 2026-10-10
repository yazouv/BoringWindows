// Pas de console en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clock;
mod controller;
mod demo;
mod geometry;
mod gestures;
mod mascot;
mod platform;
mod shelf;
#[cfg(test)]
mod translations;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    // `boringwindows hook` : relais appelé par Claude Code. Il sort avant
    // toute initialisation de l'UI, et ne doit rien écrire d'autre sur stdout.
    match std::env::args().nth(1).as_deref() {
        Some("hook") => std::process::exit(bw_claude::hook::run()),
        Some("doctor") => return doctor(),
        _ => {}
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Après une mise à jour, l'ancienne version peut mettre un instant à se fermer.
    let restarted = std::env::args().any(|a| a == "--restarted");
    let mut instance = platform::single_instance();
    for _ in 0..50 {
        if instance.is_some() || !restarted {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        instance = platform::single_instance();
    }
    let Some(_instance) = instance else {
        log::info!("BoringWindows est déjà lancé");
        return Ok(());
    };

    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into());
    platform::configure_backend(selector).select()?;

    controller::run(std::env::args().any(|a| a == "--settings"))
}

/// `boringwindows doctor` : diagnostic Claude Code, affiché et enregistré.
fn doctor() -> anyhow::Result<()> {
    platform::attach_parent_console();
    let report = bw_claude::doctor::run(&std::env::current_exe()?);
    println!("{report}");
    let dir = bw_claude::install::data_dir();
    let path = dir.join("doctor.txt");
    if std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&path, &report))
        .is_ok()
    {
        println!("Rapport enregistré dans {}", path.display());
    }
    Ok(())
}
