//! The OpenAPI document (`docs/api.md`, the head): the copy kept in the repository is the one the code writes,
//! at the repository's version, and it states every route the server's router serves.

use eye_server::openapi::{openapi, openapi_json};
use serde_json::json;

#[test]
fn the_kept_document_is_the_one_the_code_writes() {
  let kept = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/openapi.json")).unwrap();

  assert!(
    kept == openapi_json(),
    "docs/openapi.json differs from what the code writes; write it again with \
     `cargo run -p eye-server --example openapi > docs/openapi.json`"
  );
}

#[test]
fn the_document_states_every_route_the_server_serves() {
  let document = openapi();

  assert_eq!(document["info"]["version"], env!("CARGO_PKG_VERSION"));
  assert_eq!(document["x-events"].as_array().unwrap().len(), 5);

  let paths = document["paths"].as_object().unwrap();

  assert_eq!(paths.len(), 8);
  assert_eq!(paths.values().map(|one| one.as_object().unwrap().len()).sum::<usize>(), 10,);

  for path in [
    "/health",
    "/looks",
    "/looks/{lookId}",
    "/uploads",
    "/uploads/{uploadId}",
    "/uploads/{uploadId}/content",
    "/modes",
    "/events",
  ] {
    assert!(
      !document["paths"][path].as_object().unwrap().is_empty(),
      "the document holds no `{path}`"
    );
  }

  assert_eq!(document["components"]["securitySchemes"]["bearer"]["type"], json!("http"));
}
