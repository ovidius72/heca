//! `heca --keys-show` — every binding name and the key it resolves to (F003/P086/T366).
//!
//! **Why this exists.** Keys used to be discoverable by reading one file. They no longer are: a
//! component's keys are in its own `[[keys.component]]` entry, a mode's in `[[keys.mode]]`, and a
//! plugin's are in no file at all — it registers them at runtime. Reading the config can therefore
//! never answer "what runs this action"; only the built keymaps can, and this prints them.
//!
//! It runs **before the window**, so it is also the answer for a script: no GPU, no event loop.
//! The flag itself is dispatched by [`crate::app::cli`], which owns every command line heca answers
//! and renders `--help` from the same table — so a flag cannot exist without being documented. This
//! module keeps what is its own: how the listing is built.

use crate::keymap::BindingIndex;

/// The flag that selects this mode.
pub(crate) const FLAG: &str = "--keys-show";

/// The listing, as text — separated from the printing so it can be tested without a process.
pub(crate) fn render(index: &BindingIndex, json: bool) -> String {
    if json {
        return render_json(index);
    }
    // Width from the content, so a long plugin id does not push every column off the terminal and a
    // short list is not padded to a width nothing needs.
    let name_width = index.keys().map(|a| a.len()).max().unwrap_or(0).clamp(8, 40);
    let key_width = index
        .values()
        .flatten()
        .map(|b| b.key.len())
        .max()
        .unwrap_or(0)
        .clamp(3, 24);

    let mut out = format!("{:<name_width$}  {:<key_width$}  LAYER\n", "BINDING", "KEY");
    for (action, bound) in index {
        for b in bound {
            out.push_str(&format!(
                "{action:<name_width$}  {:<key_width$}  {}\n",
                b.key, b.layer
            ));
        }
    }
    out
}

fn render_json(index: &BindingIndex) -> String {
    let entries: Vec<String> = index
        .iter()
        .flat_map(|(action, bound)| {
            bound.iter().map(move |b| {
                format!(
                    "  {{\"action\": {}, \"key\": {}, \"layer\": {}}}",
                    quote(action),
                    quote(&b.key),
                    quote(&b.layer),
                )
            })
        })
        .collect();
    format!("[\n{}\n]\n", entries.join(",\n"))
}

/// A JSON string literal. Hand-rolled rather than pulled through `serde_json`: the only values here
/// are action ids, combos and layer labels, and the escape set below covers every character JSON
/// requires escaping in them.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::index_binding;

    fn index() -> BindingIndex {
        let mut index = BindingIndex::new();
        index_binding(&mut index, "close", "[keys]", "prefix+x");
        index_binding(&mut index, "workspaces.cursor_down", "[[keys.component]] workspaces", "j");
        index_binding(&mut index, "workspaces.cursor_down", "[[keys.mode]] sidebar", "j");
        index_binding(&mut index, "docker.restart", "plugin", "r");
        index
    }

    /// The listing is the discovery path, so it must show **every** layer — the whole reason it
    /// reads the built keymaps rather than the config file.
    #[test]
    fn every_layer_is_listed_not_just_the_flat_map() {
        let out = render(&index(), false);
        assert!(out.contains("[keys]"), "{out}");
        assert!(out.contains("[[keys.component]] workspaces"), "{out}");
        assert!(out.contains("plugin"), "{out}");
    }

    /// An action bound in two layers lists both: picking one would be a guess, and both are real.
    #[test]
    fn an_action_bound_twice_shows_both_bindings() {
        let out = render(&index(), false);
        let lines: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("workspaces.cursor_down"))
            .collect();
        assert_eq!(lines.len(), 2, "{out}");
    }

    #[test]
    fn an_action_bound_nowhere_prints_nothing() {
        let out = render(&BindingIndex::new(), false);
        assert_eq!(out.lines().count(), 1, "the header alone: {out}");
        assert!(!out.contains("\"\""), "never an empty binding: {out}");
    }

    #[test]
    fn json_carries_the_same_three_facts_per_binding() {
        let out = render(&index(), true);
        assert!(out.contains(r#""action": "close""#), "{out}");
        assert!(out.contains(r#""key": "prefix+x""#), "{out}");
        assert!(out.contains(r#""layer": "[keys]""#), "{out}");
        assert_eq!(out.matches("\"action\"").count(), 4, "{out}");
    }
}
