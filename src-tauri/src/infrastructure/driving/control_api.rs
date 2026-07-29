//! A tiny localhost-only control API for listing and toggling bots, so the app
//! can be driven from the command line (e.g. `curl`) without the UI. Bound to
//! `127.0.0.1` only — no auth, local machine only.
//!
//! Routes:
//! - `GET  /bots`              → `{ "bots": [{ id, name, running, ready }] }`
//! - `POST /bots/<id>/start`   → start a bot
//! - `POST /bots/<id>/stop`    → stop a bot
//! - `POST /bots/<id>/toggle`  → start if stopped, stop if running

use serde_json::json;
use tauri::AppHandle;
use tiny_http::{Header, Method, Response, Server};

use crate::{bot, config};

const ADDR: &str = "127.0.0.1:8787";

/// Spawn the control server on a background thread. Failure to bind (e.g. port
/// in use) is logged and non-fatal.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || match Server::http(ADDR) {
        Ok(server) => {
            for request in server.incoming_requests() {
                let (status, body) = route(&app, request.method(), request.url());
                let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .expect("valid header");
                let response = Response::from_string(body)
                    .with_status_code(status)
                    .with_header(header);
                let _ = request.respond(response);
            }
        }
        Err(e) => eprintln!("api: could not bind {ADDR}: {e}"),
    });
}

fn route(app: &AppHandle, method: &Method, url: &str) -> (u16, String) {
    let path = url.split('?').next().unwrap_or(url);
    let parts: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    match (method, parts.as_slice()) {
        (Method::Get, ["bots"]) => (200, list(app)),
        (Method::Post, ["bots", id, action]) => toggle(app, id, action),
        _ => (404, json!({ "error": "not found" }).to_string()),
    }
}

fn list(app: &AppHandle) -> String {
    let running = bot::running_ids(app);
    let bots: Vec<_> = config::load_bots(app)
        .iter()
        .map(|b| {
            json!({
                "id": b.id,
                "name": b.name,
                "running": running.contains(&b.id),
                "ready": b.is_ready(),
            })
        })
        .collect();
    json!({ "bots": bots }).to_string()
}

fn toggle(app: &AppHandle, id: &str, action: &str) -> (u16, String) {
    let Some(bot) = config::load_bot(app, id) else {
        return (
            404,
            json!({ "error": format!("no bot with id {id}") }).to_string(),
        );
    };
    let running = bot::running_ids(app).contains(&bot.id);

    let start = match action {
        "start" => true,
        "stop" => false,
        "toggle" => !running,
        _ => return (404, json!({ "error": "unknown action" }).to_string()),
    };

    if start {
        if !bot.is_ready() {
            return (
                400,
                json!({ "error": "bot is not ready (needs Discord token + model)", "id": bot.id })
                    .to_string(),
            );
        }
        bot::start(app, &bot.id);
    } else {
        bot::stop(app, &bot.id);
    }

    (
        200,
        json!({ "ok": true, "id": bot.id, "name": bot.name, "running": start }).to_string(),
    )
}
