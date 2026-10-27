//! The routes `docs/api.md` states, answered against a stand-in Lab-arena node: a look walks its chain, records
//! what every rung did, and answers from the first that does.

use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Json, Router};
use bmo_eye_client::*;
use eye_server::config::Settings;
use eye_server::routes::{App, app};
use eye_server::store::Store;

/// A stand-in for one Lab-arena instance: it says it reads pictures and answers every chat with one sentence.
async fn stand_in() -> (String, tokio::task::JoinHandle<()>) {
  let router = Router::new()
    .route("/health", get(|| async { Json(serde_json::json!({"status": "ok", "images": true})) }))
    .route(
      "/v1/chat/completions",
      post(|Json(body): Json<serde_json::Value>| async move {
        let images = body["messages"][0]["content"].as_array().map(|c| c.iter().filter(|p| p["type"] == "image_url").count()).unwrap_or(0);
        Json(serde_json::json!({
          "choices": [{"message": {"content": format!("answer: {images} pictures")}}],
          "timings": {"prompt_n": 7 + 300 * images, "predicted_n": 5, "prompt_ms": 120.0, "predicted_ms": 50.0}
        }))
      }),
    );
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  let handle = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
  (format!("{}", addr), handle)
}

async fn service() -> (String, String, std::path::PathBuf) {
  let (node, handle) = stand_in().await;
  std::mem::forget(handle);
  static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
  let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
  let dir = std::env::temp_dir().join(format!("bmo-eye-test-{}-{n}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  let settings = Settings::default();
  let mut settings = settings;
  settings.listen = "127.0.0.1:0".into();
  settings.root = dir.to_string_lossy().to_string();
  settings.lab-arena.nodes = vec![eye_server::config::Node {
    name: "stand-in".into(),
    address: node,
    model: "qwen3.8-flash-next-iq3_s".into(),
    key_file: "/dev/null".into(),
  }];
  settings.hosted.openrouter.enabled = false;
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
  (format!("http://{addr}"), token, dir)
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
  assert_eq!(client.look(upload.id).await.is_err(), true);
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
