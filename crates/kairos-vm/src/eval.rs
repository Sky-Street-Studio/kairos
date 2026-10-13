use std::collections::BTreeMap;

use kairos_script::{AssignOp, BinOp, Expr, UnOp, Value};

pub trait Scope {
    fn lookup(&self, name: &str) -> Option<&Value>;
    /// The next 64 random bits.
    fn random(&self) -> u64;
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::None => false,
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Float(n) => *n != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::List(items) => !items.is_empty(),
        Value::Dict(map) => !map.is_empty(),
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
        Value::List(items) => format!("[{}]", items.iter().map(repr).collect::<Vec<_>>().join(", ")),
        Value::Dict(map) => {
            let entries: Vec<_> = map.iter().map(|(k, v)| format!("{}: {}", quote(k), repr(v))).collect();
            format!("{{{}}}", entries.join(", "))
        }
    }
}

/// How a value looks inside a list or dictionary, with strings quoted as in Python.
fn repr(v: &Value) -> String {
    match v {
        Value::Str(s) => quote(s),
        other => display(other),
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
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
        Expr::List(items) => Ok(Value::List(items.iter().map(|e| eval(e, scope)).collect::<Result<_, _>>()?)),
        Expr::Dict(entries) => {
            let mut map = BTreeMap::new();
            for (k, v) in entries {
                map.insert(key(eval(k, scope)?)?, eval(v, scope)?);
            }
            Ok(Value::Dict(map))
        }
        Expr::Index(target, index) => index_of(&eval(target, scope)?, &eval(index, scope)?).cloned(),
        Expr::Call(name, args) => {
            let args = args.iter().map(|e| eval(e, scope)).collect::<Result<Vec<_>, _>>()?;
            builtin(name, &args, scope)
        }
        Expr::Method(receiver, name, args) => {
            let receiver = eval(receiver, scope)?;
            let args = args.iter().map(|e| eval(e, scope)).collect::<Result<Vec<_>, _>>()?;
            method(&receiver, name, &args)
        }
    }
}

pub fn key(v: Value) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s),
        other => Err(format!("dictionary keys must be strings, not {}", type_name(&other))),
    }
}

pub fn index_of<'a>(target: &'a Value, index: &Value) -> Result<&'a Value, String> {
    match (target, index) {
        (Value::List(items), Value::Int(i)) => {
            let at = list_index(items.len(), *i)?;
            Ok(&items[at])
        }
        (Value::Dict(map), Value::Str(k)) => {
            map.get(k).ok_or_else(|| format!("the dictionary has no key {}", quote(k)))
        }
        _ => Err(format!("cannot index {} with {}", type_name(target), type_name(index))),
    }
}

/// Resolves a Python-style index, where -1 is the last item.
pub fn list_index(len: usize, i: i64) -> Result<usize, String> {
    let at = if i < 0 { len as i64 + i } else { i };
    if (0..len as i64).contains(&at) { Ok(at as usize) } else { Err(format!("index {i} is out of range")) }
}

fn arity(name: &str, args: &[Value], counts: std::ops::RangeInclusive<usize>) -> Result<(), String> {
    if counts.contains(&args.len()) {
        Ok(())
    } else if counts.start() == counts.end() {
        Err(format!("`{name}` takes {} argument(s), got {}", counts.start(), args.len()))
    } else {
        Err(format!("`{name}` takes {} to {} arguments, got {}", counts.start(), counts.end(), args.len()))
    }
}

fn builtin(name: &str, args: &[Value], scope: &impl Scope) -> Result<Value, String> {
    let int = |v: &Value| match v {
        Value::Int(n) => Ok(*n),
        other => Err(format!("`{name}` needs an integer, not {}", type_name(other))),
    };
    match name {
        "len" => {
            arity(name, args, 1..=1)?;
            let n = match &args[0] {
                Value::List(items) => items.len(),
                Value::Dict(map) => map.len(),
                Value::Str(s) => s.chars().count(),
                other => return Err(format!("{} has no length", type_name(other))),
            };
            Ok(Value::Int(n as i64))
        }
        "min" | "max" => {
            let items = match args {
                [Value::List(items)] => items.as_slice(),
                _ => args,
            };
            let mut best = items.first().ok_or_else(|| format!("`{name}` needs at least one value"))?;
            for item in &items[1..] {
                let better = binary(if name == "min" { BinOp::Lt } else { BinOp::Gt }, item, best)?;
                if truthy(&better) {
                    best = item;
                }
            }
            Ok(best.clone())
        }
        "abs" => {
            arity(name, args, 1..=1)?;
            match &args[0] {
                Value::Int(n) => n.checked_abs().map(Value::Int).ok_or_else(|| "integer overflow".into()),
                Value::Float(n) => Ok(Value::Float(n.abs())),
                other => Err(format!("`abs` needs a number, not {}", type_name(other))),
            }
        }
        "int" => {
            arity(name, args, 1..=1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(*n)),
                Value::Bool(b) => Ok(Value::Int(*b as i64)),
                Value::Float(n) if n.is_finite() => Ok(Value::Int(n.trunc() as i64)),
                Value::Str(s) => {
                    s.trim().parse().map(Value::Int).map_err(|_| format!("cannot turn {} into an integer", quote(s)))
                }
                other => Err(format!("cannot turn {} into an integer", type_name(other))),
            }
        }
        "float" => {
            arity(name, args, 1..=1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::Float(*n as f64)),
                Value::Float(n) => Ok(Value::Float(*n)),
                Value::Str(s) => {
                    s.trim().parse().map(Value::Float).map_err(|_| format!("cannot turn {} into a float", quote(s)))
                }
                other => Err(format!("cannot turn {} into a float", type_name(other))),
            }
        }
        "str" => {
            arity(name, args, 1..=1)?;
            Ok(Value::Str(display(&args[0])))
        }
        "round" => {
            arity(name, args, 1..=1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(*n)),
                Value::Float(n) if n.is_finite() => Ok(Value::Int(n.round_ties_even() as i64)),
                other => Err(format!("`round` needs a number, not {}", type_name(other))),
            }
        }
        "range" => {
            arity(name, args, 1..=2)?;
            let (start, end) = match args {
                [end] => (0, int(end)?),
                [start, end] => (int(start)?, int(end)?),
                _ => unreachable!("arity checked"),
            };
            if end.saturating_sub(start) > 100_000 {
                return Err("`range` is limited to 100000 items".into());
            }
            Ok(Value::List((start..end).map(Value::Int).collect()))
        }
        "random" => {
            arity(name, args, 0..=0)?;
            Ok(Value::Float((scope.random() >> 11) as f64 / (1u64 << 53) as f64))
        }
        "randint" => {
            arity(name, args, 2..=2)?;
            let (low, high) = (int(&args[0])?, int(&args[1])?);
            if low > high {
                return Err(format!("`randint({low}, {high})` has an empty range"));
            }
            let span = (high as i128 - low as i128 + 1) as u128;
            Ok(Value::Int((low as i128 + (scope.random() as u128 % span) as i128) as i64))
        }
        "choice" => {
            arity(name, args, 1..=1)?;
            match &args[0] {
                Value::List(items) if !items.is_empty() => {
                    Ok(items[(scope.random() % items.len() as u64) as usize].clone())
                }
                Value::List(_) => Err("cannot choose from an empty list".into()),
                other => Err(format!("`choice` needs a list, not {}", type_name(other))),
            }
        }
        _ => Err(format!("there is no function `{name}`")),
    }
}

/// Applies `$ target[k1][k2] op= rhs`; with no keys, assigns to the variable itself.
pub fn assign_at(target: &mut Value, keys: &[Value], op: AssignOp, rhs: Value) -> Result<(), String> {
    let Some((last, path)) = keys.split_last() else {
        let v = assign(op, Some(&*target), rhs)?;
        *target = v;
        return Ok(());
    };
    let mut container = target;
    for k in path {
        container = index_mut(container, k)?;
    }
    if let Value::Dict(map) = container {
        let k = key(last.clone())?;
        if op != AssignOp::Set && !map.contains_key(&k) {
            return Err(format!("the dictionary has no key {}", quote(&k)));
        }
        let v = assign(op, map.get(&k), rhs)?;
        map.insert(k, v);
        return Ok(());
    }
    let slot = index_mut(container, last)?;
    let v = assign(op, Some(&*slot), rhs)?;
    *slot = v;
    Ok(())
}

fn index_mut<'a>(target: &'a mut Value, index: &Value) -> Result<&'a mut Value, String> {
    let kind = type_name(target);
    match (target, index) {
        (Value::List(items), Value::Int(i)) => {
            let at = list_index(items.len(), *i)?;
            Ok(&mut items[at])
        }
        (Value::Dict(map), Value::Str(k)) => {
            map.get_mut(k).ok_or_else(|| format!("the dictionary has no key {}", quote(k)))
        }
        _ => Err(format!("cannot index {kind} with {}", type_name(index))),
    }
}

/// Runs a method that changes a list or dictionary in place, such as `xs.append(1)`.
pub fn mutate(target: &mut Value, method: &str, mut args: Vec<Value>) -> Result<(), String> {
    let kind = type_name(target);
    match (target, method) {
        (Value::List(items), "append") => {
            arity(method, &args, 1..=1)?;
            items.push(args.remove(0));
        }
        (Value::List(items), "extend") => match args.as_slice() {
            [Value::List(more)] => items.extend(more.iter().cloned()),
            _ => return Err("`extend` takes one list".into()),
        },
        (Value::List(items), "insert") => match args.as_slice() {
            [Value::Int(i), value] => {
                let len = items.len() as i64;
                let at = if *i < 0 { (len + i).max(0) } else { (*i).min(len) };
                items.insert(at as usize, value.clone());
            }
            _ => return Err("`insert` takes an index and a value".into()),
        },
        (Value::List(items), "remove") => {
            arity(method, &args, 1..=1)?;
            let at = items.iter().position(|v| equal(v, &args[0]));
            items.remove(at.ok_or_else(|| format!("{} is not in the list", repr(&args[0])))?);
        }
        (Value::List(items), "pop") => {
            arity(method, &args, 0..=1)?;
            if items.is_empty() {
                return Err("cannot pop from an empty list".into());
            }
            let at = match args.first() {
                Some(Value::Int(i)) => list_index(items.len(), *i)?,
                Some(other) => return Err(format!("`pop` needs an integer index, not {}", type_name(other))),
                None => items.len() - 1,
            };
            items.remove(at);
        }
        (Value::Dict(map), "pop") => {
            arity(method, &args, 1..=2)?;
            let k = key(args[0].clone())?;
            if map.remove(&k).is_none() && args.len() == 1 {
                return Err(format!("the dictionary has no key {}", quote(&k)));
            }
        }
        (Value::List(items), "clear") => {
            arity(method, &args, 0..=0)?;
            items.clear();
        }
        (Value::Dict(map), "clear") => {
            arity(method, &args, 0..=0)?;
            map.clear();
        }
        _ => return Err(format!("{kind} has no method `{method}` that changes it")),
    }
    Ok(())
}

pub const FUNCTIONS: [&str; 12] =
    ["len", "min", "max", "abs", "int", "float", "str", "round", "range", "random", "randint", "choice"];
const READING: [&str; 5] = ["get", "keys", "values", "count", "index"];
const MUTATING: [&str; 6] = ["append", "extend", "insert", "remove", "pop", "clear"];

/// Unknown function and method names in `expr`, for `kairos check`.
pub fn unknown_names(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Lit(_) | Expr::Var(_) => {}
        Expr::Unary(_, e) => unknown_names(e, out),
        Expr::Binary(_, a, b) | Expr::Index(a, b) => {
            unknown_names(a, out);
            unknown_names(b, out);
        }
        Expr::List(items) => items.iter().for_each(|e| unknown_names(e, out)),
        Expr::Dict(entries) => entries.iter().for_each(|(k, v)| {
            unknown_names(k, out);
            unknown_names(v, out);
        }),
        Expr::Call(name, args) => {
            if !FUNCTIONS.contains(&name.as_str()) {
                out.push(format!("there is no function `{name}`"));
            }
            args.iter().for_each(|e| unknown_names(e, out));
        }
        Expr::Method(receiver, name, args) => {
            if !READING.contains(&name.as_str()) && !MUTATING.contains(&name.as_str()) {
                out.push(format!("there is no method `{name}`"));
            }
            unknown_names(receiver, out);
            args.iter().for_each(|e| unknown_names(e, out));
        }
    }
}

/// Problems with a `$ name.method(...)` statement's method, for `kairos check`.
pub fn unknown_mutation(method: &str) -> Option<String> {
    if MUTATING.contains(&method) {
        None
    } else if READING.contains(&method) {
        Some(format!("`{method}` does not change anything, so it does nothing as a statement"))
    } else {
        Some(format!("there is no method `{method}`"))
    }
}

/// The expressions inside `[...]` in dialogue or choice text.
pub fn interpolations(text: &str) -> Result<Vec<Expr>, String> {
    let mut exprs = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        rest = &rest[i + 1..];
        if let Some(r) = rest.strip_prefix('[') {
            rest = r;
            continue;
        }
        let end = closing_bracket(rest).ok_or("unclosed `[` in text")?;
        exprs.push(kairos_script::parse_expr(&rest[..end]).map_err(|d| d.message)?);
        rest = &rest[end + 1..];
    }
    Ok(exprs)
}

fn method(receiver: &Value, name: &str, args: &[Value]) -> Result<Value, String> {
    match (receiver, name) {
        (Value::Dict(map), "get") => {
            arity(name, args, 1..=2)?;
            let k = key(args[0].clone())?;
            Ok(map.get(&k).cloned().unwrap_or_else(|| args.get(1).cloned().unwrap_or(Value::None)))
        }
        (Value::Dict(map), "keys") => {
            arity(name, args, 0..=0)?;
            Ok(Value::List(map.keys().cloned().map(Value::Str).collect()))
        }
        (Value::Dict(map), "values") => {
            arity(name, args, 0..=0)?;
            Ok(Value::List(map.values().cloned().collect()))
        }
        (Value::List(items), "count") => {
            arity(name, args, 1..=1)?;
            Ok(Value::Int(items.iter().filter(|v| equal(v, &args[0])).count() as i64))
        }
        (Value::List(items), "index") => {
            arity(name, args, 1..=1)?;
            let at = items.iter().position(|v| equal(v, &args[0]));
            at.map(|i| Value::Int(i as i64)).ok_or_else(|| format!("{} is not in the list", repr(&args[0])))
        }
        (_, m) if MUTATING.contains(&m) => Err(format!(
            "`{m}` changes {} in place; use it as its own statement, like `$ xs.{m}(...)`",
            type_name(receiver)
        )),
        _ => Err(format!("{} has no method `{name}`", type_name(receiver))),
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
        BinOp::In => contains(b, a),
        BinOp::NotIn => contains(b, a).map(|v| Bool(!truthy(&v))),
        BinOp::Add => match (a, b) {
            (Str(x), Str(y)) => Ok(Str(format!("{x}{y}"))),
            (Value::List(x), Value::List(y)) => Ok(Value::List(x.iter().chain(y).cloned().collect())),
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

fn contains(container: &Value, item: &Value) -> Result<Value, String> {
    Ok(Value::Bool(match (container, item) {
        (Value::List(items), _) => items.iter().any(|v| equal(v, item)),
        (Value::Dict(map), Value::Str(k)) => map.contains_key(k),
        (Value::Str(s), Value::Str(part)) => s.contains(part.as_str()),
        _ => return Err(format!("cannot look for {} in {}", type_name(item), type_name(container))),
    }))
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
        Value::List(_) => "a list",
        Value::Dict(_) => "a dictionary",
    }
}

/// Replaces each `[expression]` with its value; `[[` is a literal `[`.
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
        let end = closing_bracket(rest).ok_or("unclosed `[` in text")?;
        let expr = kairos_script::parse_expr(&rest[..end]).map_err(|d| d.message)?;
        out.push_str(&display(&eval(&expr, scope)?));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The byte offset of the `]` closing an interpolation, skipping nested brackets and strings.
fn closing_bracket(text: &str) -> Option<usize> {
    let mut depth = 0;
    let mut quote = None;
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        match quote {
            Some(_) if escaped => escaped = false,
            Some(_) if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '[' | '(' | '{' => depth += 1,
                ']' if depth == 0 => return Some(i),
                ']' | ')' | '}' => depth -= 1,
                _ => {}
            },
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Empty;

    impl Scope for Empty {
        fn lookup(&self, _: &str) -> Option<&Value> {
            None
        }

        fn random(&self) -> u64 {
            0
        }
    }

    #[test]
    fn every_checked_name_is_implemented() {
        for name in FUNCTIONS {
            let err = builtin(name, &[], &Empty).err().unwrap_or_default();
            assert!(!err.contains("no function"), "{name}: {err}");
        }
        let containers = [Value::List(Vec::new()), Value::Dict(BTreeMap::new())];
        for name in READING {
            assert!(
                containers.iter().any(|c| !method(c, name, &[]).err().unwrap_or_default().contains("has no method")),
                "{name}"
            );
        }
        for name in MUTATING {
            assert!(
                containers.iter().any(|c| !mutate(&mut c.clone(), name, Vec::new())
                    .err()
                    .unwrap_or_default()
                    .contains("that changes it")),
                "{name}"
            );
        }
    }
}
