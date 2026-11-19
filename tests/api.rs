//! The routes `docs/api.md` states, answered against stand-in backends: a look walks its chain, records what
//! every rung did, and answers from the first that does.

use std::sync::{Arc, Mutex};

use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use bmo_eye_client::*;
use eye_server::config::Settings;
use eye_server::routes::{App, app};
use eye_server::store::Store;
use serde_json::Value;

/// What a stand-in answers a chat with, made from the request it was sent.
type Reply = Arc<dyn Fn(&Value) -> Value + Send + Sync>;

/// The requests a stand-in was sent, in order.
type Received = Arc<Mutex<Vec<Value>>>;

/// A stand-in for one chat backend, a Lab-arena instance or OpenRouter: it says it reads pictures, answers every
/// chat with what `reply` makes of it, and keeps every request.
async fn stand_in(reply: Reply) -> (String, Received) {
  let received: Received = Arc::default();
  let kept = received.clone();
  let chat = post(move |Json(body): Json<Value>| {
    let answer = reply(&body);
    kept.lock().unwrap().push(body);
    async move { Json(answer) }
  });
  let router = Router::new()
    .route("/health", get(|| async { Json(serde_json::json!({"status": "ok", "images": true})) }))
    .route("/v1/chat/completions", chat.clone())
    .route("/chat/completions", chat);
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
  (format!("{addr}"), received)
}

/// A reply that counts the pictures it was sent, as a Lab-arena timing block reports them.
fn counting() -> Reply {
  Arc::new(|body| {
    let images = body["messages"][0]["content"]
      .as_array()
      .map(|c| c.iter().filter(|p| p["type"] == "image_url").count())
      .unwrap_or(0);
    serde_json::json!({
      "choices": [{"message": {"content": format!("answer: {images} pictures")}}],
      "timings": {"prompt_n": 7 + 300 * images, "predicted_n": 5, "prompt_ms": 120.0, "predicted_ms": 50.0}
    })
  })
}

/// A reply of one text, ended for the reason given.
fn chat(content: &str, finish_reason: &str) -> Reply {
  let answer = serde_json::json!({
    "choices": [{"message": {"content": content}, "finish_reason": finish_reason}],
  });
  Arc::new(move |_| answer.clone())
}

/// A service whose chain is the stand-in Lab-arena nodes named, then a stand-in OpenRouter where one is given.
struct Service {
  base: String,
  token: String,
  dir: std::path::PathBuf,
  openrouter: Option<Received>,
}

async fn service_with(nodes: Vec<(&str, Reply)>, openrouter: Option<Reply>) -> Service {
  static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
  let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
  let dir = std::env::temp_dir().join(format!("bmo-eye-test-{}-{n}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  let mut settings = Settings {
    listen: "127.0.0.1:0".into(),
    root: dir.to_string_lossy().to_string(),
    ..Settings::default()
  };
  settings.lab-arena.nodes = Vec::new();
  for (name, reply) in nodes {
    let (address, _) = stand_in(reply).await;
    settings.lab-arena.nodes.push(eye_server::config::Node {
      name: name.into(),
      address,
      model: "qwen3.8-flash-next-iq3_s".into(),
      key_file: "/dev/null".into(),
    });
  }
  settings.hosted.openrouter.enabled = openrouter.is_some();
  let openrouter = match openrouter {
    Some(reply) => {
      let (address, received) = stand_in(reply).await;
      let key = dir.join("openrouter.key");
      std::fs::write(&key, "test-key").unwrap();
      settings.hosted.openrouter.address = format!("http://{address}");
      settings.hosted.openrouter.key_file = key.to_string_lossy().to_string();
      Some(received)
    }
    None => None,
  };
  settings.hosted.gemini.enabled = false;
  settings.speech.enabled = false;
  let store = Store::open(&dir).unwrap();
  let token = "test-token".to_string();
  let router = app(App {
    settings: Arc::new(settings),
    store,
    token: Arc::new(token.clone()),
  });
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
  Service {
    base: format!("http://{addr}"),
    token,
    dir,
    openrouter,
  }
}

async fn service() -> (String, String, std::path::PathBuf) {
  let s = service_with(vec![("stand-in", counting())], None).await;
  (s.base, s.token, s.dir)
}

/// One `see` look at one picture, with the options given, held until it ends.
async fn see(service: &Service, options: Options) -> Look {
  let client = EyeClient::new(&service.base).with_token(&service.token);
  let upload = client.upload(png(), "image/png").await.unwrap();
  client
    .create(
      &NewLook {
        mode: Mode::See,
        question: "who is shown?".into(),
        sources: vec![SourceRef::Upload {
          upload_id: upload.id,
        }],
        options,
        labels: Default::default(),
      },
      30_000,
    )
    .await
    .unwrap()
}

fn json_format() -> Options {
  Options {
    format: "json".into(),
    ..Options::default()
  }
}

fn png() -> Vec<u8> {
  // A one-pixel PNG is enough: the stand-in counts pictures, it does not read them.
  const ONE: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R', 0, 0, 0, 1, 0,
    0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0x0a, b'I', b'D', b'A', b'T', 0x78, 0x9c,
    0x63, 0, 0, 0, 2, 0, 1, 0xfd, 0x8a, 0xb4, 0x68, 0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60,
    0x82,
  ];
  ONE.to_vec()
}

#[tokio::test]
async fn a_look_walks_its_chain_and_records_every_rung() {
  let (base, token, dir) = service().await;
  let client = EyeClient::new(&base).with_token(&token);
  let upload = client.upload(png(), "image/png").await.unwrap();
  let look = client
    .create(
      &NewLook {
        mode: Mode::See,
        question: "what is shown?".into(),
        sources: vec![SourceRef::Upload {
          upload_id: upload.id,
        }],
        options: Options::default(),
        labels: Default::default(),
      },
      30_000,
    )
    .await
    .unwrap();
  assert_eq!(look.state, "done", "{:?}", look.error);
  assert_eq!(look.backend.as_ref().unwrap().name, "lab-arena");
  assert_eq!(look.backend.as_ref().unwrap().node.as_deref(), Some("stand-in"));
  assert_eq!(look.chain[0].state, "answered");
  assert!(look.answer.unwrap().text.contains("1 pictures"));
  assert_eq!(look.usage.prompt_tokens, Some(307));
  let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn an_upload_is_taken_up_to_its_ceiling_and_refused_over_it() {
  let (base, token, dir) = service().await;
  let client = EyeClient::new(&base).with_token(&token);
  let mut picture = png();
  picture.resize(3 * 1024 * 1024, 0);
  let upload = client.upload(picture, "image/png").await.unwrap();
  assert_eq!(upload.bytes, 3 * 1024 * 1024);
  let mut picture = png();
  picture.resize(21 * 1024 * 1024, 0);
  let err = client.upload(picture, "image/png").await.unwrap_err();
  assert!(err.starts_with("413 too-large"), "{err}");
  let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_look_without_a_token_is_refused() {
  let (base, _, dir) = service().await;
  let client = EyeClient::new(&base);
  let err = client.health().await.unwrap_err();
  assert!(err.contains("401"), "{err}");
  let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_mode_refuses_what_it_does_not_read() {
  let (base, token, dir) = service().await;
  let client = EyeClient::new(&base).with_token(&token);
  let upload = client.upload(png(), "image/png").await.unwrap();
  let path = dir.join("not-a-video.txt");
  std::fs::write(&path, "text").unwrap();
  let err = client
    .create(
      &NewLook {
        mode: Mode::Watch,
        question: "what is this?".into(),
        sources: vec![SourceRef::Path {
          path: path.to_string_lossy().to_string(),
        }],
        options: Options::default(),
        labels: Default::default(),
      },
      0,
    )
    .await
    .unwrap_err();
  assert!(err.contains("unsupported-source"), "{err}");
  assert!(client.look(upload.id).await.is_err());
  let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn the_modes_answer_the_chain_that_stands() {
  let (base, token, dir) = service().await;
  let client = EyeClient::new(&base).with_token(&token);
  let modes = client.modes().await.unwrap();
  assert_eq!(modes.len(), 3);
  let see = modes.iter().find(|m| m.mode == "see").unwrap();
  assert_eq!(see.chain[0].name, "lab-arena");
  assert_eq!(see.chain[0].state, "ready");
  assert_eq!(see.chain.last().unwrap().name, "caller");
  let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn an_answer_cut_at_its_token_cap_fails_its_rung_and_the_next_answers() {
  let service = service_with(
    vec![
      ("first", chat("```json\n", "length")),
      ("second", chat("```json\n{\"people\": []}\n```", "stop")),
    ],
    None,
  )
  .await;
  let look = see(&service, json_format()).await;
  assert_eq!(look.state, "done", "{:?}", look.error);
  assert_eq!(look.chain[0].state, "failed");
  assert_eq!(look.chain[0].reason.as_deref(), Some("the answer was cut at the 600-token cap"));
  assert_eq!(look.backend.as_ref().unwrap().node.as_deref(), Some("second"));
  let answer = look.answer.unwrap();
  assert_eq!(answer.text, "{\"people\": []}");
  assert_eq!(answer.format, "json");
  let _ = std::fs::remove_dir_all(service.dir);
}

#[tokio::test]
async fn a_json_look_whose_answer_holds_no_document_asks_the_next_backend() {
  let service = service_with(
    vec![
      ("first", chat("The picture shows one adult.", "stop")),
      ("second", chat("[{\"index\": 0}]", "stop")),
    ],
    None,
  )
  .await;
  let look = see(&service, json_format()).await;
  assert_eq!(look.state, "done", "{:?}", look.error);
  assert_eq!(look.chain[0].reason.as_deref(), Some("the answer holds no JSON document"));
  assert_eq!(look.answer.unwrap().text, "[{\"index\": 0}]");
  let _ = std::fs::remove_dir_all(service.dir);
}

#[tokio::test]
async fn the_openrouter_rung_asks_with_its_reasoning_off() {
  let service = service_with(Vec::new(), Some(chat("One adult is shown.", "stop"))).await;
  let look = see(&service, Options::default()).await;
  assert_eq!(look.state, "done", "{:?}", look.error);
  assert_eq!(look.backend.as_ref().unwrap().name, "openrouter");
  let received = service.openrouter.as_ref().unwrap().lock().unwrap().clone();
  assert_eq!(received[0]["reasoning"], serde_json::json!({"enabled": false}));
  let _ = std::fs::remove_dir_all(service.dir);
}

#[tokio::test]
async fn a_look_that_did_not_ask_for_the_hand_off_fails_with_every_rung_s_reason() {
  let service = service_with(vec![("first", chat("", "stop"))], None).await;
  let look = see(&service, Options::default()).await;
  assert_eq!(look.state, "failed");
  assert_eq!(
    look.error.as_deref(),
    Some(
      "every backend of the chain failed: lab-arena first failed: the engine answered no words; caller skipped: the \
       look did not ask for the caller's hand-off"
    )
  );
  assert!(look.answer.is_none());
  let _ = std::fs::remove_dir_all(service.dir);
}

#[tokio::test]
async fn a_look_that_asks_for_the_hand_off_ends_on_the_caller() {
  let service = service_with(vec![("first", chat("", "stop"))], None).await;
  let options = Options {
    hand_off: true,
    ..Options::default()
  };
  let look = see(&service, options).await;
  assert_eq!(look.state, "done", "{:?}", look.error);
  assert_eq!(look.backend.as_ref().unwrap().name, "caller");
  assert_eq!(look.chain.last().unwrap().state, "handed-off");
  let _ = std::fs::remove_dir_all(service.dir);
}

/// One call of the tool host's to `eye/see` at a picture on disk, under a session record whose labels are `labels`.
async fn tool_see(service: &Service, labels: Value) -> (Value, String) {
  let picture = service.dir.join("picture.png");
  std::fs::write(&picture, png()).unwrap();
  let path = std::fs::canonicalize(&picture).unwrap().to_string_lossy().to_string();
  let call = serde_json::json!({
    "protocol": 1,
    "kind": "call",
    "entry": "eye/see",
    "arguments": {"path": path, "question": "who is shown?"},
    "session": {"holderName": "tester", "labels": labels},
  });
  let client = EyeClient::new(&service.base).with_token(&service.token);
  (eye_server::tool::answer(&call, &client).await, path)
}

#[tokio::test]
async fn an_agent_s_call_no_backend_answers_is_answered_with_the_picture_and_the_question() {
  let service = service_with(vec![("first", chat("", "stop"))], None).await;
  let (answer, path) =
    tool_see(&service, serde_json::json!({"line": "chat", "reads": ["image", "pdf"]})).await;
  assert_eq!(answer["isError"], false, "{answer}");
  assert_eq!(answer["structuredContent"]["backend"]["name"], "caller");
  let text = answer["content"][0]["text"].as_str().unwrap();
  assert!(text.contains("Question: who is shown?\n"), "{text}");
  assert!(text.contains(&format!("Source: {path} (image/png)\n")), "{text}");
  assert!(text.contains("- lab-arena first failed: the engine answered no words\n"), "{text}");
  assert_eq!(
    answer["content"][1],
    serde_json::json!({
      "type": "image",
      "data": base64::engine::general_purpose::STANDARD.encode(png()),
      "mimeType": "image/png",
    })
  );
  let _ = std::fs::remove_dir_all(service.dir);
}

#[tokio::test]
async fn a_shell_s_call_no_backend_answers_is_refused_with_every_rung_s_reason() {
  let service = service_with(vec![("first", chat("", "stop"))], None).await;
  let (answer, _) = tool_see(&service, serde_json::json!({"line": "shell"})).await;
  assert_eq!(answer["isError"], true, "{answer}");
  let text = answer["content"][0]["text"].as_str().unwrap();
  assert!(
    text.contains("caller skipped: the look did not ask for the caller's hand-off"),
    "{text}"
  );
  let _ = std::fs::remove_dir_all(service.dir);
}

#[test]
fn the_hand_off_is_asked_where_the_record_reads_the_mode_s_material() {
  use eye_server::tool::asks_hand_off;
  let pictures = serde_json::json!({"labels": {"line": "worker", "reads": ["image"]}});
  let documents = serde_json::json!({"labels": {"line": "worker", "reads": ["pdf"]}});
  let videos = serde_json::json!({"labels": {"line": "worker", "reads": ["video"]}});
  assert!(asks_hand_off(Mode::See, &pictures));
  assert!(asks_hand_off(Mode::Read, &pictures));
  assert!(!asks_hand_off(Mode::Watch, &pictures), "no harness reads a video in a tool's answer");
  assert!(asks_hand_off(Mode::Read, &documents));
  assert!(
    !asks_hand_off(Mode::See, &documents),
    "a see look hands a picture, which this model cannot read"
  );
  assert!(!asks_hand_off(Mode::See, &videos), "a model that reads only video reads no picture");
  assert!(!asks_hand_off(Mode::Read, &videos));
  assert!(
    !asks_hand_off(Mode::See, &serde_json::json!({"labels": {"line": "worker"}})),
    "a record naming no media is a run on a text-only model"
  );
  assert!(!asks_hand_off(Mode::See, &serde_json::json!({"holderName": "schedule"})));
}
