use super::schema::declare_settings;
use serde::{Deserialize, Serialize};

declare_settings! {
    defaults: ["session_text"];
    /// Text shared by the session picker and lifecycle dialogs.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SessionText {
        pub unavailable: String,
        pub terminated: String,
        pub detached: String,
        pub deleted: String,
        pub server_unavailable: String,
        pub restart: String,
        pub exit: String,
        pub cancel: String,
        pub accept: String,
        pub terminate: String,
        pub delete: String,
        /// Termination warning; `{name}` is the session captured before the prompt opens.
        pub confirm_terminate: String,
        /// Deletion warning; `{name}` is the session captured before the prompt opens.
        pub confirm_delete: String,
        /// Textual y/N confirmation prompt; Enter defaults to cancellation.
        pub confirm_prompt: String,
        pub folder_placeholder: String,
        pub create_folder: String,
        pub counts: String,
    }
}
