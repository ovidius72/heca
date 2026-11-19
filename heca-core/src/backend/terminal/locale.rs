//! **The locale a shell heca starts runs in.**
//!
//! A terminal hands its shell a locale; a program started from the macOS Dock or Finder gets none
//! from the system (launchd sets no `LANG`), so a shell heca started ran in the C locale. There zsh
//! counts a multi-byte UTF-8 character — `❯`, `⬢`, a Nerd Font icon in the prompt — as several
//! columns, loses track of where its cursor is, and its erase leaves text on the line that cannot
//! be deleted; completions also sort in byte order. Other terminals (Ghostty, iTerm2, Terminal)
//! set the locale themselves; so does heca, here, and only when none was inherited.

use std::sync::OnceLock;

/// **The `LANG` to give a shell, or `None` when the environment heca runs in already says one.**
///
/// Any of `LC_ALL`, `LC_CTYPE` or `LANG` set to something non-empty is the user's choice and is
/// kept as it is; otherwise the system's own locale, in UTF-8.
pub(crate) fn missing_lang() -> Option<String> {
    lang_for(|name| std::env::var(name).ok(), system_locale)
}

/// [`missing_lang`] given how to read the environment and the system's locale — the testable rule.
fn lang_for(env: impl Fn(&str) -> Option<String>, system: impl FnOnce() -> String) -> Option<String> {
    let inherited = ["LC_ALL", "LC_CTYPE", "LANG"].iter().any(|name| env(name).is_some_and(|value| !value.is_empty()));
    (!inherited).then(system)
}

/// The system's locale as a UTF-8 `LANG`, worked out once.
fn system_locale() -> String {
    static LOCALE: OnceLock<String> = OnceLock::new();
    LOCALE
        .get_or_init(|| utf8_locale(apple_locale().as_deref(), |name| std::path::Path::new("/usr/share/locale").join(name).is_dir()))
        .clone()
}

/// The user's region on macOS (`defaults read -g AppleLocale`, e.g. `it_IT`, `en_US@rg=itzzzz`);
/// `None` elsewhere or when it cannot be read.
fn apple_locale() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("defaults").args(["read", "-g", "AppleLocale"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// **A UTF-8 locale that exists**: the user's region in UTF-8 when the system has it, else
/// `en_US.UTF-8` when that exists, else `C.UTF-8` (what Linux and most systems ship).
fn utf8_locale(apple: Option<&str>, exists: impl Fn(&str) -> bool) -> String {
    let region = apple.map(|locale| locale.split('@').next().unwrap_or(locale));
    region
        .map(|region| format!("{region}.UTF-8"))
        .into_iter()
        .chain(["en_US.UTF-8".to_owned()])
        .find(|candidate| exists(candidate))
        .unwrap_or_else(|| "C.UTF-8".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| set.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_owned())
    }

    /// **A locale the user already has is kept**, whichever of the three says it.
    #[test]
    fn an_inherited_locale_is_kept() {
        for name in ["LC_ALL", "LC_CTYPE", "LANG"] {
            assert_eq!(lang_for(env(&[(name, "fr_FR.UTF-8")]), || unreachable!()), None, "{name}");
        }
    }

    /// **No locale — the Dock's case — gets the system's**, and an empty one counts as none.
    #[test]
    fn no_locale_gets_the_systems() {
        assert_eq!(lang_for(env(&[]), || "it_IT.UTF-8".into()).as_deref(), Some("it_IT.UTF-8"));
        assert_eq!(lang_for(env(&[("LANG", "")]), || "it_IT.UTF-8".into()).as_deref(), Some("it_IT.UTF-8"));
    }

    /// **The region in UTF-8 when it exists**, its `@…` options dropped; then `en_US`; then C.
    #[test]
    fn the_system_locale_is_a_utf8_one_that_exists() {
        let mac = |name: &str| ["it_IT.UTF-8", "en_US.UTF-8"].contains(&name);
        assert_eq!(utf8_locale(Some("it_IT"), mac), "it_IT.UTF-8");
        assert_eq!(utf8_locale(Some("it_IT@rg=itzzzz"), mac), "it_IT.UTF-8");
        assert_eq!(utf8_locale(Some("xx_YY"), mac), "en_US.UTF-8", "a region the system lacks");
        assert_eq!(utf8_locale(None, |_| false), "C.UTF-8", "not macOS");
    }
}
