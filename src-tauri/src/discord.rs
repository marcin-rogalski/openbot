//! Discord side of the bot: connect via serenity, decide whether a message is
//! for us, and drive the model reply. Every step emits activity events so the
//! chat preview shows what's happening.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serenity::all::{
    Client, Context, EventHandler, GatewayIntents, GetMessages, Message, Ready, User, UserId,
};
use serenity::async_trait;
use tauri::{AppHandle, Manager};

use crate::bot::{self, ActivityKind, BotManager};
use crate::config::BotConfig;
use crate::model::{self, ChatMessage};

/// How many recent channel messages to send as context.
const HISTORY_LIMIT: u8 = 8;
/// Discord's hard message length limit.
const DISCORD_MAX: usize = 2000;

/// An open follow-up window for a channel: the bot recently replied here, so it
/// considers the next few messages from anyone as possibly continuing the
/// conversation (subject to the relevance gate).
struct ActiveConvo {
    count: u32,
    started: Instant,
}

struct Handler {
    app: AppHandle,
    cfg: BotConfig,
    windows: Mutex<HashMap<serenity::all::ChannelId, ActiveConvo>>,
}

impl Handler {
    fn new(app: AppHandle, cfg: BotConfig) -> Self {
        Self { app, cfg, windows: Mutex::new(HashMap::new()) }
    }

    /// Is this channel in an active follow-up window? Consumes one message of
    /// the window's budget and expires it when spent. Any participant counts —
    /// the relevance gate decides whether to actually engage.
    fn in_window(&self, msg: &Message) -> bool {
        let mut windows = self.windows.lock().unwrap();
        let Some(convo) = windows.get_mut(&msg.channel_id) else {
            return false;
        };
        let expired = convo.started.elapsed()
            > Duration::from_secs(self.cfg.followup_window_secs)
            || convo.count >= self.cfg.followup_window_messages;
        if expired {
            windows.remove(&msg.channel_id);
            return false;
        }
        convo.count += 1;
        true
    }

    /// (Re)open the follow-up window after the bot replies in a channel.
    fn refresh_window(&self, msg: &Message) {
        self.windows
            .lock()
            .unwrap()
            .insert(msg.channel_id, ActiveConvo { count: 0, started: Instant::now() });
    }

    async fn reply(
        &self,
        ctx: &Context,
        msg: &Message,
        history: &[Message],
        bot_id: UserId,
        bot_name: &str,
    ) {
        let channel = channel_label(ctx, msg).await;
        bot::emit_activity(
            &self.app,
            ActivityKind::Message,
            Some(msg.author.name.clone()),
            channel.clone(),
            humanize(&msg.content, &msg.mentions, bot_id, bot_name),
        );
        bot::emit_activity(
            &self.app,
            ActivityKind::ModelCall,
            None,
            None,
            format!("→ {}", self.cfg.model_name),
        );
        let _ = msg.channel_id.broadcast_typing(&ctx.http).await;

        let messages = build_messages(&self.cfg, history, msg, bot_id, bot_name);
        match model::chat(&self.cfg, messages).await {
            Ok((reply, metrics)) => {
                bot::emit_metrics(&self.app, metrics);
                let reply = if reply.trim().is_empty() {
                    "(the model returned an empty response)".to_string()
                } else {
                    reply
                };
                bot::emit_activity(
                    &self.app,
                    ActivityKind::Reply,
                    Some("openbot".into()),
                    channel,
                    reply.clone(),
                );
                if let Err(e) = msg.channel_id.say(&ctx.http, truncate(&reply, DISCORD_MAX)).await {
                    bot::emit_log(&self.app, format!("Failed to send reply: {e}"));
                }
                self.refresh_window(msg);
            }
            Err(e) => bot::emit_log(&self.app, format!("Model error: {e}")),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        bot::emit_log(&self.app, format!("Connected to Discord as {}", ready.user.name));
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let (bot_id, bot_name) = {
            let me = ctx.cache.current_user();
            (me.id, me.name.clone())
        };

        let explicit = msg.guild_id.is_none()
            || msg.mentions_me(&ctx).await.unwrap_or(false)
            || msg
                .referenced_message
                .as_ref()
                .is_some_and(|r| r.author.id == bot_id);

        let in_window = self.in_window(&msg);
        if !explicit && !in_window {
            return;
        }

        let history = msg
            .channel_id
            .messages(&ctx.http, GetMessages::new().before(msg.id).limit(HISTORY_LIMIT))
            .await
            .unwrap_or_default();

        // Follow-up (no explicit signal): ask the model if the bot should engage.
        if !explicit {
            let context = render_context(&history, bot_id);
            bot::emit_log(&self.app, "follow-up: checking relevance…");
            let engage = model::should_engage(&self.cfg, &context, &msg.content).await;
            bot::emit_log(
                &self.app,
                format!("follow-up: {}", if engage { "yes — replying" } else { "no — ignoring" }),
            );
            if !engage {
                return;
            }
        }

        self.reply(&ctx, &msg, &history, bot_id, &bot_name).await;
    }
}

/// Supervisor: build and run the client until it stops, keeping the shard
/// manager registered so a Stop can shut it down. Reports failures as logs.
pub async fn run(app: AppHandle, cfg: BotConfig) {
    bot::emit_log(&app, "Connecting to Discord…");
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES;

    let handler = Handler::new(app.clone(), cfg.clone());
    let mut client = match Client::builder(&cfg.discord_token, intents)
        .event_handler(handler)
        .await
    {
        Ok(client) => client,
        Err(e) => {
            bot::emit_log(&app, format!("Discord connection failed: {e}"));
            bot::set_running(&app, false);
            return;
        }
    };

    app.state::<BotManager>().set_shard_manager(client.shard_manager.clone());

    if let Err(e) = client.start().await {
        bot::emit_log(&app, format!("Discord gateway error: {e}"));
    }
    // Gateway ended (stopped or errored) — make sure state reflects it.
    bot::set_running(&app, false);
}

fn build_messages(
    cfg: &BotConfig,
    history: &[Message],
    msg: &Message,
    bot_id: UserId,
    bot_name: &str,
) -> Vec<ChatMessage> {
    // Tell the model who it is, so it recognises its own @mention instead of
    // treating itself as a third party and narrating the conversation.
    let identity = format!(
        "You are taking part in a Discord conversation as \"{bot_name}\". When a message contains \
         @{bot_name} or replies to you, it is addressed to you — respond to it directly and in \
         character. Do not narrate or summarise the conversation unless you are explicitly asked to."
    );
    let mut messages =
        vec![ChatMessage::system(format!("{}\n\n{}", cfg.system_prompt, identity))];

    // `history` is newest-first; feed it oldest-first.
    for m in history.iter().rev() {
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

    let trigger = humanize(&msg.content, &msg.mentions, bot_id, bot_name);
    messages.push(ChatMessage::user(format!("{}: {}", msg.author.name, trigger)));
    messages
}

/// Replace raw Discord mention tokens (`<@id>`) with readable `@name`s. The
/// bot's own mention becomes its name so the model knows it's being addressed.
fn humanize(content: &str, mentions: &[User], bot_id: UserId, bot_name: &str) -> String {
    let mut out = content.to_string();
    for user in mentions {
        let name = if user.id == bot_id { bot_name } else { user.name.as_str() };
        out = out.replace(&format!("<@{}>", user.id), &format!("@{name}"));
        out = out.replace(&format!("<@!{}>", user.id), &format!("@{name}"));
    }
    out
}

/// Human-readable channel label for the activity feed: the channel name, or
/// "DM" for direct messages.
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
            let who = if m.author.id == bot_id { "assistant" } else { m.author.name.as_str() };
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
