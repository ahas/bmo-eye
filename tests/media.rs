//! The source-address rules of `media`: which address is a video page, which media type a look can
//! read, and which type an address declares (`docs/modes.md`, 「watch: video」).

use eye_server::media::{declared_media, is_readable_media, is_video_page};

#[test]
fn a_page_on_a_known_video_host_is_a_video_page() {
  for url in [
    "https://www.instagram.com/reel/CabcDEF1234/",
    "https://m.facebook.com/story.php?video_id=1",
    "https://x.com/someone/status/1234567890",
    "https://twitter.com/someone/status/1234567890",
    "https://vt.tiktok.com/ZSaBcDe/",
    "https://vimeo.com/123456789",
    "https://www.threads.net/@someone/post/CabcDEF1234",
  ] {
    assert!(is_video_page(url), "{url} is a page on a known video host");
  }
}

#[test]
fn a_host_that_only_holds_a_known_name_in_its_text_is_not_a_video_page() {
  for url in [
    "https://notinstagram.com/reel/x",
    "https://instagram.com.evil.test/x",
    "https://evil.test/x.com",
    "https://plain.example.test/clip.mp4",
  ] {
    assert!(!is_video_page(url), "{url} is not a page on a known video host");
  }
}

#[test]
fn a_media_type_a_look_can_read() {
  for media in ["video/mp4", "video/webm", "audio/wav", "image/png", "image/jpeg", "application/pdf"] {
    assert!(is_readable_media(media), "a look reads {media}");
  }
  for media in ["text/html", "application/octet-stream", "application/json", ""] {
    assert!(!is_readable_media(media), "a look does not read {media}");
  }
}

#[test]
fn an_address_declares_its_header_type_and_else_its_extension_type() {
  assert_eq!(declared_media(Some("video/mp4"), "https://host.test/x"), "video/mp4");
  assert_eq!(declared_media(Some("video/webm; codecs=vp9"), "https://host.test/x"), "video/webm");
  assert_eq!(
    declared_media(Some("application/octet-stream"), "https://host.test/clip.mp4"),
    "video/mp4"
  );
  assert_eq!(declared_media(None, "https://host.test/clip.mp4"), "video/mp4");
  assert_eq!(
    declared_media(Some("application/octet-stream"), "https://host.test/blob"),
    "application/octet-stream"
  );
  assert_eq!(declared_media(None, "https://host.test/blob"), "application/octet-stream");
}
