//! What a pane says about itself, as plain data: its name, program icon and git state.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PaneInfoView {
    pub(crate) icon: Glyph,
    /// Custom name if set, else the program name — the sidebar card's label.
    pub(crate) title: String,
    /// The program/application name (always the process, never the rename) — the info-bar
    /// `AppName` segment shows this, so renaming a pane doesn't hide what's running in it.
    pub(crate) app_name: String,
    /// The process/program name shown as a small dimmed label *next to* a custom name (e.g.
    /// `(nvim)`). `Some` only when the pane has a custom name and `pane_renamed_add_process_name`
    /// is on — a pane that merely tracks its process has the process name *as* its title already.
    pub(crate) process_hint: Option<String>,
    pub(crate) status: ProcessStatus,
    pub(crate) git_branch: Option<String>,
    pub(crate) git_added: Option<String>,
    pub(crate) git_modified: Option<String>,
    pub(crate) git_deleted: Option<String>,
}

fn program_glyph(icon: ProgramIcon) -> Glyph {
    match icon {
        ProgramIcon::Terminal => Glyph::Terminal,
        ProgramIcon::FileCode => Glyph::FileCode,
        ProgramIcon::Folder => Glyph::Folder,
        ProgramIcon::FolderOpen => Glyph::FolderOpen,
        ProgramIcon::GitBranch => Glyph::GitBranch,
        ProgramIcon::Gear => Glyph::Gear,
        ProgramIcon::Search => Glyph::Search,
    }
}

pub(crate) fn pane_info_view(
    programs: &ProgramsConfig,
    fallback_name: &str,
    custom_name: Option<&str>,
    runtime: Option<&PaneRuntime>,
    add_process_name: bool,
) -> PaneInfoView {
    let raw = runtime
        .and_then(|pane| pane.program.as_deref())
        .filter(|raw| !raw.is_empty())
        .unwrap_or(fallback_name);
    let program = programs.resolve(raw);
    let git = runtime.and_then(|pane| pane.git.as_ref());
    let program_name = program.name.to_string();
    // A user-set custom name wins over the process-derived program name; the icon still tracks
    // the running program. When the pane has a custom name (and the setting is on), the program
    // name is surfaced separately as `process_hint` (a small dimmed label next to the name).
    let has_custom = custom_name.is_some_and(|name| !name.is_empty());
    let title = if has_custom {
        custom_name.unwrap_or_default().to_string()
    } else {
        program_name.clone()
    };
    let process_hint = (has_custom && add_process_name).then(|| program_name.clone());
    PaneInfoView {
        icon: program_glyph(program.icon),
        title,
        app_name: program_name,
        process_hint,
        status: runtime
            .map(|pane| pane.status.clone())
            .unwrap_or(ProcessStatus::Idle),
        git_branch: git.map(git_branch),
        git_added: git.and_then(git_added),
        git_modified: git.and_then(git_modified),
        git_deleted: git.and_then(git_deleted),
    }
}

/// **How git state is worded** — one place, read by the header projection and by the sidebar row's
/// git line, so the two cannot describe one repository two ways. A repository with no branch is
/// `detached`; a count of nothing is no text at all.
pub(crate) fn git_branch(info: &heca_core::runtime::GitInfo) -> String {
    info.branch
        .clone()
        .unwrap_or_else(|| "detached".to_string())
}

/// `+N` for N added files, or `None` when there are none. See [`git_branch`].
pub(crate) fn git_added(info: &heca_core::runtime::GitInfo) -> Option<String> {
    (info.added > 0).then(|| format!("+{}", info.added))
}

/// `~N` for N modified files, or `None`. See [`git_branch`].
pub(crate) fn git_modified(info: &heca_core::runtime::GitInfo) -> Option<String> {
    (info.modified > 0).then(|| format!("~{}", info.modified))
}

/// `-N` for N deleted files, or `None`. See [`git_branch`].
pub(crate) fn git_deleted(info: &heca_core::runtime::GitInfo) -> Option<String> {
    (info.deleted > 0).then(|| format!("-{}", info.deleted))
}

/// Left-truncate `text` to `max_chars`, keeping the **tail** with a leading
/// ellipsis (`…/HypeSupport`) — paths read most usefully from the end.
pub(crate) fn truncate_path_left(text: &str, max_chars: usize) -> String {
    let len = text.chars().count();
    if len <= max_chars {
        return text.to_string();
    }
    match max_chars {
        0 => String::new(),
        1 => "…".to_string(),
        n => {
            let tail: String = text.chars().skip(len - (n - 1)).collect();
            format!("…{tail}")
        }
    }
}

/// A path shown home-relative (`/Users/x/proj` → `~/proj`).
pub(crate) fn home_relative_path(path: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::Path::new(&home);
        if let Ok(rest) = path.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}
