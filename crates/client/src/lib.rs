//! The vision unit's types and client, as `docs/api.md` states them.
//!
//! One shape serves every mode and every backend: a look is a question put to a source set, and its answer
//! records which backend of the chain answered and what every rung before it did.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One mode's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
  See,
  Watch,
  Read,
}

impl Mode {
  pub fn as_str(self) -> &'static str {
    match self {
      Mode::See => "see",
      Mode::Watch => "watch",
      Mode::Read => "read",
    }
  }

  pub fn parse(word: &str) -> Option<Mode> {
    match word {
      "see" => Some(Mode::See),
      "watch" => Some(Mode::Watch),
      "read" => Some(Mode::Read),
      _ => None,
    }
  }
}

/// A look's place in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum State {
  Queued,
  Running,
  Done,
  Failed,
  Stopped,
}

impl State {
  pub fn as_str(self) -> &'static str {
    match self {
      State::Queued => "queued",
      State::Running => "running",
      State::Done => "done",
      State::Failed => "failed",
      State::Stopped => "stopped",
    }
  }
}

/// How a source is named: bytes the caller sent here, an address, or a path on this machine.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum SourceRef {
  Upload {
    /// Also read as `upload_id`, the name bmo-paint sends.
    #[serde(alias = "upload_id")]
    upload_id: i64,
  },
  Url {
    url: String,
  },
  Path {
    path: String,
  },
}

/// A source as the look read it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Source {
  pub kind: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub upload_id: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub path: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub media_type: Option<String>,
  pub bytes: i64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub width: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub height: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub duration_ms: Option<i64>,
}

/// The mode's own options, with the defaults filled in.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Options {
  pub format: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub detail: Option<String>,
  pub fps: f64,
  pub max_frames: i64,
  pub start_s: f64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub end_s: Option<f64>,
  pub speech: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pages: Option<Vec<i64>>,
  pub language: String,
  pub tables: bool,
  /// Whether the caller's own model takes the sources where no backend of the server answers: a caller with
  /// no model of its own leaves it false, and its look fails rather than ends handed off.
  #[serde(default)]
  pub hand_off: bool,
}

impl Default for Options {
  fn default() -> Self {
    Options {
      format: "text".into(),
      detail: Some("high".into()),
      fps: 1.0,
      max_frames: 12,
      start_s: 0.0,
      end_s: None,
      speech: true,
      pages: None,
      language: "ko".into(),
      tables: true,
      hand_off: false,
    }
  }
}

/// A backend of a chain, as the look names it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackendRef {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub node: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
}

/// One rung of the chain a look walked.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainEntry {
  pub backend: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub node: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  pub state: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
  pub ms: i64,
}

/// The answer, whole.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
  pub text: String,
  pub format: String,
}

/// What was said in a video, as the speech model read it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Speech {
  pub text: String,
  pub language: String,
  pub model: String,
  pub ms: i64,
}

/// What was taken from a video.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Frames {
  pub count: i64,
  pub fps: f64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tokens_per_frame: Option<i64>,
}

/// One page of a document as `read` answered it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Page {
  pub page: i64,
  pub text: String,
}

/// What the answer cost.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prompt_tokens: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub output_tokens: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub first_token_ms: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub total_ms: Option<i64>,
}

/// One look.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Look {
  pub id: i64,
  pub mode: String,
  pub state: String,
  pub question: String,
  pub sources: Vec<Source>,
  pub options: Options,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub backend: Option<BackendRef>,
  pub chain: Vec<ChainEntry>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub answer: Option<Answer>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub speech: Option<Speech>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub frames: Option<Frames>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pages: Option<Vec<Page>>,
  pub usage: Usage,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
  pub labels: std::collections::BTreeMap<String, String>,
  pub created_at: i64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub started_at: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ended_at: Option<i64>,
}

/// A page of looks.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LookPage {
  pub items: Vec<Look>,
  pub more: bool,
}

/// A look taken in.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewLook {
  pub mode: Mode,
  pub question: String,
  pub sources: Vec<SourceRef>,
  #[serde(default)]
  pub options: Options,
  #[serde(default)]
  pub labels: std::collections::BTreeMap<String, String>,
}

/// Bytes a caller sent here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Upload {
  pub id: i64,
  pub media_type: String,
  pub bytes: i64,
  pub sha256: String,
  pub created_at: i64,
}

/// One rung of a mode's chain, as `GET /modes` answers it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainRung {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub node: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  pub state: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

/// One mode and the chain it asks.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModeInfo {
  pub mode: String,
  pub chain: Vec<ChainRung>,
}

/// The service's health.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
  pub ready: bool,
  pub reason: Option<String>,
  pub version: String,
}

/// A failure's body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Failure {
  pub code: String,
  pub error: String,
}

/// The unit's client, calling the routes `docs/api.md` states.
#[derive(Debug, Clone)]
pub struct EyeClient {
  base: String,
  http: reqwest::Client,
  token: Option<String>,
}

impl EyeClient {
  pub fn new(base: impl Into<String>) -> EyeClient {
    EyeClient {
      base: base.into(),
      http: reqwest::Client::new(),
      token: None,
    }
  }

  pub fn with_token(mut self, token: impl Into<String>) -> EyeClient {
    self.token = Some(token.into());
    self
  }

  fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    let mut r = self.http.request(method, format!("{}{}", self.base, path));
    if let Some(t) = &self.token {
      r = r.bearer_auth(t);
    }
    r
  }

  pub async fn health(&self) -> Result<Health, String> {
    self.call(self.req(reqwest::Method::GET, "/health")).await
  }

  pub async fn modes(&self) -> Result<Vec<ModeInfo>, String> {
    self.call(self.req(reqwest::Method::GET, "/modes")).await
  }

  pub async fn create(&self, look: &NewLook, wait_ms: i64) -> Result<Look, String> {
    self
      .call(self.req(reqwest::Method::POST, &format!("/looks?waitMs={wait_ms}")).json(look))
      .await
  }

  pub async fn look(&self, id: i64) -> Result<Look, String> {
    self.call(self.req(reqwest::Method::GET, &format!("/looks/{id}"))).await
  }

  pub async fn looks(&self, limit: i64) -> Result<LookPage, String> {
    self.call(self.req(reqwest::Method::GET, &format!("/looks?limit={limit}"))).await
  }

  pub async fn stop(&self, id: i64) -> Result<(), String> {
    let r = self
      .req(reqwest::Method::DELETE, &format!("/looks/{id}"))
      .send()
      .await
      .map_err(|e| e.to_string())?;
    if r.status().is_success() { Ok(()) } else { Err(r.text().await.unwrap_or_default()) }
  }

  pub async fn upload(&self, bytes: Vec<u8>, media_type: &str) -> Result<Upload, String> {
    self
      .call(
        self
          .req(reqwest::Method::POST, "/uploads")
          .header("content-type", media_type)
          .body(bytes),
      )
      .await
  }

  pub async fn upload_bytes(&self, id: i64) -> Result<Vec<u8>, String> {
    let r = self
      .req(reqwest::Method::GET, &format!("/uploads/{id}/content"))
      .send()
      .await
      .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
      return Err(r.text().await.unwrap_or_default());
    }
    r.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
  }

  async fn call<T: serde::de::DeserializeOwned>(&self, r: reqwest::RequestBuilder) -> Result<T, String> {
    let resp = r.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
      let why = serde_json::from_str::<Failure>(&body)
        .map(|f| format!("{}: {}", f.code, f.error))
        .unwrap_or(body);
      return Err(format!("{} {}", status.as_u16(), why));
    }
    serde_json::from_str(&body).map_err(|e| format!("the answer did not read: {e}: {body}"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn upload_look(field: &str) -> String {
    format!(
      r#"{{"mode": "see", "question": "what is shown?", "sources": [{{"kind": "upload", "{field}": 7}}]}}"#
    )
  }

  fn upload_id_of(body: &str) -> i64 {
    let look: NewLook = serde_json::from_str(body).unwrap();
    match look.sources.as_slice() {
      [SourceRef::Upload { upload_id }] => *upload_id,
      other => panic!("expected one upload source, read {other:?}"),
    }
  }

  #[test]
  fn an_upload_source_reads_its_documented_name() {
    assert_eq!(upload_id_of(&upload_look("uploadId")), 7);
  }

  #[test]
  fn an_upload_source_reads_the_name_bmo_paint_sends() {
    assert_eq!(upload_id_of(&upload_look("upload_id")), 7);
  }

  #[test]
  fn an_upload_source_is_written_by_its_documented_name() {
    let written = serde_json::to_value(SourceRef::Upload { upload_id: 7 }).unwrap();
    assert_eq!(written, serde_json::json!({"kind": "upload", "uploadId": 7}));
  }
}
