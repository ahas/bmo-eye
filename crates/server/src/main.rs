//! `bmo-eye`: the vision unit's server and its command (`docs/api.md`, `docs/plan.md`).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use bmo_eye_client::*;
use clap::{Parser, Subcommand};
use eye_server::config::{Settings, expand};
use eye_server::routes::{App, app};
use eye_server::store::Store;

#[derive(Parser)]
#[command(
  name = "bmo-eye",
  version,
  about = "The vision unit: see, read and watch, through the Lab-arena engines"
)]
struct Cli {
  #[command(subcommand)]
  step: Step,
}

#[derive(Subcommand)]
enum Step {
  /// Answers the noun this command carries: `eye`.
  Nouns,
  /// The vision unit's own sub-verbs, as `bmo eye` hands them.
  Eye {
    #[command(subcommand)]
    step: Eye,
  },
  /// An image, as `bmo see` hands it: the eye group's own first-class verb.
  See(Put),
  /// A document's text, as `bmo read` hands it.
  Read(Put),
  /// A video, as `bmo watch` hands it.
  Watch(Watching),
  /// Runs the service.
  Serve {
    #[arg(long)]
    settings: Option<PathBuf>,
  },
  /// Answers 0 once the service answers ready: the deployment's health command.
  Health {
    #[arg(long)]
    settings: Option<PathBuf>,
  },
  /// Answers one tool call of the tool host's, read from standard input (`docs/plan.md`, 「The tool」).
  Tool {
    #[arg(long)]
    settings: Option<PathBuf>,
  },
}

#[derive(Subcommand)]
enum Eye {
  /// Looks at images and answers a question about them.
  See(Put),
  /// Reads documents and answers their text.
  Read(Put),
  /// Looks at a video and answers a question about it, with what was said in it.
  Watch(Watching),
  /// Lists the looks, newest first.
  Looks {
    #[arg(long, default_value_t = 20)]
    limit: i64,
    #[arg(long)]
    json: bool,
  },
  /// Prints one look.
  Look {
    id: i64,
    #[arg(long)]
    json: bool,
  },
  /// Prints the chains and what stands behind each rung.
  Modes {
    #[arg(long)]
    json: bool,
  },
}

#[derive(clap::Args)]
struct Put {
  /// The sources: a path, an https address or a YouTube link.
  #[arg(required = true)]
  sources: Vec<String>,
  #[arg(short, long)]
  question: Option<String>,
  #[arg(long, default_value = "text")]
  format: String,
  #[arg(long, default_value_t = 600)]
  wait: i64,
  #[arg(long)]
  pages: Option<String>,
  #[arg(long, default_value = "ko")]
  language: String,
  #[arg(long)]
  json: bool,
  #[arg(long)]
  no_tables: bool,
}

#[derive(clap::Args)]
struct Watching {
  source: String,
  #[arg(short, long)]
  question: Option<String>,
  #[arg(long, default_value_t = 1.0)]
  fps: f64,
  #[arg(long, default_value_t = 12)]
  max_frames: i64,
  #[arg(long)]
  start: Option<f64>,
  #[arg(long)]
  end: Option<f64>,
  #[arg(long)]
  no_speech: bool,
  #[arg(long, default_value_t = 600)]
  wait: i64,
  #[arg(long)]
  json: bool,
}

// One of the vision unit's sub-verbs, answered as its own exit: the same verb the eye group's first-class
// commands hand over (`bmo see`, `bmo read`, `bmo watch`).
async fn hand(step: Eye) -> ExitCode {
  match command(step).await {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("bmo-eye: {e}");
      ExitCode::FAILURE
    }
  }
}

fn read_token(settings: &Settings) -> Result<String, String> {
  let path = expand(&settings.token_file);
  std::fs::read_to_string(&path)
    .map(|t| t.trim().to_string())
    .map_err(|e| format!("{}: {e}", path.display()))
}

fn source_ref(word: &str) -> SourceRef {
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

fn text_answer(look: &Look) -> String {
  match (&look.answer, &look.pages) {
    (Some(a), _) => a.text.clone(),
    (None, Some(pages)) => pages.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join("\n\n"),
    _ => String::new(),
  }
}

#[tokio::main]
async fn main() -> ExitCode {
  let cli = Cli::parse();
  match cli.step {
    Step::Nouns => {
      // The unit's own noun, and the eye group's three verbs as the platform's first-class commands
      // (docs/plan.md, 「The noun」).
      for noun in ["eye", "see", "read", "watch"] {
        println!("{noun}\t");
      }
      ExitCode::SUCCESS
    }
    Step::Serve { settings } => match serve(settings).await {
      Ok(()) => ExitCode::SUCCESS,
      Err(e) => {
        eprintln!("bmo-eye: {e}");
        ExitCode::FAILURE
      }
    },
    Step::Health { settings } => match health(settings).await {
      Ok(()) => ExitCode::SUCCESS,
      Err(e) => {
        eprintln!("bmo-eye: {e}");
        ExitCode::FAILURE
      }
    },
    Step::Tool { settings } => {
      tool(settings).await;
      ExitCode::SUCCESS
    }
    Step::See(put) => hand(Eye::See(put)).await,
    Step::Read(put) => hand(Eye::Read(put)).await,
    Step::Watch(watching) => hand(Eye::Watch(watching)).await,
    Step::Eye { step } => hand(step).await,
  }
}

async fn serve(settings: Option<PathBuf>) -> Result<(), String> {
  if let Some(path) = &settings {
    // SAFETY: the server reads its settings before it starts any task.
    unsafe { std::env::set_var("BMO_EYE_SETTINGS", path) };
  }
  let settings = Settings::load()?;
  let root = settings.root();
  std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
  let token_path = expand(&settings.token_file);
  if !token_path.exists() {
    let token = format!("{:032x}", rand_u128());
    std::fs::write(&token_path, &token).map_err(|e| format!("{}: {e}", token_path.display()))?;
    set_private(&token_path);
  }
  let token = std::fs::read_to_string(&token_path).map_err(|e| format!("{}: {e}", token_path.display()))?;
  let store = Store::open(&root)?;
  let listen = settings.listen.clone();
  let app = app(App {
    settings: Arc::new(settings),
    store,
    token: Arc::new(token.trim().into()),
  });
  let listener = tokio::net::TcpListener::bind(&listen).await.map_err(|e| format!("{listen}: {e}"))?;
  eprintln!("bmo-eye: serving on http://{listen}");
  axum::serve(listener, app)
    .with_graceful_shutdown(shutdown())
    .await
    .map_err(|e| e.to_string())
}

async fn shutdown() {
  let _ = tokio::signal::ctrl_c().await;
}

fn rand_u128() -> u128 {
  use std::time::{SystemTime, UNIX_EPOCH};
  let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
  let pid = std::process::id() as u128;
  (nanos << 32) ^ (pid << 16) ^ 0x5eed_1234_abcd_9876_1122_3344_5566_7788
}

#[cfg(unix)]
fn set_private(path: &std::path::Path) {
  use std::os::unix::fs::PermissionsExt;
  let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_private(_: &std::path::Path) {}

async fn command(step: Eye) -> Result<(), String> {
  let settings = Settings::load()?;
  let token = read_token(&settings)?;
  let client = EyeClient::new(format!("http://{}", settings.listen)).with_token(token);
  match step {
    Eye::See(p) => put(client, Mode::See, p, "이 그림을 보고 질문에 답하라.").await,
    Eye::Read(p) => put(client, Mode::Read, p, "이 문서의 글자를 위에서 아래로 그대로 옮겨 적어라.").await,
    Eye::Watch(w) => {
      let question = w
        .question
        .clone()
        .unwrap_or_else(|| "이 영상이 무엇을 보여 주고 무엇을 보도하는지 답하라.".into());
      let look = NewLook {
        mode: Mode::Watch,
        question,
        sources: vec![source_ref(&w.source)],
        options: Options {
          format: "text".into(),
          fps: w.fps,
          max_frames: w.max_frames,
          start_s: w.start.unwrap_or(0.0),
          end_s: w.end,
          speech: !w.no_speech,
          pages: None,
          language: "ko".into(),
          tables: true,
          detail: None,
          hand_off: false,
        },
        labels: Default::default(),
      };
      show(client, look, w.wait, w.json).await
    }
    Eye::Looks { limit, json } => {
      let page = client.looks(limit).await?;
      if json {
        println!("{}", serde_json::to_string_pretty(&page).map_err(|e| e.to_string())?);
      } else {
        for l in page.items {
          let backend = l.backend.as_ref().map(|b| b.name.clone()).unwrap_or_else(|| "-".into());
          let node = l.backend.as_ref().and_then(|b| b.node.clone()).unwrap_or_else(|| "-".into());
          println!(
            "#{} {} {} {} {} {}ms",
            l.id,
            l.mode,
            l.state,
            backend,
            node,
            l.usage.total_ms.unwrap_or(0)
          );
        }
      }
      Ok(())
    }
    Eye::Look { id, json } => {
      let look = client.look(id).await?;
      if json {
        println!("{}", serde_json::to_string_pretty(&look).map_err(|e| e.to_string())?);
      } else {
        println!("{}", text_answer(&look));
      }
      Ok(())
    }
    Eye::Modes { json } => {
      let modes = client.modes().await?;
      if json {
        println!("{}", serde_json::to_string_pretty(&modes).map_err(|e| e.to_string())?);
      } else {
        for m in modes {
          println!("{}:", m.mode);
          for r in m.chain {
            println!(
              "  {:<10} {:<4} {:<40} {}{}",
              r.name,
              r.node.clone().unwrap_or_default(),
              r.model.clone().unwrap_or_default(),
              r.state,
              r.reason.map(|x| format!(" ({x})")).unwrap_or_default()
            );
          }
        }
      }
      Ok(())
    }
  }
}

async fn put(client: EyeClient, mode: Mode, p: Put, default_question: &str) -> Result<(), String> {
  let question = p.question.clone().unwrap_or_else(|| default_question.into());
  let pages = p
    .pages
    .as_ref()
    .map(|s| s.split(',').filter_map(|w| w.trim().parse::<i64>().ok()).collect());
  let look = NewLook {
    mode,
    question,
    sources: p.sources.iter().map(|s| source_ref(s)).collect(),
    options: Options {
      format: p.format.clone(),
      detail: None,
      fps: 1.0,
      max_frames: 12,
      start_s: 0.0,
      end_s: None,
      speech: true,
      pages,
      language: p.language.clone(),
      tables: !p.no_tables,
      hand_off: false,
    },
    labels: Default::default(),
  };
  show(client, look, p.wait, p.json).await
}

async fn show(client: EyeClient, new: NewLook, wait: i64, json: bool) -> Result<(), String> {
  let mut look = client.create(&new, wait * 1000).await?;
  // Any wait but none waits for the look's end, however long its node takes.
  if wait > 0 {
    while look.state == "queued" || look.state == "running" {
      tokio::time::sleep(std::time::Duration::from_millis(500)).await;
      look = client.look(look.id).await?;
    }
  }
  if json {
    println!("{}", serde_json::to_string_pretty(&look).map_err(|e| e.to_string())?);
  } else {
    let text = text_answer(&look);
    if !text.is_empty() {
      println!("{text}");
    }
    if let Some(s) = &look.speech {
      eprintln!("[speech] {}", s.text.chars().take(200).collect::<String>());
    }
    let backend = look
      .backend
      .as_ref()
      .map(|b| format!("{} {}", b.name, b.node.clone().unwrap_or_default()))
      .unwrap_or_else(|| "-".into());
    let whole = look.ended_at.zip(Some(look.created_at)).map(|(e, c)| e - c).unwrap_or(0);
    eprintln!(
      "[look {} {} {} {}ms whole {}ms]{}",
      look.id,
      look.state,
      backend,
      look.usage.total_ms.unwrap_or(0),
      whole,
      look.error.clone().map(|e| format!(" {e}")).unwrap_or_default()
    );
  }
  if look.state == "failed" {
    return Err(look.error.unwrap_or_else(|| "the look failed".into()));
  }
  Ok(())
}

/// `bmo-eye health`: the deployment's health command, asking the service the settings name.
async fn health(settings: Option<PathBuf>) -> Result<(), String> {
  if let Some(path) = &settings {
    // SAFETY: the process reads its settings before it asks anything.
    unsafe { std::env::set_var("BMO_EYE_SETTINGS", path) };
  }
  let settings = Settings::load()?;
  let token = read_token(&settings)?;
  let client = EyeClient::new(format!("http://{}", settings.listen)).with_token(token);
  let health = client.health().await?;
  if health.ready {
    println!("ready {} {}", health.version, settings.listen);
    Ok(())
  } else {
    Err(health.reason.unwrap_or_else(|| "not ready".into()))
  }
}

/// The longest the create route holds a look's answer open (`docs/api.md`, 「Limits」); the tool reads the look
/// again past it.
const HOLD_MS: i64 = 600_000;

/// One call of the tool host's, answered as the tool host's protocol states: the call on standard input, an
/// MCP tool result on standard output (`docs/plan.md`, 「The tool」).
async fn tool(settings: Option<PathBuf>) {
  use std::io::Read as _;
  let mut raw = String::new();
  if let Err(e) = std::io::stdin().read_to_string(&mut raw) {
    return refuse(&format!("the call did not read: {e}"));
  }
  let call: serde_json::Value = match serde_json::from_str(&raw) {
    Ok(v) => v,
    Err(e) => return refuse(&format!("the call did not read as JSON: {e}")),
  };
  let kind = call.get("kind").and_then(|k| k.as_str()).unwrap_or_default();
  if kind != "call" {
    return refuse(&format!("this tool answers a call alone, not {kind}"));
  }
  let entry = call.get("entry").and_then(|e| e.as_str()).unwrap_or_default();
  let Some(mode) = entry.strip_prefix("eye/").and_then(Mode::parse) else {
    return refuse(&format!("{entry}: no such entry of this tool"));
  };
  let args = call.get("arguments").cloned().unwrap_or_else(|| serde_json::json!({}));
  let Some(path) = args.get("path").and_then(|p| p.as_str()) else {
    return refuse("the call names no path");
  };
  let default_question = match mode {
    Mode::See => "이 그림을 보고 질문에 답하라.",
    Mode::Read => "이 문서의 글자를 위에서 아래로 그대로 옮겨 적어라.",
    Mode::Watch => "이 영상이 무엇을 보여 주고 무엇을 보도하는지 답하라.",
  };
  let question = args.get("question").and_then(|q| q.as_str()).unwrap_or(default_question).to_string();
  let pages = args
    .get("pages")
    .and_then(|p| p.as_str())
    .map(|p| p.split(',').filter_map(|w| w.trim().parse::<i64>().ok()).collect());
  let fps = args.get("fps").and_then(|f| f.as_f64()).unwrap_or(1.0);
  let max_frames = args.get("maxFrames").and_then(|f| f.as_i64()).unwrap_or(12);
  let speech = args.get("speech").and_then(|s| s.as_bool()).unwrap_or(true);
  if let Some(path) = &settings {
    // SAFETY: the process reads its settings before it asks the service anything.
    unsafe { std::env::set_var("BMO_EYE_SETTINGS", path) };
  }
  let settings = match Settings::load() {
    Ok(s) => s,
    Err(e) => return refuse(&e),
  };
  let token = match read_token(&settings) {
    Ok(t) => t,
    Err(e) => return refuse(&e),
  };
  let client = EyeClient::new(format!("http://{}", settings.listen)).with_token(token);
  let new = NewLook {
    mode,
    question,
    sources: vec![source_ref(path)],
    options: Options {
      format: "text".into(),
      detail: None,
      fps,
      max_frames,
      start_s: 0.0,
      end_s: None,
      speech,
      pages,
      language: "ko".into(),
      tables: true,
      hand_off: false,
    },
    labels: Default::default(),
  };
  // The entry is unbounded: the call waits for the look's end, holding the route open as long as it holds.
  let mut look = match client.create(&new, HOLD_MS).await {
    Ok(l) => l,
    Err(e) => return refuse(&format!("the service: {e}")),
  };
  while look.state == "queued" || look.state == "running" {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    match client.look(look.id).await {
      Ok(l) => look = l,
      Err(e) => return refuse(&format!("the service: {e}")),
    }
  }
  if look.state != "done" {
    let why = look.error.clone().unwrap_or_else(|| look.state.clone());
    return refuse(&format!("the look ended {}: {why}", look.state));
  }
  let text = match (&look.answer, &look.pages) {
    (Some(a), _) => a.text.clone(),
    (None, Some(pages)) => pages.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join("\n\n"),
    _ => String::new(),
  };
  let answer = serde_json::json!({
    "content": [{"type": "text", "text": text}],
    "isError": false,
    "structuredContent": look,
  });
  println!("{answer}");
}

/// A refusal a model reads: the tool host's answer, not a crash (`docs/plan.md`).
fn refuse(why: &str) {
  let answer = serde_json::json!({
    "content": [{"type": "text", "text": format!("bmo-eye: {why}")}],
    "isError": true,
  });
  println!("{answer}");
}
