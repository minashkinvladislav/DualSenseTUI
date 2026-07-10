use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::model::{Profile, Rgb};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const NAMED_PROFILE_DIRECTORY: &str = "saved-profiles";
const MAX_NAMED_PROFILE_NAME_LENGTH: usize = 48;
const MAX_NAMED_PROFILE_ID_BYTES: usize = 96;

/// A reusable, user-named profile stored independently from a controller's
/// auto-applied device profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedProfile {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct NamedProfileDocument {
    name: String,
    profile: Profile,
}

pub fn profile_path() -> PathBuf {
    config_root().join("DualSenseTUI").join("profile.json")
}

/// Returns the per-user socket used by the background core to serve the macOS
/// desktop shell. Keeping it beside the profile data scopes it to the current
/// user and avoids exposing controller output controls over the network.
pub fn daemon_socket_path() -> PathBuf {
    config_root().join("DualSenseTUI").join("daemon.sock")
}

/// Returns the path of the profile stored for one controller.
///
/// MAC addresses are normalized so that equivalent colon-, dash-, and
/// whitespace-separated forms select the same profile. Unexpected input is
/// still converted to a single safe filename and can never escape the profile
/// directory.
pub fn profile_path_for_device(mac_address: &str) -> PathBuf {
    config_root()
        .join("DualSenseTUI")
        .join("profiles")
        .join(device_profile_file_name(mac_address))
}

/// Returns whether a profile has been explicitly saved for this controller.
pub fn device_profile_exists(mac_address: &str) -> bool {
    profile_path_for_device(mac_address).is_file()
}

/// Returns the directory containing reusable, named profile presets.
pub fn named_profiles_path() -> PathBuf {
    config_root()
        .join("DualSenseTUI")
        .join(NAMED_PROFILE_DIRECTORY)
}

pub fn list_named_profiles() -> Result<Vec<NamedProfile>> {
    list_named_profiles_in(&named_profiles_path())
}

pub fn save_named_profile(name: &str, profile: &Profile) -> Result<NamedProfile> {
    save_named_profile_to(&named_profiles_path(), name, profile)
}

pub fn load_named_profile(id: &str) -> Result<(NamedProfile, Profile)> {
    load_named_profile_from(&named_profiles_path(), id)
}

pub fn load_profile() -> Result<Profile> {
    load_profile_from(&profile_path())
}

/// Loads the controller-specific profile when present.
///
/// A missing device profile falls back to the legacy global `profile.json`,
/// then to the default profile. This keeps existing installations working
/// until a profile is first saved for a controller. A controller whose MAC
/// address is not available also uses the global profile.
pub fn load_profile_for_device(mac_address: Option<&str>) -> Result<Profile> {
    let Some(mac_address) = mac_address.filter(|address| !address.trim().is_empty()) else {
        return load_profile();
    };

    let device_path = profile_path_for_device(mac_address);
    let global_path = profile_path();
    load_profile_with_fallback(Some(&device_path), &global_path)
}

fn load_profile_with_fallback(device_path: Option<&Path>, global_path: &Path) -> Result<Profile> {
    if let Some(device_path) = device_path.filter(|path| path.is_file()) {
        return load_profile_from(device_path);
    }

    load_profile_from(global_path)
}

fn load_profile_from(path: &Path) -> Result<Profile> {
    if !path.exists() {
        return Ok(Profile::default());
    }

    let data = fs::read_to_string(path)
        .with_context(|| format!("failed to read profile {}", path.display()))?;
    let mut profile: Profile = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse profile {}", path.display()))?;
    profile.normalize_mappings();
    Ok(profile)
}

pub fn save_profile(profile: &Profile) -> Result<PathBuf> {
    let path = profile_path();
    save_profile_to(&path, profile)?;
    Ok(path)
}

/// Saves a profile for one controller, or to the legacy global location when
/// the controller has no MAC address available.
pub fn save_profile_for_device(mac_address: Option<&str>, profile: &Profile) -> Result<PathBuf> {
    let Some(mac_address) = mac_address.filter(|address| !address.trim().is_empty()) else {
        return save_profile(profile);
    };

    let path = profile_path_for_device(mac_address);
    save_profile_to(&path, profile)?;
    Ok(path)
}

/// Updates only the persisted lightbar color for one controller.
///
/// Lightbar output is volatile and may be reset by macOS while another app
/// becomes active. Keeping this narrow update separate from `save_profile`
/// prevents a live color change from accidentally committing unrelated,
/// staged profile edits.
pub fn save_lightbar_for_device(mac_address: Option<&str>, color: Rgb) -> Result<PathBuf> {
    let global_path = profile_path();
    let (source_device_path, destination_path) =
        match mac_address.filter(|address| !address.trim().is_empty()) {
            Some(mac_address) => {
                let path = profile_path_for_device(mac_address);
                (Some(path.clone()), path)
            }
            None => (None, global_path.clone()),
        };

    save_lightbar_to(
        source_device_path.as_deref(),
        &global_path,
        &destination_path,
        color,
    )?;
    Ok(destination_path)
}

fn save_profile_to(path: &Path, profile: &Profile) -> Result<()> {
    let parent = path
        .parent()
        .context("profile path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config dir {}", parent.display()))?;

    let json = serde_json::to_string_pretty(profile).context("failed to serialize profile")?;
    atomic_write(path, json.as_bytes())
}

fn save_lightbar_to(
    device_path: Option<&Path>,
    global_path: &Path,
    destination_path: &Path,
    color: Rgb,
) -> Result<()> {
    let mut profile = load_profile_with_fallback(device_path, global_path)?;
    profile.lightbar = color;
    save_profile_to(destination_path, &profile)
}

fn save_named_profile_to(directory: &Path, name: &str, profile: &Profile) -> Result<NamedProfile> {
    let name = normalize_named_profile_name(name)?;
    let id = named_profile_id(&name)?;
    let descriptor = NamedProfile { id, name };
    let path = named_profile_path(directory, &descriptor.id);
    let parent = path
        .parent()
        .context("named profile path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create profile library {}", parent.display()))?;

    let document = NamedProfileDocument {
        name: descriptor.name.clone(),
        profile: profile.clone(),
    };
    let json =
        serde_json::to_string_pretty(&document).context("failed to serialize named profile")?;
    atomic_write(&path, json.as_bytes())?;
    Ok(descriptor)
}

fn load_named_profile_from(directory: &Path, id: &str) -> Result<(NamedProfile, Profile)> {
    let Some(descriptor) = list_named_profiles_in(directory)?
        .into_iter()
        .find(|profile| profile.id == id)
    else {
        bail!("saved profile '{id}' is unavailable");
    };

    let document = load_named_profile_document(&named_profile_path(directory, &descriptor.id))?;
    Ok((descriptor, document.profile))
}

fn list_named_profiles_in(directory: &Path) -> Result<Vec<NamedProfile>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read profile library {}", directory.display()))
        }
    };

    let mut profiles = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| {
            format!("failed to inspect profile library {}", directory.display())
        })?;
        let file_type = entry.file_type().with_context(|| {
            format!("failed to inspect saved profile {}", entry.path().display())
        })?;
        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !is_valid_named_profile_id(id) {
            continue;
        }

        let Ok(document) = load_named_profile_document(&path) else {
            continue;
        };
        let Ok(name) = normalize_named_profile_name(&document.name) else {
            continue;
        };
        let Ok(expected_id) = named_profile_id(&name) else {
            continue;
        };
        if expected_id != id {
            continue;
        }

        profiles.push(NamedProfile {
            id: id.to_string(),
            name,
        });
    }

    profiles.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(profiles)
}

fn load_named_profile_document(path: &Path) -> Result<NamedProfileDocument> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("failed to read saved profile {}", path.display()))?;
    let mut document: NamedProfileDocument = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse saved profile {}", path.display()))?;
    document.profile.normalize_mappings();
    Ok(document)
}

fn named_profile_path(directory: &Path, id: &str) -> PathBuf {
    directory.join(id).with_extension("json")
}

fn normalize_named_profile_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("profile name cannot be empty");
    }
    if name.chars().count() > MAX_NAMED_PROFILE_NAME_LENGTH {
        bail!("profile name must be at most {MAX_NAMED_PROFILE_NAME_LENGTH} characters");
    }
    if name.chars().any(char::is_control) {
        bail!("profile name cannot contain control characters");
    }
    Ok(name.to_string())
}

fn named_profile_id(name: &str) -> Result<String> {
    let mut id = String::new();
    let mut pending_separator = false;

    'characters: for character in name.chars() {
        if !character.is_alphanumeric() {
            pending_separator = true;
            continue;
        }

        if pending_separator && !id.is_empty() {
            if id.len() + 1 > MAX_NAMED_PROFILE_ID_BYTES {
                break;
            }
            id.push('-');
        }
        pending_separator = false;

        for lowercase in character.to_lowercase() {
            if id.len() + lowercase.len_utf8() > MAX_NAMED_PROFILE_ID_BYTES {
                break 'characters;
            }
            id.push(lowercase);
        }
    }

    let id = id.trim_matches('-').to_string();
    if id.is_empty() {
        bail!("profile name must include at least one letter or number");
    }
    Ok(id)
}

fn is_valid_named_profile_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_NAMED_PROFILE_ID_BYTES
        && id
            .chars()
            .all(|character| character.is_alphanumeric() || character == '-')
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let temporary_path = temporary_profile_path(path);
    let write_result = (|| -> Result<()> {
        let mut file = fs::File::create(&temporary_path).with_context(|| {
            format!(
                "failed to create temporary profile {}",
                temporary_path.display()
            )
        })?;
        file.write_all(contents).with_context(|| {
            format!(
                "failed to write temporary profile {}",
                temporary_path.display()
            )
        })?;
        file.sync_all().with_context(|| {
            format!(
                "failed to sync temporary profile {}",
                temporary_path.display()
            )
        })?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error).with_context(|| format!("failed to replace profile {}", path.display()));
    }

    Ok(())
}

fn temporary_profile_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile.json");
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", process::id(), sequence))
}

fn device_profile_file_name(mac_address: &str) -> String {
    let compact_mac = mac_address
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();

    if compact_mac.len() == 12 {
        return compact_mac
            .as_bytes()
            .chunks(2)
            .map(|octet| std::str::from_utf8(octet).expect("MAC octets are ASCII"))
            .collect::<Vec<_>>()
            .join("-")
            + ".json";
    }

    let sanitized = mac_address
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-');
    let sanitized = if sanitized.is_empty() {
        "unknown"
    } else {
        // Keep the filename below common filesystem limits even if a caller
        // accidentally passes an arbitrary diagnostic string instead of a MAC.
        &sanitized[..sanitized.len().min(96)]
    };

    format!("{sanitized}.json")
}

fn config_root() -> PathBuf {
    if let Some(path) = non_empty_var("XDG_CONFIG_HOME") {
        return path.into();
    }
    if let Some(home) = non_empty_var("HOME") {
        return Path::new(&home).join(".config");
    }
    PathBuf::from(".")
}

fn non_empty_var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_profile_replaces_existing_json_without_leaving_a_temporary_file() {
        let directory = env::temp_dir().join(format!(
            "dualsense-tui-config-test-{}-{}",
            process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join("profile.json");

        let mut profile = Profile::default();
        profile.lightbar.r = 42;
        save_profile_to(&path, &profile).unwrap();

        profile.lightbar.g = 17;
        save_profile_to(&path, &profile).unwrap();

        let saved: Profile = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved, profile);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn device_profile_file_name_normalizes_common_mac_formats() {
        let expected = "aa-bb-cc-dd-ee-ff.json";

        for mac_address in [
            "AA:BB:CC:DD:EE:FF",
            "aa-bb-cc-dd-ee-ff",
            "aabbccddeeff",
            " aa bb cc dd ee ff ",
        ] {
            assert_eq!(device_profile_file_name(mac_address), expected);
        }
    }

    #[test]
    fn device_profile_file_name_sanitizes_unexpected_input() {
        let file_name = device_profile_file_name("../../Controller Profile/1");

        assert_eq!(file_name, "controller-profile-1.json");
        assert!(!file_name.contains('/'));
        assert!(!file_name.contains('\\'));
    }

    #[test]
    fn device_profile_loads_before_legacy_global_profile() {
        let directory = test_directory();
        let legacy_path = directory.join("profile.json");
        let device_path = directory.join("profiles").join("aa-bb-cc-dd-ee-ff.json");

        let mut legacy_profile = Profile::default();
        legacy_profile.lightbar.r = 12;
        save_profile_to(&legacy_path, &legacy_profile).unwrap();

        let mut device_profile = Profile::default();
        device_profile.lightbar.r = 34;
        save_profile_to(&device_path, &device_profile).unwrap();

        assert_eq!(
            load_profile_with_fallback(Some(&device_path), &legacy_path).unwrap(),
            device_profile
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_device_profile_falls_back_to_legacy_global_profile() {
        let directory = test_directory();
        let legacy_path = directory.join("profile.json");
        let device_path = directory.join("profiles").join("aa-bb-cc-dd-ee-ff.json");

        let mut legacy_profile = Profile::default();
        legacy_profile.lightbar.g = 56;
        save_profile_to(&legacy_path, &legacy_profile).unwrap();

        assert_eq!(
            load_profile_with_fallback(Some(&device_path), &legacy_path).unwrap(),
            legacy_profile
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn saving_lightbar_preserves_other_device_profile_settings() {
        let directory = test_directory();
        let legacy_path = directory.join("profile.json");
        let device_path = directory.join("profiles").join("aa-bb-cc-dd-ee-ff.json");

        let legacy_profile = Profile {
            lightbar: Rgb::new(10, 20, 30),
            ..Profile::default()
        };
        save_profile_to(&legacy_path, &legacy_profile).unwrap();

        let mut device_profile = Profile {
            lightbar: Rgb::new(40, 50, 60),
            ..Profile::default()
        };
        device_profile.haptics.enabled = false;
        device_profile.mouse_mapping.pointer_speed = 31;
        save_profile_to(&device_path, &device_profile).unwrap();

        let color = Rgb::new(200, 120, 10);
        save_lightbar_to(Some(&device_path), &legacy_path, &device_path, color).unwrap();

        device_profile.lightbar = color;
        assert_eq!(load_profile_from(&device_path).unwrap(), device_profile);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn named_profiles_are_sorted_and_loadable() {
        let directory = test_directory();
        let library = directory.join(NAMED_PROFILE_DIRECTORY);

        let mut racing_profile = Profile::default();
        racing_profile.lightbar.r = 41;
        let racing = save_named_profile_to(&library, "Racing", &racing_profile).unwrap();

        let mut fps_profile = Profile::default();
        fps_profile.lightbar.g = 73;
        let fps = save_named_profile_to(&library, "FPS", &fps_profile).unwrap();

        assert_eq!(
            list_named_profiles_in(&library).unwrap(),
            vec![fps.clone(), racing.clone()]
        );

        let (loaded, profile) = load_named_profile_from(&library, &racing.id).unwrap();
        assert_eq!(loaded, racing);
        assert_eq!(profile, racing_profile);
        assert!(load_named_profile_from(&library, "../../racing").is_err());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn named_profiles_reject_blank_or_unsafe_names() {
        let directory = test_directory();
        let library = directory.join(NAMED_PROFILE_DIRECTORY);
        let profile = Profile::default();

        assert!(save_named_profile_to(&library, "   ", &profile).is_err());
        assert!(save_named_profile_to(&library, "///", &profile).is_err());
        assert!(!library.exists());
    }

    #[test]
    fn named_profiles_preserve_unicode_names() {
        let directory = test_directory();
        let library = directory.join(NAMED_PROFILE_DIRECTORY);
        let profile = Profile::default();

        let saved = save_named_profile_to(&library, "Гонки 2026", &profile).unwrap();
        assert_eq!(saved.name, "Гонки 2026");
        assert!(saved
            .id
            .chars()
            .all(|character| { character.is_alphanumeric() || character == '-' }));

        fs::remove_dir_all(directory).unwrap();
    }

    fn test_directory() -> PathBuf {
        env::temp_dir().join(format!(
            "dualsense-tui-config-test-{}-{}",
            process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
