//! The service's settings: where it listens, where its store stands, the local engine nodes it asks, the
//! hosted backends' keys and the road to the speech node (`docs/engines.md`, 「Where the vision unit
//! stands」).

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Settings {
  #[serde(default = "default_listen")]
  pub listen: String,
  #[serde(default = "default_root")]
  pub root: String,
  #[serde(default = "default_token_file")]
  pub token_file: String,
  #[serde(default)]
  pub local: Local,
  #[serde(default)]
  pub hosted: Hosted,
  #[serde(default)]
  pub speech: Speech,
  #[serde(default)]
  pub media: Media,
}

fn default_listen() -> String {
  "127.0.0.1:7435".into()
}

fn default_root() -> String {
  "~/.local/share/bmo/data/eye".into()
}

fn default_token_file() -> String {
  "~/.local/share/bmo/data/eye/token".into()
}

/// The local OpenAI-compatible engine nodes a look asks first, in order. None is named by default: with the
/// list empty the chain starts at the hosted backends.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Local {
  #[serde(default)]
  pub nodes: Vec<Node>,
}

/// One local engine node: its name, where it answers, the key it takes, and the model name it carries.
#[derive(Debug, Clone, Deserialize)]
pub struct Node {
  pub name: String,
  pub address: String,
  pub model: String,
  pub key_file: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Hosted {
  #[serde(default)]
  pub openrouter: OpenRouter,
  #[serde(default)]
  pub gemini: Gemini,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouter {
  #[serde(default = "yes")]
  pub enabled: bool,
  /// The API's base address, the chat and key routes below it.
  #[serde(default = "default_openrouter_address")]
  pub address: String,
  #[serde(default = "default_openrouter_key")]
  pub key_file: String,
  #[serde(default = "default_openrouter_model")]
  pub model: String,
}

fn yes() -> bool {
  true
}

fn default_openrouter_address() -> String {
  "https://openrouter.ai/api/v1".into()
}

fn default_openrouter_key() -> String {
  "~/.local/share/bmo/data/env/openrouter.key".into()
}

fn default_openrouter_model() -> String {
  "qwen/qwen3.7-flash".into()
}

impl Default for OpenRouter {
  fn default() -> Self {
    OpenRouter {
      enabled: true,
      address: default_openrouter_address(),
      key_file: default_openrouter_key(),
      model: default_openrouter_model(),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gemini {
  #[serde(default = "yes")]
  pub enabled: bool,
  #[serde(default)]
  pub key_file: Option<String>,
  #[serde(default = "default_gemini_model")]
  pub model: String,
}

fn default_gemini_model() -> String {
  "gemini-3-pro".into()
}

impl Default for Gemini {
  fn default() -> Self {
    Gemini {
      enabled: true,
      key_file: None,
      model: default_gemini_model(),
    }
  }
}

/// The speech node's CPU speech model, reached through the jake hub (`docs/engines.md`, 「The speech
/// workbench」). No node is named by default: speech stands only once `agent` names the hub agent of the
/// prepared node.
#[derive(Debug, Clone, Deserialize)]
pub struct Speech {
  #[serde(default = "yes")]
  pub enabled: bool,
  #[serde(default)]
  pub agent: String,
  #[serde(default = "default_hub")]
  pub hub: String,
  #[serde(default = "default_hub_env")]
  pub hub_env: String,
  #[serde(default = "default_speech_python")]
  pub python: String,
  #[serde(default = "default_speech_script")]
  pub script: String,
  #[serde(default = "default_speech_model")]
  pub model: String,
  /// The jake console and the interpreter that runs it, which carry the audio to the node.
  #[serde(default = "default_console")]
  pub console: String,
  #[serde(default = "default_bun")]
  pub bun: String,
}

fn default_console() -> String {
  "~/jake/src/console/main.ts".into()
}

fn default_bun() -> String {
  "~/.bun/bin/bun".into()
}

fn default_hub() -> String {
  "http://127.0.0.1:8791".into()
}

fn default_hub_env() -> String {
  "~/.local/share/jake/hub.env".into()
}

fn default_speech_python() -> String {
  "~/eye/venv-whisper/bin/python".into()
}

fn default_speech_script() -> String {
  "~/eye/whisper.py".into()
}

fn default_speech_model() -> String {
  "faster-whisper large-v3 int8 cpu".into()
}

impl Default for Speech {
  fn default() -> Self {
    Speech {
      enabled: true,
      agent: String::new(),
      hub: default_hub(),
      hub_env: default_hub_env(),
      python: default_speech_python(),
      script: default_speech_script(),
      model: default_speech_model(),
      console: default_console(),
      bun: default_bun(),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Media {
  #[serde(default = "default_ffmpeg")]
  pub ffmpeg: String,
  #[serde(default = "default_ytdlp")]
  pub ytdlp: String,
  #[serde(default = "default_work")]
  pub work_dir: String,
}

fn default_ffmpeg() -> String {
  "ffmpeg".into()
}

fn default_ytdlp() -> String {
  "~/.local/share/bmo/data/eye/venv/bin/yt-dlp".into()
}

fn default_work() -> String {
  "~/.local/share/bmo/data/eye/work".into()
}

impl Default for Media {
  fn default() -> Self {
    Media {
      ffmpeg: default_ffmpeg(),
      ytdlp: default_ytdlp(),
      work_dir: default_work(),
    }
  }
}

impl Default for Settings {
  fn default() -> Self {
    Settings {
      listen: default_listen(),
      root: default_root(),
      token_file: default_token_file(),
      local: Local::default(),
      hosted: Hosted::default(),
      speech: Speech::default(),
      media: Media::default(),
    }
  }
}

/// `~` at the front of a path stands for the home directory.
pub fn expand(path: &str) -> PathBuf {
  if let Some(rest) = path.strip_prefix("~/")
    && let Some(home) = std::env::var_os("HOME")
  {
    return Path::new(&home).join(rest);
  }
  PathBuf::from(path)
}

impl Settings {
  /// The settings file the environment names, else the service's own under the data root, else the
  /// defaults of this installation.
  pub fn load() -> Result<Settings, String> {
    let path = std::env::var("BMO_EYE_SETTINGS").ok().map(PathBuf::from).unwrap_or_else(|| {
      let root = std::env::var("BMO_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| expand("~/.local/share/bmo"));
      root.join("eye/settings.toml")
    });
    match std::fs::read_to_string(&path) {
      Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
      Err(e) => Err(format!("{}: {e}", path.display())),
    }
  }

  pub fn root(&self) -> PathBuf {
    expand(&self.root)
  }
}
