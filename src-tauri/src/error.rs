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
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Invalid(e.to_string())
    }
}
