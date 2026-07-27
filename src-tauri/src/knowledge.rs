//! Local knowledge index for a Google Drive tool instance. One SQLite file per
//! instance holds the parsed chunks, their embeddings (as BLOBs), and an FTS5
//! keyword index. Retrieval is hybrid: FTS5 keyword ranking fused with
//! brute-force cosine over the embeddings (ample at a personal KB's scale, and no
//! native vector extension to bundle/sign).
//!
//! The index is a derived cache — it can always be rebuilt from Drive (see the
//! Drive `reindex` op). All SQLite work runs on a blocking thread.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sources (
    id INTEGER PRIMARY KEY,
    drive_id TEXT UNIQUE,
    name TEXT NOT NULL,
    mime TEXT,
    added_ts INTEGER,
    embed_model TEXT
);
CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL,
    ord INTEGER NOT NULL,
    text TEXT NOT NULL,
    embedding BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS chunks_source ON chunks(source_id);
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts
    USING fts5(text, content='chunks', content_rowid='id');
CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
";

/// Metadata for a source file being indexed.
pub struct SourceMeta {
    pub drive_id: String,
    pub name: String,
    pub mime: String,
    pub embed_model: String,
}

/// One retrieved chunk with its source citation.
pub struct Hit {
    pub name: String,
    pub drive_id: String,
    pub text: String,
}

fn sqlerr(e: rusqlite::Error) -> String {
    format!("knowledge db: {e}")
}

fn db_path(app: &AppHandle, instance_id: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("no app data dir: {e}"))?
        .join("knowledge");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir failed: {e}"))?;
    Ok(dir.join(format!("{instance_id}.db")))
}

fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(sqlerr)?;
    conn.execute_batch(SCHEMA).map_err(sqlerr)?;
    Ok(conn)
}

fn encode(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn decode(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// Replace any existing source with the same `drive_id`, then insert its chunks.
pub async fn upsert_source(
    app: &AppHandle,
    instance_id: &str,
    meta: SourceMeta,
    chunks: Vec<(String, Vec<f32>)>,
) -> Result<(), String> {
    let path = db_path(app, instance_id)?;
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut conn = open(&path)?;
        let tx = conn.transaction().map_err(sqlerr)?;

        if let Some(old_id) = source_id(&tx, &meta.drive_id)? {
            tx.execute("DELETE FROM chunks WHERE source_id = ?1", [old_id])
                .map_err(sqlerr)?;
            tx.execute("DELETE FROM sources WHERE id = ?1", [old_id])
                .map_err(sqlerr)?;
        }

        let now = now_ms();
        tx.execute(
            "INSERT INTO sources (drive_id, name, mime, added_ts, embed_model) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![meta.drive_id, meta.name, meta.mime, now, meta.embed_model],
        )
        .map_err(sqlerr)?;
        let source_id = tx.last_insert_rowid();

        for (ord, (text, embedding)) in chunks.iter().enumerate() {
            tx.execute(
                "INSERT INTO chunks (source_id, ord, text, embedding) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![source_id, ord as i64, text, encode(embedding)],
            )
            .map_err(sqlerr)?;
        }

        tx.commit().map_err(sqlerr)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("db task failed: {e}"))?
}

pub async fn has_source(
    app: &AppHandle,
    instance_id: &str,
    drive_id: &str,
) -> Result<bool, String> {
    let path = db_path(app, instance_id)?;
    let drive_id = drive_id.to_string();
    tokio::task::spawn_blocking(move || -> Result<bool, String> {
        let conn = open(&path)?;
        Ok(source_id(&conn, &drive_id)?.is_some())
    })
    .await
    .map_err(|e| format!("db task failed: {e}"))?
}

pub async fn list_sources(
    app: &AppHandle,
    instance_id: &str,
) -> Result<Vec<(String, String, i64)>, String> {
    let path = db_path(app, instance_id)?;
    tokio::task::spawn_blocking(move || -> Result<Vec<(String, String, i64)>, String> {
        let conn = open(&path)?;
        let mut stmt = conn
            .prepare(
                "SELECT s.name, s.drive_id, COUNT(c.id) \
                 FROM sources s LEFT JOIN chunks c ON c.source_id = s.id \
                 GROUP BY s.id ORDER BY s.name",
            )
            .map_err(sqlerr)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?)))
            .map_err(sqlerr)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sqlerr)
    })
    .await
    .map_err(|e| format!("db task failed: {e}"))?
}

/// Hybrid retrieval: fuse FTS5 keyword ranking with cosine over embeddings via
/// reciprocal-rank fusion, and return the top `k` chunks with citations.
pub async fn search(
    app: &AppHandle,
    instance_id: &str,
    query_embedding: Vec<f32>,
    query_text: String,
    k: usize,
) -> Result<Vec<Hit>, String> {
    let path = db_path(app, instance_id)?;
    tokio::task::spawn_blocking(move || -> Result<Vec<Hit>, String> {
        const CANDIDATES: usize = 30;
        const RRF_K: f32 = 60.0;
        let conn = open(&path)?;

        // Vector candidates: brute-force cosine over all chunk embeddings.
        let mut cosine: Vec<(i64, f32)> = {
            let mut stmt = conn
                .prepare("SELECT id, embedding FROM chunks")
                .map_err(sqlerr)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))
                .map_err(sqlerr)?;
            let qnorm = norm(&query_embedding);
            let mut scored = Vec::new();
            for row in rows {
                let (id, blob) = row.map_err(sqlerr)?;
                let emb = decode(&blob);
                if emb.len() == query_embedding.len() && qnorm > 0.0 {
                    scored.push((id, cosine_sim(&query_embedding, &emb, qnorm)));
                }
            }
            scored
        };
        cosine.sort_by(|a, b| b.1.total_cmp(&a.1));
        cosine.truncate(CANDIDATES);

        // Keyword candidates via FTS5 (ordered best-first by bm25).
        let keyword: Vec<i64> = match fts_query(&query_text) {
            Some(match_expr) => {
                let mut stmt = conn
                    .prepare(
                        "SELECT rowid FROM chunks_fts WHERE chunks_fts MATCH ?1 \
                         ORDER BY bm25(chunks_fts) LIMIT ?2",
                    )
                    .map_err(sqlerr)?;
                let rows = stmt
                    .query_map(rusqlite::params![match_expr, CANDIDATES as i64], |r| {
                        r.get(0)
                    })
                    .map_err(sqlerr)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(sqlerr)?
            }
            None => Vec::new(),
        };

        // Reciprocal-rank fusion.
        let mut fused: std::collections::HashMap<i64, f32> = std::collections::HashMap::new();
        for (rank, (id, _)) in cosine.iter().enumerate() {
            *fused.entry(*id).or_default() += 1.0 / (RRF_K + rank as f32);
        }
        for (rank, id) in keyword.iter().enumerate() {
            *fused.entry(*id).or_default() += 1.0 / (RRF_K + rank as f32);
        }

        let mut ranked: Vec<(i64, f32)> = fused.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        ranked.truncate(k.max(1));

        // Fetch metadata + text for the winners, preserving fused order.
        let mut hits = Vec::new();
        let mut stmt = conn
            .prepare(
                "SELECT s.name, s.drive_id, c.text \
                 FROM chunks c JOIN sources s ON s.id = c.source_id WHERE c.id = ?1",
            )
            .map_err(sqlerr)?;
        for (id, _score) in ranked {
            let row = stmt.query_row([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            });
            if let Ok((name, drive_id, text)) = row {
                hits.push(Hit {
                    name,
                    drive_id,
                    text,
                });
            }
        }
        Ok(hits)
    })
    .await
    .map_err(|e| format!("db task failed: {e}"))?
}

fn source_id(conn: &Connection, drive_id: &str) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT id FROM sources WHERE drive_id = ?1",
        [drive_id],
        |r| r.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(sqlerr(other)),
    })
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine_sim(q: &[f32], v: &[f32], qnorm: f32) -> f32 {
    let dot: f32 = q.iter().zip(v).map(|(a, b)| a * b).sum();
    let vnorm = norm(v);
    if vnorm == 0.0 {
        0.0
    } else {
        dot / (qnorm * vnorm)
    }
}

/// Build a safe FTS5 MATCH expression: quote each term, OR them. `None` if the
/// query has no usable terms.
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() > 1)
        .map(|t| format!("\"{}\"", t.to_lowercase()))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" OR "))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let v = vec![0.5f32, -1.0, 2.0];
        assert_eq!(decode(&encode(&v)), v);
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![3.0f32, 4.0];
        assert!((cosine_sim(&v, &v, norm(&v)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let q = vec![1.0f32, 0.0];
        let v = vec![0.0f32, 1.0];
        assert!(cosine_sim(&q, &v, norm(&q)).abs() < 1e-6);
    }

    #[test]
    fn fts_query_filters_short_terms() {
        assert_eq!(fts_query("the cat"), Some("\"the\" OR \"cat\"".to_string()));
        assert!(fts_query("a !").is_none());
    }
}
