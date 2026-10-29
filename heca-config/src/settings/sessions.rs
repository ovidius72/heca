use super::schema::declare_settings;
use serde::{Deserialize, Serialize};

declare_settings! {
    defaults: ["session_text"];
    /// Text shared by the session picker and lifecycle dialogs.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SessionText {
        pub unavailable: String,
        pub terminated: String,
        pub server_unavailable: String,
        pub restart: String,
        pub exit: String,
        pub cancel: String,
        pub accept: String,
        pub folder_placeholder: String,
        pub create_folder: String,
        pub counts: String,
    }
}
