//! Driving adapter for archival ingestion. Three inbound surfaces call here:
//! the attachment gate (Discord message/backfill), bot-generated artifacts
//! (transcripts), and save-link. Each runs the ingestion usecases/ports and logs
//! the outcome to the activity feed.

use tauri::AppHandle;

use crate::application::services::foldering;
use crate::application::usecases::archive_attachment::ArchiveOutcome;
use crate::compose::{driven, driving};
use crate::infrastructure::bot;
use crate::infrastructure::config::BotConfig;
use crate::tools::{AttachmentRef, AttachmentSink};

/// The bot's standing rules, used to steer archival/foldering decisions.
fn guidance(app: &AppHandle, bot: &BotConfig) -> String {
    if bot.memory_enabled {
        crate::infrastructure::driving::memory::guidance(
            &crate::infrastructure::driving::memory::load(app, &bot.id),
        )
    } else {
        String::new()
    }
}

/// Relevance-gate an attachment, then archive + index it. Returns whether it was
/// kept.
pub async fn deliver_attachment(
    app: &AppHandle,
    bot: &BotConfig,
    sink: &AttachmentSink,
    att: &AttachmentRef,
    context: &str,
) -> bool {
    let AttachmentSink::Drive {
        instance_id,
        instance_name,
        client_id,
        client_secret,
        folder_id,
    } = sink;

    let uc =
        driving::archive_attachment(app, bot, instance_id, client_id, client_secret, folder_id);
    match uc
        .run(
            &guidance(app, bot),
            context,
            &att.filename,
            &att.content_type,
            &att.url,
        )
        .await
    {
        Ok(ArchiveOutcome::Skipped) => {
            bot::emit_log(
                app,
                &bot.id,
                format!("attachment \"{}\": skipped (not relevant)", att.filename),
            );
            false
        }
        Ok(ArchiveOutcome::Archived { drive_id, indexed }) => {
            bot::emit_tool_activity(
                app,
                &bot.id,
                format!(
                    "archive_attachment {{name={:?}, to={:?}}} → id={drive_id}",
                    att.filename, instance_name
                ),
                format!("📎 Archived \"{}\" to {}", att.filename, instance_name),
            );
            if indexed {
                bot::emit_log(
                    app,
                    &bot.id,
                    format!("indexed \"{}\" into the knowledge base", att.filename),
                );
            }
            true
        }
        Err(e) => {
            bot::emit_log(
                app,
                &bot.id,
                format!("attachment \"{}\": archive failed: {e}", att.filename),
            );
            false
        }
    }
}

/// Store a bot-generated text file (transcript/summary) into the sink's folder
/// and index it. Returns the new Drive file id.
pub async fn store_text_artifact(
    app: &AppHandle,
    bot: &BotConfig,
    sink: &AttachmentSink,
    filename: &str,
    content: &str,
    context: &str,
) -> Option<String> {
    let AttachmentSink::Drive {
        instance_id,
        instance_name,
        client_id,
        client_secret,
        folder_id,
    } = sink;

    let drive = driven::drive_storage(app, client_id, client_secret, folder_id);
    let policy = driven::archive_policy(bot);
    let target =
        foldering::choose_folder(&*drive, &*policy, &guidance(app, bot), context, filename).await;

    let drive_id = match drive.create(Some(&target), filename, content).await {
        Ok(id) => id,
        Err(e) => {
            bot::emit_log(app, &bot.id, format!("store \"{filename}\": failed: {e}"));
            return None;
        }
    };
    bot::emit_tool_activity(
        app,
        &bot.id,
        format!("store_artifact {{name={filename:?}, to={instance_name:?}}} → id={drive_id}"),
        format!("💾 Saved \"{filename}\" to {instance_name}"),
    );
    let _ = driving::index_document(app, bot, instance_id)
        .run(&drive_id, filename, "text/markdown", content)
        .await;
    Some(drive_id)
}

/// Copy a Drive file (from a link/id) into the tool's folder and index it.
pub async fn save_link(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder: &str,
    url: &str,
) -> String {
    let drive = driven::drive_storage(app, client_id, client_secret, folder);
    let meta = match drive.file_meta(url).await {
        Ok(m) => m,
        Err(e) => {
            return format!(
                "error: can't access that link ({e}). It must be shared with this bot's Google \
                 account (or set to 'anyone with the link')."
            )
        }
    };
    let new_id = match drive.copy_into(url, folder).await {
        Ok(id) => id,
        Err(e) => return format!("error: couldn't copy the file: {e}"),
    };
    let mut note = format!("saved \"{}\" to the folder (id={new_id})", meta.name);
    if let Ok(text) = drive.read(&new_id).await {
        let _ = driving::index_document(app, bot, instance_id)
            .run(&new_id, &meta.name, &meta.mime_type, &text)
            .await;
        note.push_str(" and indexed it into the knowledge base");
    }
    note
}
