//! The service's settings: where it listens, where its store stands, the Lab-arena nodes it asks, the hosted
//! backends' keys and the road to n2 (`docs/engines.md`, 「Where the vision unit stands」).

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
  pub lab-arena: Lab-arena,
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

#[derive(Debug, Clone, Deserialize)]
pub struct Lab-arena {
  #[serde(default = "default_nodes")]
  pub nodes: Vec<Node>,
}

impl Default for Lab-arena {
  fn default() -> Self {
    Lab-arena {
      nodes: default_nodes(),
    }
  }
}

/// One Lab-arena instance: where it answers, the key it takes, and the model name it carries.
#[derive(Debug, Clone, Deserialize)]
pub struct Node {
  pub name: String,
  pub address: String,
  pub model: String,
  pub key_file: String,
}

fn default_nodes() -> Vec<Node> {
  [
    ("n3", "127.0.0.1:18101", "qwen3.8-flash-next-iq3_s", "n3"),
    ("n4", "127.0.0.1:18102", "qwen3.8-flash-next-coder-iq1_m", "n4"),
    ("n5", "127.0.0.1:18103", "qwen3.8-flash-next-coder-iq1_m", "n5"),
    ("n6", "127.0.0.1:18104", "qwen3.8-flash-next-coder-iq1_m", "n6"),
  ]
  .into_iter()
  .map(|(name, address, model, key)| Node {
    name: name.into(),
    address: address.into(),
    model: model.into(),
    key_file: format!("~/.local/share/bmo-lab-arena/keys/{key}.key"),
  })
  .collect()
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
  #[serde(default = "default_openrouter_key")]
  pub key_file: String,
  #[serde(default = "default_openrouter_model")]
  pub model: String,
}

fn yes() -> bool {
  true
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

/// n2's CPU speech model, reached through the jake hub (`docs/engines.md`, 「The workbench on n2」).
#[derive(Debug, Clone, Deserialize)]
pub struct Speech {
  #[serde(default = "yes")]
  pub enabled: bool,
  #[serde(default = "default_agent")]
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
  "/home/ahas/repos/ahas/jake/src/console/main.ts".into()
}

fn default_bun() -> String {
  "/home/ahas/.bun/bin/bun".into()
}

fn default_agent() -> String {
  "n2".into()
}

fn default_hub() -> String {
  "http://127.0.0.1:8791".into()
}

fn default_hub_env() -> String {
  "~/.local/share/jake/hub.env".into()
}

fn default_speech_python() -> String {
  "/home/labuser/eye/venv-whisper/bin/python".into()
}

fn default_speech_script() -> String {
  "/home/labuser/eye/whisper.py".into()
}

fn default_speech_model() -> String {
  "faster-whisper large-v3 int8 cpu".into()
}

impl Default for Speech {
  fn default() -> Self {
    Speech {
      enabled: true,
      agent: default_agent(),
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
      lab-arena: Lab-arena::default(),
      hosted: Hosted::default(),
      speech: Speech::default(),
      media: Media::default(),
    }
  }
}

/// `~` at the front of a path stands for the home directory.
pub fn expand(path: &str) -> PathBuf {
  if let Some(rest) = path.strip_prefix("~/") {
    if let Some(home) = std::env::var_os("HOME") {
      return Path::new(&home).join(rest);
    }
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
