use kairos_script::{AssignOp, BinOp, Expr, UnOp, Value};

pub trait Scope {
    fn lookup(&self, name: &str) -> Option<&Value>;
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::None => false,
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Float(n) => *n != 0.0,
        Value::Str(s) => !s.is_empty(),
    }
}

pub fn display(v: &Value) -> String {
    match v {
        Value::None => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Int(n) => n.to_string(),
        Value::Float(n) if n.fract() == 0.0 && n.is_finite() => format!("{n:.1}"),
        Value::Float(n) => n.to_string(),
        Value::Str(s) => s.clone(),
    }
}

pub fn eval(expr: &Expr, scope: &impl Scope) -> Result<Value, String> {
    match expr {
        Expr::Lit(v) => Ok(v.clone()),
        Expr::Var(name) => scope.lookup(name).cloned().ok_or_else(|| format!("`{name}` is not defined")),
        Expr::Unary(UnOp::Not, e) => Ok(Value::Bool(!truthy(&eval(e, scope)?))),
        Expr::Unary(UnOp::Neg, e) => match eval(e, scope)? {
            Value::Int(n) => n.checked_neg().map(Value::Int).ok_or_else(|| "integer overflow".into()),
            Value::Float(n) => Ok(Value::Float(-n)),
            v => Err(format!("cannot negate {}", type_name(&v))),
        },
        Expr::Binary(BinOp::And, a, b) => {
            let a = eval(a, scope)?;
            if truthy(&a) { eval(b, scope) } else { Ok(a) }
        }
        Expr::Binary(BinOp::Or, a, b) => {
            let a = eval(a, scope)?;
            if truthy(&a) { Ok(a) } else { eval(b, scope) }
        }
        Expr::Binary(op, a, b) => binary(*op, &eval(a, scope)?, &eval(b, scope)?),
    }
}

pub fn assign(op: AssignOp, current: Option<&Value>, rhs: Value) -> Result<Value, String> {
    let bin = match op {
        AssignOp::Set => return Ok(rhs),
        AssignOp::Add => BinOp::Add,
        AssignOp::Sub => BinOp::Sub,
        AssignOp::Mul => BinOp::Mul,
        AssignOp::Div => BinOp::Div,
    };
    let current = current.ok_or("cannot update a variable that is not defined")?;
    binary(bin, current, &rhs)
}

fn binary(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    use Value::{Bool, Float, Int, Str};
    let num = |v: &Value| match v {
        Int(n) => Some(*n as f64),
        Float(n) => Some(*n),
        Bool(b) => Some(*b as i64 as f64),
        _ => None,
    };
    let int = |v: &Value| match v {
        Int(n) => Some(*n),
        Bool(b) => Some(*b as i64),
        _ => None,
    };
    let overflow = || "integer overflow".to_string();
    let mismatch = || format!("cannot apply {op:?} to {} and {}", type_name(a), type_name(b));
    match op {
        BinOp::Eq => Ok(Bool(equal(a, b))),
        BinOp::Ne => Ok(Bool(!equal(a, b))),
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            let ord = match (a, b) {
                (Str(x), Str(y)) => x.partial_cmp(y),
                _ => num(a).zip(num(b)).and_then(|(x, y)| x.partial_cmp(&y)),
            }
            .ok_or_else(mismatch)?;
            Ok(Bool(match op {
                BinOp::Lt => ord.is_lt(),
                BinOp::Le => ord.is_le(),
                BinOp::Gt => ord.is_gt(),
                _ => ord.is_ge(),
            }))
        }
        BinOp::Add => match (a, b) {
            (Str(x), Str(y)) => Ok(Str(format!("{x}{y}"))),
            _ => arith(a, b, i64::checked_add, |x, y| x + y),
        },
        BinOp::Sub => arith(a, b, i64::checked_sub, |x, y| x - y),
        BinOp::Mul => arith(a, b, i64::checked_mul, |x, y| x * y),
        BinOp::Div => {
            let (x, y) = num(a).zip(num(b)).ok_or_else(mismatch)?;
            if y == 0.0 { Err("division by zero".into()) } else { Ok(Float(x / y)) }
        }
        BinOp::Rem => {
            let (x, y) = int(a).zip(int(b)).ok_or_else(mismatch)?;
            if y == 0 { Err("division by zero".into()) } else { x.checked_rem_euclid(y).map(Int).ok_or_else(overflow) }
        }
        BinOp::And | BinOp::Or => unreachable!("short-circuit ops are handled in eval"),
    }
}

fn arith(
    a: &Value,
    b: &Value,
    int_op: fn(i64, i64) -> Option<i64>,
    float_op: fn(f64, f64) -> f64,
) -> Result<Value, String> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => int_op(*x, *y).map(Value::Int).ok_or_else(|| "integer overflow".into()),
        (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
            let f = |v: &Value| {
                if let Value::Int(n) = v {
                    *n as f64
                } else if let Value::Float(n) = v {
                    *n
                } else {
                    0.0
                }
            };
            Ok(Value::Float(float_op(f(a), f(b))))
        }
        _ => Err(format!("cannot do arithmetic on {} and {}", type_name(a), type_name(b))),
    }
}

fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => *x as f64 == *y,
        _ => a == b,
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::None => "None",
        Value::Bool(_) => "a boolean",
        Value::Int(_) => "an integer",
        Value::Float(_) => "a float",
        Value::Str(_) => "a string",
    }
}

/// Replaces `[name]` with the variable's value; `[[` is a literal `[`.
pub fn interpolate(text: &str, scope: &impl Scope) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        if let Some(r) = rest.strip_prefix('[') {
            out.push('[');
            rest = r;
            continue;
        }
        let end = rest.find(']').ok_or("unclosed `[` in text")?;
        let name = rest[..end].trim();
        let v = scope.lookup(name).ok_or_else(|| format!("`{name}` is not defined"))?;
        out.push_str(&display(v));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
