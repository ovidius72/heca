//! Read-only command output and project trust operations.

use crate::actions::ActionCatalog;
use super::Invocation;

/// `--trust` / `--untrust`: remember (or forget) the project file in the folder's project, as it
/// stands now. The trust list is `heca_config::trust`; nothing here knows how it is kept.
pub(super) fn run_trust(inv: &Invocation, trust: bool) -> String {
    let start = inv
        .get("folder")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    match heca_config::loader::read_project_in(&start) {
        Ok(Some(project)) if trust => {
            if heca_config::trust::trust(&project.dir, &project.hash) {
                format!("Trusted {} as it is now.", project.path.display())
            } else {
                "Could not remember the trust (no data directory to write to).".into()
            }
        }
        Ok(Some(project)) => {
            heca_config::trust::untrust(&project.dir);
            format!("No longer trusting {}.", project.path.display())
        }
        Ok(None) => "No .heca/config.toml found from there upward.".into(),
        Err(e) => format!("config error: {e}"),
    }
}

pub(super) fn run_show_config() -> String {
    match heca_config::loader::config_sources() {
        Ok(found) => {
            let mut lines: Vec<String> = found
                .sources
                .iter()
                .map(|s| format!("{} = {}  # {}", s.key, s.value, s.file.display()))
                .collect();
            if let Some(path) = found.untrusted_project {
                lines.push(format!(
                    "# {} (not trusted, ignored) — run `heca --trust` in that folder to apply it",
                    path.display()
                ));
            }
            if lines.is_empty() {
                "No settings file sets anything; heca's defaults apply.".into()
            } else {
                lines.join("\n")
            }
        }
        Err(e) => format!("config error: {e}"),
    }
}

pub(super) fn run_keys_show(inv: &Invocation) -> String {
    let config = heca_config::loader::AppConfig::load();
    // Conflicts are reported by the app proper; here they would be noise on stdout a script has to
    // filter, and the question asked is "what is bound", not "what collided".
    let mut conflicts = crate::app::conflicts::Conflicts::default();
    let keymaps = crate::app::registry::build_keymaps(&config.config, &mut conflicts);
    crate::app::keys_show::render(&keymaps.by_action, inv.json)
}

/// A category as one plain word. `ActionInfo` carries it as a serialisable value whose debug form
/// is quoted; the listing wants the word, not a literal.
fn category(c: &impl std::fmt::Debug) -> String {
    format!("{c:?}").trim_matches('"').to_string()
}

pub(super) fn run_list_actions(inv: &Invocation) -> String {
    let catalog = ActionCatalog::with_builtins();
    let mut all = catalog.describe_all();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    if inv.json {
        return serde_json::to_string_pretty(&all).unwrap_or_default() + "\n";
    }
    let name_width = all
        .iter()
        .map(|a| a.name.len())
        .max()
        .unwrap_or(0)
        .clamp(8, 40);
    let mut out = format!(
        "{:<name_width$}  {:<10}  ARGS  DESCRIPTION\n",
        "ACTION", "CATEGORY"
    );
    for a in &all {
        // The argument count is what tells a reader whether to go and ask `--describe-action`.
        let args_col = if a.args.is_empty() {
            "-".to_string()
        } else {
            a.args.len().to_string()
        };
        out.push_str(&format!(
            "{:<name_width$}  {:<10}  {args_col:<4}  {}\n",
            a.name,
            category(&a.category),
            a.description,
        ));
    }
    out
}
pub(super) fn run_describe_action(inv: &Invocation) -> String {
    // Declared required, so it is here: the dispatcher refused the invocation otherwise.
    let name = inv.get("name").unwrap_or_default();
    let catalog = ActionCatalog::with_builtins();
    let Some(info) = catalog.describe(name) else {
        return format!("no action named '{name}'. `heca --list-actions` prints them.\n");
    };
    if inv.json {
        return serde_json::to_string_pretty(&info).unwrap_or_default() + "\n";
    }
    let mut out = format!(
        "{}  ({})\n{}\n",
        info.name,
        category(&info.category),
        info.description
    );
    if info.args.is_empty() {
        out.push_str("\nTakes no arguments.\n");
        return out;
    }
    out.push_str("\nArguments:\n");
    for a in &info.args {
        let need = if a.required { "required" } else { "optional" };
        out.push_str(&format!("  {} ({need}, {:?})\n", a.name, a.kind));
        out.push_str(&format!("      {}\n", a.description));
        if !a.values.is_empty() {
            out.push_str(&format!("      one of: {}\n", a.values.join(", ")));
        }
    }
    out
}
