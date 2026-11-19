//! The service's store: looks, uploads and the events, in SQLite under the data root (`docs/api.md`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bmo_eye_client::*;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Clone)]
pub struct Store {
  conn: Arc<Mutex<Connection>>,
  root: PathBuf,
}

fn now_ms() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}

impl Store {
  pub fn open(root: &Path) -> Result<Store, String> {
    std::fs::create_dir_all(root.join("uploads")).map_err(|e| format!("{}: {e}", root.display()))?;
    let conn = Connection::open(root.join("eye.db")).map_err(|e| e.to_string())?;
    conn
      .execute_batch(
        "pragma journal_mode=wal;
         create table if not exists looks (
           id integer primary key autoincrement, mode text not null, state text not null,
           question text not null, sources text not null, options text not null,
           backend text, chain text not null, answer text, speech text, frames text, pages text,
           usage text not null, error text, labels text not null,
           created_at integer not null, started_at integer, ended_at integer);
         create table if not exists uploads (
           id integer primary key autoincrement, media_type text not null, bytes integer not null,
           sha256 text not null, created_at integer not null);
         create table if not exists events (
           id integer primary key autoincrement, kind text not null, at integer not null, data text not null);",
      )
      .map_err(|e| e.to_string())?;
    Ok(Store {
      conn: Arc::new(Mutex::new(conn)),
      root: root.to_path_buf(),
    })
  }

  pub fn root(&self) -> &Path {
    &self.root
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
    self.conn.lock().unwrap_or_else(|e| e.into_inner())
  }

  pub fn create_look(&self, look: &NewLook, sources: &[Source]) -> Result<Look, String> {
    let created = now_ms();
    let labels = serde_json::to_string(&look.labels).map_err(|e| e.to_string())?;
    let options = serde_json::to_string(&look.options).map_err(|e| e.to_string())?;
    let sources = serde_json::to_string(sources).map_err(|e| e.to_string())?;
    let conn = self.lock();
    conn
      .execute(
        "insert into looks (mode, state, question, sources, options, chain, usage, labels, created_at)
         values (?1, 'queued', ?2, ?3, ?4, '[]', '{}', ?5, ?6)",
        params![look.mode.as_str(), look.question, sources, options, labels, created],
      )
      .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    drop(conn);
    self.look(id)
  }

  pub fn look(&self, id: i64) -> Result<Look, String> {
    let conn = self.lock();
    let row = conn
      .query_row(
        "select id, mode, state, question, sources, options, backend, chain, answer, speech, frames, pages,
                usage, error, labels, created_at, started_at, ended_at from looks where id = ?1",
        params![id],
        read_look,
      )
      .optional()
      .map_err(|e| e.to_string())?;
    row.ok_or_else(|| "not-found".to_string())
  }

  pub fn looks(
    &self,
    limit: i64,
    after: Option<i64>,
    mode: Option<&str>,
    state: Option<&str>,
  ) -> Result<LookPage, String> {
    let conn = self.lock();
    let mut sql = String::from(
      "select id, mode, state, question, sources, options, backend, chain, answer, speech, frames, pages,
              usage, error, labels, created_at, started_at, ended_at from looks where id > ?1",
    );
    if mode.is_some() {
      sql.push_str(" and mode = ?2");
    }
    if state.is_some() {
      sql.push_str(if mode.is_some() { " and state = ?3" } else { " and state = ?2" });
    }
    sql.push_str(" order by id desc limit ?4");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mode_v = mode.unwrap_or("");
    let state_v = state.unwrap_or("");
    let take = limit.max(1) + 1;
    let (p1, p2, p3) = match (mode, state) {
      (Some(_), Some(_)) => (after.unwrap_or(i64::MIN), mode_v, state_v),
      (Some(_), None) => (after.unwrap_or(i64::MIN), mode_v, state_v),
      (None, Some(_)) => (after.unwrap_or(i64::MIN), state_v, state_v),
      (None, None) => (after.unwrap_or(i64::MIN), mode_v, state_v),
    };
    let rows = stmt
      .query_map(params![p1, p2, p3, take], read_look)
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())?;
    let more = rows.len() as i64 > limit;
    Ok(LookPage {
      items: rows.into_iter().take(limit as usize).collect(),
      more,
    })
  }

  pub fn set_running(&self, id: i64) -> Result<(), String> {
    let conn = self.lock();
    conn
      .execute(
        "update looks set state = 'running', started_at = ?2 where id = ?1 and state = 'queued'",
        params![id, now_ms()],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  pub fn finish(&self, id: i64, state: State, outcome: &Outcome) -> Result<(), String> {
    let chain = serde_json::to_string(&outcome.chain).map_err(|e| e.to_string())?;
    let backend = outcome
      .backend
      .as_ref()
      .map(serde_json::to_string)
      .transpose()
      .map_err(|e| e.to_string())?;
    let answer = outcome
      .answer
      .as_ref()
      .map(serde_json::to_string)
      .transpose()
      .map_err(|e| e.to_string())?;
    let speech = outcome
      .speech
      .as_ref()
      .map(serde_json::to_string)
      .transpose()
      .map_err(|e| e.to_string())?;
    let frames = outcome
      .frames
      .as_ref()
      .map(serde_json::to_string)
      .transpose()
      .map_err(|e| e.to_string())?;
    let pages = outcome
      .pages
      .as_ref()
      .map(serde_json::to_string)
      .transpose()
      .map_err(|e| e.to_string())?;
    let usage = serde_json::to_string(&outcome.usage).map_err(|e| e.to_string())?;
    let conn = self.lock();
    conn
      .execute(
        "update looks set state = ?2, backend = ?3, chain = ?4, answer = ?5, speech = ?6, frames = ?7,
                pages = ?8, usage = ?9, error = ?10, ended_at = ?11 where id = ?1",
        params![
          id,
          state.as_str(),
          backend,
          chain,
          answer,
          speech,
          frames,
          pages,
          usage,
          outcome.error,
          now_ms()
        ],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  /// A look a caller stopped. Answers whether this call is what stopped it.
  pub fn stop(&self, id: i64) -> Result<bool, String> {
    let conn = self.lock();
    let n = conn
      .execute(
        "update looks set state = 'stopped', error = 'stopped by its caller', ended_at = ?2
         where id = ?1 and state in ('queued', 'running')",
        params![id, now_ms()],
      )
      .map_err(|e| e.to_string())?;
    Ok(n > 0)
  }

  /// Whether the service can read its store now: its database answers and its upload folder can be listed.
  pub fn readable(&self) -> Result<(), String> {
    let conn = self.lock();
    conn
      .prepare("select 1")
      .map(|_| ())
      .map_err(|e| format!("its database does not answer: {e}"))?;
    let folder = self.upload_dir();
    std::fs::read_dir(&folder)
      .map(|_| ())
      .map_err(|e| format!("its upload folder {} cannot be read: {e}", folder.display()))
  }

  pub fn create_upload(&self, media_type: &str, bytes: &[u8], sha256: &str) -> Result<Upload, String> {
    let created = now_ms();
    let conn = self.lock();
    conn
      .execute(
        "insert into uploads (media_type, bytes, sha256, created_at) values (?1, ?2, ?3, ?4)",
        params![media_type, bytes.len() as i64, sha256, created],
      )
      .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    drop(conn);
    std::fs::write(self.upload_path(id), bytes).map_err(|e| e.to_string())?;
    Ok(Upload {
      id,
      media_type: media_type.into(),
      bytes: bytes.len() as i64,
      sha256: sha256.into(),
      created_at: created,
    })
  }

  pub fn upload(&self, id: i64) -> Result<Upload, String> {
    let conn = self.lock();
    conn
      .query_row(
        "select id, media_type, bytes, sha256, created_at from uploads where id = ?1",
        params![id],
        |r| {
          Ok(Upload {
            id: r.get(0)?,
            media_type: r.get(1)?,
            bytes: r.get(2)?,
            sha256: r.get(3)?,
            created_at: r.get(4)?,
          })
        },
      )
      .optional()
      .map_err(|e| e.to_string())?
      .ok_or_else(|| "not-found".to_string())
  }

  pub fn upload_dir(&self) -> PathBuf {
    self.root.join("uploads")
  }

  pub fn upload_path(&self, id: i64) -> PathBuf {
    self.upload_dir().join(format!("{id}.bin"))
  }

  pub fn upload_bytes(&self, id: i64) -> Result<Vec<u8>, String> {
    let _ = self.upload(id)?;
    std::fs::read(self.upload_path(id)).map_err(|e| e.to_string())
  }

  pub fn delete_upload(&self, id: i64) -> Result<(), String> {
    let conn = self.lock();
    conn
      .execute("delete from uploads where id = ?1", params![id])
      .map_err(|e| e.to_string())?;
    drop(conn);
    let _ = std::fs::remove_file(self.upload_path(id));
    Ok(())
  }

  pub fn event(&self, kind: &str, data: &serde_json::Value) -> Result<i64, String> {
    let conn = self.lock();
    conn
      .execute(
        "insert into events (kind, at, data) values (?1, ?2, ?3)",
        params![kind, now_ms(), data.to_string()],
      )
      .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
  }

  pub fn events_since(&self, after: i64, limit: i64) -> Result<Vec<(i64, String, i64, String)>, String> {
    let conn = self.lock();
    let mut stmt = conn
      .prepare("select id, kind, at, data from events where id > ?1 order by id limit ?2")
      .map_err(|e| e.to_string())?;
    let rows = stmt
      .query_map(params![after, limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
      .map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| e.to_string())?;
    Ok(rows)
  }
}

/// What a backend's answer, or the chain's failure, left on a look.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
  pub backend: Option<BackendRef>,
  pub chain: Vec<ChainEntry>,
  pub answer: Option<Answer>,
  pub speech: Option<Speech>,
  pub frames: Option<Frames>,
  pub pages: Option<Vec<Page>>,
  pub usage: Usage,
  pub error: Option<String>,
}

fn read_look(r: &rusqlite::Row<'_>) -> rusqlite::Result<Look> {
  let json = |i: usize| -> Option<String> { r.get::<_, Option<String>>(i).ok().flatten() };
  Ok(Look {
    id: r.get(0)?,
    mode: r.get(1)?,
    state: r.get(2)?,
    question: r.get(3)?,
    sources: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
    options: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
    backend: json(6).and_then(|s| serde_json::from_str(&s).ok()),
    chain: serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default(),
    answer: json(8).and_then(|s| serde_json::from_str(&s).ok()),
    speech: json(9).and_then(|s| serde_json::from_str(&s).ok()),
    frames: json(10).and_then(|s| serde_json::from_str(&s).ok()),
    pages: json(11).and_then(|s| serde_json::from_str(&s).ok()),
    usage: json(12).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
    error: r.get(13)?,
    labels: serde_json::from_str(&r.get::<_, String>(14)?).unwrap_or_default(),
    created_at: r.get(15)?,
    started_at: r.get(16)?,
    ended_at: r.get(17)?,
  })
}
