//! The service's OpenAPI 3.1 document: every route with its parameters, its body, its answers and the codes
//! it can answer, the bodies' schemas written from the types the routes take and answer, and the events the
//! service writes. The repository keeps it as `docs/openapi.json`, and a test holds the kept document to this
//! one; a change to a route or a body writes it again with
//! `cargo run -p eye-server --example openapi > docs/openapi.json`.

use schemars::JsonSchema;
use schemars::generate::{SchemaGenerator, SchemaSettings};
use serde_json::{Map, Value, json};

use crate::routes::{EventQuery, ListQuery, LookQuery};
use bmo_eye_client::{Failure, Health, Look, LookPage, ModeInfo, NewLook, Upload};

/// Where the schemas the routes refer to stand in the document.
const SCHEMAS: &str = "/components/schemas";

/// The schema generator the document's types are written with, keeping its definitions under `SCHEMAS`.
fn generator(definitions_path: &'static str) -> SchemaGenerator {
  SchemaSettings::draft2020_12()
    .for_deserialize()
    .with(|settings| {
      settings.definitions_path = definitions_path.into();
      settings.meta_schema = None;
    })
    .into_generator()
}

const DESCRIPTION: &str = "The vision unit's server: one request and one answer shape over three modes (see, watch, \
  read), each with its own chain of backends. A look is a question put to a set of sources; the server holds no \
  model and no card, asks its chain over the Lab-arena engines and the hosted backends, and records in the look what \
  every rung of that chain did. Bodies are JSON with camelCase keys; instants are integers of epoch milliseconds \
  ending in `At` and spans are milliseconds ending in `Ms`. A list is a page of `items` with a `more` flag, the \
  next page asked with the last item's `id` as `after`. Every failure is a non-2xx status in the common error \
  shape with a `code` and an `error` sentence; the codes a route can answer stand under `x-codes`. A look that \
  could not be answered is not a failure of the route: it is a member whose `state` is `failed` and whose `error` \
  says why. The events the service writes stand under `x-events`.";

/// Each code a route can answer: its status and what it means (`docs/api.md`, 「Errors of the routes」).
const CODES: &[(&str, u16, &str)] = &[
  (
    "bad-request",
    400,
    "the body or query is not the shape the route takes, a mode is unknown, or an option is not the one the mode takes",
  ),
  ("unauthorized", 401, "the bearer token is missing or wrong"),
  ("not-found", 404, "no look or upload of that id"),
  ("no-route", 404, "no route answers this method and path"),
  ("too-large", 413, "an upload over the ceiling, or more sources than the route takes"),
  ("unsupported-source", 422, "the mode takes no such source"),
  ("failed", 500, "a store call behind the route failed; the sentence is the store's"),
  (
    "not-ready",
    503,
    "the server is starting and cannot take a look yet; `Retry-After` says when to ask again",
  ),
];

/// The event kinds the service writes (`docs/api.md`, 「Events」).
const EVENTS: &[&str] = &["look.created", "look.running", "look.done", "look.failed", "look.stopped"];

/// One route as the document states it.
struct Route {
  method: &'static str,
  path: &'static str,
  id: &'static str,
  summary: &'static str,
  parameters: Vec<Value>,
  /// The body's content, by its media type, none for a route that takes none.
  body: Option<Value>,
  /// The success's status and content, none for an answer with no body.
  answer: (u16, Option<Value>),
  /// The codes of its own, beside the token's and those every route can answer.
  codes: Vec<&'static str>,
}

impl Route {
  fn operation(self) -> Value {
    let mut codes: Vec<&'static str> = vec!["unauthorized"];

    if !self.parameters.is_empty() || self.body.is_some() {
      codes.push("bad-request");
    }

    let own: Vec<&'static str> = self.codes.into_iter().filter(|code| !codes.contains(code)).collect();

    codes.extend(own);

    let (status, content) = self.answer;
    let mut success = json!({ "description": self.summary });

    if let Some(content) = content {
      success["content"] = content;
    }

    let mut responses = Map::new();
    responses.insert(status.to_string(), success);
    responses.insert("default".into(), json!({ "$ref": "#/components/responses/Error" }));

    let mut operation = json!({
      "operationId": self.id,
      "tags": [tag_of(self.path)],
      "summary": self.summary,
      "responses": responses,
      "x-codes": codes,
    });

    if !self.parameters.is_empty() {
      operation["parameters"] = self.parameters.into();
    }

    if let Some(body) = self.body {
      operation["requestBody"] = json!({ "required": true, "content": body });
    }

    operation
  }
}

// The tag a route stands under: the area its path opens on.
fn tag_of(path: &str) -> &str {
  let area = &path[1..];
  let brace = area.find('{').unwrap_or(area.len());
  let end = area.find('/').unwrap_or(area.len());
  let end = if brace < end { brace } else { end };

  &area[..end]
}

fn json_of<T: JsonSchema>(generator: &mut SchemaGenerator) -> Value {
  json!({ "application/json": { "schema": generator.subschema_for::<T>() } })
}

fn schema_of<T: JsonSchema>(generator: &mut SchemaGenerator) -> Value {
  generator.subschema_for::<T>().to_value()
}

/// A path or header parameter the routes carry by hand: a string the caller names.
fn key(name: &str, description: &str, place: &str) -> Value {
  json!({ "name": name, "in": place, "required": true, "description": description, "schema": { "type": "string" } })
}

/// The query parameters of a query type, each property one parameter.
fn query<Query: JsonSchema>(generator: &mut SchemaGenerator) -> Vec<Value> {
  let schema = Query::json_schema(generator).to_value();
  let Some(properties) = schema["properties"].as_object() else {
    return Vec::new();
  };

  properties
    .iter()
    .map(|(name, property)| {
      let mut parameter = json!({ "name": name, "in": "query", "required": false, "schema": property });

      if let Some(description) = property.get("description") {
        parameter["description"] = description.clone();
      }

      parameter
    })
    .collect()
}

/// Raw bytes in a body or an answer, named by the media type it is read as.
fn bytes(description: &str) -> Value {
  json!({
    "application/octet-stream": {
      "schema": {
        "type": "string",
        "format": "binary",
        "description": description,
      },
    },
  })
}

/// Every route of the server, each as the document states it.
fn routes(generator: &mut SchemaGenerator) -> Vec<Route> {
  vec![
    Route {
      method: "get",
      path: "/health",
      id: "health",
      summary: "The service's health: whether it can take a look, and the version it runs.",
      parameters: Vec::new(),
      body: None,
      answer: (200, Some(json_of::<Health>(generator))),
      codes: Vec::new(),
    },
    Route {
      method: "post",
      path: "/looks",
      id: "createLook",
      summary: "Take a look in: one mode, one question, one set of sources. The look is created either way and runs to its end whatever the hold; `waitMs` says how long the route holds the answer open.",
      parameters: query::<LookQuery>(generator),
      body: Some(json_of::<NewLook>(generator)),
      answer: (201, Some(json_of::<Look>(generator))),
      codes: vec!["not-found", "too-large", "unsupported-source", "not-ready", "failed"],
    },
    Route {
      method: "get",
      path: "/looks",
      id: "listLooks",
      summary: "The looks, newest first, a page at a time; narrowed by mode and state.",
      parameters: query::<ListQuery>(generator),
      body: None,
      answer: (200, Some(json_of::<LookPage>(generator))),
      codes: vec!["failed"],
    },
    Route {
      method: "get",
      path: "/looks/{lookId}",
      id: "getLook",
      summary: "One look, as its record stands now.",
      parameters: vec![key("lookId", "The look's id.", "path")],
      body: None,
      answer: (200, Some(json_of::<Look>(generator))),
      codes: vec!["not-found", "no-route"],
    },
    Route {
      method: "delete",
      path: "/looks/{lookId}",
      id: "stopLook",
      summary: "Stop a running look, or let go of one that ended; the record stays.",
      parameters: vec![key("lookId", "The look's id.", "path")],
      body: None,
      answer: (204, None),
      codes: vec!["not-found", "no-route"],
    },
    Route {
      method: "post",
      path: "/uploads",
      id: "createUpload",
      summary: "Send the bytes here first and name the upload in the look: the body is the bytes, `Content-Type` names the media type the mode reads.",
      parameters: Vec::new(),
      body: Some(bytes("The bytes of a picture, a document or a video, as a look will name them.")),
      answer: (201, Some(json_of::<Upload>(generator))),
      codes: vec!["too-large", "unsupported-source", "failed"],
    },
    Route {
      method: "get",
      path: "/uploads/{uploadId}/content",
      id: "uploadContent",
      summary: "The bytes of an upload, answered in the media type it was sent as; a `Range` header answers 206 with that span.",
      parameters: vec![
        key("uploadId", "The upload's id.", "path"),
        json!({
          "name": "Range",
          "in": "header",
          "required": false,
          "description": "The span of the bytes wanted.",
          "schema": { "type": "string" },
        }),
      ],
      body: None,
      answer: (200, Some(bytes("The upload's bytes, in the media type the upload was sent as."))),
      codes: vec!["not-found", "no-route"],
    },
    Route {
      method: "delete",
      path: "/uploads/{uploadId}",
      id: "deleteUpload",
      summary: "Let go of an upload the caller no longer needs.",
      parameters: vec![key("uploadId", "The upload's id.", "path")],
      body: None,
      answer: (204, None),
      codes: vec!["not-found", "no-route", "failed"],
    },
    Route {
      method: "get",
      path: "/modes",
      id: "getModes",
      summary: "The three modes as they stand, each with the chain it asks in order and every backend's state.",
      parameters: Vec::new(),
      body: None,
      answer: (
        200,
        Some(json!({
          "application/json": {
            "schema": {
              "type": "array",
              "items": schema_of::<ModeInfo>(generator),
            },
          },
        })),
      ),
      codes: Vec::new(),
    },
    Route {
      method: "get",
      path: "/events",
      id: "events",
      summary: "The service's one stream: each event names a look and its change.",
      parameters: query::<EventQuery>(generator),
      body: None,
      answer: (
        200,
        Some(json!({
          "text/event-stream": {
            "schema": {
              "type": "string",
              "description": "Server-sent events: each event's `event` names a look and its change, its `id` is the service's event number and its `data` is `{\"id\", \"kind\", \"at\", \"data\"}`.",
            },
          },
        })),
      ),
      codes: Vec::new(),
    },
  ]
}

/// The OpenAPI document.
pub fn openapi() -> Value {
  let mut generator = generator(SCHEMAS);
  let mut paths = Map::new();

  for route in routes(&mut generator) {
    let (path, method) = (route.path, route.method);

    paths.entry(path).or_insert_with(|| json!({}))[method] = route.operation();
  }

  let error = generator.subschema_for::<Failure>();
  let codes: Vec<String> = CODES
    .iter()
    .map(|(code, status, meaning)| format!("- `{code}` ({status}): {meaning}"))
    .collect();

  json!({
    "openapi": "3.1.0",
    "info": { "title": "bmo-eye", "version": env!("CARGO_PKG_VERSION"), "description": DESCRIPTION },
    "servers": [{ "url": "http://127.0.0.1:7435", "description": "the vision unit's server on the operations machine" }],
    "security": [{ "bearer": [] }],
    "tags": [{ "name": "health" }, { "name": "looks" }, { "name": "uploads" }, { "name": "modes" }, { "name": "events" }],
    "paths": paths,
    "components": {
      "schemas": generator.take_definitions(true),
      "responses": {
        "Error": {
          "description": format!("A failure, in the common error shape. The codes:\n{}", codes.join("\n")),
          "content": { "application/json": { "schema": error } },
        },
      },
      "securitySchemes": {
        "bearer": {
          "type": "http",
          "scheme": "bearer",
          "description": "The service's token, from the caller's own configuration; every route takes a request only with it.",
        },
      },
    },
    "x-events": EVENTS,
  })
}

/// The document as the repository keeps it: two-space indented JSON with a closing newline.
pub fn openapi_json() -> String {
  let mut text = serde_json::to_string_pretty(&openapi()).unwrap_or_default();

  text.push('\n');
  text
}
