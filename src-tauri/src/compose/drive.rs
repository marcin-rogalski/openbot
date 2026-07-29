//! Compose the Drive storage slice. Per tool instance (its credentials + folder).

use std::sync::Arc;

use tauri::AppHandle;

use crate::application::ports::drive::DriveStorage;
use crate::infrastructure::driven::gdrive_storage::GDriveStorage;

pub fn compose_drive_storage(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> Arc<dyn DriveStorage> {
    Arc::new(GDriveStorage::new(
        app.clone(),
        client_id.to_string(),
        client_secret.to_string(),
        folder_id.to_string(),
    ))
}
