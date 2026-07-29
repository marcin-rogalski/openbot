//! Driving adapter: the Drive tool's CRUD ops. Translates the model's tool call
//! into `DriveStorage` port calls and formats the domain results into the tool
//! strings the loop feeds back. (Knowledge ask/reindex and ingestion live in
//! their own slices.)

use crate::application::ports::drive::DriveStorage;
use crate::domain::drive::DriveEntry;

/// Cap a read result so a huge file can't blow up the model context.
const MAX_READ_CHARS: usize = 6000;

pub async fn search(storage: &dyn DriveStorage, query: &str) -> String {
    match storage.search(query).await {
        Ok(files) => format_files(&files),
        Err(e) => format!("error: {e}"),
    }
}

pub async fn list(storage: &dyn DriveStorage) -> String {
    match storage.list().await {
        Ok(files) => format_files(&files),
        Err(e) => format!("error: {e}"),
    }
}

pub async fn read(storage: &dyn DriveStorage, id_or_link: &str) -> String {
    match storage.read(id_or_link).await {
        Ok(text) => truncate(&text, MAX_READ_CHARS),
        Err(e) => format!("error: {e}"),
    }
}

pub async fn create(storage: &dyn DriveStorage, parent: &str, name: &str, content: &str) -> String {
    match storage.create(non_empty(parent), name, content).await {
        Ok(id) => format!("created file id={id}"),
        Err(e) => format!("error: {e}"),
    }
}

pub async fn create_folder(storage: &dyn DriveStorage, parent: &str, name: &str) -> String {
    match storage.create_folder(non_empty(parent), name).await {
        Ok(id) => format!("created folder id={id}"),
        Err(e) => format!("error: {e}"),
    }
}

pub async fn update(storage: &dyn DriveStorage, id: &str, content: &str) -> String {
    match storage.update(id, content).await {
        Ok(()) => "updated".to_string(),
        Err(e) => format!("error: {e}"),
    }
}

pub async fn trash(storage: &dyn DriveStorage, id: &str) -> String {
    match storage.trash(id).await {
        Ok(()) => "moved to trash".to_string(),
        Err(e) => format!("error: {e}"),
    }
}

fn non_empty(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn format_files(files: &[DriveEntry]) -> String {
    if files.is_empty() {
        return "no files found".to_string();
    }
    files
        .iter()
        .map(|f| {
            let modified = f.modified.as_deref().unwrap_or("");
            format!(
                "- id={} name=\"{}\" type={} modified={}",
                f.id, f.name, f.mime_type, modified
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push_str("…[truncated]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_maps_blank_to_none() {
        assert_eq!(non_empty("  "), None);
        assert_eq!(non_empty(" child "), Some("child"));
    }

    #[test]
    fn format_files_lists_or_reports_empty() {
        assert_eq!(format_files(&[]), "no files found");
        let e = DriveEntry {
            id: "1".into(),
            name: "n".into(),
            mime_type: "text/plain".into(),
            modified: None,
        };
        assert!(format_files(&[e]).contains("id=1"));
    }
}
