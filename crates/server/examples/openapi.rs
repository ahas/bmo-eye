//! Prints the service's OpenAPI document as the repository keeps it in `docs/openapi.json`.

fn main() {
  print!("{}", eye_server::openapi::openapi_json());
}
