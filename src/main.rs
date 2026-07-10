mod app;
mod audio_capture;
mod audio_reactive;
mod config;
mod dualsense;
mod launch_agent;
mod mapping;
mod model;
mod ui;

fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => app::run(),
        [command] if command == "--daemon" => app::run_daemon(),
        [command] if command == "--gui-service" => app::run_gui_service(),
        [command] if command == "--install-agent" => {
            let status = launch_agent::install_current_executable()?;
            println!(
                "Installed and loaded {} at {}",
                launch_agent::LAUNCH_AGENT_LABEL,
                status.plist_path.display()
            );
            Ok(())
        }
        [command] if command == "--uninstall-agent" => {
            launch_agent::uninstall()?;
            println!("Removed {}", launch_agent::LAUNCH_AGENT_LABEL);
            Ok(())
        }
        [command] if command == "--agent-status" => {
            let status = launch_agent::status()?;
            println!(
                "{}\n  plist: {}\n  installed: {}\n  loaded: {}",
                launch_agent::LAUNCH_AGENT_LABEL,
                status.plist_path.display(),
                status.installed,
                status.loaded
            );
            Ok(())
        }
        [command] if command == "--request-event-posting-access" => {
            let mapper = mapping::MouseMapper::new();
            let granted = mapper.request_event_posting_access();
            println!(
                "DualSenseTUI event-posting access: {}",
                mapper.permission_status()
            );
            if !granted {
                eprintln!(
                    "Grant event-posting access to the signed DualSenseTUI.app bundle, then run this command again."
                );
            }
            Ok(())
        }
        [command] if command == "--help" || command == "-h" => {
            print_usage();
            Ok(())
        }
        _ => {
            print_usage();
            anyhow::bail!("unknown command: {}", arguments.join(" "))
        }
    }
}

fn print_usage() {
    println!(
        "Usage: DualSenseTUI [--daemon | --gui-service | --install-agent | --uninstall-agent | --agent-status | --request-event-posting-access]"
    );
}
