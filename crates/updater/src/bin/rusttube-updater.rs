use anyhow::Result;
use rusttube_updater::UpdateManager;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    // Usage: rusttube-updater <install_dir> <staged_binary_path> <target_version>
    if args.len() < 4 {
        eprintln!("Usage: rusttube-updater <install_dir> <staged_binary_path> <target_version>");
        std::process::exit(1);
    }

    let install_dir = PathBuf::from(&args[1]);
    let staged_binary = PathBuf::from(&args[2]);
    let target_version = &args[3];

    // Wait briefly for the main RustTube process to release its executable file lock
    thread::sleep(Duration::from_millis(800));

    let exe_name = if cfg!(windows) {
        "rusttube.exe"
    } else {
        "rusttube"
    };

    UpdateManager::apply_staged_binary_with_rollback(&install_dir, &staged_binary, exe_name)?;

    // Relaunch updated application with verification flag
    let relaunch_path = install_dir.join(exe_name);
    if relaunch_path.exists() {
        let _ = Command::new(relaunch_path)
            .arg("--verified-update")
            .arg(target_version)
            .spawn();
    }

    Ok(())
}
