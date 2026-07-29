//! Driving adapter: the Drive tool's knowledge ops (ask / reindex / list_sources).
//! Translates the tool call into the ask/reindex usecases (+ the index port) and
//! formats results for the model / progress for the channel.

use tauri::AppHandle;

use crate::application::usecases::reindex_knowledge::ReindexProgress;
use crate::compose::{driven, driving};
use crate::domain::knowledge::KnowledgePassage;
use crate::infrastructure::bot;
use crate::infrastructure::config::BotConfig;
use crate::tools::Progress;

/// Answer a question from the knowledge base; returns cited passages for the
/// model to synthesise from, or a note when the index is empty.
pub async fn ask(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    question: &str,
    k: usize,
) -> String {
    match driving::ask_knowledge(app, bot, instance_id)
        .run(question, k)
        .await
    {
        Ok(hits) if hits.is_empty() => {
            "the knowledge index is empty — run reindex first, then ask again".to_string()
        }
        Ok(hits) => format_hits(question, &hits),
        Err(e) => format!("error: {e}"),
    }
}

/// List the files currently in the knowledge index.
pub async fn list_sources(app: &AppHandle, instance_id: &str) -> String {
    match driven::knowledge_index(app, instance_id)
        .list_sources()
        .await
    {
        Ok(list) if list.is_empty() => "the knowledge index is empty".to_string(),
        Ok(list) => list
            .iter()
            .map(|s| {
                format!(
                    "- {} (drive_id={}, {} chunks)",
                    s.name, s.drive_id, s.chunks
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) => format!("error: {e}"),
    }
}

/// Rebuild the index from the Drive folder, reporting live progress.
#[allow(clippy::too_many_arguments)]
pub async fn reindex(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
    progress: &Progress,
) -> String {
    bot::emit_log(app, &bot.id, "reindex: scanning Drive…");
    progress.report("🔄 Rebuilding the knowledge index — scanning Drive…");

    let uc = driving::reindex_knowledge(app, bot, instance_id, client_id, client_secret, folder_id);
    let report = |p: ReindexProgress| {
        progress.report_with(
            format!(
                "🔄 Rebuilding the knowledge index — {}/{}: \"{}\"",
                p.seen, p.total, p.name
            ),
            format!("{}%", p.seen * 100 / p.total.max(1)),
        );
    };
    match uc.run(&report).await {
        Ok(c) => format!(
            "indexed {} new file(s), skipped {}, failed {}",
            c.indexed, c.skipped, c.failed
        ),
        Err(e) => format!("error: {e}"),
    }
}

fn format_hits(question: &str, hits: &[KnowledgePassage]) -> String {
    let mut out = format!(
        "Knowledge for \"{question}\". Synthesise an answer grounded ONLY in these passages and \
         cite files by name.\n\n"
    );
    for h in hits {
        out.push_str(&format!(
            "### {} (drive_id={})\n{}\n\n",
            h.name,
            h.drive_id,
            h.text.trim()
        ));
    }
    out.trim().to_string()
}
