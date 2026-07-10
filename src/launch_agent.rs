//! Per-user macOS LaunchAgent support for the background DualSense service.
//!
//! The LaunchAgent deliberately invokes the same executable with `--daemon`.
//! This keeps the installed item self-contained and means that upgrades only
//! need to reinstall the agent to point it at the new executable location.

use std::path::PathBuf;

use anyhow::Result;

#[cfg(not(target_os = "macos"))]
use anyhow::bail;
#[cfg(any(target_os = "macos", test))]
use anyhow::Context;
#[cfg(any(target_os = "macos", test))]
use std::path::Path;
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::{
    env, fs,
    io::Write,
    process,
    sync::atomic::{AtomicU64, Ordering},
};

/// The label registered with `launchd` for the background service.
pub const LAUNCH_AGENT_LABEL: &str = "com.github.minashkinvladislav.dualsensetui.autostart";

#[cfg(any(target_os = "macos", test))]
const PLIST_FILE_NAME: &str = "com.github.minashkinvladislav.dualsensetui.autostart.plist";
#[cfg(any(target_os = "macos", test))]
const DAEMON_ARGUMENT: &str = "--daemon";

#[cfg(target_os = "macos")]
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Installation state for this user's LaunchAgent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchAgentStatus {
    /// The expected on-disk plist location.
    pub plist_path: PathBuf,
    /// Whether the plist currently exists on disk.
    pub installed: bool,
    /// Whether launchd currently has the agent loaded.
    pub loaded: bool,
}

/// Install and load a per-user LaunchAgent for the current executable.
///
/// The agent runs the executable with `--daemon`, starts at login, and is kept
/// alive by launchd. Reinstalling first attempts to unload any previous copy so
/// a changed executable path takes effect immediately.
pub fn install_current_executable() -> Result<LaunchAgentStatus> {
    #[cfg(target_os = "macos")]
    {
        install_current_executable_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        unsupported_platform()
    }
}

/// Unload and remove this user's LaunchAgent plist.
pub fn uninstall() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        uninstall_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        unsupported_platform()
    }
}

/// Return whether this user's LaunchAgent is installed and currently loaded.
pub fn status() -> Result<LaunchAgentStatus> {
    #[cfg(target_os = "macos")]
    {
        status_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        unsupported_platform()
    }
}

#[cfg(target_os = "macos")]
fn install_current_executable_macos() -> Result<LaunchAgentStatus> {
    let executable = env::current_exe().context("failed to determine the current executable")?;
    let plist_path = plist_path()?;
    let plist = render_plist(&executable)?;

    write_plist(&plist_path, plist.as_bytes())?;

    // `bootout` returns a non-zero status when this is the first installation,
    // which is expected. Bootstrap below is the operation whose failure makes
    // installation unsuccessful.
    let _ = launchctl_bootout(&plist_path);
    launchctl_bootstrap(&plist_path)?;

    status_macos()
}

#[cfg(target_os = "macos")]
fn uninstall_macos() -> Result<()> {
    let plist_path = plist_path()?;

    // It is also valid for the service not to be loaded, so continue with
    // removal if launchctl reports that it cannot find the job.
    let _ = launchctl_bootout(&plist_path);

    match fs::remove_file(&plist_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to remove LaunchAgent plist {}",
                plist_path.display()
            )
        }),
    }
}

#[cfg(target_os = "macos")]
fn status_macos() -> Result<LaunchAgentStatus> {
    let plist_path = plist_path()?;
    let domain = user_launch_domain()?;
    let service_target = format!("{domain}/{LAUNCH_AGENT_LABEL}");

    let output = Command::new("/bin/launchctl")
        .arg("print")
        .arg(&service_target)
        .output()
        .context("failed to execute launchctl print")?;

    Ok(LaunchAgentStatus {
        installed: plist_path.is_file(),
        loaded: output.status.success(),
        plist_path,
    })
}

#[cfg(target_os = "macos")]
fn launchctl_bootstrap(plist_path: &Path) -> Result<()> {
    let domain = user_launch_domain()?;
    let output = Command::new("/bin/launchctl")
        .arg("bootstrap")
        .arg(&domain)
        .arg(plist_path)
        .output()
        .context("failed to execute launchctl bootstrap")?;

    if output.status.success() {
        return Ok(());
    }

    Err(launchctl_failure("bootstrap", &domain, &output))
}

#[cfg(target_os = "macos")]
fn launchctl_bootout(plist_path: &Path) -> Result<()> {
    let domain = user_launch_domain()?;
    let output = Command::new("/bin/launchctl")
        .arg("bootout")
        .arg(&domain)
        .arg(plist_path)
        .output()
        .context("failed to execute launchctl bootout")?;

    if output.status.success() {
        return Ok(());
    }

    Err(launchctl_failure("bootout", &domain, &output))
}

#[cfg(target_os = "macos")]
fn launchctl_failure(action: &str, domain: &str, output: &std::process::Output) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let detail = if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    };

    anyhow::anyhow!(
        "launchctl {action} failed for {LAUNCH_AGENT_LABEL} in {domain} (status {}){detail}",
        output.status
    )
}

#[cfg(target_os = "macos")]
fn user_launch_domain() -> Result<String> {
    let output = Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .context("failed to determine the current user ID")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "/usr/bin/id -u failed with status {}",
            output.status
        ));
    }

    let uid = String::from_utf8(output.stdout)
        .context("/usr/bin/id -u returned a non-UTF-8 user ID")?
        .trim()
        .parse::<u32>()
        .context("/usr/bin/id -u returned an invalid user ID")?;

    Ok(format!("gui/{uid}"))
}

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf> {
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .context("HOME is not set; cannot determine the LaunchAgents directory")?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        anyhow::bail!("HOME must be an absolute path to install a LaunchAgent");
    }

    Ok(plist_path_for_home(&home))
}

#[cfg(target_os = "macos")]
fn write_plist(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("LaunchAgent plist path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create LaunchAgents directory {}",
            parent.display()
        )
    })?;

    let temporary_path = temporary_plist_path(path);
    let result = (|| -> Result<()> {
        let mut file = fs::File::create(&temporary_path).with_context(|| {
            format!(
                "failed to create temporary LaunchAgent plist {}",
                temporary_path.display()
            )
        })?;
        file.write_all(contents).with_context(|| {
            format!(
                "failed to write temporary LaunchAgent plist {}",
                temporary_path.display()
            )
        })?;
        file.sync_all().with_context(|| {
            format!(
                "failed to sync temporary LaunchAgent plist {}",
                temporary_path.display()
            )
        })?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error)
            .with_context(|| format!("failed to replace LaunchAgent plist {}", path.display()));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn temporary_plist_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(PLIST_FILE_NAME);
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", process::id(), sequence))
}

#[cfg(any(target_os = "macos", test))]
fn plist_path_for_home(home: &Path) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(PLIST_FILE_NAME)
}

#[cfg(any(target_os = "macos", test))]
fn render_plist(executable: &Path) -> Result<String> {
    if !executable.is_absolute() {
        anyhow::bail!(
            "the LaunchAgent executable path must be absolute: {}",
            executable.display()
        );
    }

    let executable = executable
        .to_str()
        .context("the LaunchAgent executable path is not valid UTF-8")?;
    let executable = xml_escape(executable)?;

    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{executable}</string>
        <string>{DAEMON_ARGUMENT}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
"#
    ))
}

#[cfg(any(target_os = "macos", test))]
fn xml_escape(value: &str) -> Result<String> {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if !is_valid_xml_character(character) {
            anyhow::bail!("the executable path contains a character invalid in XML 1.0");
        }

        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    Ok(escaped)
}

#[cfg(any(target_os = "macos", test))]
fn is_valid_xml_character(character: char) -> bool {
    matches!(
        character as u32,
        0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
    )
}

#[cfg(not(target_os = "macos"))]
fn unsupported_platform<T>() -> Result<T> {
    bail!("LaunchAgent autostart is only available on macOS")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_path_is_in_the_users_launch_agents_directory() {
        let home = Path::new("/tmp/dualsense-tui-home");
        assert_eq!(
            plist_path_for_home(home),
            home.join("Library")
                .join("LaunchAgents")
                .join(PLIST_FILE_NAME)
        );
    }

    #[test]
    fn rendered_plist_uses_the_daemon_argument_and_escapes_xml() {
        let plist = render_plist(Path::new(
            "/Applications/DualSense & Co/<alpha>/DualSense \"TUI\"'.app",
        ))
        .unwrap();

        assert!(plist.contains(&format!("<string>{LAUNCH_AGENT_LABEL}</string>")));
        assert!(plist.contains(
            "<string>/Applications/DualSense &amp; Co/&lt;alpha&gt;/DualSense &quot;TUI&quot;&apos;.app</string>"
        ));
        assert!(plist.contains(&format!("<string>{DAEMON_ARGUMENT}</string>")));
        assert!(plist.contains("<key>RunAtLoad</key>\n    <true/>"));
        assert!(plist.contains("<key>KeepAlive</key>\n    <true/>"));
    }

    #[test]
    fn rendered_plist_rejects_a_relative_executable_path() {
        let error = render_plist(Path::new("DualSenseTUI")).unwrap_err();
        assert!(error.to_string().contains("must be absolute"));
    }

    #[test]
    fn xml_escape_rejects_characters_not_allowed_in_xml_1_0() {
        assert!(xml_escape("valid\u{0001}path").is_err());
    }
}
