//! Discord side of a single bot: connect via serenity, decide whether a message
//! is for us, run the ReAct tool-loop, and reply. Every emitted event carries
//! the bot's id so the UI can scope streams.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serenity::all::{
    Attachment, ChannelId, Client, Context, CreateAttachment, CreateMessage, EditMessage,
    EventHandler, GatewayIntents, GetMessages, GuildId, Message, Ready, ScheduledEvent,
    ScheduledEventStatus, User, UserId,
};
use serenity::async_trait;
use songbird::driver::DecodeMode;
use songbird::{CoreEvent, Event, SerenityInit};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex as AsyncMutex;

use crate::bot::{self, ActivityKind, BotManager, Decision, Policy};
use crate::config::{self, BotConfig, GlobalConfig};
use crate::model::{self, ChatMessage};
use crate::tools::{self, AttachmentRef, AttachmentSink, ResolvedTool};
use crate::voice::{Meeting, Receiver};

/// How many recent messages to fetch (covers the raw window plus a batch to
/// compact once they scroll past it).
const HISTORY_LIMIT: u8 = 30;
/// Recent messages kept verbatim in the model context; older ones are folded
/// into a running summary.
const RAW_WINDOW: usize = 10;
/// Compact once at least this many messages have scrolled past the raw window.
const COMPACT_EVERY: usize = 8;
const DISCORD_MAX: usize = 2000;
/// Cap how many messages one reply may be split across, to avoid flooding.
const MAX_REPLY_MESSAGES: usize = 6;
const MAX_TOOL_ITERS: usize = 10;
/// Emit a streaming "thinking" update to the UI every this many new characters.
const STREAM_EMIT_CHARS: usize = 40;
/// Cap the attachments considered per message, to bound per-attachment gate calls.
const MAX_ATTACHMENTS: usize = 10;
/// Read attachments up to this size inline (covers typical PDFs/docs); larger
/// files are left to the archiving gate.
const MAX_ATTACHMENT_BYTES: u32 = 10_000_000;
const MAX_ATTACHMENT_CHARS: usize = 8000;
/// Cap audio considered for transcription (matches typical Whisper upload limits).
const MAX_AUDIO_BYTES: u32 = 25_000_000;
/// Cap the transcript text fed back into the reply's inline context.
const MAX_TRANSCRIPT_CONTEXT_CHARS: usize = 8000;

/// An open follow-up window for a channel.
struct ActiveConvo {
    count: u32,
    started: Instant,
}

/// A per-channel rolling summary of messages that have scrolled past the raw
/// window, so long threads fit in context.
#[derive(Default)]
struct ChannelMemory {
    summary: String,
    /// Newest message id already folded into `summary`.
    summarized_through: Option<u64>,
}

struct Handler {
    app: AppHandle,
    bot_id: String,
    /// This client instance's epoch; if it's no longer the current epoch for the
    /// bot, a superseding client has taken over and we ignore messages.
    epoch: u64,
    bot: BotConfig,
    catalog: Vec<ResolvedTool>,
    /// Tools subscribed to the attachment gate (resolved once at start).
    sinks: Vec<AttachmentSink>,
    windows: Mutex<HashMap<ChannelId, ActiveConvo>>,
    convos: Mutex<HashMap<ChannelId, ChannelMemory>>,
    /// Active voice-channel transcriptions, keyed by guild.
    meetings: AsyncMutex<HashMap<GuildId, Arc<Meeting>>>,
}

impl Handler {
    fn new(
        app: AppHandle,
        bot_id: String,
        epoch: u64,
        bot: BotConfig,
        global: GlobalConfig,
    ) -> Self {
        let catalog = tools::catalog(&global, &bot);
        let sinks = tools::attachment_sinks(&global, &bot);
        Self {
            app,
            bot_id,
            epoch,
            bot,
            catalog,
            sinks,
            windows: Mutex::new(HashMap::new()),
            convos: Mutex::new(HashMap::new()),
            meetings: AsyncMutex::new(HashMap::new()),
        }
    }

    /// Fold messages that have scrolled past the raw window into this channel's
    /// running summary (only when enough have accumulated), and return the
    /// current summary. `history` is newest-first.
    async fn compact(&self, channel_id: ChannelId, history: &[Message]) -> String {
        let (previous, through) = {
            let convos = self.convos.lock().unwrap();
            match convos.get(&channel_id) {
                Some(mem) => (mem.summary.clone(), mem.summarized_through),
                None => (String::new(), None),
            }
        };

        // Older-than-window messages not yet folded (still newest-first).
        let to_fold: Vec<&Message> = history
            .iter()
            .skip(RAW_WINDOW)
            .filter(|m| through.is_none_or(|t| m.id.get() > t))
            .collect();
        if to_fold.len() < COMPACT_EVERY {
            return previous;
        }

        let newest_id = to_fold.first().map(|m| m.id.get());
        let text = to_fold
            .iter()
            .rev() // oldest-first for a readable summary
            .map(|m| format!("{}: {}", m.author.name, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let summary = match model::summarize_conversation(&self.bot, &previous, &text).await {
            Ok(summary) => summary,
            Err(_) => return previous,
        };

        {
            let mut convos = self.convos.lock().unwrap();
            let mem = convos.entry(channel_id).or_default();
            mem.summary = summary.clone();
            mem.summarized_through = newest_id;
        }
        bot::emit_log(
            &self.app,
            &self.bot_id,
            format!(
                "compacted {} older message(s) into the running summary",
                to_fold.len()
            ),
        );
        summary
    }

    fn in_window(&self, msg: &Message) -> bool {
        let mut windows = self.windows.lock().unwrap();
        let Some(convo) = windows.get_mut(&msg.channel_id) else {
            return false;
        };
        let expired = convo.started.elapsed() > Duration::from_secs(self.bot.followup_window_secs)
            || convo.count >= self.bot.followup_window_messages;
        if expired {
            windows.remove(&msg.channel_id);
            return false;
        }
        convo.count += 1;
        true
    }

    fn refresh_window(&self, msg: &Message) {
        self.windows.lock().unwrap().insert(
            msg.channel_id,
            ActiveConvo {
                count: 0,
                started: Instant::now(),
            },
        );
    }

    async fn reply(
        &self,
        ctx: &Context,
        msg: &Message,
        history: &[Message],
        bot_id: UserId,
        bot_name: &str,
        audio_texts: &[(String, String)],
    ) {
        let channel = channel_label(ctx, msg).await;
        bot::emit_activity(
            &self.app,
            &self.bot_id,
            ActivityKind::Message,
            Some(msg.author.name.clone()),
            channel.clone(),
            humanize(&msg.content, &msg.mentions, bot_id, bot_name),
        );
        let _ = msg.channel_id.broadcast_typing(&ctx.http).await;
        bot::emit_thinking(&self.app, &self.bot_id, true);
        // Live progress message in the channel — edited as the bot works and
        // finally rewritten into the answer.
        let mut status = msg.channel_id.say(&ctx.http, "💭 Thinking…").await.ok();

        // Read text-ish attachments inline so the model can act on them this turn,
        // plus any transcripts of audio attachments already produced this message.
        let mut attachments = self.attachment_texts(msg).await;
        attachments.extend(audio_texts.iter().cloned());
        // Fold older messages into a running summary so long threads fit.
        let summary = self.compact(msg.channel_id, history).await;
        let mut messages =
            self.build_messages(history, msg, bot_id, bot_name, &attachments, &summary);
        let mut final_text: Option<String> = None;
        let mut error: Option<String> = None;
        let mut sources: Vec<String> = Vec::new();

        for iter in 0..MAX_TOOL_ITERS {
            if iter > 0 {
                set_status(ctx, &mut status, "💭 Thinking…").await;
            }
            // Live "thinking" entry the UI fills in as tokens stream, so a loop
            // is visible.
            let stream_id = bot::stream_start(&self.app, &self.bot_id);
            let app = self.app.clone();
            let bot_id = self.bot_id.clone();
            let sid = stream_id.clone();
            let mut emitted = 0usize;
            let result = model::chat(&self.bot, messages.clone(), |accumulated: &str| {
                if accumulated.len() >= emitted + STREAM_EMIT_CHARS {
                    emitted = accumulated.len();
                    bot::stream_update(&app, &bot_id, &sid, accumulated);
                }
            })
            .await;
            let (text, metrics) = match result {
                Ok(result) => result,
                Err(e) => {
                    bot::emit_log(&self.app, &self.bot_id, format!("Model error: {e}"));
                    error = Some(e);
                    break;
                }
            };
            // Final content (flush any un-emitted tail).
            bot::stream_update(&self.app, &self.bot_id, &stream_id, &text);
            bot::emit_metrics(&self.app, &self.bot_id, metrics);

            match model::parse_tool_call(&text) {
                None => {
                    final_text = Some(text);
                    break;
                }
                Some(call) => {
                    let result = self
                        .run_tool(ctx, msg.channel_id, &call, &mut sources, &mut status)
                        .await;
                    messages.push(ChatMessage::assistant(text));
                    messages.push(ChatMessage::user(format!(
                        "TOOL_RESULT: {result}\n(If more steps are needed, output the next \
                         TOOL_CALL; otherwise reply with your final answer to the user.)"
                    )));
                    let _ = msg.channel_id.broadcast_typing(&ctx.http).await;
                }
            }
        }

        let reply = if let Some(e) = error {
            format!("Sorry — I hit an error while working on that: {e}")
        } else {
            let cleaned = final_text
                .as_deref()
                .map(sanitize_reply)
                .unwrap_or_default();
            if cleaned.is_empty() {
                "I ran the tools but didn't produce a final answer.".to_string()
            } else {
                with_sources(&sources, &cleaned)
            }
        };
        bot::emit_activity(
            &self.app,
            &self.bot_id,
            ActivityKind::Reply,
            Some(self.bot.name.clone()),
            channel,
            reply.clone(),
        );
        let mut chunks = split_message(&reply, DISCORD_MAX).into_iter();
        // Rewrite the status message into the first chunk; send the rest.
        if let Some(first) = chunks.next() {
            match status.as_mut() {
                Some(m) => {
                    let _ = m.edit(&ctx.http, EditMessage::new().content(first)).await;
                }
                None => {
                    let _ = msg.channel_id.say(&ctx.http, first).await;
                }
            }
        }
        for chunk in chunks {
            if let Err(e) = msg.channel_id.say(&ctx.http, chunk).await {
                bot::emit_log(
                    &self.app,
                    &self.bot_id,
                    format!("Failed to send reply: {e}"),
                );
                break;
            }
        }
        bot::emit_thinking(&self.app, &self.bot_id, false);
        self.refresh_window(msg);
    }

    async fn run_tool(
        &self,
        ctx: &Context,
        channel_id: ChannelId,
        call: &model::ToolCall,
        sources: &mut Vec<String>,
        status: &mut Option<Message>,
    ) -> String {
        let name = call.tool.as_str();

        let Some(tool) = tools::find(&self.catalog, name) else {
            bot::emit_log(&self.app, &self.bot_id, format!("unknown tool '{name}'"));
            return format!("error: unknown tool '{name}'");
        };

        // Fresh bot config so an "always allow/deny" set mid-session applies now.
        let policy_key = tool.policy_key();
        let fresh = config::load_bot(&self.app, &self.bot_id).unwrap_or_else(|| self.bot.clone());
        let allowed = match bot::policy_for(&fresh, &policy_key, tool.is_write()) {
            Policy::Allow => true,
            Policy::Deny => {
                bot::emit_log(&self.app, &self.bot_id, format!("{name}: denied by policy"));
                return "denied by policy".to_string();
            }
            Policy::Ask => {
                set_status(ctx, status, "⏳ Waiting for your approval…").await;
                match bot::request_approval(&self.app, &self.bot_id, name, &call.args).await {
                    Decision::Approve => true,
                    Decision::Deny => false,
                    Decision::AlwaysAllow => {
                        config::set_tool_policy(&self.app, &self.bot_id, &policy_key, "allow");
                        true
                    }
                    Decision::AlwaysDeny => {
                        config::set_tool_policy(&self.app, &self.bot_id, &policy_key, "deny");
                        false
                    }
                }
            }
        };
        if !allowed {
            bot::emit_log(&self.app, &self.bot_id, format!("{name}: denied"));
            return "denied by user".to_string();
        }

        // Show what the bot is doing while the tool runs (before any progress).
        set_status(ctx, status, &tool.active_label(&call.args)).await;

        let result = if tool.is_backfill() {
            self.backfill(ctx, channel_id, tool, &call.args).await
        } else {
            // Run the tool, live-updating the status message from its progress
            // reports (throttled to respect Discord's edit rate limits).
            let (progress, mut rx) = tools::Progress::channel();
            let exec = tools::execute(&self.app, &self.bot_id, tool, &call.args, &progress);
            tokio::pin!(exec);
            let mut last_edit = Instant::now() - Duration::from_secs(10);
            loop {
                tokio::select! {
                    r = &mut exec => break r,
                    Some(update) = rx.recv() => {
                        if last_edit.elapsed() >= Duration::from_millis(1500) {
                            set_status(ctx, status, &update).await;
                            last_edit = Instant::now();
                        }
                    }
                }
            }
        };
        for url in tool.source_urls(&call.args, &result) {
            if !sources.contains(&url) {
                sources.push(url);
            }
        }
        let summary = tool.summary(&call.args, &result);
        set_status(ctx, status, &summary).await;
        bot::emit_tool_activity(
            &self.app,
            &self.bot_id,
            format!("{name} {} → {}", call.args, first_line(&result)),
            summary,
        );
        result
    }

    /// Extract text from the trigger message's attachments so their content can
    /// be read inline this turn (the user attaches a file and asks the bot to act
    /// on it). Handles text-ish files and PDFs; bounded by size + char cap.
    async fn attachment_texts(&self, msg: &Message) -> Vec<(String, String)> {
        if !self.bot.attachments_enabled {
            return Vec::new();
        }
        let mut out = Vec::new();
        for a in msg.attachments.iter().take(MAX_ATTACHMENTS) {
            if a.size > MAX_ATTACHMENT_BYTES {
                continue; // too big to read inline (may still be archived by the gate)
            }
            let bytes = match download_bytes(&a.url).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    bot::emit_log(
                        &self.app,
                        &self.bot_id,
                        format!("attachment \"{}\": read failed: {e}", a.filename),
                    );
                    continue;
                }
            };
            // PDF parsing is CPU-bound — keep it off the async runtime.
            let filename = a.filename.clone();
            let mime = a.content_type.clone().unwrap_or_default();
            let text = tokio::task::spawn_blocking(move || {
                crate::ingest::extract_text(&bytes, &filename, &mime)
            })
            .await
            .ok()
            .flatten();
            if let Some(text) = text {
                out.push((
                    a.filename.clone(),
                    truncate(text.trim(), MAX_ATTACHMENT_CHARS),
                ));
            }
        }
        out
    }

    /// Forward each attachment on a message to every subscribed sink. Audio is
    /// skipped here — the transcription path owns it.
    async fn dispatch_attachments(&self, msg: &Message) {
        let context = format!("{}: {}", msg.author.name, msg.content);
        for attachment in msg.attachments.iter().take(MAX_ATTACHMENTS) {
            let mime = attachment.content_type.clone().unwrap_or_default();
            if self.bot.transcription_enabled
                && crate::ingest::is_audio(&attachment.filename, &mime)
            {
                continue;
            }
            let att = attachment_ref(attachment);
            for sink in &self.sinks {
                tools::deliver_attachment(&self.app, &self.bot, sink, &att, &context).await;
            }
        }
    }

    /// Transcribe audio attachments on a message: post a transcript + summary
    /// `.md` back to the channel, index them into the knowledge base when a Drive
    /// tool is enabled, and return each transcript so the reply can use it inline.
    async fn handle_audio(&self, ctx: &Context, msg: &Message) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let clips: Vec<&Attachment> = msg
            .attachments
            .iter()
            .take(MAX_ATTACHMENTS)
            .filter(|a| {
                crate::ingest::is_audio(&a.filename, &a.content_type.clone().unwrap_or_default())
            })
            .collect();
        if clips.is_empty() {
            return out;
        }

        bot::emit_thinking(&self.app, &self.bot_id, true);
        let drive_sink = self
            .sinks
            .iter()
            .find(|s| matches!(s, AttachmentSink::Drive { .. }));

        for a in clips {
            if a.size > MAX_AUDIO_BYTES {
                let _ = msg
                    .channel_id
                    .say(
                        &ctx.http,
                        format!(
                            "🎙️ \"{}\" is too large to transcribe (over 25 MB).",
                            a.filename
                        ),
                    )
                    .await;
                continue;
            }
            // A persistent "transcribing" note (typing indicators time out after
            // ~10 s); removed once the transcript is posted.
            let mut note = msg
                .channel_id
                .say(&ctx.http, format!("🎙️ Transcribing **{}**…", a.filename))
                .await
                .ok();
            bot::emit_log(
                &self.app,
                &self.bot_id,
                format!("transcribing \"{}\"…", a.filename),
            );

            let bytes = match download_bytes(&a.url).await {
                Ok(b) => b,
                Err(e) => {
                    bot::emit_log(
                        &self.app,
                        &self.bot_id,
                        format!("audio \"{}\": download failed: {e}", a.filename),
                    );
                    set_status(
                        ctx,
                        &mut note,
                        &format!("🎙️ Couldn't fetch \"{}\": {e}", a.filename),
                    )
                    .await;
                    continue;
                }
            };
            let mime = a.content_type.clone().unwrap_or_default();
            // Normalise to WAV client-side (mp3/m4a/flac/ogg → PCM via symphonia)
            // so the transcription server needs no extra codecs. Decoding is
            // CPU-bound, so keep it off the async runtime. Falls back to the raw
            // bytes if the codec isn't supported (e.g. Opus).
            let (send_bytes, send_name, send_mime) = {
                let raw = bytes.clone();
                let filename = a.filename.clone();
                let ct = mime.clone();
                let decoded = tokio::task::spawn_blocking(move || {
                    crate::audio::decode_to_wav(&raw, &filename, &ct)
                })
                .await
                .ok()
                .flatten();
                match decoded {
                    Some(wav) => (wav, "audio.wav".to_string(), "audio/wav".to_string()),
                    None => (bytes, a.filename.clone(), mime.clone()),
                }
            };
            let transcript =
                match model::transcribe(&self.bot, send_bytes, &send_name, &send_mime).await {
                    Ok(t) => t,
                    Err(e) => {
                        bot::emit_log(
                            &self.app,
                            &self.bot_id,
                            format!("audio \"{}\": transcription failed: {e}", a.filename),
                        );
                        set_status(
                            ctx,
                            &mut note,
                            &format!("🎙️ Couldn't transcribe \"{}\": {e}", a.filename),
                        )
                        .await;
                        continue;
                    }
                };
            let summary = model::summarize_transcript(&self.bot, &transcript)
                .await
                .unwrap_or_else(|e| format!("_(summary unavailable: {e})_"));

            let stem = a
                .filename
                .rsplit_once('.')
                .map(|(s, _)| s)
                .unwrap_or(&a.filename);
            let when = msg.timestamp.to_string();
            let transcript_md = format!(
                "# Transcript — {name}\n\n- Source: Discord, posted by {author}\n- Transcribed: \
                 {when}\n\n---\n\n{transcript}\n",
                name = a.filename,
                author = msg.author.name,
            );
            let summary_md = format!(
                "# Summary — {name}\n\n- Source: Discord, posted by {author}\n- Transcribed: \
                 {when}\n\n{summary}\n",
                name = a.filename,
                author = msg.author.name,
            );
            let transcript_file = format!("{stem}.transcript.md");
            let summary_file = format!("{stem}.summary.md");

            // Post both files back to the channel.
            let builder = CreateMessage::new()
                .content(format!(
                    "🎙️ Transcribed **{}** — transcript + summary attached.",
                    a.filename
                ))
                .files(vec![
                    CreateAttachment::bytes(transcript_md.clone().into_bytes(), &transcript_file),
                    CreateAttachment::bytes(summary_md.clone().into_bytes(), &summary_file),
                ]);
            if let Err(e) = msg.channel_id.send_message(&ctx.http, builder).await {
                bot::emit_log(
                    &self.app,
                    &self.bot_id,
                    format!("audio \"{}\": send failed: {e}", a.filename),
                );
            }
            // The result message replaces the "Transcribing…" note.
            if let Some(m) = &note {
                let _ = m.delete(&ctx.http).await;
            }
            bot::emit_tool_activity(
                &self.app,
                &self.bot_id,
                format!(
                    "transcribe {{name={:?}}} → {} chars",
                    a.filename,
                    transcript.chars().count()
                ),
                format!("🎙️ Transcribed \"{}\"", a.filename),
            );

            // Index into the knowledge base when a Drive tool is enabled.
            if let Some(sink) = drive_sink {
                let context = format!(
                    "audio transcript from Discord, posted by {}",
                    msg.author.name
                );
                tools::store_text_artifact(
                    &self.app,
                    &self.bot,
                    sink,
                    &transcript_file,
                    &transcript_md,
                    &context,
                )
                .await;
                tools::store_text_artifact(
                    &self.app,
                    &self.bot,
                    sink,
                    &summary_file,
                    &summary_md,
                    &context,
                )
                .await;
            }

            out.push((
                transcript_file,
                truncate(transcript.trim(), MAX_TRANSCRIPT_CONTEXT_CHARS),
            ));
        }

        bot::emit_thinking(&self.app, &self.bot_id, false);
        out
    }

    /// On-demand backfill: sweep recent channel history and run found
    /// attachments through this Drive tool's archiving gate.
    async fn backfill(
        &self,
        ctx: &Context,
        channel_id: ChannelId,
        tool: &ResolvedTool,
        args: &serde_json::Value,
    ) -> String {
        let Some(sink) = tool.drive_sink() else {
            return "error: not a Drive tool".to_string();
        };
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
            .clamp(1, 100) as u8;

        let messages = channel_id
            .messages(&ctx.http, GetMessages::new().limit(limit))
            .await
            .unwrap_or_default();

        let mut scanned = 0usize;
        let mut archived = 0usize;
        for m in &messages {
            scanned += 1;
            for attachment in m.attachments.iter().take(MAX_ATTACHMENTS) {
                let att = attachment_ref(attachment);
                let context = format!("{}: {}", m.author.name, m.content);
                if tools::deliver_attachment(&self.app, &self.bot, &sink, &att, &context).await {
                    archived += 1;
                }
            }
        }
        format!("scanned {scanned} messages, archived {archived} file(s)")
    }

    /// Route a join/leave voice command to the right action.
    async fn handle_voice_command(&self, ctx: &Context, msg: &Message, cmd: VoiceCmd) {
        let Some(guild_id) = msg.guild_id else {
            let _ = msg
                .channel_id
                .say(&ctx.http, "Voice transcription only works in a server.")
                .await;
            return;
        };
        match cmd {
            VoiceCmd::Join => {
                // The channel the requester is currently in (from the voice-state cache).
                let voice_channel = ctx.cache.guild(guild_id).and_then(|g| {
                    g.voice_states
                        .get(&msg.author.id)
                        .and_then(|vs| vs.channel_id)
                });
                match voice_channel {
                    Some(vc) => self.voice_join(ctx, guild_id, vc, msg.channel_id).await,
                    None => {
                        let _ = msg
                            .channel_id
                            .say(&ctx.http, "Join a voice channel first, then ask me again.")
                            .await;
                    }
                }
            }
            VoiceCmd::Leave => self.voice_leave(ctx, guild_id, msg.channel_id).await,
        }
    }

    /// Join a voice channel and start transcribing, announcing that we're doing
    /// so (consent). No-op if already in a meeting for this guild.
    async fn voice_join(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        voice_channel: ChannelId,
        text_channel: ChannelId,
    ) {
        let Some(manager) = songbird::get(ctx).await else {
            let _ = text_channel
                .say(&ctx.http, "Voice support isn't available right now.")
                .await;
            return;
        };
        if self.meetings.lock().await.contains_key(&guild_id) {
            let _ = text_channel
                .say(&ctx.http, "I'm already transcribing a call in this server.")
                .await;
            return;
        }
        match manager.join(guild_id, voice_channel).await {
            Ok(call) => {
                let meeting = Meeting::new(self.app.clone(), self.bot_id.clone(), self.bot.clone());
                {
                    let mut handler = call.lock().await;
                    handler.add_global_event(
                        Event::Core(CoreEvent::SpeakingStateUpdate),
                        Receiver::new(meeting.clone()),
                    );
                    handler.add_global_event(
                        Event::Core(CoreEvent::VoiceTick),
                        Receiver::new(meeting.clone()),
                    );
                }
                self.meetings.lock().await.insert(guild_id, meeting);
                bot::emit_tool_activity(
                    &self.app,
                    &self.bot_id,
                    format!("voice_join {{guild={guild_id}, channel={voice_channel}}}"),
                    "🎙️ Joined a voice channel to transcribe".to_string(),
                );
                let _ = text_channel
                    .say(
                        &ctx.http,
                        "🎙️ I've joined the voice channel and I'm transcribing this conversation. \
                         I'll post a transcript and summary when I leave — just say “stop” or \
                         “leave” to end.",
                    )
                    .await;
            }
            Err(e) => {
                let _ = text_channel
                    .say(&ctx.http, format!("Couldn't join the voice channel: {e}"))
                    .await;
            }
        }
    }

    /// Leave the voice channel and produce the meeting transcript + summary.
    async fn voice_leave(&self, ctx: &Context, guild_id: GuildId, text_channel: ChannelId) {
        let meeting = self.meetings.lock().await.remove(&guild_id);
        if let Some(manager) = songbird::get(ctx).await {
            let _ = manager.remove(guild_id).await;
        }
        match meeting {
            Some(meeting) => self.finalize_meeting(ctx, text_channel, meeting).await,
            None => {
                let _ = text_channel
                    .say(&ctx.http, "I'm not transcribing a call here.")
                    .await;
            }
        }
    }

    /// Assemble and deliver the transcript + summary for a finished meeting.
    async fn finalize_meeting(
        &self,
        ctx: &Context,
        text_channel: ChannelId,
        meeting: Arc<Meeting>,
    ) {
        let _ = text_channel
            .say(&ctx.http, "🎙️ Wrapping up — preparing the transcript…")
            .await;
        bot::emit_thinking(&self.app, &self.bot_id, true);

        let Some(rendered) = meeting.render().await else {
            bot::emit_thinking(&self.app, &self.bot_id, false);
            let _ = text_channel
                .say(&ctx.http, "I didn't capture any speech to transcribe.")
                .await;
            return;
        };

        // Resolve speaker ids to display names.
        let mut names = HashMap::new();
        for id in &rendered.user_ids {
            let name = ctx
                .http
                .get_user(UserId::new(*id))
                .await
                .map(|u| u.name)
                .unwrap_or_else(|_| "Unknown".to_string());
            names.insert(*id, name);
        }
        let body = rendered.body(&names);

        let transcript_md = format!(
            "# Meeting transcript\n\n- Source: Discord voice channel\n- Duration: ~{} min\n\n\
             ---\n\n{body}\n",
            rendered.minutes,
        );
        let summary = model::summarize_transcript(&self.bot, &body)
            .await
            .unwrap_or_else(|e| format!("_(summary unavailable: {e})_"));
        let summary_md = format!("# Meeting summary\n\n{summary}\n");

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let transcript_file = format!("meeting-{stamp}.transcript.md");
        let summary_file = format!("meeting-{stamp}.summary.md");

        let builder = CreateMessage::new()
            .content("🎙️ Meeting transcript + summary attached.")
            .files(vec![
                CreateAttachment::bytes(transcript_md.clone().into_bytes(), &transcript_file),
                CreateAttachment::bytes(summary_md.clone().into_bytes(), &summary_file),
            ]);
        if let Err(e) = text_channel.send_message(&ctx.http, builder).await {
            bot::emit_log(
                &self.app,
                &self.bot_id,
                format!("meeting: send failed: {e}"),
            );
        }

        if let Some(sink) = self
            .sinks
            .iter()
            .find(|s| matches!(s, AttachmentSink::Drive { .. }))
        {
            let context = "meeting transcript from a Discord voice channel";
            tools::store_text_artifact(
                &self.app,
                &self.bot,
                sink,
                &transcript_file,
                &transcript_md,
                context,
            )
            .await;
            tools::store_text_artifact(
                &self.app,
                &self.bot,
                sink,
                &summary_file,
                &summary_md,
                context,
            )
            .await;
        }
        bot::emit_thinking(&self.app, &self.bot_id, false);
    }

    fn build_messages(
        &self,
        history: &[Message],
        msg: &Message,
        bot_id: UserId,
        bot_name: &str,
        attachments: &[(String, String)],
        summary: &str,
    ) -> Vec<ChatMessage> {
        let identity = format!(
            "You are taking part in a Discord conversation as \"{bot_name}\". When a message \
             contains @{bot_name} or replies to you, it is addressed to you — respond directly and \
             in character. Do not narrate or summarise the conversation unless explicitly asked."
        );
        let mut system = format!("{}\n\n{}", self.bot.system_prompt, identity);
        if !self.catalog.is_empty() {
            system.push_str(&tools::prompt_section(&self.catalog));
        }
        if !summary.trim().is_empty() {
            system.push_str(&format!(
                "\n\n## Earlier conversation (summary of messages before the recent ones)\n{summary}"
            ));
        }
        if !attachments.is_empty() {
            system.push_str(
                "\n\nThe user's message includes attached file contents inline (marked \
                 [Attached file …]). Read them and act on any instructions they contain.",
            );
        }
        // Load memories fresh so ones saved this session apply next turn.
        if self.bot.memory_enabled {
            let memories = crate::memory::load(&self.app, &self.bot_id);
            system.push_str(&crate::memory::system_section(&memories));
        }
        let mut messages = vec![ChatMessage::system(system)];

        // Only the recent raw window goes in verbatim (oldest-first); older
        // messages are represented by the summary above.
        for m in history.iter().take(RAW_WINDOW).rev() {
            let content = humanize(&m.content, &m.mentions, bot_id, bot_name);
            if content.trim().is_empty() {
                continue;
            }
            if m.author.id == bot_id {
                messages.push(ChatMessage::assistant(content));
            } else {
                messages.push(ChatMessage::user(format!("{}: {}", m.author.name, content)));
            }
        }

        let mut trigger = humanize(&msg.content, &msg.mentions, bot_id, bot_name);
        for (name, text) in attachments {
            trigger.push_str(&format!("\n\n[Attached file \"{name}\":\n{text}\n]"));
        }
        messages.push(ChatMessage::user(format!(
            "{}: {}",
            msg.author.name, trigger
        )));
        messages
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        bot::emit_log(
            &self.app,
            &self.bot_id,
            format!("Connected to Discord as {}", ready.user.name),
        );
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        // Ignore messages if a newer client has superseded this one (or the bot
        // was stopped) — guarantees a single client processes each message even
        // during a stop/start overlap.
        if bot::current_epoch(&self.app, &self.bot_id) != Some(self.epoch) {
            return;
        }
        let (bot_user_id, bot_user_name) = {
            let me = ctx.cache.current_user();
            (me.id, me.name.clone())
        };

        let explicit = msg.guild_id.is_none()
            || msg.mentions_me(&ctx).await.unwrap_or(false)
            || msg
                .referenced_message
                .as_ref()
                .is_some_and(|r| r.author.id == bot_user_id);

        let in_window = self.in_window(&msg);
        if !explicit && !in_window {
            return;
        }

        // Voice-channel commands: join/leave to transcribe a live call.
        if explicit {
            if let Some(cmd) = voice_command(&msg.content) {
                self.handle_voice_command(&ctx, &msg, cmd).await;
                return;
            }
        }

        // Transcribe audio attachments (post transcript + summary, index them);
        // returns transcripts so the reply can use them inline this turn.
        let audio_texts = if self.bot.transcription_enabled && !msg.attachments.is_empty() {
            self.handle_audio(&ctx, &msg).await
        } else {
            Vec::new()
        };
        // Forward remaining (non-audio) attachments to subscribed tools.
        if self.bot.attachments_enabled && !self.sinks.is_empty() && !msg.attachments.is_empty() {
            self.dispatch_attachments(&msg).await;
        }

        let history = msg
            .channel_id
            .messages(
                &ctx.http,
                GetMessages::new().before(msg.id).limit(HISTORY_LIMIT),
            )
            .await
            .unwrap_or_default();

        if !explicit {
            // Only the recent window matters for the relevance gate.
            let recent = &history[..history.len().min(RAW_WINDOW)];
            let context = render_context(recent, bot_user_id);
            bot::emit_log(&self.app, &self.bot_id, "follow-up: checking relevance…");
            let engage = model::should_engage(&self.bot, &context, &msg.content).await;
            bot::emit_log(
                &self.app,
                &self.bot_id,
                format!(
                    "follow-up: {}",
                    if engage {
                        "yes — replying"
                    } else {
                        "no — ignoring"
                    }
                ),
            );
            if !engage {
                return;
            }
        }

        self.reply(
            &ctx,
            &msg,
            &history,
            bot_user_id,
            &bot_user_name,
            &audio_texts,
        )
        .await;
    }

    /// A scheduled event went live — if it's tied to a voice channel, offer to
    /// join and transcribe it (Phase 3). The user confirms with a "join" command.
    async fn guild_scheduled_event_update(&self, ctx: Context, event: ScheduledEvent) {
        if !self.bot.transcription_enabled {
            return;
        }
        if !matches!(event.status, ScheduledEventStatus::Active) {
            return;
        }
        let Some(voice_channel) = event.channel_id else {
            return; // external event, no voice channel to join
        };
        let Some(text) = ctx
            .cache
            .guild(event.guild_id)
            .and_then(|g| g.system_channel_id)
        else {
            return; // nowhere obvious to announce
        };
        let _ = text
            .say(
                &ctx.http,
                format!(
                    "📅 **{}** just started in <#{}>. Want me to join and transcribe it? \
                     Mention me with “join the call”.",
                    event.name, voice_channel
                ),
            )
            .await;
        bot::emit_log(
            &self.app,
            &self.bot_id,
            format!("scheduled event active: {}", event.name),
        );
    }
}

/// A parsed voice command from an addressed message.
enum VoiceCmd {
    Join,
    Leave,
}

/// Detect a join/leave voice-transcription command in an addressed message.
/// Requires both an action word and a voice-context word, to avoid false hits.
fn voice_command(content: &str) -> Option<VoiceCmd> {
    let c = content.to_lowercase();
    let has = |ws: &[&str]| ws.iter().any(|w| c.contains(w));
    const CONTEXT: &[&str] = &[
        "voice",
        "call",
        " vc",
        "meeting",
        "channel",
        "transcri",
        "recording",
        "listening",
    ];
    if !has(CONTEXT) {
        return None;
    }
    if has(&["leave", "stop", "hang up", "disconnect", "wrap up"]) {
        return Some(VoiceCmd::Leave);
    }
    if has(&["join", "come in", "hop in", "sit in"]) {
        return Some(VoiceCmd::Join);
    }
    None
}

/// Supervisor: build and run one bot's client until it stops, then mark it
/// stopped so the UI updates.
pub async fn run(app: AppHandle, bot_id: String, epoch: u64, bot: BotConfig, global: GlobalConfig) {
    bot::emit_log(&app, &bot_id, "Connecting to Discord…");
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES;

    let token = bot.discord_token.clone();
    let handler = Handler::new(app.clone(), bot_id.clone(), epoch, bot, global);
    // Decode received voice to PCM so we can transcribe it (voice.rs).
    let songbird_config = songbird::Config::default().decode_mode(DecodeMode::Decode);
    let mut client = match Client::builder(&token, intents)
        .event_handler(handler)
        .register_songbird_from_config(songbird_config)
        .await
    {
        Ok(client) => client,
        Err(e) => {
            bot::emit_log(&app, &bot_id, format!("Discord connection failed: {e}"));
            bot::stop(&app, &bot_id);
            return;
        }
    };

    app.state::<BotManager>()
        .set_shard_manager(&bot_id, client.shard_manager.clone());

    if let Err(e) = client.start().await {
        bot::emit_log(&app, &bot_id, format!("Discord gateway error: {e}"));
    }
    bot::stop(&app, &bot_id);
}

/// Replace raw Discord mention tokens (`<@id>`) with readable `@name`s so the
/// model knows when it's being addressed.
fn humanize(content: &str, mentions: &[User], bot_id: UserId, bot_name: &str) -> String {
    let mut out = content.to_string();
    for user in mentions {
        let name = if user.id == bot_id {
            bot_name
        } else {
            user.name.as_str()
        };
        out = out.replace(&format!("<@{}>", user.id), &format!("@{name}"));
        out = out.replace(&format!("<@!{}>", user.id), &format!("@{name}"));
    }
    out
}

async fn channel_label(ctx: &Context, msg: &Message) -> Option<String> {
    if msg.guild_id.is_none() {
        return Some("DM".into());
    }
    match msg.channel_id.to_channel(ctx).await {
        Ok(channel) => channel.guild().map(|c| c.name),
        Err(_) => None,
    }
}

fn render_context(history: &[Message], bot_id: UserId) -> String {
    history
        .iter()
        .rev()
        .map(|m| {
            let who = if m.author.id == bot_id {
                "assistant"
            } else {
                m.author.name.as_str()
            };
            format!("{who}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

/// Edit the live progress message in the channel (no-op if it wasn't posted).
async fn set_status(ctx: &Context, status: &mut Option<Message>, text: &str) {
    if let Some(m) = status.as_mut() {
        let _ = m
            .edit(
                &ctx.http,
                EditMessage::new().content(truncate(text, DISCORD_MAX)),
            )
            .await;
    }
}

/// Split a reply into Discord-sized messages, breaking on paragraph/line/word
/// boundaries where possible. Capped at `MAX_REPLY_MESSAGES` so a runaway reply
/// can't flood a channel; the last piece is marked if content was dropped.
fn split_message(text: &str, max: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return vec![text.to_string()];
    }

    let mut parts = Vec::new();
    let mut start = 0;
    while start < chars.len() && parts.len() < MAX_REPLY_MESSAGES {
        let hard_end = (start + max).min(chars.len());
        let end = if hard_end == chars.len() {
            hard_end
        } else {
            let slice = &chars[start..hard_end];
            match slice
                .iter()
                .rposition(|&c| c == '\n')
                .or_else(|| slice.iter().rposition(|&c| c.is_whitespace()))
            {
                Some(rel) if rel > 0 => start + rel + 1,
                _ => hard_end,
            }
        };
        let piece: String = chars[start..end].iter().collect();
        let piece = piece.trim().to_string();
        if !piece.is_empty() {
            parts.push(piece);
        }
        start = end;
    }

    if start < chars.len() {
        if let Some(last) = parts.last_mut() {
            last.push_str(" …(truncated)");
        }
    }
    parts
}

fn attachment_ref(a: &Attachment) -> AttachmentRef {
    AttachmentRef {
        filename: a.filename.clone(),
        content_type: a.content_type.clone().unwrap_or_default(),
        url: a.url.clone(),
    }
}

async fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("read failed: {e}"))
}

/// Prepend a compact "Sources" header (deduped, capped) when web tools were
/// used. `<url>` angle brackets suppress Discord's link previews.
fn with_sources(sources: &[String], reply: &str) -> String {
    if sources.is_empty() {
        return reply.to_string();
    }
    const MAX_SOURCES: usize = 5;
    let list = sources
        .iter()
        .take(MAX_SOURCES)
        .map(|u| format!("<{u}>"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("**Sources**\n{list}\n\n{reply}")
}

/// Strip internal ReAct scaffolding the model sometimes echoes.
fn sanitize_reply(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let t = line.trim_start();
            !t.starts_with("TOOL_CALL")
                && !t.starts_with("TOOL_RESULT")
                && !t.starts_with("(If more steps are needed")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_short_is_single() {
        assert_eq!(split_message("hello", 2000), vec!["hello".to_string()]);
    }

    #[test]
    fn split_long_is_multiple() {
        assert!(split_message(&"word ".repeat(1000), 2000).len() > 1);
    }

    #[test]
    fn voice_command_detection() {
        assert!(matches!(
            voice_command("please join the voice call"),
            Some(VoiceCmd::Join)
        ));
        assert!(matches!(
            voice_command("ok stop the recording"),
            Some(VoiceCmd::Leave)
        ));
        assert!(voice_command("hello there").is_none());
        assert!(voice_command("join the party").is_none());
    }

    #[test]
    fn sanitize_strips_scaffolding() {
        let s = sanitize_reply("Answer line\nTOOL_CALL {\"x\":1}\nTOOL_RESULT: y");
        assert_eq!(s, "Answer line");
    }

    #[test]
    fn with_sources_prepends_header() {
        let out = with_sources(&["http://a".to_string()], "body");
        assert!(out.contains("**Sources**"));
        assert!(out.ends_with("body"));
        assert_eq!(with_sources(&[], "body"), "body");
    }

    #[test]
    fn first_line_and_truncate() {
        assert_eq!(first_line("a\nb"), "a");
        assert_eq!(truncate("abc", 10), "abc");
        assert!(truncate(&"x".repeat(20), 5).ends_with('…'));
    }
}
