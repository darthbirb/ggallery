use std::fmt;

pub type Result<T> = std::result::Result<T, AppError>;

/// Everything that can go wrong, in one enum. Commands return this; it
/// serialises to `{ kind, message }` so the frontend can branch on `kind`
/// without parsing prose.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("no library is open")]
    NoLibrary,

    #[error("this library is already open in another instance")]
    AlreadyOpen,

    #[error("{0} is not inside the library root")]
    OutsideLibrary(String),

    #[error("{0}")]
    Media(String),

    #[error("{0} is unavailable — put it in tools/ or on PATH")]
    ToolMissing(&'static str),

    #[error("{0}")]
    Invalid(String),

    #[error("filesystem watcher: {0}")]
    Watch(String),

    /// A folder record whose directory has gone missing from disk — moved or
    /// deleted outside the app. Every raw filesystem error this could
    /// otherwise surface as ("the system cannot find the path specified")
    /// gets caught at the point it would happen and turned into this
    /// instead, naming the folder so the frontend has something to act on.
    /// A real fix removes the directory from the model entirely (PLAN.md
    /// §M2.6); this is the interim one (docs/DESIGN.md §M2.5d).
    #[error("\"{title}\" is missing from disk — it may have been moved or deleted outside the app")]
    FolderMissing { id: i64, title: String },
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Db(_) => "db",
            AppError::Json(_) => "json",
            AppError::NoLibrary => "no-library",
            AppError::AlreadyOpen => "already-open",
            AppError::OutsideLibrary(_) => "outside-library",
            AppError::Media(_) => "media",
            AppError::ToolMissing(_) => "tool-missing",
            AppError::Invalid(_) => "invalid",
            AppError::Watch(_) => "watch",
            AppError::FolderMissing { .. } => "folder-missing",
        }
    }

    /// The folder this error names, when it names one — `folder-missing` is
    /// the only case today, but reading it through one method rather than
    /// matching the variant directly keeps the frontend-facing shape
    /// (`folderId`) independent of how many variants end up carrying one.
    pub fn folder_id(&self) -> Option<i64> {
        match self {
            AppError::FolderMissing { id, .. } => Some(*id),
            _ => None,
        }
    }

    /// The folder's own title, sent alongside `folderId` so the frontend
    /// never has to re-derive "which folder" from local state that might not
    /// (yet) agree with what the backend just looked up.
    pub fn folder_title(&self) -> Option<&str> {
        match self {
            AppError::FolderMissing { title, .. } => Some(title),
            _ => None,
        }
    }

    pub fn invalid(msg: impl fmt::Display) -> Self {
        AppError::Invalid(msg.to_string())
    }

    pub fn media(msg: impl fmt::Display) -> Self {
        AppError::Media(msg.to_string())
    }
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 4)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.serialize_field("folderId", &self.folder_id())?;
        st.serialize_field("folderTitle", &self.folder_title())?;
        st.end()
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Invalid(e.to_string())
    }
}

impl From<notify::Error> for AppError {
    fn from(e: notify::Error) -> Self {
        AppError::Watch(e.to_string())
    }
}
