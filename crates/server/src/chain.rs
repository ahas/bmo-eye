//! The chain a look walks: the local engine nodes the settings name, then OpenRouter, then the caller's own
//! model (`docs/modes.md`). Each rung answers whether it takes the look, and what it answers.

use bmo_eye_client::*;

use crate::config::{Settings, expand};
use crate::media::{Material, message};
use crate::store::Outcome;

/// One rung of a mode's chain.
#[derive(Debug, Clone)]
pub enum Rung {
  Local {
    node: String,
    address: String,
    model: String,
    key_file: String,
  },
  OpenRouter {
    address: String,
    model: String,
    key_file: String,
  },
  Gemini {
    model: String,
    key_file: Option<String>,
  },
  Caller,
}

/// One rung's answer.
#[derive(Debug, Clone)]
pub struct Reply {
  pub text: String,
  pub prompt_tokens: Option<i64>,
  pub output_tokens: Option<i64>,
  pub first_token_ms: Option<i64>,
  pub total_ms: i64,
}

impl Rung {
  pub fn name(&self) -> &'static str {
    match self {
      Rung::Local { .. } => "local",
      Rung::OpenRouter { .. } => "openrouter",
      Rung::Gemini { .. } => "gemini",
      Rung::Caller => "caller",
    }
  }

  pub fn node(&self) -> Option<String> {
    match self {
      Rung::Local { node, .. } => Some(node.clone()),
      _ => None,
    }
  }

  pub fn model(&self) -> Option<String> {
    match self {
      Rung::Local { model, .. } | Rung::OpenRouter { model, .. } | Rung::Gemini { model, .. } => {
        Some(model.clone())
      }
      Rung::Caller => None,
    }
  }
}

/// A look's chain.
pub struct Chain {
  pub rungs: Vec<Rung>,
}

impl Chain {
  pub fn for_mode(settings: &Settings, mode: Mode) -> Chain {
    let mut rungs: Vec<Rung> = settings
      .local
      .nodes
      .iter()
      .map(|n| Rung::Local {
        node: n.name.clone(),
        address: n.address.clone(),
        model: n.model.clone(),
        key_file: n.key_file.clone(),
      })
      .collect();
    if settings.hosted.openrouter.enabled {
      rungs.push(Rung::OpenRouter {
        address: settings.hosted.openrouter.address.clone(),
        model: settings.hosted.openrouter.model.clone(),
        key_file: settings.hosted.openrouter.key_file.clone(),
      });
    }
    if mode == Mode::Watch && settings.hosted.gemini.enabled {
      rungs.push(Rung::Gemini {
        model: settings.hosted.gemini.model.clone(),
        key_file: settings.hosted.gemini.key_file.clone(),
      });
    }
    rungs.push(Rung::Caller);
    Chain { rungs }
  }

  /// Whether this rung takes the look at all, and why not where it does not.
  pub fn takes(
    _settings: &Settings,
    rung: &Rung,
    mode: Mode,
    options: &Options,
    material: &Material,
    source_kinds: &[String],
  ) -> Result<(), String> {
    match rung {
      Rung::Local { .. } => {
        if material.images.is_empty() && mode != Mode::Read {
          return Err("no picture was taken from the source".into());
        }
        Ok(())
      }
      Rung::OpenRouter { key_file, .. } => {
        let path = expand(key_file);
        match std::fs::read_to_string(&path) {
          Ok(k) if !k.trim().is_empty() => Ok(()),
          _ => Err(format!("no OpenRouter key stands at {}", path.display())),
        }
      }
      Rung::Gemini { key_file, .. } => {
        if mode != Mode::Watch {
          return Err("Gemini takes video only in this chain".into());
        }
        if source_kinds.iter().any(|k| k == "path") {
          return Err("a hosted API cannot reach a path on this machine".into());
        }
        match key_file {
          Some(f) if expand(f).exists() => Ok(()),
          _ => Err("no Gemini key is configured".into()),
        }
      }
      Rung::Caller => {
        if !options.hand_off {
          return Err("the look did not ask for the caller's hand-off".into());
        }
        if mode == Mode::Watch
          && let Some(seconds) = material.duration_s
          && seconds > 30.0
        {
          return Err(format!("the caller's transport carries no {seconds:.0} s video"));
        }
        Ok(())
      }
    }
  }

  /// Walk the chain and answer from the first rung that does.
  pub async fn walk(
    &self,
    settings: &Settings,
    mode: Mode,
    options: &Options,
    material: &Material,
    source_kinds: &[String],
  ) -> Outcome {
    let mut chain: Vec<ChainEntry> = Vec::new();
    for rung in &self.rungs {
      let started = std::time::Instant::now();
      let mut entry = ChainEntry {
        backend: rung.name().into(),
        node: rung.node(),
        model: rung.model(),
        state: "failed".into(),
        reason: None,
        ms: 0,
      };
      if let Err(why) = Chain::takes(settings, rung, mode, options, material, source_kinds) {
        entry.state = "skipped".into();
        entry.reason = Some(why);
        entry.ms = started.elapsed().as_millis() as i64;
        chain.push(entry);
        continue;
      }
      match rung {
        Rung::Caller => {
          entry.state = "handed-off".into();
          entry.reason = Some("the caller's own model takes this input".into());
          entry.ms = started.elapsed().as_millis() as i64;
          chain.push(entry);
          return Outcome {
            backend: Some(BackendRef {
              name: "caller".into(),
              node: None,
              model: None,
            }),
            chain,
            ..Outcome::default()
          };
        }
        _ => match answer(settings, rung, mode, material)
          .await
          .and_then(|reply| shaped(reply, &options.format))
        {
          Ok(reply) => {
            entry.state = "answered".into();
            entry.ms = started.elapsed().as_millis() as i64;
            chain.push(entry);
            return Outcome {
              backend: Some(BackendRef {
                name: rung.name().into(),
                node: rung.node(),
                model: rung.model(),
              }),
              chain,
              answer: Some(Answer {
                text: reply.text,
                format: answer_format(mode, &options.format),
              }),
              speech: material.speech.clone(),
              frames: material.frames.clone(),
              pages: material.pages.clone(),
              usage: Usage {
                prompt_tokens: reply.prompt_tokens,
                output_tokens: reply.output_tokens,
                first_token_ms: reply.first_token_ms,
                total_ms: Some(reply.total_ms),
              },
              error: None,
            };
          }
          Err(why) => {
            entry.state = "failed".into();
            entry.reason = Some(why);
            entry.ms = started.elapsed().as_millis() as i64;
            chain.push(entry);
          }
        },
      }
    }
    Outcome {
      error: Some(ran_out(&chain)),
      chain,
      ..Outcome::default()
    }
  }
}

/// The error of a look the chain ran out on: every rung's reason, so a caller reading only the error reads
/// why.
fn ran_out(chain: &[ChainEntry]) -> String {
  format!("every backend of the chain failed: {}", rung_reasons(chain).join("; "))
}

/// Each rung of a chain report as a sentence: the backend and its node, what it did, and why.
pub fn rung_reasons(chain: &[ChainEntry]) -> Vec<String> {
  chain
    .iter()
    .map(|entry| {
      let rung = match &entry.node {
        Some(node) => format!("{} {node}", entry.backend),
        None => entry.backend.clone(),
      };
      format!("{rung} {}: {}", entry.state, entry.reason.as_deref().unwrap_or("no reason given"))
    })
    .collect()
}

/// The format an answer is reported in: `json` where the look asked for it, else the mode's own.
fn answer_format(mode: Mode, asked: &str) -> String {
  if asked == JSON {
    return JSON.into();
  }
  match mode {
    Mode::Read => "markdown".into(),
    _ => "text".into(),
  }
}

const JSON: &str = "json";

/// A rung's reply in the format the look asked for, or why it is none: under `json`, the JSON document
/// alone.
fn shaped(reply: Reply, format: &str) -> Result<Reply, String> {
  if format != JSON {
    return Ok(reply);
  }
  let text = json_document(&reply.text)?;
  Ok(Reply { text, ..reply })
}

/// The JSON document an answer holds: the answer itself where it reads whole, else the first fenced code
/// block of it that reads.
fn json_document(text: &str) -> Result<String, String> {
  use pulldown_cmark::{Event, Parser, Tag, TagEnd};
  let reads = |candidate: &str| serde_json::from_str::<serde_json::Value>(candidate).is_ok();
  let whole = text.trim();
  if reads(whole) {
    return Ok(whole.to_string());
  }
  let mut block: Option<String> = None;
  for event in Parser::new(text) {
    match event {
      Event::Start(Tag::CodeBlock(_)) => block = Some(String::new()),
      Event::Text(part) => {
        if let Some(open) = block.as_mut() {
          open.push_str(&part);
        }
      }
      Event::End(TagEnd::CodeBlock) => {
        if let Some(closed) = block.take()
          && reads(closed.trim())
        {
          return Ok(closed.trim().to_string());
        }
      }
      _ => {}
    }
  }
  Err("the answer holds no JSON document".into())
}

fn max_tokens(mode: Mode) -> i64 {
  match mode {
    Mode::See => 600,
    Mode::Read => 1600,
    Mode::Watch => 600,
  }
}

async fn answer(
  _settings: &Settings,
  rung: &Rung,
  mode: Mode,
  material: &Material,
) -> Result<Reply, String> {
  // Each backend turns its thinking off through its own field; a model left thinking spends the token cap on
  // the thought and is cut before it answers.
  let (url, key_file, model, thinking_off) = match rung {
    Rung::Local {
      address,
      model,
      key_file,
      ..
    } => (
      format!("http://{address}/v1/chat/completions"),
      key_file,
      model,
      ("chat_template_kwargs", serde_json::json!({"enable_thinking": false})),
    ),
    Rung::OpenRouter {
      address,
      model,
      key_file,
    } => (
      format!("{address}/chat/completions"),
      key_file,
      model,
      ("reasoning", serde_json::json!({"enabled": false})),
    ),
    Rung::Gemini { .. } => return Err("the Gemini rung is not built".into()),
    Rung::Caller => return Err("the caller rung answers nothing".into()),
  };
  let key = std::fs::read_to_string(expand(key_file))
    .map_err(|e| format!("{}: {e}", expand(key_file).display()))?;
  let key = key.trim().to_string();
  let cap = max_tokens(mode);
  let mut body = serde_json::json!({
    "model": model,
    "max_tokens": cap,
    "temperature": 0,
    "timings_per_token": true,
    "messages": message(material),
  });
  body[thinking_off.0] = thinking_off.1;
  let started = std::time::Instant::now();
  // The node is waited for however long it reads: the call ends with its answer or its connection.
  let resp = reqwest::Client::new()
    .post(&url)
    .bearer_auth(key)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("{url}: {e}"))?;
  let status = resp.status();
  let text = resp.text().await.map_err(|e| e.to_string())?;
  if !status.is_success() {
    return Err(format!("{status}: {}", text.chars().take(300).collect::<String>()));
  }
  let value: serde_json::Value =
    serde_json::from_str(&text).map_err(|e| format!("the answer did not read: {e}"))?;
  let content = whole_answer(&value, cap)?;
  let timings = value.get("timings");
  let prompt_tokens = timings
    .and_then(|t| t.get("prompt_n"))
    .and_then(|v| v.as_i64())
    .or_else(|| value.get("usage").and_then(|u| u.get("prompt_tokens")).and_then(|v| v.as_i64()));
  let output_tokens = timings
    .and_then(|t| t.get("predicted_n"))
    .and_then(|v| v.as_i64())
    .or_else(|| value.get("usage").and_then(|u| u.get("completion_tokens")).and_then(|v| v.as_i64()));
  let first = timings.and_then(|t| t.get("prompt_ms")).and_then(|v| v.as_f64()).map(|v| v as i64);
  Ok(Reply {
    text: content,
    prompt_tokens,
    output_tokens,
    first_token_ms: first,
    total_ms: started.elapsed().as_millis() as i64,
  })
}

/// The text of an answer that ended on its own, or why it is no answer: one cut at the token cap, ended for
/// any other reason, or holding no words.
fn whole_answer(value: &serde_json::Value, cap: i64) -> Result<String, String> {
  let choice = value.get("choices").and_then(|c| c.get(0));
  match choice.and_then(|c| c.get("finish_reason")).and_then(|f| f.as_str()) {
    None | Some("stop") => {}
    Some("length") => return Err(format!("the answer was cut at the {cap}-token cap")),
    Some(other) => return Err(format!("the engine ended its answer on {other}")),
  }
  let content = choice
    .and_then(|c| c.get("message"))
    .and_then(|m| m.get("content"))
    .and_then(|c| c.as_str())
    .unwrap_or_default();
  if content.trim().is_empty() {
    return Err("the engine answered no words".into());
  }
  Ok(content.to_string())
}

/// The chains as `GET /modes` shows them: what each rung is and whether it stands now.
pub async fn modes(settings: &Settings) -> Vec<ModeInfo> {
  let mut out = Vec::new();
  for mode in [Mode::See, Mode::Read, Mode::Watch] {
    let chain = Chain::for_mode(settings, mode);
    let mut rungs = Vec::new();
    for rung in &chain.rungs {
      let (state, reason) = match rung {
        Rung::Local { address, .. } => {
          match reqwest::Client::new().get(format!("http://{address}/health")).send().await {
            Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
              Ok(v) if v.get("images").and_then(|i| i.as_bool()) == Some(true) => {
                ("ready".to_string(), None)
              }
              Ok(_) => ("down".to_string(), Some("the node reads no pictures".to_string())),
              Err(e) => ("down".to_string(), Some(e.to_string())),
            },
            Ok(r) => ("down".to_string(), Some(format!("the node answered {}", r.status()))),
            Err(e) => ("down".to_string(), Some(e.to_string())),
          }
        }
        Rung::OpenRouter {
          address, key_file, ..
        } => {
          let path = expand(key_file);
          match std::fs::read_to_string(&path).map(|k| k.trim().to_string()) {
            Ok(k) if k.is_empty() => {
              ("unconfigured".to_string(), Some(format!("no key stands at {}", path.display())))
            }
            Ok(k) => {
              // The key's own record says whether it may spend: a key at a zero limit answers every call 403.
              match key_limit(address, &k).await {
                Ok(Some(limit)) if limit <= 0.0 => {
                  ("unconfigured".to_string(), Some("the key's spending limit is zero".to_string()))
                }
                _ => ("ready".to_string(), None),
              }
            }
            Err(_) => ("unconfigured".to_string(), Some(format!("no key stands at {}", path.display()))),
          }
        }
        Rung::Gemini { key_file, .. } => match key_file {
          Some(f) if expand(f).exists() => ("ready".to_string(), None),
          _ => ("unconfigured".to_string(), Some("no Gemini key is configured".to_string())),
        },
        Rung::Caller => ("ready".to_string(), None),
      };
      rungs.push(ChainRung {
        name: rung.name().into(),
        node: rung.node(),
        model: rung.model(),
        state,
        reason,
      });
    }
    out.push(ModeInfo {
      mode: mode.as_str().into(),
      chain: rungs,
    });
  }
  out
}

/// What OpenRouter says a key may spend, where it says anything.
async fn key_limit(address: &str, key: &str) -> Result<Option<f64>, String> {
  let resp = reqwest::Client::new()
    .get(format!("{address}/key"))
    .bearer_auth(key)
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.status().is_success() {
    return Err(format!("{}", resp.status()));
  }
  let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
  Ok(value.get("data").and_then(|d| d.get("limit")).and_then(|l| l.as_f64()))
}
