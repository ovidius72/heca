//! **`install_command` — put `heca` on the command line**, like an editor's "Install the shell
//! command". Opened from its icon, heca lives inside its app bundle, which no `PATH` contains; this
//! links that executable to a folder that is (`~/.local/bin` on macOS and Linux) or adds its folder
//! to the user's `PATH` (Windows), so `heca` opens in the folder it is typed in.

use std::path::PathBuf;

/// What installing did, in the words the notification shows.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Installed {
    /// Where `heca` can now be run from.
    pub(crate) at: PathBuf,
    /// A line for the shell's startup file, when the folder is not on the `PATH` this process sees.
    pub(crate) path_hint: Option<String>,
}

/// **Link `source` as `heca` in `bin`**, replacing a link already there (an older install) but never
/// a real file — that is someone else's `heca`, and it is said instead.
#[cfg(unix)]
pub(crate) fn link(
    source: &std::path::Path,
    bin: &std::path::Path,
    path_var: Option<&std::ffi::OsStr>,
) -> Result<Installed, String> {
    let at = bin.join("heca");
    std::fs::create_dir_all(bin)
        .map_err(|error| format!("could not make {}: {error}", bin.display()))?;
    match std::fs::symlink_metadata(&at) {
        Ok(meta) if meta.file_type().is_symlink() => {
            std::fs::remove_file(&at)
                .map_err(|error| format!("could not replace {}: {error}", at.display()))?;
        }
        Ok(_) => {
            return Err(format!(
                "{} is already there and is not a link heca made; remove it and try again",
                at.display()
            ));
        }
        Err(_) => {}
    }
    std::os::unix::fs::symlink(source, &at)
        .map_err(|error| format!("could not link {}: {error}", at.display()))?;
    let on_path = path_var.is_some_and(|path| std::env::split_paths(path).any(|dir| dir == bin));
    let path_hint = (!on_path).then(|| format!("export PATH=\"{}:$PATH\"", bin.display()));
    Ok(Installed { at, path_hint })
}

/// The executable to link: the AppImage file itself when heca runs as one (its own path is a
/// mount that changes every run), otherwise this executable.
#[cfg(unix)]
fn source() -> Result<PathBuf, String> {
    if let Some(image) = std::env::var_os("APPIMAGE") {
        return Ok(PathBuf::from(image));
    }
    std::env::current_exe()
        .and_then(|exe| exe.canonicalize())
        .map_err(|error| format!("could not find heca's own executable: {error}"))
}

/// **Install, for this system.**
#[cfg(unix)]
pub(crate) fn install() -> Result<Installed, String> {
    let home = dirs::home_dir().ok_or("no home folder to install into")?;
    link(
        &source()?,
        &home.join(".local").join("bin"),
        std::env::var_os("PATH").as_deref(),
    )
}

/// **Windows: add heca's folder to the user's `PATH`**, through PowerShell's own setter (`setx`
/// cuts a long `PATH` short). New terminals see it; ones already open keep their old `PATH`.
#[cfg(windows)]
pub(crate) fn install() -> Result<Installed, String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("could not find heca's own executable: {error}"))?;
    let dir = exe
        .parent()
        .ok_or("heca's executable has no folder")?
        .to_path_buf();
    let dir_text = dir.display().to_string();
    let script = format!(
        "$p=[Environment]::GetEnvironmentVariable('Path','User'); \
         if (($p -split ';') -notcontains '{dir_text}') {{ \
         [Environment]::SetEnvironmentVariable('Path', ($p.TrimEnd(';') + ';{dir_text}'), 'User') }}"
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(|error| format!("could not run PowerShell: {error}"))?;
    if !status.success() {
        return Err(format!("PowerShell could not add {dir_text} to your PATH"));
    }
    Ok(Installed {
        at: exe,
        path_hint: None,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// **A link in a folder on the `PATH`, replacing an older link but never a real file**, and the
    /// line to add when the folder is not on the `PATH`.
    #[test]
    fn heca_is_linked_and_an_old_link_replaced_but_never_a_real_file() {
        let root = tempfile::tempdir().unwrap();
        let (app, bin) = (root.path().join("heca.app-exe"), root.path().join("bin"));
        std::fs::write(&app, b"#!/bin/sh\n").unwrap();
        let installed = link(&app, &bin, Some(std::ffi::OsStr::new("/usr/bin"))).unwrap();
        assert_eq!(std::fs::read_link(&installed.at).unwrap(), app);
        assert!(
            installed
                .path_hint
                .as_deref()
                .is_some_and(|hint| hint.contains(&bin.display().to_string())),
            "not on the PATH: say what to add"
        );

        let newer = root.path().join("newer-exe");
        std::fs::write(&newer, b"").unwrap();
        let path = std::env::join_paths([bin.clone()]).unwrap();
        let again = link(&newer, &bin, Some(&path)).unwrap();
        assert_eq!(
            std::fs::read_link(&again.at).unwrap(),
            newer,
            "an older link is replaced"
        );
        assert_eq!(again.path_hint, None, "on the PATH: nothing to add");

        std::fs::remove_file(&again.at).unwrap();
        std::fs::write(&again.at, b"someone else's heca").unwrap();
        assert!(
            link(&newer, &bin, None).is_err(),
            "a real file is never replaced"
        );
        assert_eq!(std::fs::read(&again.at).unwrap(), b"someone else's heca");
    }
}
