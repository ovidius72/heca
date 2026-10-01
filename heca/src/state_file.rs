//! Machine-written state beside the config — the one implementation lives in `heca-config`, where
//! the project-trust list needs it too.

pub(crate) use heca_config::state_file::{data_path, write_json};
