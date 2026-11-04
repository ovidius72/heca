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
        /// Window title for the first window on a session; `{folder}` is the session folder shown
        /// home-relative, or the session name when it has none.
        pub window_title: String,
        /// Window title for each later window on the same session; `{folder}` as above and
        /// `{number}` is the window's ordinal.
        pub window_title_numbered: String,
        /// The follow picker's row that stops mirroring another window.
        pub stop_following: String,
    }
}
