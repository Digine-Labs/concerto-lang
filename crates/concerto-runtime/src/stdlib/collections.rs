use std::collections::HashMap;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

/// Handle constructor calls like `std::collections::Set::new()`.
pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "Set::new" => Ok(make_set(vec![])),
        "Set::from" => {
            let elements = match args.into_iter().next() {
                Some(Value::Array(arr)) => deduplicate(arr),
                _ => vec![],
            };
            Ok(make_set(elements))
        }
        "Queue::new" => Ok(make_queue(vec![])),
        "Stack::new" => Ok(make_stack(vec![])),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::collections::{}",
            name
        ))),
    }
}

/// Handle method calls on Set/Queue/Stack struct values.
/// Called from exec_call_method when type_name matches.
pub fn call_collection_method(object: Value, method: &str, args: Vec<Value>) -> Result<Value> {
    let (type_name, fields) = match object {
        Value::Struct { type_name, fields } => (type_name, fields),
        _ => {
            return Err(RuntimeError::TypeError(
                "collection method called on non-struct".to_string(),
            ))
        }
    };

    let elements = match fields.get("elements") {
        Some(Value::Array(arr)) => arr.clone(),
        _ => vec![],
    };

    match type_name.as_str() {
        "Set" => call_set_method(elements, method, args),
        "Queue" => call_queue_method(elements, method, args),
        "Stack" => call_stack_method(elements, method, args),
        _ => Err(RuntimeError::TypeError(format!(
            "no method '{}' on {}",
            method, type_name
        ))),
    }
}

// --- Set ---

fn call_set_method(elements: Vec<Value>, method: &str, args: Vec<Value>) -> Result<Value> {
    match method {
        "insert" => {
            let val = args.into_iter().next().ok_or_else(|| {
                RuntimeError::TypeError("Set::insert missing argument".to_string())
            })?;
            let mut new_elements = elements;
            if !contains_value(&new_elements, &val) {
                new_elements.push(val);
            }
            Ok(make_set(new_elements))
        }
        "remove" => {
            let val = args.into_iter().next().ok_or_else(|| {
                RuntimeError::TypeError("Set::remove missing argument".to_string())
            })?;
            let new_elements: Vec<Value> = elements
                .into_iter()
                .filter(|e| !values_equal(e, &val))
                .collect();
            Ok(make_set(new_elements))
        }
        "contains" => {
            let val = args.first().ok_or_else(|| {
                RuntimeError::TypeError("Set::contains missing argument".to_string())
            })?;
            Ok(Value::Bool(contains_value(&elements, val)))
        }
        "len" => Ok(Value::Int(elements.len() as i64)),
        "is_empty" => Ok(Value::Bool(elements.is_empty())),
        "union" => {
            let other = extract_set_elements(&args)?;
            let mut result = elements;
            for item in other {
                if !contains_value(&result, &item) {
                    result.push(item);
                }
            }
            Ok(make_set(result))
        }
        "intersection" => {
            let other = extract_set_elements(&args)?;
            let result: Vec<Value> = elements
                .into_iter()
                .filter(|e| contains_value(&other, e))
                .collect();
            Ok(make_set(result))
        }
        "difference" => {
            let other = extract_set_elements(&args)?;
            let result: Vec<Value> = elements
                .into_iter()
                .filter(|e| !contains_value(&other, e))
                .collect();
            Ok(make_set(result))
        }
        _ => Err(RuntimeError::TypeError(format!(
            "no method '{}' on Set",
            method
        ))),
    }
}

// --- Queue ---

fn call_queue_method(elements: Vec<Value>, method: &str, args: Vec<Value>) -> Result<Value> {
    match method {
        "enqueue" => {
            let val = args.into_iter().next().ok_or_else(|| {
                RuntimeError::TypeError("Queue::enqueue missing argument".to_string())
            })?;
            let mut new_elements = elements;
            new_elements.push(val);
            Ok(make_queue(new_elements))
        }
        "dequeue" => {
            if elements.is_empty() {
                Ok(Value::Option(None))
            } else {
                Ok(Value::Option(Some(Box::new(elements[0].clone()))))
            }
        }
        "peek" => Ok(match elements.first() {
            Some(v) => Value::Option(Some(Box::new(v.clone()))),
            None => Value::Option(None),
        }),
        "len" => Ok(Value::Int(elements.len() as i64)),
        "is_empty" => Ok(Value::Bool(elements.is_empty())),
        "rest" => {
            // Return queue without the front element (for use after dequeue)
            if elements.is_empty() {
                Ok(make_queue(vec![]))
            } else {
                Ok(make_queue(elements[1..].to_vec()))
            }
        }
        _ => Err(RuntimeError::TypeError(format!(
            "no method '{}' on Queue",
            method
        ))),
    }
}

// --- Stack ---

fn call_stack_method(elements: Vec<Value>, method: &str, args: Vec<Value>) -> Result<Value> {
    match method {
        "push" => {
            let val = args.into_iter().next().ok_or_else(|| {
                RuntimeError::TypeError("Stack::push missing argument".to_string())
            })?;
            let mut new_elements = elements;
            new_elements.push(val);
            Ok(make_stack(new_elements))
        }
        "pop" => Ok(match elements.last() {
            Some(v) => Value::Option(Some(Box::new(v.clone()))),
            None => Value::Option(None),
        }),
        "peek" => Ok(match elements.last() {
            Some(v) => Value::Option(Some(Box::new(v.clone()))),
            None => Value::Option(None),
        }),
        "len" => Ok(Value::Int(elements.len() as i64)),
        "is_empty" => Ok(Value::Bool(elements.is_empty())),
        "rest" => {
            // Return stack without the top element (for use after pop)
            if elements.is_empty() {
                Ok(make_stack(vec![]))
            } else {
                Ok(make_stack(elements[..elements.len() - 1].to_vec()))
            }
        }
        _ => Err(RuntimeError::TypeError(format!(
            "no method '{}' on Stack",
            method
        ))),
    }
}

// --- Helpers ---

fn make_set(elements: Vec<Value>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("elements".to_string(), Value::Array(elements));
    Value::Struct {
        type_name: "Set".to_string(),
        fields,
    }
}

fn make_queue(elements: Vec<Value>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("elements".to_string(), Value::Array(elements));
    Value::Struct {
        type_name: "Queue".to_string(),
        fields,
    }
}

fn make_stack(elements: Vec<Value>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("elements".to_string(), Value::Array(elements));
    Value::Struct {
        type_name: "Stack".to_string(),
        fields,
    }
}

fn deduplicate(arr: Vec<Value>) -> Vec<Value> {
    let mut result = Vec::new();
    for item in arr {
        if !contains_value(&result, &item) {
            result.push(item);
        }
    }
    result
}

fn contains_value(arr: &[Value], val: &Value) -> bool {
    arr.iter().any(|e| values_equal(e, val))
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Nil, Value::Nil) => true,
        _ => false,
    }
}

fn extract_set_elements(args: &[Value]) -> Result<Vec<Value>> {
    match args.first() {
        Some(Value::Struct { type_name, fields }) if type_name == "Set" => {
            match fields.get("elements") {
                Some(Value::Array(arr)) => Ok(arr.clone()),
                _ => Ok(vec![]),
            }
        }
        _ => Err(RuntimeError::TypeError("expected Set argument".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_new() {
        let set = call("Set::new", vec![]).unwrap();
        match &set {
            Value::Struct { type_name, .. } => assert_eq!(type_name, "Set"),
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn set_insert() {
        let set = call("Set::new", vec![]).unwrap();
        let set = call_collection_method(set, "insert", vec![Value::String("a".into())]).unwrap();
        let set = call_collection_method(set, "insert", vec![Value::String("b".into())]).unwrap();
        assert_eq!(
            call_collection_method(set, "len", vec![]).unwrap(),
            Value::Int(2)
        );
    }

    #[test]
    fn set_insert_duplicate() {
        let set = call("Set::new", vec![]).unwrap();
        let set = call_collection_method(set, "insert", vec![Value::Int(1)]).unwrap();
        let set = call_collection_method(set, "insert", vec![Value::Int(1)]).unwrap();
        assert_eq!(
            call_collection_method(set, "len", vec![]).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn set_contains() {
        let set = call("Set::new", vec![]).unwrap();
        let set = call_collection_method(set, "insert", vec![Value::String("x".into())]).unwrap();
        assert_eq!(
            call_collection_method(set.clone(), "contains", vec![Value::String("x".into())])
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call_collection_method(set, "contains", vec![Value::String("y".into())]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn set_remove() {
        let set = call("Set::new", vec![]).unwrap();
        let set = call_collection_method(set, "insert", vec![Value::Int(1)]).unwrap();
        let set = call_collection_method(set, "insert", vec![Value::Int(2)]).unwrap();
        let set = call_collection_method(set, "remove", vec![Value::Int(1)]).unwrap();
        assert_eq!(
            call_collection_method(set, "len", vec![]).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn set_union() {
        let a = call("Set::new", vec![]).unwrap();
        let a = call_collection_method(a, "insert", vec![Value::Int(1)]).unwrap();
        let a = call_collection_method(a, "insert", vec![Value::Int(2)]).unwrap();
        let b = call("Set::new", vec![]).unwrap();
        let b = call_collection_method(b, "insert", vec![Value::Int(2)]).unwrap();
        let b = call_collection_method(b, "insert", vec![Value::Int(3)]).unwrap();
        let union = call_collection_method(a, "union", vec![b]).unwrap();
        assert_eq!(
            call_collection_method(union, "len", vec![]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn set_intersection() {
        let a = call("Set::new", vec![]).unwrap();
        let a = call_collection_method(a, "insert", vec![Value::Int(1)]).unwrap();
        let a = call_collection_method(a, "insert", vec![Value::Int(2)]).unwrap();
        let b = call("Set::new", vec![]).unwrap();
        let b = call_collection_method(b, "insert", vec![Value::Int(2)]).unwrap();
        let b = call_collection_method(b, "insert", vec![Value::Int(3)]).unwrap();
        let inter = call_collection_method(a, "intersection", vec![b]).unwrap();
        assert_eq!(
            call_collection_method(inter, "len", vec![]).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn queue_enqueue_peek_dequeue() {
        let q = call("Queue::new", vec![]).unwrap();
        let q = call_collection_method(q, "enqueue", vec![Value::String("first".into())]).unwrap();
        let q = call_collection_method(q, "enqueue", vec![Value::String("second".into())]).unwrap();
        let peek = call_collection_method(q.clone(), "peek", vec![]).unwrap();
        assert_eq!(
            peek,
            Value::Option(Some(Box::new(Value::String("first".into()))))
        );
        let dequeued = call_collection_method(q.clone(), "dequeue", vec![]).unwrap();
        assert_eq!(
            dequeued,
            Value::Option(Some(Box::new(Value::String("first".into()))))
        );
        let q = call_collection_method(q, "rest", vec![]).unwrap();
        assert_eq!(
            call_collection_method(q, "len", vec![]).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn stack_push_peek_pop() {
        let s = call("Stack::new", vec![]).unwrap();
        let s = call_collection_method(s, "push", vec![Value::Int(1)]).unwrap();
        let s = call_collection_method(s, "push", vec![Value::Int(2)]).unwrap();
        let peek = call_collection_method(s.clone(), "peek", vec![]).unwrap();
        assert_eq!(peek, Value::Option(Some(Box::new(Value::Int(2)))));
        let popped = call_collection_method(s.clone(), "pop", vec![]).unwrap();
        assert_eq!(popped, Value::Option(Some(Box::new(Value::Int(2)))));
        let s = call_collection_method(s, "rest", vec![]).unwrap();
        assert_eq!(
            call_collection_method(s, "len", vec![]).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn queue_empty_dequeue() {
        let q = call("Queue::new", vec![]).unwrap();
        assert_eq!(
            call_collection_method(q, "dequeue", vec![]).unwrap(),
            Value::Option(None)
        );
    }

    #[test]
    fn stack_empty_pop() {
        let s = call("Stack::new", vec![]).unwrap();
        assert_eq!(
            call_collection_method(s, "pop", vec![]).unwrap(),
            Value::Option(None)
        );
    }

    #[test]
    fn unknown_constructor() {
        assert!(call("Deque::new", vec![]).is_err());
    }
}
