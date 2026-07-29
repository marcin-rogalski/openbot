//! Semantic foldering service: choose which subfolder a file belongs in, using
//! the archival policy over the tool folder's subfolders. No IO of its own —
//! depends on the drive + policy ports.

use crate::application::ports::drive::DriveStorage;
use crate::application::ports::ingestion::ArchivePolicy;

/// The target folder id for `filename`: the policy picks from the tool folder's
/// subfolders (rule-guided), resolved to an id; the tool folder itself when
/// there are no subfolders or no clear match.
pub async fn choose_folder(
    drive: &dyn DriveStorage,
    policy: &dyn ArchivePolicy,
    guidance: &str,
    context: &str,
    filename: &str,
) -> String {
    let root = drive.folder_id().to_string();
    let subfolders = drive.list_folders().await.unwrap_or_default();
    if subfolders.is_empty() {
        return root;
    }
    let names: Vec<String> = subfolders.iter().map(|f| f.name.clone()).collect();
    match policy
        .pick_folder(guidance, context, filename, &names)
        .await
    {
        Some(name) => subfolders
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.id.clone())
            .unwrap_or(root),
        None => root,
    }
}
