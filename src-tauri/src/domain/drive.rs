//! Drive storage domain: the business representation of a stored file. No API
//! shapes, no HTTP — the Drive REST wire format stays in the adapter.

/// A file (or folder) in the bot's Drive storage.
#[derive(Clone, Debug)]
pub struct DriveEntry {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub modified: Option<String>,
}

const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

impl DriveEntry {
    pub fn is_folder(&self) -> bool {
        self.mime_type == FOLDER_MIME
    }
}
