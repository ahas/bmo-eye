//! The routes `docs/api.md` states, and the work behind a look.

use std::ops::Bound;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bmo_eye_client::State as LookState;
use bmo_eye_client::*;
use futures_util::stream::Stream;
use headers::{Header, Range};
use schemars::JsonSchema;
use serde::Deserialize;
use sha2::Digest as _;

use crate::chain::{Chain, modes};
use crate::config::{Settings, expand};
use crate::media::{self, Material};
use crate::store::{Outcome, Store};

#[derive(Clone)]
pub struct App {
  pub settings: Arc<Settings>,
  pub store: Store,
  pub token: Arc<String>,
}

/// The largest upload of a picture or a document.
const PICTURE_CEILING: usize = 20 * 1024 * 1024;
/// The largest upload of a video, and so the body limit the upload route reads up to.
const VIDEO_CEILING: usize = 2 * 1024 * 1024 * 1024;

/// The longest question a look may ask, in characters (`docs/api.md`, 「What a caller asks」).
const QUESTION_CHARACTERS: usize = 8_000;

/// How long a caller waits before it asks again where the service cannot take a look yet.
const RETRY_AFTER_S: u64 = 5;

pub fn app(app: App) -> Router {
  let state = Arc::new(app);
  Router::new()
    .route("/health", get(health))
    .route("/looks", get(list_looks).post(create_look))
    .route("/looks/{look_id}", get(get_look).delete(delete_look))
    .route("/uploads", post(create_upload).layer(DefaultBodyLimit::max(VIDEO_CEILING)))
    .route("/uploads/{upload_id}/content", get(upload_content))
    .route("/uploads/{upload_id}", delete(delete_upload))
    .route("/modes", get(get_modes))
    .route("/events", get(events))
    .fallback(no_route)
    .layer(axum::middleware::from_fn_with_state(state.clone(), auth))
    .with_state(state)
}

async fn auth(
  State(app): State<Arc<App>>,
  req: axum::extract::Request,
  next: axum::middleware::Next,
) -> Response {
  let ok = req
    .headers()
    .get(axum::http::header::AUTHORIZATION)
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "))
    .map(|t| t == app.token.as_str())
    .unwrap_or(false);
  if !ok {
    return failure(StatusCode::UNAUTHORIZED, "unauthorized", "the token is missing or wrong");
  }
  next.run(req).await
}

pub fn failure(status: StatusCode, code: &str, error: impl Into<String>) -> Response {
  (
    status,
    Json(Failure {
      code: code.into(),
      error: error.into(),
    }),
  )
    .into_response()
}

async fn no_route() -> Response {
  failure(StatusCode::NOT_FOUND, "no-route", "no route answers this method and path")
}

/// 503 `not-ready`: the service cannot take a look yet, and the wait a caller should take before asking again.
fn not_ready(why: impl Into<String>) -> Response {
  (
    StatusCode::SERVICE_UNAVAILABLE,
    [(header::RETRY_AFTER, RETRY_AFTER_S.to_string())],
    Json(Failure {
      code: "not-ready".into(),
      error: why.into(),
    }),
  )
    .into_response()
}

/// How the service stands: whether it answers a look, and where it cannot, which of its two checks failed.
pub enum Stands {
  Ready,
  Store { why: String },
  Backends { why: String },
}

impl Stands {
  /// Whether the service answers a look now.
  pub fn ready(self: &Stands) -> bool {
    matches!(self, Stands::Ready)
  }

  /// The sentence `GET /health` answers in `reason`: null where the service stands.
  pub fn reason(self: &Stands) -> Option<String> {
    match self {
      Stands::Ready => None,
      Stands::Store { why } | Stands::Backends { why } => Some(why.clone()),
    }
  }
}

/// The check behind a health answer: the service reads its store and asks the state of every chain, the same
/// check `GET /modes` spells out.
pub async fn readiness(app: &Arc<App>) -> Stands {
  if let Err(why) = app.store.readable() {
    return Stands::Store { why };
  }
  let chains = modes(&app.settings).await;
  let rungs = chains
    .iter()
    .flat_map(|chain| chain.chain.iter())
    .filter(|rung| rung.name != "caller")
    .collect::<Vec<&ChainRung>>();
  if !rungs.iter().any(|rung| rung.state == "ready" || rung.state == "busy") {
    return Stands::Backends {
      why: no_backend_reached(&rungs),
    };
  }
  Stands::Ready
}

/// Why a service whose chains hold no standing backend says so: every rung and what it found.
fn no_backend_reached(rungs: &[&ChainRung]) -> String {
  let found = rungs.iter().map(|rung| rung_words(rung)).collect::<Vec<String>>().join("; ");
  if found.is_empty() {
    return "no backend of any chain could be reached".into();
  }
  format!("no backend of any chain could be reached: {found}")
}

/// One rung of a mode's chain as a sentence: its name, the node it stands on, what it is and why.
fn rung_words(rung: &ChainRung) -> String {
  let where_it_stands = match &rung.node {
    Some(node) => format!("{} {node}", rung.name),
    None => rung.name.clone(),
  };
  format!(
    "{where_it_stands} {}: {}",
    rung.state,
    rung.reason.as_deref().unwrap_or("no reason given")
  )
}

async fn health(State(app): State<Arc<App>>) -> Response {
  let stands = readiness(&app).await;
  Json(Health {
    ready: stands.ready(),
    reason: stands.reason(),
    version: env!("CARGO_PKG_VERSION").into(),
  })
  .into_response()
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LookQuery {
  wait_ms: Option<i64>,
}

async fn create_look(
  State(app): State<Arc<App>>,
  q: Result<Query<LookQuery>, axum::extract::rejection::QueryRejection>,
  body: Result<Json<NewLook>, axum::extract::rejection::JsonRejection>,
) -> Response {
  // A body or a query that does not read is refused with the documented failure (`docs/api.md`): the
  // extractor's own answer never reaches a caller.
  let q = match q {
    Ok(Query(q)) => q,
    Err(e) => return failure(StatusCode::BAD_REQUEST, "bad-request", e.body_text()),
  };
  let body = match body {
    Ok(Json(body)) => body,
    Err(e) => return failure(StatusCode::BAD_REQUEST, "bad-request", e.body_text()),
  };
  // Its store holds the look: one that cannot be read answers `not-ready` with the wait it should take.
  if let Err(why) = app.store.readable() {
    return not_ready(why);
  }
  let wait = q.wait_ms.unwrap_or(0).clamp(0, 600_000);
  if body.question.trim().is_empty() {
    return failure(StatusCode::BAD_REQUEST, "bad-request", "a look needs a question");
  }
  if body.question.chars().count() > QUESTION_CHARACTERS {
    return failure(
      StatusCode::BAD_REQUEST,
      "bad-request",
      format!("a look asks a question of at most {QUESTION_CHARACTERS} characters"),
    );
  }
  if body.sources.is_empty() {
    return failure(StatusCode::BAD_REQUEST, "bad-request", "a look needs at least one source");
  }
  let named = body.sources.len();
  let word = body.mode.as_str();
  let ceiling = body.mode.source_ceiling();
  if named > ceiling {
    return failure(
      StatusCode::PAYLOAD_TOO_LARGE,
      "too-large",
      format!("the look names {named} sources and {word} takes at most {ceiling}"),
    );
  }
  // Beside the source ceiling, the mode answers its own option shapes: a name it holds none of,
  // or a value outside the shape the modes' table states, is refused (`docs/api.md`).
  if let Some(why) = body.options.refusal_for(body.mode) {
    return failure(StatusCode::BAD_REQUEST, "bad-request", why);
  }
  // The sources are read once here, so a missing upload or an unreadable path is refused as a request.
  let mut described = Vec::new();
  for source in &body.sources {
    match source {
      SourceRef::Upload { upload_id } => {
        let upload = match app.store.upload(*upload_id) {
          Ok(u) => u,
          Err(_) => return failure(StatusCode::NOT_FOUND, "not-found", format!("no upload {upload_id}")),
        };
        described.push(media::describe(source, upload.bytes, &upload.media_type));
      }
      SourceRef::Path { path } => {
        let p = expand(path);
        let size = match std::fs::metadata(&p) {
          Ok(m) => m.len() as i64,
          Err(e) => {
            return failure(StatusCode::UNPROCESSABLE_ENTITY, "unsupported-source", format!("{path}: {e}"));
          }
        };
        let media = media::media_type_of(path).unwrap_or("application/octet-stream");
        if let Err(why) = fits(body.mode, media) {
          return failure(StatusCode::UNPROCESSABLE_ENTITY, "unsupported-source", format!("{path}: {why}"));
        }
        described.push(media::describe(source, size, media));
      }
      SourceRef::Url { url } if !url.starts_with("https://") => {
        return failure(
          StatusCode::UNPROCESSABLE_ENTITY,
          "unsupported-source",
          format!("{url}: an address must be https"),
        );
      }
      SourceRef::Url { url } => {
        described.push(media::describe(
          source,
          0,
          media::media_type_of(url).unwrap_or("application/octet-stream"),
        ));
      }
    }
  }
  let look = match app.store.create_look(body, &described) {
    Ok(l) => l,
    Err(e) => return failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", e),
  };
  let _ = app.store.event("look.created", &serde_json::json!({"id": look.id}));
  let id = look.id;
  let app2 = app.clone();
  tokio::spawn(async move { run_look(app2, id).await });
  if wait == 0 {
    return (StatusCode::CREATED, Json(app.store.look(id).unwrap_or(look))).into_response();
  }
  let started = std::time::Instant::now();
  loop {
    match app.store.look(id) {
      Ok(l) if l.state == "done" || l.state == "failed" || l.state == "stopped" => {
        return (StatusCode::CREATED, Json(l)).into_response();
      }
      Ok(_) if started.elapsed().as_millis() as i64 >= wait => {
        return (StatusCode::CREATED, Json(app.store.look(id).unwrap_or(look))).into_response();
      }
      Ok(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
      Err(e) => return failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", e),
    }
  }
}

fn fits(mode: Mode, media: &str) -> Result<(), String> {
  let ok = match mode {
    Mode::See => media.starts_with("image/"),
    Mode::Read => media.starts_with("image/") || media == "application/pdf",
    Mode::Watch => media.starts_with("video/"),
  };
  if ok { Ok(()) } else { Err(format!("{media} is not what {} reads", mode.as_str())) }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
  after: Option<i64>,
  limit: Option<i64>,
  mode: Option<String>,
  state: Option<String>,
}

async fn list_looks(
  State(app): State<Arc<App>>,
  q: Result<Query<ListQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
  let q = match q {
    Ok(Query(q)) => q,
    Err(e) => return failure(StatusCode::BAD_REQUEST, "bad-request", e.body_text()),
  };
  let limit = q.limit.unwrap_or(50).clamp(1, 200);
  match app.store.looks(limit, q.after, q.mode.as_deref(), q.state.as_deref()) {
    Ok(page) => Json(page).into_response(),
    Err(e) => failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", e),
  }
}

async fn get_look(State(app): State<Arc<App>>, Path(word): Path<String>) -> Response {
  let id = match word.parse::<i64>() {
    Ok(id) => id,
    Err(_) => return failure(StatusCode::NOT_FOUND, "no-route", format!("{word} is not a look id")),
  };
  match app.store.look(id) {
    Ok(l) => Json(l).into_response(),
    Err(_) => failure(StatusCode::NOT_FOUND, "not-found", format!("no look {id}")),
  }
}

async fn delete_look(State(app): State<Arc<App>>, Path(word): Path<String>) -> Response {
  let id = match word.parse::<i64>() {
    Ok(id) => id,
    Err(_) => return failure(StatusCode::NOT_FOUND, "no-route", format!("{word} is not a look id")),
  };
  match app.store.look(id) {
    Err(_) => failure(StatusCode::NOT_FOUND, "not-found", format!("no look {id}")),
    Ok(l) if l.state == "queued" || l.state == "running" => match app.store.stop(id) {
      Ok(true) => {
        let _ = app.store.event("look.stopped", &serde_json::json!({"id": id}));
        StatusCode::NO_CONTENT.into_response()
      }
      _ => StatusCode::NO_CONTENT.into_response(),
    },
    Ok(_) => StatusCode::NO_CONTENT.into_response(),
  }
}

async fn create_upload(
  State(app): State<Arc<App>>,
  headers: HeaderMap,
  body: Result<axum::body::Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
  let body = match body {
    Ok(body) => body,
    Err(e) if e.status() == StatusCode::PAYLOAD_TOO_LARGE => {
      return failure(StatusCode::PAYLOAD_TOO_LARGE, "too-large", "the body is over the ceiling");
    }
    Err(e) => return failure(StatusCode::BAD_REQUEST, "bad-request", e.body_text()),
  };
  let media = headers
    .get(axum::http::header::CONTENT_TYPE)
    .and_then(|v| v.to_str().ok())
    .map(|s| s.split(';').next().unwrap_or(s).trim().to_string())
    .unwrap_or_else(|| "application/octet-stream".into());
  let ceiling = if media.starts_with("video/") { VIDEO_CEILING } else { PICTURE_CEILING };
  if body.len() > ceiling {
    return failure(
      StatusCode::PAYLOAD_TOO_LARGE,
      "too-large",
      format!("{} bytes is over the ceiling", body.len()),
    );
  }
  if !(media.starts_with("image/") || media == "application/pdf" || media.starts_with("video/")) {
    return failure(
      StatusCode::UNPROCESSABLE_ENTITY,
      "unsupported-source",
      format!("{media} is not a source a mode reads"),
    );
  }
  let digest = format!("{:x}", sha2::Sha256::digest(&body));
  match app.store.create_upload(&media, &body, &digest) {
    Ok(u) => (StatusCode::CREATED, Json(u)).into_response(),
    Err(e) => failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", e),
  }
}

/// The span a request's `Range` header asks for, both ends counted in: none where the request names no
/// header, where it does not read, or where an upload of `total` bytes holds no such span — the whole bytes
/// are the answer then.
pub fn asked_span(headers: &HeaderMap, total: usize) -> Option<(usize, usize)> {
  let asked = headers.get(header::RANGE)?;
  let range = Range::decode(&mut std::iter::once(asked)).ok()?;
  let (start, end) = range.satisfiable_ranges(total as u64).next()?;
  let from = match start {
    Bound::Included(from) => from,
    Bound::Excluded(from) => from + 1,
    Bound::Unbounded => 0,
  };
  let last = total.saturating_sub(1) as u64;
  let to = match end {
    Bound::Included(to) => to.min(last),
    Bound::Excluded(to) => to.checked_sub(1)?.min(last),
    Bound::Unbounded => last,
  };
  (from <= to && from < total as u64).then_some((from as usize, to as usize))
}

async fn upload_content(
  State(app): State<Arc<App>>,
  headers: HeaderMap,
  Path(word): Path<String>,
) -> Response {
  let id = match word.parse::<i64>() {
    Ok(id) => id,
    Err(_) => return failure(StatusCode::NOT_FOUND, "no-route", format!("{word} is not an upload id")),
  };
  match app.store.upload_bytes(id) {
    Ok(bytes) => {
      let media = app
        .store
        .upload(id)
        .map(|u| u.media_type)
        .unwrap_or_else(|_| "application/octet-stream".into());
      let total = bytes.len();
      match asked_span(&headers, total) {
        None => {
          ([(header::ACCEPT_RANGES, "bytes".into()), (header::CONTENT_TYPE, media)], bytes).into_response()
        }
        Some((from, to)) => (
          StatusCode::PARTIAL_CONTENT,
          [
            (header::ACCEPT_RANGES, "bytes".into()),
            (header::CONTENT_TYPE, media),
            (header::CONTENT_RANGE, format!("bytes {from}-{to}/{total}")),
          ],
          bytes[from..=to].to_vec(),
        )
          .into_response(),
      }
    }
    Err(_) => failure(StatusCode::NOT_FOUND, "not-found", format!("no upload {id}")),
  }
}

async fn delete_upload(State(app): State<Arc<App>>, Path(word): Path<String>) -> Response {
  let id = match word.parse::<i64>() {
    Ok(id) => id,
    Err(_) => return failure(StatusCode::NOT_FOUND, "no-route", format!("{word} is not an upload id")),
  };
  match app.store.upload(id) {
    Err(_) => failure(StatusCode::NOT_FOUND, "not-found", format!("no upload {id}")),
    Ok(_) => match app.store.delete_upload(id) {
      Ok(()) => StatusCode::NO_CONTENT.into_response(),
      Err(e) => failure(StatusCode::INTERNAL_SERVER_ERROR, "failed", e),
    },
  }
}

async fn get_modes(State(app): State<Arc<App>>) -> Response {
  Json(modes(&app.settings).await).into_response()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EventQuery {
  after: Option<i64>,
}

async fn events(
  State(app): State<Arc<App>>,
  q: Result<Query<EventQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
  let q = match q {
    Ok(Query(q)) => q,
    Err(e) => return failure(StatusCode::BAD_REQUEST, "bad-request", e.body_text()),
  };
  let mut last = q.after.unwrap_or(0);
  let store = app.store.clone();
  let stream = async_stream(move |tx| async move {
    while let Ok(rows) = store.events_since(last, 200) {
      for (id, kind, at, data) in rows {
        last = id;
        let payload = serde_json::json!({"id": id, "kind": kind, "at": at, "data": serde_json::from_str::<serde_json::Value>(&data).unwrap_or_default()});
        if tx
          .send(Event::default().event(kind).id(id.to_string()).data(payload.to_string()))
          .await
          .is_err()
        {
          return;
        }
      }
      tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
  });
  Sse::new(stream).into_response()
}

/// A stream built from a loop, without another dependency.
fn async_stream<T, F, Fut>(build: F) -> impl Stream<Item = Result<T, std::convert::Infallible>> + Send
where
  F: FnOnce(tokio::sync::mpsc::Sender<T>) -> Fut,
  Fut: std::future::Future<Output = ()> + Send + 'static,
  T: Send + 'static,
{
  let (tx, mut rx) = tokio::sync::mpsc::channel::<T>(64);
  tokio::spawn(build(tx));
  futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx)).map(Ok)
}

use futures_util::StreamExt as _;

/// The work behind one look: the sources, the frames and speech, then the chain.
pub async fn run_look(app: Arc<App>, id: i64) {
  let look = match app.store.look(id) {
    Ok(l) => l,
    Err(_) => return,
  };
  if look.state == "stopped" {
    return;
  }
  let _ = app.store.set_running(id);
  let _ = app.store.event("look.running", &serde_json::json!({"id": id}));
  let mode = Mode::parse(&look.mode).unwrap_or(Mode::See);
  let outcome = prepare_and_answer(&app, &look, mode).await;
  let state = if outcome.error.is_none() { LookState::Done } else { LookState::Failed };
  if let Err(e) = app.store.finish(id, state, &outcome) {
    let failed = Outcome {
      error: Some(e),
      ..outcome
    };
    let _ = app.store.finish(id, LookState::Failed, &failed);
    let _ = app.store.event("look.failed", &serde_json::json!({"id": id}));
    return;
  }
  let _ = app.store.event(
    if state == LookState::Done { "look.done" } else { "look.failed" },
    &serde_json::json!({"id": id, "backend": outcome.backend.map(|b| b.name)}),
  );
}

async fn prepare_and_answer(app: &Arc<App>, look: &Look, mode: Mode) -> Outcome {
  let settings = &app.settings;
  let question = look.question.clone();
  let _registered: Vec<SourceRef> = look
    .sources
    .iter()
    .map(|s| match s.kind.as_str() {
      "upload" => SourceRef::Upload {
        upload_id: s.upload_id.unwrap_or(0),
      },
      "url" => SourceRef::Url {
        url: s.url.clone().unwrap_or_default(),
      },
      _ => SourceRef::Path {
        path: s.path.clone().unwrap_or_default(),
      },
    })
    .collect();
  let kinds: Vec<String> = look.sources.iter().map(|s| s.kind.clone()).collect();
  let mut images = Vec::new();
  let mut material = Material {
    text: question.clone(),
    ..Material::default()
  };
  let mut video: Option<(std::path::PathBuf, bool)> = None;
  for (i, source) in look.sources.iter().enumerate() {
    let (bytes, media, path) =
      match media::read_source(settings, &app.store, source, &app.settings.root()).await {
        Ok(v) => v,
        Err(e) => {
          return Outcome {
            error: Some(e),
            ..Outcome::default()
          };
        }
      };
    match mode {
      Mode::Watch => {
        // A path the caller named is never touched; a copy this service wrote in its work folder is its own
        // to remove.
        video = match path {
          // Only a path the caller named is the caller's; anything the service fetched or wrote is its own
          // to remove.
          Some(p) => Some((p, look.sources.get(i).map(|s| s.kind.as_str()) != Some("path"))),
          None => {
            let p = expand(&settings.media.work_dir).join(format!("look-{}-{i}.video", look.id));
            let written = p.parent().map(|dir| std::fs::create_dir_all(dir).is_ok()).unwrap_or(false)
              && std::fs::write(&p, &bytes).is_ok();
            if written { Some((p, true)) } else { None }
          }
        };
      }
      _ => {
        if media == "application/pdf" {
          let p = expand(&settings.media.work_dir).join(format!("look-{}-{i}.pdf", look.id));
          if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
          }
          if let Err(e) = std::fs::write(&p, &bytes) {
            return Outcome {
              error: Some(e.to_string()),
              ..Outcome::default()
            };
          }
          match media::pdf_pages(settings, &p, look.options.pages.as_deref()) {
            Ok(mut parts) => images.append(&mut parts),
            Err(e) => {
              return Outcome {
                error: Some(e),
                ..Outcome::default()
              };
            }
          }
        } else {
          images.push(media::ImagePart {
            bytes,
            media_type: media,
          });
        }
      }
    }
  }
  let _pages: Option<Vec<Page>> = None;
  if mode == Mode::Watch {
    let Some((path, ours)) = video else {
      return Outcome {
        error: Some("the video could not be read".into()),
        ..Outcome::default()
      };
    };
    let (frames, fps) = match media::frames(
      settings,
      &path,
      look.options.max_frames,
      look.options.fps,
      look.options.start_s,
      look.options.end_s,
    ) {
      Ok(v) => v,
      Err(e) => {
        return Outcome {
          error: Some(format!("the frames: {e}")),
          ..Outcome::default()
        };
      }
    };
    if frames.is_empty() {
      return Outcome {
        error: Some("no frame was taken from the video".into()),
        ..Outcome::default()
      };
    }
    material.duration_s = media::probe_duration(settings, &path).ok();
    material.frames = Some(Frames {
      count: frames.len() as i64,
      fps,
      tokens_per_frame: None,
    });
    images = frames;
    if look.options.speech {
      match media::audio(settings, &path) {
        Ok(wav) => {
          let heard = media::speech(settings, &wav, &look.options.language).await;
          let _ = std::fs::remove_file(&wav);
          match heard {
            Ok(s) => {
              material.text = format!("{question}\n\n[음성 녹취]\n{}", s.text);
              material.speech = Some(s);
            }
            Err(e) => {
              eprintln!("bmo-eye: the speech of look {}: {e}", look.id);
              material.text = format!("{question}\n\n[음성 녹취를 읽지 못했다: {e}]");
            }
          }
        }
        Err(e) => {
          eprintln!("bmo-eye: the audio of look {}: {e}", look.id);
          material.text = format!("{question}\n\n[음성 녹취를 읽지 못했다: {e}]");
        }
      }
    }
    if ours {
      let _ = std::fs::remove_file(&path);
    }
  }
  material.images = images.clone();
  if mode == Mode::Read && images.len() > 1 {
    // A document is asked page by page, so `pages` carries each page's own answer.
    let chain = Chain::for_mode(settings, mode);
    let mut collected = Vec::new();
    let mut first: Option<Outcome> = None;
    for (i, image) in images.into_iter().enumerate() {
      let one = Material {
        text: question.clone(),
        images: vec![image],
        ..Material::default()
      };
      let outcome = chain.walk(settings, mode, &look.options, &one, &kinds).await;
      match &outcome.answer {
        Some(a) => collected.push(Page {
          page: i as i64 + 1,
          text: a.text.clone(),
        }),
        None => {
          if first.is_none() {
            first = Some(outcome);
          }
          break;
        }
      }
      if first.is_none() {
        first = Some(outcome);
      }
    }
    let mut outcome = first.unwrap_or_default();
    if collected.len() == look.sources.len().max(1) || !collected.is_empty() {
      outcome.pages = Some(collected);
    }
    return outcome;
  }
  let chain = Chain::for_mode(settings, mode);
  chain.walk(settings, mode, &look.options, &material, &kinds).await
}
