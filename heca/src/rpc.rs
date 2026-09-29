//! **The RPC: an action by its catalog name.**
//!
//! One map. Every action — built-in, a component's, a plugin's — is registered once in the
//! [`ActionCatalog`](crate::actions::ActionCatalog), and that is the only place the RPC looks a
//! name up. A line is the action's name and its arguments:
//!
//! ```text
//! split_horizontal
//! focus_pane 42                      positional, in the order the action declares its args
//! place_pane 5 0 1 pane_idx=0        or by name, in any order
//! docker.stop_selected --dock docker.right
//! ```
//!
//! The line becomes an [`Intent`](crate::chrome::Intent) — exactly what a plugin, a key binding or
//! a menu entry hands the dispatcher — so the RPC reaches every action the moment it is
//! registered, and nothing here has to be written for a new one. It used to keep its own list of
//! 94 hand-written commands, each with its own spelling and argument parsing, beside the catalog
//! (F003/P082/T509).

/// Errors from reading or running an RPC line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcError {
    UnknownCommand(String),
    MissingArgument {
        cmd: String,
        arg: String,
    },
    /// More positional values than the action declares arguments.
    UnexpectedArgument {
        cmd: String,
        value: String,
    },
    /// The app is not yet initialized (no state available).
    NotInitialized,
    /// The action exists but its [`ActionPolicy`](crate::app::interaction::ActionPolicy) does not
    /// permit it in the current domain — a `TiledOnly` call while a float owns the screen, a
    /// component's cursor verb while its dock is not being driven (F003/P086/T372).
    Blocked(String),
    /// Declared, but nothing here can run it: its component is not mounted, or it belongs to a
    /// plugin across a boundary that does not exist yet. **Not** silent success.
    NotRunnable(String),
    /// Called without the arguments it declares as required — `describe-action <name>` lists them
    /// (F003/P085/T358).
    MissingArgs(String),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpcError::UnknownCommand(cmd) => write!(f, "unknown command: {cmd}"),
            RpcError::MissingArgument { cmd, arg } => {
                write!(f, "command '{cmd}' missing argument: {arg}")
            }
            RpcError::UnexpectedArgument { cmd, value } => write!(
                f,
                "command '{cmd}' takes no more arguments, got: {value} — see 'describe-action {cmd}'"
            ),
            RpcError::Blocked(name) => {
                write!(f, "action '{name}' is not allowed right now")
            }
            RpcError::NotRunnable(name) => {
                write!(
                    f,
                    "action '{name}' has no runnable owner (component not mounted?)"
                )
            }
            RpcError::MissingArgs(name) => write!(
                f,
                "action '{name}' needs arguments it was not given — see 'describe-action {name}'"
            ),
            RpcError::NotInitialized => write!(f, "app not initialized"),
        }
    }
}

impl std::error::Error for RpcError {}

/// **Action introspection** (action-task-C): answer a metadata *query* against the runtime
/// [`ActionCatalog`](crate::actions::ActionCatalog), returning JSON. `None` if `input` is not an
/// introspection command (the caller then runs it as an action, [`parse_rpc`]).
///
/// This is a separate path from [`parse_rpc`] because a query returns **data**, not an action to
/// run. Two commands:
///   list-actions            → JSON array of every action's metadata (built-in AND plugin)
///   describe-action <name>  → JSON of one action, or an error if the name is unknown
pub fn introspect(
    catalog: &crate::actions::ActionCatalog,
    input: &str,
) -> Option<Result<String, RpcError>> {
    let input = input.trim();
    let mut parts = input.split_whitespace();
    let cmd = parts.next()?.to_lowercase();
    match cmd.as_str() {
        "list-actions" => Some(to_json(&catalog.describe_all(), &cmd)),
        "describe-action" => Some(match parts.next() {
            Some(name) => match catalog.describe(name) {
                Some(info) => to_json(&info, &cmd),
                None => Err(RpcError::UnknownCommand(format!(
                    "describe-action: unknown action '{name}'"
                ))),
            },
            None => Err(RpcError::MissingArgument {
                cmd: "describe-action".to_string(),
                arg: "name".to_string(),
            }),
        }),
        _ => None,
    }
}

fn to_json<T: serde::Serialize>(value: &T, cmd: &str) -> Result<String, RpcError> {
    serde_json::to_string(value).map_err(|e| RpcError::UnknownCommand(format!("{cmd}: {e}")))
}

/// One RPC line, read but not yet matched to an action: `name [--dock <id>] [value …] [key=value …]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcCall {
    /// The action's catalog name.
    pub action: String,
    /// Values given without a name, in order — named by [`into_intent`](Self::into_intent) from
    /// the action's declared arguments.
    pub positional: Vec<String>,
    /// `key=value` arguments, in the order written.
    pub named: Vec<(String, String)>,
    /// `--dock <id>`: **which placement** the call is aimed at (F003/P085/T358). Not an argument —
    /// it never reaches the action. Without it the host resolves the owner itself (`owning_mount`).
    pub dock: Option<String>,
}

/// Read one RPC line. Knows nothing about any action: which names exist and what they take is the
/// catalog's, asked by [`RpcCall::into_intent`].
pub fn parse_rpc(input: &str) -> Result<RpcCall, RpcError> {
    let mut parts = input.split_whitespace();
    let action = parts
        .next()
        .ok_or_else(|| RpcError::UnknownCommand("<empty>".to_string()))?
        .to_string();
    let mut call = RpcCall {
        action,
        positional: Vec::new(),
        named: Vec::new(),
        dock: None,
    };
    while let Some(part) = parts.next() {
        if part == "--dock" {
            let id = parts.next().ok_or_else(|| RpcError::MissingArgument {
                cmd: call.action.clone(),
                arg: "--dock <id>".to_string(),
            })?;
            call.dock = Some(id.to_string());
        } else if let Some((key, value)) = part.split_once('=') {
            call.named.push((key.to_string(), value.to_string()));
        } else {
            call.positional.push(part.to_string());
        }
    }
    Ok(call)
}

impl RpcCall {
    /// The [`Intent`](crate::chrome::Intent) this call means, named against the `catalog` — the
    /// same map every other caller of an action uses. Positional values take the action's declared
    /// argument names in order; whether the arguments are right is then judged on dispatch, as for
    /// any caller.
    pub fn into_intent(
        self,
        catalog: &crate::actions::ActionCatalog,
    ) -> Result<(crate::chrome::Intent, Option<String>), RpcError> {
        let meta = catalog
            .find(&self.action)
            .ok_or_else(|| RpcError::UnknownCommand(self.action.clone()))?;
        if let Some(extra) = self.positional.get(meta.args.len()) {
            return Err(RpcError::UnexpectedArgument {
                cmd: self.action,
                value: extra.clone(),
            });
        }
        let names = meta.args.iter().map(|a| a.name.clone());
        let mut intent = crate::chrome::Intent::new(&self.action);
        for (key, value) in names.zip(self.positional).chain(self.named) {
            intent = intent.arg(key, crate::chrome::PropValue::Text(value));
        }
        Ok((intent, self.dock))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::ActionCatalog;
    use crate::chrome::PropValue;

    fn intent_of(line: &str) -> Result<(crate::chrome::Intent, Option<String>), RpcError> {
        parse_rpc(line)?.into_intent(&ActionCatalog::with_builtins())
    }

    fn arg(intent: &crate::chrome::Intent, key: &str) -> Option<String> {
        intent
            .args
            .get(key)
            .and_then(PropValue::as_text)
            .map(str::to_string)
    }

    #[test]
    fn introspect_lists_and_describes_actions() {
        let catalog = crate::actions::ActionCatalog::with_builtins();

        // Not an introspection command → None (falls through to execute).
        assert!(introspect(&catalog, "focus-left").is_none());

        // list-actions → JSON array covering every catalogued action.
        let json = introspect(&catalog, "list-actions").unwrap().unwrap();
        let list: Vec<crate::actions::ActionInfo> = serde_json::from_str(&json).unwrap();
        assert_eq!(list.len(), catalog.all().count());
        assert!(list.iter().any(|a| a.name == "close"));

        // describe-action <name> → one action, with its confirm toggle key surfaced.
        let json = introspect(&catalog, "describe-action close")
            .unwrap()
            .unwrap();
        let info: crate::actions::ActionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info.name, "close");
        assert_eq!(info.confirm.as_deref(), Some("close"));
        let json = introspect(&catalog, "describe-action focus_left")
            .unwrap()
            .unwrap();
        assert!(
            json.contains("\"policy\":\"tiled_only\""),
            "a policy crosses the wire by its snake_case name: {json}"
        );
        let info: crate::actions::ActionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(
            info.policy,
            crate::app::interaction::ActionPolicy::TiledOnly
        );
        assert!(info.confirm.is_none());

        // Errors: unknown action, missing argument.
        assert!(
            introspect(&catalog, "describe-action nope")
                .unwrap()
                .is_err()
        );
        assert!(introspect(&catalog, "describe-action").unwrap().is_err());
    }

    /// Positional values are named by the action's own declaration, in order.
    #[test]
    fn positional_values_take_the_declared_names() {
        let (intent, dock) = intent_of("place_pane 5 0 1 pane_idx=0").unwrap();
        assert_eq!(intent.action, "place_pane");
        assert_eq!(arg(&intent, "pane_id").as_deref(), Some("5"));
        assert_eq!(arg(&intent, "ws_idx").as_deref(), Some("0"));
        assert_eq!(arg(&intent, "col_idx").as_deref(), Some("1"));
        assert_eq!(arg(&intent, "pane_idx").as_deref(), Some("0"));
        assert_eq!(dock, None);
    }

    /// **Every catalogued action is reachable by its name, and nothing else is.** The RPC has no
    /// list of its own to fall behind.
    #[test]
    fn every_catalogued_action_is_reachable_by_its_name() {
        for d in crate::actions::builtins() {
            let (intent, _) = intent_of(d.name).unwrap_or_else(|e| panic!("{}: {e}", d.name));
            assert_eq!(intent.action, d.name);
        }
        assert!(matches!(
            intent_of("split-h"),
            Err(RpcError::UnknownCommand(_))
        ));
    }

    #[test]
    fn too_many_values_are_reported_not_dropped() {
        assert_eq!(
            intent_of("focus_pane 1 2").map(|_| ()),
            Err(RpcError::UnexpectedArgument {
                cmd: "focus_pane".to_string(),
                value: "2".to_string(),
            }),
        );
    }

    /// `--dock` names the seating and never reaches the action as an argument.
    #[test]
    fn the_dock_flag_targets_a_placement_and_never_reaches_the_action() {
        let call = parse_rpc("focus_left --dock docker.right").unwrap();
        assert_eq!(call.dock.as_deref(), Some("docker.right"));
        assert!(call.positional.is_empty() && call.named.is_empty());
        assert!(matches!(
            parse_rpc("focus_left --dock"),
            Err(RpcError::MissingArgument { .. })
        ));
    }

    #[test]
    fn an_empty_line_is_an_error() {
        assert!(matches!(parse_rpc("   "), Err(RpcError::UnknownCommand(_))));
    }
}
