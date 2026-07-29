//! Port: a bot's Drive storage (scoped to one tool instance's folder + account).
//! A driven adapter implements it; the credentials and REST details live there.

use async_trait::async_trait;

use crate::domain::drive::DriveEntry;

#[async_trait]
pub trait DriveStorage: Send + Sync {
    /// Full-text/name search within the tool's folder subtree.
    async fn search(&self, query: &str) -> Result<Vec<DriveEntry>, String>;
    /// List the folder's immediate children.
    async fn list(&self) -> Result<Vec<DriveEntry>, String>;
    /// Read a file's text by id or by a pasted share link.
    async fn read(&self, id_or_link: &str) -> Result<String, String>;
    /// Create a text file; `parent` defaults to the tool's folder when `None`.
    async fn create(
        &self,
        parent: Option<&str>,
        name: &str,
        content: &str,
    ) -> Result<String, String>;
    /// Create a subfolder; `parent` defaults to the tool's folder when `None`.
    async fn create_folder(&self, parent: Option<&str>, name: &str) -> Result<String, String>;
    /// Replace a file's content by id.
    async fn update(&self, id: &str, content: &str) -> Result<(), String>;
    /// Move a file to trash by id.
    async fn trash(&self, id: &str) -> Result<(), String>;

    // --- Ingestion support ---------------------------------------------------

    /// The tool folder's immediate subfolders (for semantic foldering).
    async fn list_folders(&self) -> Result<Vec<DriveEntry>, String>;
    /// Upload raw bytes into `parent` (a subfolder id, or the tool folder).
    async fn upload_binary(
        &self,
        parent: &str,
        name: &str,
        bytes: Vec<u8>,
        mime: &str,
    ) -> Result<String, String>;
    /// Metadata for a file by id or share link (name + mime).
    async fn file_meta(&self, id_or_link: &str) -> Result<DriveEntry, String>;
    /// Copy a file (by id or share link) into `dest` folder; returns the new id.
    async fn copy_into(&self, id_or_link: &str, dest: &str) -> Result<String, String>;
    /// The tool folder's own id (the default archive destination).
    fn folder_id(&self) -> &str;
}
