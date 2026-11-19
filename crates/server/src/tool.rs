//! One call of the tool host's, answered with the look it makes (`docs/plan.md`, 「The tool」): the look the call
//! asks for, whether it asks for the caller's hand-off, and the tool result the look ends in (`docs/api.md`,
//! 「Handing the input to the caller」).

use base64::Engine as _;
use bmo_eye_client::{EyeClient, Look, Mode, NewLook, NewOptions, Source, SourceRef};
use serde_json::{Value, json};

use crate::chain::rung_reasons;

/// The longest the create route holds a look's answer open (`docs/api.md`, 「Limits」); the call reads the look
/// again past it.
const HOLD_MS: i64 = 600_000;

/// How long the call waits between two reads of a look the hold did not see end.
const POLL: std::time::Duration = std::time::Duration::from_millis(500);

/// The media types a model's API takes as a picture.
const PICTURE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// The largest picture an answer carries: one whose base64 fits in the 5 MB a model's API takes for one picture.
const PICTURE_CEILING: u64 = 5 * 1024 * 1024 / 4 * 3;

/// A source as a word names it: an address where it is one, else a path on this machine, made absolute.
pub fn source_ref(word: &str) -> SourceRef {
  if word.starts_with("https://")
    || word.starts_with("http://")
    || word.contains("youtube.com")
    || word.contains("youtu.be")
  {
    SourceRef::Url { url: word.into() }
  } else {
    SourceRef::Path {
      path: std::fs::canonicalize(word)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| word.into()),
    }
  }
}

/// The question a mode asks where its caller asks none: the language of the answer is the question's.
pub fn default_question(mode: Mode) -> &'static str {
  match mode {
    Mode::See => "이 그림을 보고 질문에 답하라.",
    Mode::Read => "이 문서의 글자를 위에서 아래로 그대로 옮겨 적어라.",
    Mode::Watch => "이 영상이 무엇을 보여 주고 무엇을 보도하는지 답하라.",
  }
}

/// A look's answer as text: the answer, else its pages one after another, else nothing.
pub fn answer_text(look: &Look) -> String {
  match (&look.answer, &look.pages) {
    (Some(a), _) => a.text.clone(),
    (None, Some(pages)) => pages.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join("\n\n"),
    _ => String::new(),
  }
}

/// Whether a call of `mode` under this session record asks for the hand-off: the record's `reads` label —
/// the media the run's model reads natively, as its issuer judged them from the road the run goes out on —
/// naming the media the mode's material arrives in. A record with no label is a run that reads no media, and
/// no harness reads a video in a tool's answer, so `watch` never asks.
pub fn asks_hand_off(mode: Mode, session: &Value) -> bool {
  let reads = |kind: &str| {
    session["labels"]["reads"]
      .as_array()
      .is_some_and(|media| media.iter().any(|one| one.as_str() == Some(kind)))
  };

  match mode {
    Mode::See => reads("image"),
    Mode::Read => reads("image") || reads("pdf"),
    Mode::Watch => false,
  }
}

/// The tool result of one call, as the tool host hands it: the entry, its arguments and the session record.
pub async fn answer(call: &Value, client: &EyeClient) -> Value {
  let look = match asked(call) {
    Ok(new) => new,
    Err(why) => return refusal(&why),
  };
  match ended(client, &look).await {
    Ok(look) => ended_answer(&look),
    Err(why) => refusal(&format!("the service: {why}")),
  }
}

/// A refusal a model reads: the tool host's answer, not a crash (`docs/plan.md`).
pub fn refusal(why: &str) -> Value {
  json!({
    "content": [{"type": "text", "text": format!("bmo-eye: {why}")}],
    "isError": true,
  })
}

/// The look a call asks for, or why the call asks for none.
fn asked(call: &Value) -> Result<NewLook, String> {
  let kind = call["kind"].as_str().unwrap_or_default();
  if kind != "call" {
    return Err(format!("this tool answers a call alone, not {kind}"));
  }
  let entry = call["entry"].as_str().unwrap_or_default();
  let Some(mode) = entry.strip_prefix("eye/").and_then(Mode::parse) else {
    return Err(format!("{entry}: no such entry of this tool"));
  };
  let args = &call["arguments"];
  let Some(path) = args["path"].as_str() else {
    return Err("the call names no path".into());
  };
  Ok(NewLook {
    mode,
    question: args["question"].as_str().unwrap_or(default_question(mode)).to_string(),
    sources: vec![source_ref(path)],
    options: asked_options(mode, args, &call["session"]),
    labels: Default::default(),
  })
}

/// The options an entry asks: only the names its mode holds, handed on where the call's arguments
/// give them, the rest left unset so the run fills their defaults (`docs/api.md`, 「Modes」).
fn asked_options(mode: Mode, args: &Value, session: &Value) -> NewOptions {
  let hand_off = Some(asks_hand_off(mode, session));
  match mode {
    Mode::Watch => NewOptions {
      fps: args["fps"].as_f64(),
      max_frames: args["maxFrames"].as_i64(),
      speech: args["speech"].as_bool(),
      hand_off,
      ..NewOptions::default()
    },
    Mode::Read => NewOptions {
      pages: args["pages"]
        .as_str()
        .map(|p| p.split(',').filter_map(|w| w.trim().parse::<i64>().ok()).collect()),
      hand_off,
      ..NewOptions::default()
    },
    Mode::See => NewOptions {
      hand_off,
      ..NewOptions::default()
    },
  }
}

/// The look created and read until it has ended: the entry is unbounded, so the call waits however long it runs.
async fn ended(client: &EyeClient, new: &NewLook) -> Result<Look, String> {
  let mut look = client.create(new, HOLD_MS).await.map_err(|e| e.to_string())?;
  while look.state == "queued" || look.state == "running" {
    tokio::time::sleep(POLL).await;
    look = client.look(look.id).await.map_err(|e| e.to_string())?;
  }
  Ok(look)
}

/// The tool result an ended look answers: its answer, the material where it was handed off, or why it failed.
fn ended_answer(look: &Look) -> Value {
  if look.state != "done" {
    let why = look.error.clone().unwrap_or_else(|| look.state.clone());
    return refusal(&format!("the look ended {}: {why}", look.state));
  }
  let content = if look.backend.as_ref().is_some_and(|b| b.name == "caller") {
    material(look)
  } else {
    vec![json!({"type": "text", "text": answer_text(look)})]
  };
  json!({"content": content, "isError": false, "structuredContent": look})
}

/// A handed-off look's material, as the agent opens it: the question, each source by its path or address,
/// every rung's reason, and each picture small enough to send as an image piece.
fn material(look: &Look) -> Vec<Value> {
  let mut text = String::from(
    "No backend of the vision unit answered this look, so it is handed to you: open the sources below \
     yourself and answer the question.\n\n",
  );
  text.push_str(&format!("Question: {}\n", look.question));
  for source in &look.sources {
    text.push_str(&format!("Source: {}\n", source_named(source)));
  }
  let tried: Vec<_> = look.chain.iter().filter(|e| e.state != "handed-off").cloned().collect();
  text.push_str("\nWhy no backend answered:\n");
  for reason in rung_reasons(&tried) {
    text.push_str(&format!("- {reason}\n"));
  }
  let mut content = vec![json!({"type": "text", "text": text})];
  content.extend(look.sources.iter().filter_map(picture));
  content
}

/// A source as the agent reaches it: a path on this machine with its media type, an address, or an upload.
fn source_named(source: &Source) -> String {
  let media = source.media_type.as_deref().unwrap_or("of an unknown type");
  match (&source.path, &source.url, source.upload_id) {
    (Some(path), _, _) => format!("{path} ({media})"),
    (_, Some(url), _) => url.clone(),
    (_, _, Some(id)) => format!("upload {id} of the vision unit ({media})"),
    _ => format!("a {} source", source.kind),
  }
}

/// A picture at a path as an image piece, where a model's API takes its type and its size.
fn picture(source: &Source) -> Option<Value> {
  let media = source.media_type.as_deref()?;
  if source.kind != "path" || !PICTURE_TYPES.contains(&media) {
    return None;
  }
  let path = source.path.as_deref()?;
  let size = std::fs::metadata(path).ok()?.len();
  if size > PICTURE_CEILING {
    return None;
  }
  let bytes = std::fs::read(path).ok()?;
  Some(json!({
    "type": "image",
    "data": base64::engine::general_purpose::STANDARD.encode(bytes),
    "mimeType": media,
  }))
}
