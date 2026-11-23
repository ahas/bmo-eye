//! What a look is made of: the bytes of its sources, a video's frames and its speech, and the prompt they
//! are asked with (`docs/modes.md`).

use std::path::{Path, PathBuf};

use base64::Engine as _;
use bmo_eye_client::*;

use crate::config::{Settings, expand};
use crate::store::Store;

/// One picture on its way to a node.
#[derive(Debug, Clone)]
pub struct ImagePart {
  pub bytes: Vec<u8>,
  pub media_type: String,
}

/// Everything a chain's rung is asked with.
#[derive(Debug, Clone, Default)]
pub struct Material {
  pub text: String,
  /// A video's length in seconds, where the look has one.
  pub duration_s: Option<f64>,
  pub images: Vec<ImagePart>,
  pub frames: Option<Frames>,
  pub speech: Option<Speech>,
  pub pages: Option<Vec<Page>>,
}

pub fn media_type_of(path: &str) -> Option<&'static str> {
  match Path::new(path)
    .extension()
    .and_then(|e| e.to_str())
    .map(|e| e.to_ascii_lowercase())
    .as_deref()
  {
    Some("png") => Some("image/png"),
    Some("jpg") | Some("jpeg") => Some("image/jpeg"),
    Some("webp") => Some("image/webp"),
    Some("tif") | Some("tiff") => Some("image/tiff"),
    Some("pdf") => Some("application/pdf"),
    Some("mp4") => Some("video/mp4"),
    Some("webm") => Some("video/webm"),
    Some("mov") => Some("video/quicktime"),
    Some("wav") => Some("audio/wav"),
    Some("mp3") => Some("audio/mpeg"),
    _ => None,
  }
}

fn data_url(part: &ImagePart) -> String {
  format!(
    "data:{};base64,{}",
    part.media_type,
    base64::engine::general_purpose::STANDARD.encode(&part.bytes)
  )
}

fn run(cmd: &mut tokio::process::Command) -> Result<std::process::Output, String> {
  let out = cmd.output();
  let out = futures_block(out)?;
  if !out.status.success() {
    return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
  }
  Ok(out)
}

/// Block on a process's answer inside the async handler; the work is one short-lived child.
fn futures_block(
  fut: impl std::future::Future<Output = std::io::Result<std::process::Output>>,
) -> Result<std::process::Output, String> {
  tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(fut)).map_err(|e| e.to_string())
}

pub fn is_youtube(url: &str) -> bool {
  url.contains("youtube.com/watch") || url.contains("youtu.be/") || url.contains("youtube.com/shorts")
}

/// The page hosts that hold a video behind a login wall or a script: the address itself is never media,
/// so `yt-dlp`'s extractor for the host is what reaches the file.
const VIDEO_PAGE_HOSTS: [&str; 7] = [
  "instagram.com",
  "tiktok.com",
  "x.com",
  "twitter.com",
  "facebook.com",
  "vimeo.com",
  "threads.net",
];

/// Whether an address is a video page on one of the known hosts, at the host or a subdomain of it.
pub fn is_video_page(url: &str) -> bool {
  let Ok(parsed) = url::Url::parse(url) else { return false };
  let Some(host) = parsed.host_str().map(|h| h.to_ascii_lowercase()) else {
    return false;
  };
  VIDEO_PAGE_HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

/// Whether a media type is one a look can read.
pub fn is_readable_media(media: &str) -> bool {
  media.starts_with("image/")
    || media.starts_with("video/")
    || media.starts_with("audio/")
    || media == "application/pdf"
}

/// The media type an address declares: its content type, and where that says nothing readable
/// (`application/octet-stream` or nothing at all) the type its extension carries.
pub fn declared_media(header: Option<&str>, url: &str) -> String {
  let declared = header
    .map(|v| v.split(';').next().unwrap_or(v).trim())
    .filter(|v| !v.is_empty() && *v != "application/octet-stream");
  declared
    .map(str::to_string)
    .or_else(|| media_type_of(url).map(str::to_string))
    .unwrap_or_else(|| "application/octet-stream".into())
}

/// A YouTube link, fetched by yt-dlp into the work folder: the video at 720p at most, with its audio.
pub async fn youtube(settings: &Settings, url: &str) -> Result<(Vec<u8>, PathBuf), String> {
  pull_video(settings, url, &format!("yt-{}", std::process::id())).await
}

/// Pull a video into the work folder with the configured yt-dlp: at 720p at most, with its audio, merged
/// into an mp4 named after the stem.
async fn pull_video(settings: &Settings, url: &str, stem: &str) -> Result<(Vec<u8>, PathBuf), String> {
  let dir = expand(&settings.media.work_dir);
  tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
  let target = dir.join(format!("{stem}.mp4"));
  let out = tokio::process::Command::new(expand(&settings.media.ytdlp))
    .arg("--no-warnings")
    .arg("-f")
    .arg("bv*[height<=720]+ba/b[height<=720]")
    .arg("--merge-output-format")
    .arg("mp4")
    .arg("-o")
    .arg(&target)
    .arg(url)
    .output()
    .await
    .map_err(|e| format!("yt-dlp: {e}"))?;
  if !out.status.success() {
    return Err(format!("yt-dlp: {}", String::from_utf8_lossy(&out.stderr).trim()));
  }
  let bytes = tokio::fs::read(&target).await.map_err(|e| format!("{}: {e}", target.display()))?;
  Ok((bytes, target))
}

/// The file a video page holds, pulled by yt-dlp into the work folder under a name no other source of
/// the same process takes.
async fn video_page(settings: &Settings, url: &str) -> Result<(Vec<u8>, String, Option<PathBuf>), String> {
  let nanos = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_nanos())
    .unwrap_or(0);
  let stem = format!("video-{}-{nanos}", std::process::id());
  let (bytes, path) = pull_video(settings, url, &stem).await.map_err(|e| format!("{url}: {e}"))?;
  Ok((bytes, "video/mp4".into(), Some(path)))
}

/// The source's bytes and type, read where the source stands.
pub async fn read_source(
  settings: &Settings,
  store: &Store,
  source: &Source,
  root: &Path,
) -> Result<(Vec<u8>, String, Option<PathBuf>), String> {
  let kind = source.kind.as_str();
  match kind {
    "upload" => {
      let id = source.upload_id.ok_or("an upload source without an id")?;
      let bytes = store.upload_bytes(id)?;
      let media = store.upload(id)?.media_type;
      Ok((bytes, media, None))
    }
    "path" => {
      let path = source.path.clone().ok_or("a path source without a path")?;
      let p = expand(&path);
      if !p.starts_with(expand(&root.to_string_lossy())) && !p.is_absolute() {
        return Err(format!("{path}: a path source must be absolute"));
      }
      let bytes = std::fs::read(&p).map_err(|e| format!("{path}: {e}"))?;
      let media = media_type_of(&path).unwrap_or("application/octet-stream").to_string();
      Ok((bytes, media, Some(p)))
    }
    "url" => {
      let url = source.url.clone().ok_or("a url source without a url")?;
      if !url.starts_with("https://") {
        return Err(format!("{url}: a source address must be https"));
      }
      if is_youtube(&url) {
        let (bytes, path) = youtube(settings, &url).await?;
        return Ok((bytes, "video/mp4".into(), Some(path)));
      }
      if is_video_page(&url) {
        return video_page(settings, &url).await;
      }
      let resp = reqwest::get(&url).await.map_err(|e| format!("{url}: {e}"))?;
      let status = resp.status();
      if !status.is_success() {
        return Err(format!("{url}: {status}"));
      }
      let header = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
      let media = declared_media(header.as_deref(), &url);
      let bytes = resp.bytes().await.map_err(|e| format!("{url}: {e}"))?.to_vec();
      if is_readable_media(&media) {
        return Ok((bytes, media, None));
      }
      // The address answered something a look cannot read, usually an HTML page: it may still hold a
      // video that an yt-dlp extractor knows.
      match video_page(settings, &url).await {
        Ok(resolved) => Ok(resolved),
        // Bytes the server labelled with no type at all may be media ffmpeg reads; they pass as they are.
        Err(_) if media == "application/octet-stream" => Ok((bytes, media, None)),
        Err(e) => Err(format!("{url}: {media} is not a media the look can read ({e})")),
      }
    }
    other => Err(format!("{other}: unknown source kind")),
  }
}

/// A look's sources as the answer records them.
pub fn describe(source: &SourceRef, bytes: i64, media: &str) -> Source {
  match source {
    SourceRef::Upload { upload_id } => Source {
      kind: "upload".into(),
      upload_id: Some(*upload_id),
      url: None,
      path: None,
      media_type: Some(media.into()),
      bytes,
      width: None,
      height: None,
      duration_ms: None,
    },
    SourceRef::Url { url } => Source {
      kind: "url".into(),
      upload_id: None,
      url: Some(url.clone()),
      path: None,
      media_type: Some(media.into()),
      bytes,
      width: None,
      height: None,
      duration_ms: None,
    },
    SourceRef::Path { path } => Source {
      kind: "path".into(),
      upload_id: None,
      url: None,
      path: Some(path.clone()),
      media_type: Some(media.into()),
      bytes,
      width: None,
      height: None,
      duration_ms: None,
    },
  }
}

/// A video's frames, taken with ffmpeg into the work folder.
pub fn frames(
  settings: &Settings,
  video: &Path,
  count: i64,
  fps: f64,
  start: f64,
  end: Option<f64>,
) -> Result<(Vec<ImagePart>, f64), String> {
  let ffmpeg = &settings.media.ffmpeg;
  let dir = expand(&settings.media.work_dir).join(format!("frames-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  let duration = probe_duration(settings, video)?;
  let from = start.max(0.0);
  let to = end.unwrap_or(duration).min(duration);
  let span = (to - from).max(0.001);
  // The reader is what bounds the count: frames are taken evenly across the span at the rate the count
  // allows, never above the caller's fps.
  let by_count = count as f64 / span;
  let rate = fps.min(by_count).max(0.01);
  let mut cmd = tokio::process::Command::new(ffmpeg);
  cmd
    .arg("-v")
    .arg("error")
    .arg("-ss")
    .arg(format!("{from:.3}"))
    .arg("-t")
    .arg(format!("{span:.3}"))
    .arg("-i")
    .arg(video)
    .arg("-vf")
    .arg(format!("fps={rate:.4},scale='min(1280,iw)':-2"))
    .arg("-q:v")
    .arg("3")
    .arg(dir.join("f%05d.jpg"));
  run(&mut cmd)?;
  let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
    .map_err(|e| e.to_string())?
    .filter_map(|e| e.ok().map(|e| e.path()))
    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jpg"))
    .collect();
  files.sort();
  files.truncate(count as usize);
  let mut parts = Vec::new();
  for f in &files {
    parts.push(ImagePart {
      bytes: std::fs::read(f).map_err(|e| e.to_string())?,
      media_type: "image/jpeg".into(),
    });
  }
  let effective = if parts.len() as f64 > 0.0 { parts.len() as f64 / span } else { 0.0 };
  let _ = std::fs::remove_dir_all(&dir);
  Ok((parts, effective))
}

pub fn probe_duration(settings: &Settings, video: &Path) -> Result<f64, String> {
  let ffprobe = settings.media.ffmpeg.replace("ffmpeg", "ffprobe");
  let mut cmd = tokio::process::Command::new(ffprobe);
  cmd
    .arg("-v")
    .arg("error")
    .arg("-show_entries")
    .arg("format=duration")
    .arg("-of")
    .arg("csv=p=0")
    .arg(video);
  let out = run(&mut cmd)?;
  String::from_utf8_lossy(&out.stdout)
    .trim()
    .parse::<f64>()
    .map_err(|e| format!("the video's length did not read: {e}"))
}

/// The video's audio, as a 16 kHz mono WAV the speech model reads.
pub fn audio(settings: &Settings, video: &Path) -> Result<PathBuf, String> {
  let dir = expand(&settings.media.work_dir);
  std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  let wav = dir.join(format!("audio-{}.wav", std::process::id()));
  let mut cmd = tokio::process::Command::new(&settings.media.ffmpeg);
  cmd
    .arg("-v")
    .arg("error")
    .arg("-y")
    .arg("-i")
    .arg(video)
    .arg("-vn")
    .arg("-ac")
    .arg("1")
    .arg("-ar")
    .arg("16000")
    .arg("-c:a")
    .arg("pcm_s16le")
    .arg(&wav);
  run(&mut cmd)?;
  Ok(wav)
}

/// Read a video's speech on the speech node's CPU through the jake hub (`docs/engines.md`).
///
/// The audio is carried to the node by the hub's own upload (a command line is no place for megabytes),
/// read by the whisper the node installed in stage 1, and the transcript is answered back on the exec's
/// stream.
pub async fn speech(settings: &Settings, wav: &Path, language: &str) -> Result<Speech, String> {
  if !settings.speech.enabled {
    return Err("speech is turned off in the settings".into());
  }
  if settings.speech.agent.trim().is_empty() {
    return Err("no speech node is named in the settings (speech.agent)".into());
  }
  let started = std::time::Instant::now();
  let stem = format!("bmo-eye-speech-{}", std::process::id());
  let remote = format!("/tmp/{stem}.wav");
  let out_json = format!("/tmp/{stem}.json");
  let token = hub_token(&settings.speech.hub_env)?;
  // 1. Carry the file.
  let console = expand(&settings.speech.console);
  let upload = tokio::process::Command::new(expand(&settings.speech.bun))
    .arg(&console)
    .arg("upload")
    .arg(wav)
    .arg(&remote)
    .arg("--agent")
    .arg(&settings.speech.agent)
    .output()
    .await
    .map_err(|e| format!("the hub's upload: {e}"))?;
  if !upload.status.success() {
    return Err(format!("the hub's upload: {}", String::from_utf8_lossy(&upload.stderr).trim()));
  }
  // 2. Read it there.
  let command = format!(
    "{} {} {} {} {} > /dev/null 2>&1; cat {}; rm -f {} {}",
    settings.speech.python, settings.speech.script, remote, out_json, language, out_json, remote, out_json
  );
  let body = serde_json::json!({"shell": "linux", "command": command});
  let resp = reqwest::Client::new()
    .post(format!("{}/exec?agent={}", settings.speech.hub, settings.speech.agent))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("the hub: {e}"))?;
  if !resp.status().is_success() {
    return Err(format!("the hub answered {}: {}", resp.status(), resp.text().await.unwrap_or_default()));
  }
  let stream = resp.text().await.map_err(|e| e.to_string())?;
  let mut out = String::new();
  for line in stream.lines() {
    if let Some(data) = line.strip_prefix("data: ")
      && let Ok(ev) = serde_json::from_str::<serde_json::Value>(data)
      && let Some(chunk) = ev.get("chunk").and_then(|c| c.get("base64")).and_then(|b| b.as_str())
      && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(chunk)
    {
      out.push_str(&String::from_utf8_lossy(&bytes));
    }
  }
  let json: serde_json::Value = serde_json::from_str(out.trim()).map_err(|e| {
    format!("the transcript did not read: {e}: {}", out.chars().take(200).collect::<String>())
  })?;
  let text = json.get("text").and_then(|t| t.as_str()).unwrap_or_default().to_string();
  if text.is_empty() {
    return Err("the speech model answered no words".into());
  }
  Ok(Speech {
    text,
    language: json.get("language").and_then(|l| l.as_str()).unwrap_or(language).to_string(),
    model: settings.speech.model.clone(),
    ms: started.elapsed().as_millis() as i64,
  })
}

fn hub_token(env_file: &str) -> Result<String, String> {
  let path = expand(env_file);
  let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
  for line in text.lines() {
    if let Some(v) = line.strip_prefix("JAKE_CONTROL_TOKEN=") {
      return Ok(v.trim().to_string());
    }
  }
  Err(format!("{}: no JAKE_CONTROL_TOKEN", path.display()))
}

/// One page of a PDF as a picture.
pub fn pdf_pages(settings: &Settings, pdf: &Path, pages: Option<&[i64]>) -> Result<Vec<ImagePart>, String> {
  let dir = expand(&settings.media.work_dir).join(format!("pdf-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  let first = pages.map(|p| p.iter().copied().min().unwrap_or(1)).unwrap_or(1);
  let last = pages.map(|p| p.iter().copied().max().unwrap_or(0)).unwrap_or(0);
  let mut cmd = tokio::process::Command::new("pdftoppm");
  cmd.arg("-r").arg("150").arg("-png").arg("-f").arg(first.max(1).to_string());
  if last > 0 {
    cmd.arg("-l").arg(last.to_string());
  }
  cmd.arg(pdf).arg(dir.join("p"));
  run(&mut cmd).map_err(|e| format!("pdftoppm: {e}"))?;
  let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
    .map_err(|e| e.to_string())?
    .filter_map(|e| e.ok().map(|e| e.path()))
    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
    .collect();
  files.sort();
  let mut parts = Vec::new();
  for f in &files {
    parts.push(ImagePart {
      bytes: std::fs::read(f).map_err(|e| e.to_string())?,
      media_type: "image/png".into(),
    });
  }
  let _ = std::fs::remove_dir_all(&dir);
  if parts.is_empty() {
    return Err("the PDF held no page".into());
  }
  Ok(parts)
}

/// The message a rung is sent: the prompt and the pictures.
pub fn message(material: &Material) -> serde_json::Value {
  let mut content = vec![serde_json::json!({"type": "text", "text": material.text})];
  for image in &material.images {
    content.push(serde_json::json!({"type": "image_url", "image_url": {"url": data_url(image)}}));
  }
  serde_json::json!([{"role": "user", "content": content}])
}
