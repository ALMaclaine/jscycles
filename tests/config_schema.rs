//! `jscycles.schema.json` must describe only keys the configuration parser
//! accepts, with values of the declared types, and its limit defaults must be
//! the built-in ones.
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "Schema fixtures are fixed; a malformed schema must fail the test"
)]

use serde_json::{Map, Value, json};

fn schema() -> Value {
    serde_json::from_str(include_str!("../jscycles.schema.json")).expect("schema is JSON")
}

/// A value of the schema's declared type for every property, recursively.
fn sample(node: &Value) -> Value {
    match node["type"].as_str() {
        Some("object") => {
            let mut out = Map::new();
            if let Some(properties) = node["properties"].as_object() {
                for (key, child) in properties {
                    let _ = out.insert(key.clone(), sample(child));
                }
            }
            if node["additionalProperties"].is_object() {
                let _ = out.insert("@scope/*".to_owned(), sample(&node["additionalProperties"]));
            }
            Value::Object(out)
        },
        Some("array") => json!(["ts"]),
        Some("boolean") => json!(true),
        Some("integer") => json!(5),
        Some("string") => json!("x"),
        other => panic!("unexpected schema type {other:?}"),
    }
}

#[test]
fn every_schema_key_is_accepted_by_the_parser() {
    let document = sample(&schema());
    let config = jscycles::Config::parse(&document.to_string())
        .unwrap_or_else(|error| panic!("{error}: {document}"));
    assert!(config.defaults.skip_type_imports);
}

#[test]
fn schema_objects_are_closed_like_the_parser() {
    let schema = schema();
    for path in [
        "",
        "/properties/scan",
        "/properties/defaults",
        "/properties/limits",
        "/properties/packages/additionalProperties",
    ] {
        let node = schema.pointer(path).expect("schema node");
        assert_eq!(node["additionalProperties"], json!(false), "{path}");
        let mut document = sample(node);
        let _ = document
            .as_object_mut()
            .expect("object")
            .insert("unknown_key".to_owned(), json!(1));
        let wrapped = match path {
            "" => document,
            "/properties/packages/additionalProperties" => json!({"packages": {"x": document}}),
            _ => json!({ path.rsplit('/').next().expect("name"): document }),
        };
        assert!(
            jscycles::Config::parse(&wrapped.to_string()).is_err(),
            "{path} accepts unknown keys"
        );
    }
}

#[test]
fn schema_defaults_match_the_built_in_defaults() {
    let schema = schema();
    let defaults = jscycles::Config::default();
    let limits = &schema["properties"]["limits"]["properties"];
    let parsed: jscycles::Config = jscycles::Config::parse(
        &json!({"limits": limits.as_object().expect("limits").iter()
            .map(|(key, value)| (key.clone(), value["default"].clone())).collect::<Map<_, _>>()})
        .to_string(),
    )
    .expect("defaults parse");
    assert_eq!(
        format!("{:?}", parsed.limits),
        format!("{:?}", defaults.limits)
    );
    // Every limit field appears in the schema (Debug lists each `max_*` field).
    let fields = format!("{:?}", defaults.limits).matches("max_").count();
    assert_eq!(limits.as_object().expect("limits").len(), fields);
    assert_eq!(
        schema["properties"]["defaults"]["properties"]["extensions"]["default"],
        json!(jscycles::DEFAULT_EXTENSIONS)
    );
    assert_eq!(
        schema["properties"]["scan"]["properties"]["exclude"]["default"],
        json!(defaults.scan.exclude)
    );
}
