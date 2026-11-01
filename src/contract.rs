use crate::{Error, ErrorKind};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

static CONTRACT: OnceLock<Value> = OnceLock::new();
const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";
const ZERO_UUID_LOOKAHEAD: &str = "(?!00000000-0000-0000-0000-000000000000$)";

pub(crate) fn contract() -> &'static Value {
    CONTRACT.get_or_init(|| {
        serde_json::from_str(include_str!("contract_generated.json"))
            .expect("The generated SDK contract must contain valid JSON.")
    })
}

pub(crate) fn operation(name: &str) -> Result<&'static Value, Error> {
    contract()["operations"]
        .get(name)
        .ok_or_else(|| Error::validation("The SDK operation is not part of the public contract."))
}

pub(crate) fn validate_input(value: &Value, schema: &Value) -> Result<(), Error> {
    validate(value, schema, true, 0)
}

pub(crate) fn validate_response(value: &Value, schema: &Value) -> Result<(), Error> {
    validate(value, schema, false, 0)
}

fn invalid(input: bool) -> Error {
    if input {
        Error::validation("Request parameters do not match the published contract.")
    } else {
        Error::new(
            ErrorKind::UnexpectedResponse,
            "The API response does not match the published structure.",
        )
    }
}

fn validate(value: &Value, schema: &Value, input: bool, depth: usize) -> Result<(), Error> {
    let fail = || invalid(input);
    if depth > 100 {
        return Err(fail());
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let name = reference
            .strip_prefix("#/components/schemas/")
            .ok_or_else(fail)?;
        let target = contract()["schemas"].get(name).ok_or_else(fail)?;
        return validate(value, target, input, depth + 1);
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        let count = variants
            .iter()
            .filter(|s| validate(value, s, input, depth + 1).is_ok())
            .count();
        if count == 0 || (schema.get("oneOf").is_some() && count != 1) {
            return Err(fail());
        }
        return Ok(());
    }
    if let Some(parts) = schema.get("allOf").and_then(Value::as_array) {
        for part in parts {
            validate(value, part, input, depth + 1)?;
        }
    }
    if schema.get("const").is_some_and(|v| v != value) {
        return Err(fail());
    }
    let kinds: Vec<&str> = match schema.get("type") {
        Some(Value::String(kind)) => vec![kind.as_str()],
        Some(Value::Array(kinds)) => kinds.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    };
    if value.is_null() {
        return if kinds.is_empty()
            || kinds.contains(&"null")
            || schema.get("nullable") == Some(&Value::Bool(true))
        {
            if input
                && schema
                    .get("enum")
                    .and_then(Value::as_array)
                    .is_some_and(|e| !e.contains(value))
            {
                Err(fail())
            } else {
                Ok(())
            }
        } else {
            Err(fail())
        };
    }
    let kind = kinds.iter().find(|k| **k != "null").copied();
    match kind {
        Some("object") => {
            let values = value.as_object().ok_or_else(fail)?;
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                if required
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|key| !values.contains_key(key))
                {
                    return Err(fail());
                }
            }
            let properties = schema.get("properties").and_then(Value::as_object);
            let additional = schema.get("additionalProperties");
            for (key, child) in values {
                if input {
                    if let Some(names) = schema.get("propertyNames") {
                        validate(&Value::String(key.clone()), names, true, depth + 1)?;
                    }
                }
                if let Some(child_schema) = properties.and_then(|p| p.get(key)) {
                    validate(child, child_schema, input, depth + 1)?;
                } else if let Some(child_schema @ Value::Object(_)) = additional {
                    validate(child, child_schema, input, depth + 1)?;
                } else if input && additional == Some(&Value::Bool(false)) {
                    return Err(fail());
                }
            }
            if input
                && (less(schema, "minProperties", values.len())
                    || greater(schema, "maxProperties", values.len()))
            {
                return Err(fail());
            }
        }
        Some("array") => {
            let values = value.as_array().ok_or_else(fail)?;
            for child in values {
                if let Some(items) = schema.get("items") {
                    validate(child, items, input, depth + 1)?;
                }
            }
            if input {
                if less(schema, "minItems", values.len())
                    || greater(schema, "maxItems", values.len())
                {
                    return Err(fail());
                }
                if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
                    for (index, item) in values.iter().enumerate() {
                        if values[index + 1..].contains(item) {
                            return Err(fail());
                        }
                    }
                }
            }
        }
        Some("integer") => {
            let number = value.as_i64().ok_or_else(fail)?;
            if input {
                if schema.get("format").and_then(Value::as_str) == Some("int32")
                    && i32::try_from(number).is_err()
                {
                    return Err(fail());
                }
                for key in ["minimum", "maximum"] {
                    if let Some(bound) = schema.get(key).and_then(Value::as_i64) {
                        if (key == "minimum" && number < bound)
                            || (key == "maximum" && number > bound)
                        {
                            return Err(fail());
                        }
                    }
                }
                if schema
                    .get("exclusiveMinimum")
                    .and_then(Value::as_i64)
                    .is_some_and(|n| number <= n)
                    || schema
                        .get("exclusiveMaximum")
                        .and_then(Value::as_i64)
                        .is_some_and(|n| number >= n)
                {
                    return Err(fail());
                }
            }
        }
        Some("number") => {
            let number = value.as_f64().filter(|n| n.is_finite()).ok_or_else(fail)?;
            if input
                && (schema
                    .get("minimum")
                    .and_then(Value::as_f64)
                    .is_some_and(|n| number < n)
                    || schema
                        .get("maximum")
                        .and_then(Value::as_f64)
                        .is_some_and(|n| number > n)
                    || schema
                        .get("exclusiveMinimum")
                        .and_then(Value::as_f64)
                        .is_some_and(|n| number <= n)
                    || schema
                        .get("exclusiveMaximum")
                        .and_then(Value::as_f64)
                        .is_some_and(|n| number >= n))
            {
                return Err(fail());
            }
        }
        Some("boolean") => {
            if !value.is_boolean() {
                return Err(fail());
            }
        }
        Some("string") => {
            let text = value.as_str().ok_or_else(fail)?;
            if input {
                if less(schema, "minLength", text.chars().count())
                    || greater(schema, "maxLength", text.chars().count())
                {
                    return Err(fail());
                }
                if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
                    if pattern.contains(ZERO_UUID_LOOKAHEAD) && text == ZERO_UUID {
                        return Err(fail());
                    }
                    let pattern = pattern.replace(ZERO_UUID_LOOKAHEAD, "");
                    if !Regex::new(&pattern).map_err(|_| fail())?.is_match(text) {
                        return Err(fail());
                    }
                }
                if schema.get("format").and_then(Value::as_str) == Some("uuid") && !valid_uuid(text)
                {
                    return Err(fail());
                }
                if schema.get("format").and_then(Value::as_str) == Some("uri")
                    && reqwest::Url::parse(text).is_err()
                {
                    return Err(fail());
                }
            }
        }
        Some("null") => return Err(fail()),
        None => {}
        _ => return Err(fail()),
    }
    if input {
        if schema
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|e| !e.contains(value))
        {
            return Err(fail());
        }
        if let Some(maximum) = schema.get("x-raims-max-utf8-bytes").and_then(Value::as_u64) {
            if serde_json::to_vec(value).map_err(|_| fail())?.len() as u64 > maximum {
                return Err(fail());
            }
        }
    }
    Ok(())
}

fn less(schema: &Value, key: &str, count: usize) -> bool {
    schema
        .get(key)
        .and_then(Value::as_u64)
        .is_some_and(|n| (count as u64) < n)
}

fn greater(schema: &Value, key: &str, count: usize) -> bool {
    schema
        .get(key)
        .and_then(Value::as_u64)
        .is_some_and(|n| count as u64 > n)
}

pub(crate) fn valid_uuid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(index, c)| {
            if [8, 13, 18, 23].contains(&index) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}
