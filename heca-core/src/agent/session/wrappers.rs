//! Publish only the enabled launch wrappers into the terminal's stable private PATH entry.

use std::{collections::BTreeMap, fs, io::Write, path::Path};

use super::{AgentError, AgentOptions, encode, quote, validate_program};

pub(super) fn scripts(options: &AgentOptions, directory: &Path) -> Result<BTreeMap<String, String>, AgentError> {
    let bin = directory.join("bin");
    let mut scripts = BTreeMap::new();
    for driver in options.drivers.iter() {
        for program in driver.programs() {
            if scripts.contains_key(program) { continue }
            validate_program(program)?;
            scripts.insert(program.clone(), format!(
                "#!/bin/sh\nheca_agent_bin={}\n{}\n\
                 heca_agent_executable=$(command -v -- {}) || exit 127\n\
                 printf '\\033]133;E;%s\\a' {}\n\
                 unset HECA_AGENT_BIN\n\
                 exec {} {} {} {} {} {} \"$heca_agent_executable\" \"$@\"\n",
                quote(&bin.to_string_lossy()), include_str!("path.sh"),
                quote(program), quote(&encode(program)),
                quote(&options.launcher.to_string_lossy()), super::super::launcher::FLAG,
                quote(&directory.to_string_lossy()), options.launch_timeout.as_millis(),
                options.max_report_bytes, quote(program),
            ));
        }
    }
    Ok(scripts)
}

pub(super) fn publish(bin: &Path, scripts: BTreeMap<String, String>) -> Result<(), AgentError> {
    for entry in fs::read_dir(bin)? {
        let entry = entry?;
        if !scripts.contains_key(&entry.file_name().to_string_lossy().into_owned()) {
            fs::remove_file(entry.path())?;
        }
    }
    for (program, script) in scripts {
        let path = bin.join(program);
        if fs::read_to_string(&path).ok().as_deref() == Some(script.as_str()) { continue }
        let mut file = tempfile::NamedTempFile::new_in(bin)?;
        file.write_all(script.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file().set_permissions(fs::Permissions::from_mode(0o700))?;
        }
        file.persist(path).map_err(|error| error.error)?;
    }
    Ok(())
}
