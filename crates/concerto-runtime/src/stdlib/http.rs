use std::collections::HashMap;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "get" => stdlib_get(args),
        "post" => stdlib_post(args),
        "put" => stdlib_put(args),
        "delete" => stdlib_delete(args),
        "request" => stdlib_request(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::http::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::http::{} expected String at arg {}, got {}",
            fn_name, idx, other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::http::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn extract_headers(args: &[Value], idx: usize) -> Vec<(String, String)> {
    match args.get(idx) {
        Some(Value::Map(pairs)) => pairs
            .iter()
            .map(|(k, v)| (k.clone(), v.display_string()))
            .collect(),
        _ => vec![],
    }
}

fn response_to_value(
    result: std::result::Result<reqwest::blocking::Response, reqwest::Error>,
) -> Value {
    match result {
        Ok(resp) => {
            let status = resp.status().as_u16() as i64;
            let headers: Vec<(String, Value)> = resp
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        Value::String(v.to_str().unwrap_or("").to_string()),
                    )
                })
                .collect();
            let body = resp.text().unwrap_or_default();

            let mut fields = HashMap::new();
            fields.insert("status".to_string(), Value::Int(status));
            fields.insert("body".to_string(), Value::String(body));
            fields.insert("headers".to_string(), Value::Map(headers));

            Value::Result {
                is_ok: true,
                value: Box::new(Value::Struct {
                    type_name: "HttpResponse".to_string(),
                    fields,
                }),
            }
        }
        Err(e) => Value::Result {
            is_ok: false,
            value: Box::new(Value::String(e.to_string())),
        },
    }
}

fn apply_headers(
    builder: reqwest::blocking::RequestBuilder,
    headers: &[(String, String)],
) -> reqwest::blocking::RequestBuilder {
    let mut b = builder;
    for (k, v) in headers {
        b = b.header(k.as_str(), v.as_str());
    }
    b
}

fn stdlib_get(args: Vec<Value>) -> Result<Value> {
    let url = expect_string(&args, 0, "get")?;
    let headers = extract_headers(&args, 1);
    let client = reqwest::blocking::Client::new();
    let builder = client.get(&url);
    let builder = apply_headers(builder, &headers);
    Ok(response_to_value(builder.send()))
}

fn stdlib_post(args: Vec<Value>) -> Result<Value> {
    let url = expect_string(&args, 0, "post")?;
    let body = args.get(1).cloned().unwrap_or(Value::Nil);
    let headers = extract_headers(&args, 2);
    let client = reqwest::blocking::Client::new();
    let mut builder = client.post(&url);
    if body != Value::Nil {
        let json_body = body.to_json();
        builder = builder.json(&json_body);
    }
    builder = apply_headers(builder, &headers);
    Ok(response_to_value(builder.send()))
}

fn stdlib_put(args: Vec<Value>) -> Result<Value> {
    let url = expect_string(&args, 0, "put")?;
    let body = args.get(1).cloned().unwrap_or(Value::Nil);
    let headers = extract_headers(&args, 2);
    let client = reqwest::blocking::Client::new();
    let mut builder = client.put(&url);
    if body != Value::Nil {
        let json_body = body.to_json();
        builder = builder.json(&json_body);
    }
    builder = apply_headers(builder, &headers);
    Ok(response_to_value(builder.send()))
}

fn stdlib_delete(args: Vec<Value>) -> Result<Value> {
    let url = expect_string(&args, 0, "delete")?;
    let headers = extract_headers(&args, 1);
    let client = reqwest::blocking::Client::new();
    let builder = client.delete(&url);
    let builder = apply_headers(builder, &headers);
    Ok(response_to_value(builder.send()))
}

fn stdlib_request(args: Vec<Value>) -> Result<Value> {
    let method = expect_string(&args, 0, "request")?;
    let url = expect_string(&args, 1, "request")?;
    let body = args.get(2).cloned().unwrap_or(Value::Nil);
    let headers = extract_headers(&args, 3);

    let client = reqwest::blocking::Client::new();
    let mut builder = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        "HEAD" => client.head(&url),
        _ => {
            return Err(RuntimeError::CallError(format!(
                "unsupported HTTP method: {}",
                method
            )))
        }
    };

    if body != Value::Nil {
        let json_body = body.to_json();
        builder = builder.json(&json_body);
    }
    builder = apply_headers(builder, &headers);
    Ok(response_to_value(builder.send()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_headers_from_map() {
        let headers = extract_headers(
            &[Value::Map(vec![
                ("Content-Type".into(), Value::String("application/json".into())),
                ("Authorization".into(), Value::String("Bearer token".into())),
            ])],
            0,
        );
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn extract_headers_empty() {
        let headers = extract_headers(&[], 0);
        assert!(headers.is_empty());
    }

    #[test]
    fn response_to_value_error() {
        // Test that a network error produces a Result Err
        let client = reqwest::blocking::Client::new();
        let result = client.get("http://localhost:1").send();
        let value = response_to_value(result);
        match value {
            Value::Result { is_ok: false, .. } => {}
            _ => panic!("expected Err for connection refused"),
        }
    }

    #[test]
    fn get_missing_url_error() {
        assert!(call("get", vec![]).is_err());
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }

    #[test]
    fn unsupported_method() {
        let result = call(
            "request",
            vec![Value::String("CONNECT".into()), Value::String("http://example.com".into())],
        );
        assert!(result.is_err());
    }
}
