use crate::error::{Result, RuntimeError};
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "abs" => stdlib_abs(args),
        "min" => stdlib_min(args),
        "max" => stdlib_max(args),
        "clamp" => stdlib_clamp(args),
        "round" => stdlib_round(args),
        "floor" => stdlib_floor(args),
        "ceil" => stdlib_ceil(args),
        "pow" => stdlib_pow(args),
        "sqrt" => stdlib_sqrt(args),
        "random" => stdlib_random(),
        "random_int" => stdlib_random_int(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::math::{}",
            name
        ))),
    }
}

fn expect_numeric(args: &[Value], idx: usize, fn_name: &str) -> Result<Value> {
    args.get(idx).cloned().ok_or_else(|| {
        RuntimeError::TypeError(format!("std::math::{} missing argument {}", fn_name, idx))
    })
}

fn to_f64(v: &Value, fn_name: &str) -> Result<f64> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(RuntimeError::TypeError(format!(
            "std::math::{} expected numeric, got {}",
            fn_name,
            v.type_name()
        ))),
    }
}

fn to_i64(v: &Value, fn_name: &str) -> Result<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Float(f) => Ok(*f as i64),
        _ => Err(RuntimeError::TypeError(format!(
            "std::math::{} expected numeric, got {}",
            fn_name,
            v.type_name()
        ))),
    }
}

fn stdlib_abs(args: Vec<Value>) -> Result<Value> {
    let v = expect_numeric(&args, 0, "abs")?;
    match v {
        Value::Int(n) => Ok(Value::Int(n.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        _ => Err(RuntimeError::TypeError(format!(
            "std::math::abs expected numeric, got {}",
            v.type_name()
        ))),
    }
}

fn stdlib_min(args: Vec<Value>) -> Result<Value> {
    let a = expect_numeric(&args, 0, "min")?;
    let b = expect_numeric(&args, 1, "min")?;
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x.min(y))),
        _ => {
            let fa = to_f64(&a, "min")?;
            let fb = to_f64(&b, "min")?;
            Ok(Value::Float(fa.min(fb)))
        }
    }
}

fn stdlib_max(args: Vec<Value>) -> Result<Value> {
    let a = expect_numeric(&args, 0, "max")?;
    let b = expect_numeric(&args, 1, "max")?;
    match (&a, &b) {
        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x.max(y))),
        _ => {
            let fa = to_f64(&a, "max")?;
            let fb = to_f64(&b, "max")?;
            Ok(Value::Float(fa.max(fb)))
        }
    }
}

fn stdlib_clamp(args: Vec<Value>) -> Result<Value> {
    let x = expect_numeric(&args, 0, "clamp")?;
    let lo = expect_numeric(&args, 1, "clamp")?;
    let hi = expect_numeric(&args, 2, "clamp")?;
    match (&x, &lo, &hi) {
        (Value::Int(v), Value::Int(min), Value::Int(max)) => Ok(Value::Int(*v.max(min).min(max))),
        _ => {
            let fv = to_f64(&x, "clamp")?;
            let fmin = to_f64(&lo, "clamp")?;
            let fmax = to_f64(&hi, "clamp")?;
            Ok(Value::Float(fv.max(fmin).min(fmax)))
        }
    }
}

fn stdlib_round(args: Vec<Value>) -> Result<Value> {
    let v = expect_numeric(&args, 0, "round")?;
    let f = to_f64(&v, "round")?;
    Ok(Value::Int(f.round() as i64))
}

fn stdlib_floor(args: Vec<Value>) -> Result<Value> {
    let v = expect_numeric(&args, 0, "floor")?;
    let f = to_f64(&v, "floor")?;
    Ok(Value::Int(f.floor() as i64))
}

fn stdlib_ceil(args: Vec<Value>) -> Result<Value> {
    let v = expect_numeric(&args, 0, "ceil")?;
    let f = to_f64(&v, "ceil")?;
    Ok(Value::Int(f.ceil() as i64))
}

fn stdlib_pow(args: Vec<Value>) -> Result<Value> {
    let base = expect_numeric(&args, 0, "pow")?;
    let exp = expect_numeric(&args, 1, "pow")?;
    match (&base, &exp) {
        (Value::Int(b), Value::Int(e)) => {
            if *e >= 0 {
                Ok(Value::Int(b.wrapping_pow(*e as u32)))
            } else {
                Ok(Value::Float((*b as f64).powf(*e as f64)))
            }
        }
        _ => {
            let fb = to_f64(&base, "pow")?;
            let fe = to_f64(&exp, "pow")?;
            Ok(Value::Float(fb.powf(fe)))
        }
    }
}

fn stdlib_sqrt(args: Vec<Value>) -> Result<Value> {
    let v = expect_numeric(&args, 0, "sqrt")?;
    let f = to_f64(&v, "sqrt")?;
    Ok(Value::Float(f.sqrt()))
}

/// Simple pseudo-random using SystemTime. Not cryptographic.
fn simple_random_f64() -> f64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    // Xorshift-style mixing
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    x ^= x >> 17;
    x ^= x << 31;
    x ^= x >> 8;
    (x as f64) / (u64::MAX as f64)
}

fn stdlib_random() -> Result<Value> {
    Ok(Value::Float(simple_random_f64()))
}

fn stdlib_random_int(args: Vec<Value>) -> Result<Value> {
    let min = to_i64(&expect_numeric(&args, 0, "random_int")?, "random_int")?;
    let max = to_i64(&expect_numeric(&args, 1, "random_int")?, "random_int")?;
    if min >= max {
        return Err(RuntimeError::CallError(
            "std::math::random_int: min must be less than max".to_string(),
        ));
    }
    let range = (max - min) as f64;
    let result = min + (simple_random_f64() * range) as i64;
    Ok(Value::Int(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abs_int() {
        assert_eq!(call("abs", vec![Value::Int(-42)]).unwrap(), Value::Int(42));
        assert_eq!(call("abs", vec![Value::Int(42)]).unwrap(), Value::Int(42));
    }

    #[test]
    fn abs_float() {
        assert_eq!(
            call("abs", vec![Value::Float(-3.14)]).unwrap(),
            Value::Float(3.14)
        );
    }

    #[test]
    fn min_int() {
        assert_eq!(
            call("min", vec![Value::Int(3), Value::Int(7)]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn max_int() {
        assert_eq!(
            call("max", vec![Value::Int(3), Value::Int(7)]).unwrap(),
            Value::Int(7)
        );
    }

    #[test]
    fn clamp_in_range() {
        assert_eq!(
            call(
                "clamp",
                vec![Value::Int(50), Value::Int(0), Value::Int(100)]
            )
            .unwrap(),
            Value::Int(50)
        );
    }

    #[test]
    fn clamp_below() {
        assert_eq!(
            call(
                "clamp",
                vec![Value::Int(-10), Value::Int(0), Value::Int(100)]
            )
            .unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn clamp_above() {
        assert_eq!(
            call(
                "clamp",
                vec![Value::Int(150), Value::Int(0), Value::Int(100)]
            )
            .unwrap(),
            Value::Int(100)
        );
    }

    #[test]
    fn round_value() {
        assert_eq!(
            call("round", vec![Value::Float(3.7)]).unwrap(),
            Value::Int(4)
        );
        assert_eq!(
            call("round", vec![Value::Float(3.2)]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn floor_value() {
        assert_eq!(
            call("floor", vec![Value::Float(3.7)]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn ceil_value() {
        assert_eq!(
            call("ceil", vec![Value::Float(3.2)]).unwrap(),
            Value::Int(4)
        );
    }

    #[test]
    fn pow_int() {
        assert_eq!(
            call("pow", vec![Value::Int(2), Value::Int(10)]).unwrap(),
            Value::Int(1024)
        );
    }

    #[test]
    fn sqrt_value() {
        assert_eq!(
            call("sqrt", vec![Value::Float(16.0)]).unwrap(),
            Value::Float(4.0)
        );
        assert_eq!(
            call("sqrt", vec![Value::Int(9)]).unwrap(),
            Value::Float(3.0)
        );
    }

    #[test]
    fn random_in_range() {
        let val = call("random", vec![]).unwrap();
        match val {
            Value::Float(f) => assert!((0.0..1.0).contains(&f)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn random_int_in_range() {
        let val = call("random_int", vec![Value::Int(1), Value::Int(100)]).unwrap();
        match val {
            Value::Int(n) => assert!((1..100).contains(&n)),
            _ => panic!("expected Int"),
        }
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
