//! Driven adapter: `DriveStorage` over Google Drive. Holds the tool instance's
//! credentials + root folder, delegates to the `gdrive` REST client, and maps
//! its `DriveFile` wire struct to the domain `DriveEntry`. `parent = None`
//! defaults to the tool's folder; link ids are normalised here.

use async_trait::async_trait;
use tauri::AppHandle;

use crate::application::ports::drive::DriveStorage;
use crate::domain::drive::DriveEntry;
use crate::gdrive::{self, DriveFile};

pub struct GDriveStorage {
    app: AppHandle,
    client_id: String,
    client_secret: String,
    folder_id: String,
}

impl GDriveStorage {
    pub fn new(
        app: AppHandle,
        client_id: String,
        client_secret: String,
        folder_id: String,
    ) -> Self {
        Self {
            app,
            client_id,
            client_secret,
            folder_id,
        }
    }

    fn parent(&self, parent: Option<&str>) -> String {
        parent
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .unwrap_or(&self.folder_id)
            .to_string()
    }
}

fn to_entry(f: DriveFile) -> DriveEntry {
    DriveEntry {
        id: f.id,
        name: f.name,
        mime_type: f.mime_type,
        modified: f.modified_time,
    }
}

#[async_trait]
impl DriveStorage for GDriveStorage {
    async fn search(&self, query: &str) -> Result<Vec<DriveEntry>, String> {
        let files = gdrive::search(
            &self.app,
            &self.client_id,
            &self.client_secret,
            &self.folder_id,
            query,
        )
        .await?;
        Ok(files.into_iter().map(to_entry).collect())
    }

    async fn list(&self) -> Result<Vec<DriveEntry>, String> {
        let files = gdrive::list(
            &self.app,
            &self.client_id,
            &self.client_secret,
            &self.folder_id,
        )
        .await?;
        Ok(files.into_iter().map(to_entry).collect())
    }

    async fn read(&self, id_or_link: &str) -> Result<String, String> {
        let id = gdrive::file_id_from_link(id_or_link);
        gdrive::read(&self.app, &self.client_id, &self.client_secret, &id).await
    }

    async fn create(
        &self,
        parent: Option<&str>,
        name: &str,
        content: &str,
    ) -> Result<String, String> {
        gdrive::create(
            &self.app,
            &self.client_id,
            &self.client_secret,
            &self.parent(parent),
            name,
            content,
        )
        .await
    }

    async fn create_folder(&self, parent: Option<&str>, name: &str) -> Result<String, String> {
        gdrive::create_folder(
            &self.app,
            &self.client_id,
            &self.client_secret,
            &self.parent(parent),
            name,
        )
        .await
    }

    async fn update(&self, id: &str, content: &str) -> Result<(), String> {
        gdrive::update(&self.app, &self.client_id, &self.client_secret, id, content).await
    }

    async fn trash(&self, id: &str) -> Result<(), String> {
        gdrive::trash(&self.app, &self.client_id, &self.client_secret, id).await
    }
}
