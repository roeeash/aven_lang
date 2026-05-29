use crate::ast::{Expr, ArithOp, Pattern, DiffOp, DiffKind, SelectorPath, PathSegment};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Symbol(String),
    Tagged(String, Option<Box<Value>>),  // tag, optional payload
    Nil,
    Record(Vec<(String, Value)>),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),  // sorted by key for determinism
    Set(Vec<Value>),              // deduplicated, insertion order maintained
    Fn {
        params: Vec<(String, Option<crate::ast::Type>)>,
        body: Expr,
        closure_env: Env,
    },
    NativeFn {
        name: String,
        arity: usize,
        func: Arc<dyn Fn(&[Value]) -> Result<Value, EvalError> + Send + Sync>,
    },
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "Int({})", n),
            Value::Float(n) => write!(f, "Float({})", n),
            Value::Str(s) => write!(f, "Str({:?})", s),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Symbol(s) => write!(f, "Symbol({})", s),
            Value::Tagged(tag, payload) => write!(f, "Tagged({}, {:?})", tag, payload),
            Value::Nil => write!(f, "Nil"),
            Value::Record(fields) => write!(f, "Record({:?})", fields),
            Value::List(elements) => write!(f, "List({:?})", elements),
            Value::Map(entries) => write!(f, "Map({:?})", entries),
            Value::Set(elements) => write!(f, "Set({:?})", elements),
            Value::Fn { .. } => write!(f, "Fn {{ ... }}"),
            Value::NativeFn { name, arity, .. } => write!(f, "NativeFn {{ name: {}, arity: {} }}", name, arity),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            (Value::Tagged(t1, p1), Value::Tagged(t2, p2)) => t1 == t2 && p1 == p2,
            (Value::Nil, Value::Nil) => true,
            (Value::Record(f1), Value::Record(f2)) => f1 == f2,
            (Value::List(v1), Value::List(v2)) => v1 == v2,
            (Value::Map(m1), Value::Map(m2)) => {
                // Order-independent map equality: same length + every (k,v) in m1 exists in m2
                if m1.len() != m2.len() {
                    return false;
                }
                m1.iter().all(|(k1, v1)| {
                    m2.iter().any(|(k2, v2)| k1 == k2 && v1 == v2)
                })
            }
            (Value::Set(s1), Value::Set(s2)) => s1 == s2,
            (Value::Fn { .. }, Value::Fn { .. }) => false, // Functions not comparable
            (Value::NativeFn { .. }, Value::NativeFn { .. }) => false, // NativeFns not comparable
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => {
                let s = format!("{}", n);
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    write!(f, "{}", s)
                } else {
                    write!(f, "{}.0", s)
                }
            }
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "@{}", if *b { "true" } else { "false" }),
            Value::Symbol(s) => write!(f, "{}", s),
            Value::Tagged(tag, payload) => {
                if let Some(p) = payload {
                    write!(f, "(#{} {})", tag, p)
                } else {
                    write!(f, "#{}", tag)
                }
            }
            Value::Nil => write!(f, "_"),
            Value::Record(fields) => {
                write!(f, "{{")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::List(elements) => {
                write!(f, "[")?;
                for (i, v) in elements.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    match v {
                        Value::Str(s) => write!(f, "\"{}\"", s)?,
                        _ => write!(f, "{}", v)?,
                    }
                }
                write!(f, "]")
            }
            Value::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Set(elements) => {
                write!(f, "{{")?;
                for (i, v) in elements.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "}}")
            }
            Value::Fn { .. } => write!(f, "<fn>"),
            Value::NativeFn { name, .. } => write!(f, "<native:{}>", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    UndefinedVariable(String),
    DivisionByZero,
    InvalidOperation(String),
    InvalidFunctionCall(String),
    TypeError(String),
    TypecheckFailed(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EvalError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            EvalError::DivisionByZero => write!(f, "Division by zero"),
            EvalError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            EvalError::InvalidFunctionCall(msg) => write!(f, "Invalid function call: {}", msg),
            EvalError::TypeError(msg) => write!(f, "Type error: {}", msg),
            EvalError::TypecheckFailed(msg) => write!(f, "Typecheck failed: {}", msg),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Env {
    vars: HashMap<String, Value>,
    context: HashMap<String, Value>,
    parent: Option<Box<Env>>,
}

fn read_line_from<R: std::io::BufRead>(reader: &mut R) -> Result<Value, EvalError> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => Ok(Value::Str(String::new())), // EOF
        Ok(_) => Ok(Value::Str(line.trim_end_matches(|c: char| c == '\r' || c == '\n').to_string())),
        Err(e) => Err(EvalError::InvalidOperation(format!("read_line: IO error: {}", e))),
    }
}

fn read_file_from(path: &str) -> Result<Value, EvalError> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Value::Str(content)),
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => Err(EvalError::InvalidOperation(format!("file not found: {}", path))),
            std::io::ErrorKind::IsADirectory => Err(EvalError::InvalidOperation(format!("not a file: {}", path))),
            _ => Err(EvalError::InvalidOperation(format!("read_file: IO error: {}", e))),
        },
    }
}

impl Env {
    pub fn new() -> Self {
        let mut env = Env {
            vars: HashMap::new(),
            context: HashMap::new(),
            parent: None,
        };
        Self::register_stdlib(&mut env);
        env
    }

    fn register_stdlib(env: &mut Env) {
        // aven/std/math::abs — arity 1, takes Int, returns Int
        env.define(
            "aven/std/math::abs".to_string(),
            Value::NativeFn {
                name: "aven/std/math::abs".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => match n.checked_abs() {
                            Some(abs_val) => Ok(Value::Int(abs_val)),
                            None => Err(EvalError::InvalidOperation("abs overflow".to_string())),
                        },
                        _ => Err(EvalError::TypeError("abs requires Int argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::min — arity 2, takes two Ints, returns Int
        env.define(
            "aven/std/math::min".to_string(),
            Value::NativeFn {
                name: "aven/std/math::min".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
                        _ => Err(EvalError::TypeError("min requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::max — arity 2, takes two Ints, returns Int
        env.define(
            "aven/std/math::max".to_string(),
            Value::NativeFn {
                name: "aven/std/math::max".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
                        _ => Err(EvalError::TypeError("max requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::pow — arity 2, takes Int base and exponent, returns Int
        env.define(
            "aven/std/math::pow".to_string(),
            Value::NativeFn {
                name: "aven/std/math::pow".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(base), Value::Int(exp)) => {
                            if *exp < 0 {
                                Err(EvalError::InvalidOperation(
                                    "pow requires non-negative exponent".to_string(),
                                ))
                            } else if *exp > u32::MAX as i64 {
                                Err(EvalError::InvalidOperation(
                                    "pow exponent too large".to_string(),
                                ))
                            } else {
                                match base.checked_pow(*exp as u32) {
                                    Some(result) => Ok(Value::Int(result)),
                                    None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                                }
                            }
                        }
                        _ => Err(EvalError::TypeError("pow requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::sqrt — arity 1, takes Int, returns Int (floor)
        env.define(
            "aven/std/math::sqrt".to_string(),
            Value::NativeFn {
                name: "aven/std/math::sqrt".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => {
                            if *n < 0 {
                                Err(EvalError::InvalidOperation(
                                    "sqrt requires non-negative argument".to_string(),
                                ))
                            } else if *n > (1_i64 << 53) {
                                Err(EvalError::InvalidOperation(
                                    "sqrt: input exceeds f64 precision".to_string(),
                                ))
                            } else {
                                Ok(Value::Int((*n as f64).sqrt() as i64))
                            }
                        }
                        _ => Err(EvalError::TypeError("sqrt requires Int argument".to_string())),
                    }
                }),
            },
        );

        // Register simpler aliases for testing (without module path syntax)
        // abs alias
        env.define(
            "abs".to_string(),
            Value::NativeFn {
                name: "abs".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => match n.checked_abs() {
                            Some(abs_val) => Ok(Value::Int(abs_val)),
                            None => Err(EvalError::InvalidOperation("abs overflow".to_string())),
                        },
                        _ => Err(EvalError::TypeError("abs requires Int argument".to_string())),
                    }
                }),
            },
        );

        // min alias
        env.define(
            "min".to_string(),
            Value::NativeFn {
                name: "min".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
                        _ => Err(EvalError::TypeError("min requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // max alias
        env.define(
            "max".to_string(),
            Value::NativeFn {
                name: "max".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
                        _ => Err(EvalError::TypeError("max requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // pow alias
        env.define(
            "pow".to_string(),
            Value::NativeFn {
                name: "pow".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(base), Value::Int(exp)) => {
                            if *exp < 0 {
                                Err(EvalError::InvalidOperation(
                                    "pow requires non-negative exponent".to_string(),
                                ))
                            } else if *exp > u32::MAX as i64 {
                                Err(EvalError::InvalidOperation(
                                    "pow exponent too large".to_string(),
                                ))
                            } else {
                                match base.checked_pow(*exp as u32) {
                                    Some(result) => Ok(Value::Int(result)),
                                    None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                                }
                            }
                        }
                        _ => Err(EvalError::TypeError("pow requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // sqrt alias
        env.define(
            "sqrt".to_string(),
            Value::NativeFn {
                name: "sqrt".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => {
                            if *n < 0 {
                                Err(EvalError::InvalidOperation(
                                    "sqrt requires non-negative argument".to_string(),
                                ))
                            } else if *n > (1_i64 << 53) {
                                Err(EvalError::InvalidOperation(
                                    "sqrt: input exceeds f64 precision".to_string(),
                                ))
                            } else {
                                Ok(Value::Int((*n as f64).sqrt() as i64))
                            }
                        }
                        _ => Err(EvalError::TypeError("sqrt requires Int argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/io::print — arity 1, takes Str, prints to stdout with newline, returns Nil
        env.define(
            "aven/std/io::print".to_string(),
            Value::NativeFn {
                name: "aven/std/io::print".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            println!("{}", s);
                            Ok(Value::Nil)
                        }
                        _ => Err(EvalError::TypeError("print requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/io::read_line — arity 0, reads a line from stdin, returns Str
        env.define(
            "aven/std/io::read_line".to_string(),
            Value::NativeFn {
                name: "aven/std/io::read_line".to_string(),
                arity: 0,
                func: Arc::new(|_args| {
                    read_line_from(&mut std::io::stdin().lock())
                }),
            },
        );

        // aven/std/io::write — arity 1, takes Str, prints to stdout without newline, returns Nil
        env.define(
            "aven/std/io::write".to_string(),
            Value::NativeFn {
                name: "aven/std/io::write".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            use std::io::Write;
                            print!("{}", s);
                            let _ = std::io::stdout().flush();
                            Ok(Value::Nil)
                        }
                        _ => Err(EvalError::TypeError("write requires Str argument".to_string())),
                    }
                }),
            },
        );

        // print alias
        env.define(
            "print".to_string(),
            Value::NativeFn {
                name: "print".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            println!("{}", s);
                            Ok(Value::Nil)
                        }
                        _ => Err(EvalError::TypeError("print requires Str argument".to_string())),
                    }
                }),
            },
        );

        // read_line alias
        env.define(
            "read_line".to_string(),
            Value::NativeFn {
                name: "read_line".to_string(),
                arity: 0,
                func: Arc::new(|_args| {
                    read_line_from(&mut std::io::stdin().lock())
                }),
            },
        );

        // write alias
        env.define(
            "write".to_string(),
            Value::NativeFn {
                name: "write".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            use std::io::Write;
                            print!("{}", s);
                            let _ = std::io::stdout().flush();
                            Ok(Value::Nil)
                        }
                        _ => Err(EvalError::TypeError("write requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/fs::read — arity 1, takes Str path, returns Str file contents
        env.define(
            "aven/std/fs::read".to_string(),
            Value::NativeFn {
                name: "aven/std/fs::read".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(path) => read_file_from(path),
                        _ => Err(EvalError::TypeError("fs::read requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/fs::write — arity 2, takes Str path and Str content, returns Nil
        env.define(
            "aven/std/fs::write".to_string(),
            Value::NativeFn {
                name: "aven/std/fs::write".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(path), Value::Str(content)) => {
                            match std::fs::write(path, content) {
                                Ok(_) => Ok(Value::Nil),
                                Err(e) => Err(EvalError::InvalidOperation(format!("write_file: IO error: {}", e))),
                            }
                        }
                        _ => Err(EvalError::TypeError("fs::write requires Str arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/fs::list — arity 1, takes Str path, returns Str with newline-separated filenames
        env.define(
            "aven/std/fs::list".to_string(),
            Value::NativeFn {
                name: "aven/std/fs::list".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(path) => {
                            match std::fs::read_dir(path) {
                                Ok(entries) => {
                                    let mut names = Vec::new();
                                    for entry in entries {
                                        match entry {
                                            Ok(e) => {
                                                let name = e.file_name().to_string_lossy().to_string();
                                                names.push(name);
                                            }
                                            Err(e) => return Err(EvalError::InvalidOperation(format!("read_dir entry error: {}", e))),
                                        }
                                    }
                                    names.sort();
                                    Ok(Value::Str(names.join("\n")))
                                }
                                Err(e) => match e.kind() {
                                    std::io::ErrorKind::NotFound => Err(EvalError::InvalidOperation(format!("directory not found: {}", path))),
                                    std::io::ErrorKind::NotADirectory => Err(EvalError::InvalidOperation(format!("not a directory: {}", path))),
                                    _ => Err(EvalError::InvalidOperation(format!("read_dir: IO error: {}", e))),
                                },
                            }
                        }
                        _ => Err(EvalError::TypeError("fs::list requires Str argument".to_string())),
                    }
                }),
            },
        );

        // fs_read alias
        env.define(
            "fs_read".to_string(),
            Value::NativeFn {
                name: "fs_read".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(path) => read_file_from(path),
                        _ => Err(EvalError::TypeError("fs_read requires Str argument".to_string())),
                    }
                }),
            },
        );

        // fs_write alias
        env.define(
            "fs_write".to_string(),
            Value::NativeFn {
                name: "fs_write".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(path), Value::Str(content)) => {
                            match std::fs::write(path, content) {
                                Ok(_) => Ok(Value::Nil),
                                Err(e) => Err(EvalError::InvalidOperation(format!("write_file: IO error: {}", e))),
                            }
                        }
                        _ => Err(EvalError::TypeError("fs_write requires Str arguments".to_string())),
                    }
                }),
            },
        );

        // fs_list alias
        env.define(
            "fs_list".to_string(),
            Value::NativeFn {
                name: "fs_list".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(path) => {
                            match std::fs::read_dir(path) {
                                Ok(entries) => {
                                    let mut names = Vec::new();
                                    for entry in entries {
                                        match entry {
                                            Ok(e) => {
                                                let name = e.file_name().to_string_lossy().to_string();
                                                names.push(name);
                                            }
                                            Err(e) => return Err(EvalError::InvalidOperation(format!("read_dir entry error: {}", e))),
                                        }
                                    }
                                    names.sort();
                                    Ok(Value::Str(names.join("\n")))
                                }
                                Err(e) => match e.kind() {
                                    std::io::ErrorKind::NotFound => Err(EvalError::InvalidOperation(format!("directory not found: {}", path))),
                                    std::io::ErrorKind::NotADirectory => Err(EvalError::InvalidOperation(format!("not a directory: {}", path))),
                                    _ => Err(EvalError::InvalidOperation(format!("read_dir: IO error: {}", e))),
                                },
                            }
                        }
                        _ => Err(EvalError::TypeError("fs_list requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/json::parse — arity 1, takes Str, parses JSON, returns Value
        env.define(
            "aven/std/json::parse".to_string(),
            Value::NativeFn {
                name: "aven/std/json::parse".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            match serde_json::from_str::<serde_json::Value>(s) {
                                Ok(json_val) => match json_val {
                                    serde_json::Value::Null => Ok(Value::Nil),
                                    serde_json::Value::Bool(b) => Ok(Value::Bool(b)),
                                    serde_json::Value::Number(n) => {
                                        match n.as_i64() {
                                            Some(i) => Ok(Value::Int(i)),
                                            None => Err(EvalError::InvalidOperation("JSON floats not supported".to_string())),
                                        }
                                    }
                                    serde_json::Value::String(s) => Ok(Value::Str(s)),
                                    serde_json::Value::Array(arr) => {
                                        let strs: Result<Vec<String>, EvalError> = arr.iter().map(|elem| {
                                            match elem {
                                                serde_json::Value::Null => Ok("null".to_string()),
                                                serde_json::Value::Bool(b) => Ok(b.to_string()),
                                                serde_json::Value::Number(n) => {
                                                    match n.as_i64() {
                                                        Some(i) => Ok(i.to_string()),
                                                        None => Err(EvalError::InvalidOperation("JSON floats not supported".to_string())),
                                                    }
                                                }
                                                serde_json::Value::String(s) => {
                                                    let json_str = serde_json::Value::String(s.clone()).to_string();
                                                    Ok(json_str)
                                                }
                                                _ => Err(EvalError::InvalidOperation("nested JSON objects/arrays not supported".to_string())),
                                            }
                                        }).collect();
                                        match strs {
                                            Ok(s) => Ok(Value::Str(s.join("\n"))),
                                            Err(e) => Err(e),
                                        }
                                    }
                                    serde_json::Value::Object(_) => Err(EvalError::InvalidOperation("JSON objects not supported in M6.4".to_string())),
                                },
                                Err(_) => Err(EvalError::InvalidOperation("invalid JSON".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("json_parse requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/json::serialize — arity 1, takes Value, returns JSON Str
        env.define(
            "aven/std/json::serialize".to_string(),
            Value::NativeFn {
                name: "aven/std/json::serialize".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Nil => Ok(Value::Str("null".to_string())),
                        Value::Bool(b) => Ok(Value::Str(b.to_string())),
                        Value::Int(n) => Ok(Value::Str(n.to_string())),
                        Value::Str(s) => {
                            let json_str = serde_json::Value::String(s.clone()).to_string();
                            Ok(Value::Str(json_str))
                        }
                        Value::NativeFn { .. } => Err(EvalError::TypeError("NativeFn cannot be serialized to JSON".to_string())),
                        _ => Err(EvalError::InvalidOperation("value type not supported for JSON serialization".to_string())),
                    }
                }),
            },
        );

        // json_parse alias
        env.define(
            "json_parse".to_string(),
            Value::NativeFn {
                name: "json_parse".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            match serde_json::from_str::<serde_json::Value>(s) {
                                Ok(json_val) => match json_val {
                                    serde_json::Value::Null => Ok(Value::Nil),
                                    serde_json::Value::Bool(b) => Ok(Value::Bool(b)),
                                    serde_json::Value::Number(n) => {
                                        match n.as_i64() {
                                            Some(i) => Ok(Value::Int(i)),
                                            None => Err(EvalError::InvalidOperation("JSON floats not supported".to_string())),
                                        }
                                    }
                                    serde_json::Value::String(s) => Ok(Value::Str(s)),
                                    serde_json::Value::Array(arr) => {
                                        let strs: Result<Vec<String>, EvalError> = arr.iter().map(|elem| {
                                            match elem {
                                                serde_json::Value::Null => Ok("null".to_string()),
                                                serde_json::Value::Bool(b) => Ok(b.to_string()),
                                                serde_json::Value::Number(n) => {
                                                    match n.as_i64() {
                                                        Some(i) => Ok(i.to_string()),
                                                        None => Err(EvalError::InvalidOperation("JSON floats not supported".to_string())),
                                                    }
                                                }
                                                serde_json::Value::String(s) => {
                                                    let json_str = serde_json::Value::String(s.clone()).to_string();
                                                    Ok(json_str)
                                                }
                                                _ => Err(EvalError::InvalidOperation("nested JSON objects/arrays not supported".to_string())),
                                            }
                                        }).collect();
                                        match strs {
                                            Ok(s) => Ok(Value::Str(s.join("\n"))),
                                            Err(e) => Err(e),
                                        }
                                    }
                                    serde_json::Value::Object(_) => Err(EvalError::InvalidOperation("JSON objects not supported in M6.4".to_string())),
                                },
                                Err(_) => Err(EvalError::InvalidOperation("invalid JSON".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("json_parse requires Str argument".to_string())),
                    }
                }),
            },
        );

        // json_serialize alias
        env.define(
            "json_serialize".to_string(),
            Value::NativeFn {
                name: "json_serialize".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Nil => Ok(Value::Str("null".to_string())),
                        Value::Bool(b) => Ok(Value::Str(b.to_string())),
                        Value::Int(n) => Ok(Value::Str(n.to_string())),
                        Value::Str(s) => {
                            let json_str = serde_json::Value::String(s.clone()).to_string();
                            Ok(Value::Str(json_str))
                        }
                        Value::NativeFn { .. } => Err(EvalError::TypeError("NativeFn cannot be serialized to JSON".to_string())),
                        _ => Err(EvalError::InvalidOperation("value type not supported for JSON serialization".to_string())),
                    }
                }),
            },
        );

        // aven/std/time::now — arity 0, returns current Unix timestamp in milliseconds as Int
        env.define(
            "aven/std/time::now".to_string(),
            Value::NativeFn {
                name: "aven/std/time::now".to_string(),
                arity: 0,
                func: Arc::new(|_args| {
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                    Ok(Value::Int(ms))
                }),
            },
        );

        // time_now alias
        env.define(
            "time_now".to_string(),
            Value::NativeFn {
                name: "time_now".to_string(),
                arity: 0,
                func: Arc::new(|_args| {
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                    Ok(Value::Int(ms))
                }),
            },
        );

        // aven/std/time::sleep — arity 1, takes Int milliseconds, returns Nil
        env.define(
            "aven/std/time::sleep".to_string(),
            Value::NativeFn {
                name: "aven/std/time::sleep".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(ms) => {
                            if *ms < 0 {
                                Err(EvalError::InvalidOperation("sleep requires non-negative duration".to_string()))
                            } else {
                                std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                                Ok(Value::Nil)
                            }
                        }
                        _ => Err(EvalError::TypeError("sleep requires Int argument".to_string())),
                    }
                }),
            },
        );

        // time_sleep alias
        env.define(
            "time_sleep".to_string(),
            Value::NativeFn {
                name: "time_sleep".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(ms) => {
                            if *ms < 0 {
                                Err(EvalError::InvalidOperation("sleep requires non-negative milliseconds".to_string()))
                            } else {
                                std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                                Ok(Value::Nil)
                            }
                        }
                        _ => Err(EvalError::TypeError("sleep requires Int argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/time::format — arity 2, takes Int timestamp and Str format string, returns Str
        env.define(
            "aven/std/time::format".to_string(),
            Value::NativeFn {
                name: "aven/std/time::format".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(ts), Value::Str(fmt)) => {
                            use chrono::TimeZone;
                            match chrono::Utc.timestamp_opt(*ts, 0).single() {
                                Some(dt) => {
                                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dt.format(fmt.as_str()).to_string())) {
                                        Ok(formatted) => Ok(Value::Str(formatted)),
                                        Err(_) => Err(EvalError::InvalidOperation("invalid format string".to_string())),
                                    }
                                }
                                None => Err(EvalError::InvalidOperation("invalid timestamp for formatting".to_string())),
                            }
                        }
                        (Value::Int(_), _) => Err(EvalError::TypeError("format requires Str format string".to_string())),
                        _ => Err(EvalError::TypeError("format requires Int timestamp".to_string())),
                    }
                }),
            },
        );

        // time_format alias
        env.define(
            "time_format".to_string(),
            Value::NativeFn {
                name: "time_format".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(ts), Value::Str(fmt)) => {
                            use chrono::TimeZone;
                            match chrono::Utc.timestamp_opt(*ts, 0).single() {
                                Some(dt) => {
                                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dt.format(fmt.as_str()).to_string())) {
                                        Ok(formatted) => Ok(Value::Str(formatted)),
                                        Err(_) => Err(EvalError::InvalidOperation("invalid format string".to_string())),
                                    }
                                }
                                None => Err(EvalError::InvalidOperation("invalid timestamp for formatting".to_string())),
                            }
                        }
                        (Value::Int(_), _) => Err(EvalError::TypeError("format requires Str format string".to_string())),
                        _ => Err(EvalError::TypeError("format requires Int timestamp".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::list — arity 1, takes Str with newline-separated elements, returns normalized Str
        env.define(
            "aven/std/collections::list".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::list".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            let parts: Vec<&str> = s.split('\n').filter(|p| !p.is_empty()).collect();
                            Ok(Value::Str(parts.join("\n")))
                        }
                        _ => Err(EvalError::TypeError("list requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::map — arity 2, takes NativeFn and Str newline-list, applies fn to each element
        env.define(
            "aven/std/collections::map".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::map".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::NativeFn { func, .. }, Value::Str(s)) => {
                            let parts: Vec<&str> = s.split('\n').filter(|p| !p.is_empty()).collect();
                            let mut results = Vec::new();
                            for part in parts {
                                let result = func(&[Value::Str(part.to_string())])?;
                                match result {
                                    Value::Str(st) => results.push(st),
                                    _ => return Err(EvalError::TypeError("map function must return Str".to_string())),
                                }
                            }
                            Ok(Value::Str(results.join("\n")))
                        }
                        (Value::Fn { .. }, _) => Err(EvalError::TypeError("map requires NativeFn".to_string())),
                        (_, Value::Str(_)) => Err(EvalError::TypeError("map requires NativeFn as first argument".to_string())),
                        _ => Err(EvalError::TypeError("map requires NativeFn and Str arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::set — arity 1, takes Str newline-list, returns deduplicated and sorted Str
        use std::collections::BTreeSet;
        env.define(
            "aven/std/collections::set".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::set".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            let parts: Vec<&str> = s.split('\n').filter(|p| !p.is_empty()).collect();
                            let mut set = BTreeSet::new();
                            for part in parts {
                                set.insert(part.to_string());
                            }
                            let result: Vec<String> = set.into_iter().collect();
                            Ok(Value::Str(result.join("\n")))
                        }
                        _ => Err(EvalError::TypeError("set requires Str argument".to_string())),
                    }
                }),
            },
        );

        // col_list alias
        env.define(
            "col_list".to_string(),
            Value::NativeFn {
                name: "col_list".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            let parts: Vec<&str> = s.split('\n').filter(|p| !p.is_empty()).collect();
                            Ok(Value::Str(parts.join("\n")))
                        }
                        _ => Err(EvalError::TypeError("list requires Str argument".to_string())),
                    }
                }),
            },
        );

        // col_map alias
        env.define(
            "col_map".to_string(),
            Value::NativeFn {
                name: "col_map".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::NativeFn { func, .. }, Value::Str(s)) => {
                            let parts: Vec<&str> = s.split('\n').filter(|p| !p.is_empty()).collect();
                            let mut results = Vec::new();
                            for part in parts {
                                let result = func(&[Value::Str(part.to_string())])?;
                                match result {
                                    Value::Str(st) => results.push(st),
                                    _ => return Err(EvalError::TypeError("map function must return Str".to_string())),
                                }
                            }
                            Ok(Value::Str(results.join("\n")))
                        }
                        (Value::Fn { .. }, _) => Err(EvalError::TypeError("map requires NativeFn".to_string())),
                        (_, Value::Str(_)) => Err(EvalError::TypeError("map requires NativeFn as first argument".to_string())),
                        _ => Err(EvalError::TypeError("map requires NativeFn and Str arguments".to_string())),
                    }
                }),
            },
        );

        // col_set alias
        env.define(
            "col_set".to_string(),
            Value::NativeFn {
                name: "col_set".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            let parts: Vec<&str> = s.split('\n').filter(|p| !p.is_empty()).collect();
                            let mut set = BTreeSet::new();
                            for part in parts {
                                set.insert(part.to_string());
                            }
                            let result: Vec<String> = set.into_iter().collect();
                            Ok(Value::Str(result.join("\n")))
                        }
                        _ => Err(EvalError::TypeError("set requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/http::get — arity 1, takes Str URL, returns Str response body
        env.define(
            "aven/std/http::get".to_string(),
            Value::NativeFn {
                name: "aven/std/http::get".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(url) => {
                            match ureq::get(url).call() {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("http::get requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/http::post — arity 2, takes Str URL and Str body, returns Str response body
        env.define(
            "aven/std/http::post".to_string(),
            Value::NativeFn {
                name: "aven/std/http::post".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(url), Value::Str(body)) => {
                            match ureq::post(url).send_string(body) {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        (Value::Str(_), _) => Err(EvalError::TypeError("http::post requires Str body".to_string())),
                        _ => Err(EvalError::TypeError("http::post requires Str URL".to_string())),
                    }
                }),
            },
        );

        // aven/std/http::put — arity 2, takes Str URL and Str body, returns Str response body
        env.define(
            "aven/std/http::put".to_string(),
            Value::NativeFn {
                name: "aven/std/http::put".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(url), Value::Str(body)) => {
                            match ureq::put(url).send_string(body) {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        (Value::Str(_), _) => Err(EvalError::TypeError("http::put requires Str body".to_string())),
                        _ => Err(EvalError::TypeError("http::put requires Str URL".to_string())),
                    }
                }),
            },
        );

        // aven/std/http::delete — arity 1, takes Str URL, returns Str response body
        env.define(
            "aven/std/http::delete".to_string(),
            Value::NativeFn {
                name: "aven/std/http::delete".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(url) => {
                            match ureq::delete(url).call() {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("http::delete requires Str argument".to_string())),
                    }
                }),
            },
        );

        // http_get alias
        env.define(
            "http_get".to_string(),
            Value::NativeFn {
                name: "http_get".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(url) => {
                            match ureq::get(url).call() {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("http_get requires Str argument".to_string())),
                    }
                }),
            },
        );

        // http_post alias
        env.define(
            "http_post".to_string(),
            Value::NativeFn {
                name: "http_post".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(url), Value::Str(body)) => {
                            match ureq::post(url).send_string(body) {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        (Value::Str(_), _) => Err(EvalError::TypeError("http_post requires Str body".to_string())),
                        _ => Err(EvalError::TypeError("http_post requires Str URL".to_string())),
                    }
                }),
            },
        );

        // http_put alias
        env.define(
            "http_put".to_string(),
            Value::NativeFn {
                name: "http_put".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(url), Value::Str(body)) => {
                            match ureq::put(url).send_string(body) {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        (Value::Str(_), _) => Err(EvalError::TypeError("http_put requires Str body".to_string())),
                        _ => Err(EvalError::TypeError("http_put requires Str URL".to_string())),
                    }
                }),
            },
        );

        // http_delete alias
        env.define(
            "http_delete".to_string(),
            Value::NativeFn {
                name: "http_delete".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(url) => {
                            match ureq::delete(url).call() {
                                Ok(response) => response.into_string().map(Value::Str).map_err(|e| EvalError::InvalidOperation(e.to_string())),
                                Err(ureq::Error::Status(code, _)) => Err(EvalError::InvalidOperation(format!("HTTP error: {}", code))),
                                Err(e) => Err(EvalError::InvalidOperation(e.to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("http_delete requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::len — arity 1, takes Str, returns Int length
        env.define(
            "aven/std/str::len".to_string(),
            Value::NativeFn {
                name: "aven/std/str::len".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => Ok(Value::Int(s.len() as i64)),
                        _ => Err(EvalError::TypeError("str_len requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::get — arity 2, takes Str and Int index, returns single-char Str or empty string
        env.define(
            "aven/std/str::get".to_string(),
            Value::NativeFn {
                name: "aven/std/str::get".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(s), Value::Int(idx)) => {
                            if *idx < 0 || *idx >= s.len() as i64 {
                                Ok(Value::Str(String::new()))
                            } else {
                                let c = s.chars().nth(*idx as usize).unwrap_or(' ');
                                Ok(Value::Str(c.to_string()))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_get requires Str and Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::sub — arity 3, takes Str, Int start, Int end, returns substring
        env.define(
            "aven/std/str::sub".to_string(),
            Value::NativeFn {
                name: "aven/std/str::sub".to_string(),
                arity: 3,
                func: Arc::new(|args| {
                    if args.len() != 3 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 3 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1], &args[2]) {
                        (Value::Str(s), Value::Int(start), Value::Int(end)) => {
                            if *start < 0 || *start as usize > s.len() || *end < 0 || *end as usize > s.len() || *start >= *end {
                                Ok(Value::Str(String::new()))
                            } else {
                                let s_start = s.chars().take(*start as usize).count();
                                let s_end = s.chars().take(*end as usize).count();
                                Ok(Value::Str(s[s_start..s_end].to_string()))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_sub requires Str and two Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::trim — arity 1, takes Str, returns trimmed Str
        env.define(
            "aven/std/str::trim".to_string(),
            Value::NativeFn {
                name: "aven/std/str::trim".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => Ok(Value::Str(s.trim().to_string())),
                        _ => Err(EvalError::TypeError("str_trim requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::to_int — arity 1, takes Str, returns Int (0 on parse failure)
        env.define(
            "aven/std/str::to_int".to_string(),
            Value::NativeFn {
                name: "aven/std/str::to_int".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() {
                                Ok(Value::Int(0))
                            } else if let Ok(n) = trimmed.parse::<i64>() {
                                Ok(Value::Int(n))
                            } else {
                                Ok(Value::Int(0))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_to_int requires Str argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::from_int — arity 1, takes Int, returns Str
        env.define(
            "aven/std/str::from_int".to_string(),
            Value::NativeFn {
                name: "aven/std/str::from_int".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Str(n.to_string())),
                        _ => Err(EvalError::TypeError("str_from_int requires Int argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::eq — arity 2, takes two Str arguments, returns Bool
        env.define(
            "aven/std/str::eq".to_string(),
            Value::NativeFn {
                name: "aven/std/str::eq".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a == b)),
                        _ => Err(EvalError::TypeError("str_eq requires two Str arguments".to_string())),
                    }
                }),
            },
        );

        // Short aliases
        env.define(
            "str_len".to_string(),
            Value::NativeFn {
                name: "str_len".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => Ok(Value::Int(s.len() as i64)),
                        _ => Err(EvalError::TypeError("str_len requires Str argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_get".to_string(),
            Value::NativeFn {
                name: "str_get".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(s), Value::Int(idx)) => {
                            if *idx < 0 || *idx >= s.len() as i64 {
                                Ok(Value::Str(String::new()))
                            } else {
                                let c = s.chars().nth(*idx as usize).unwrap_or(' ');
                                Ok(Value::Str(c.to_string()))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_get requires Str and Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::find — arity 2, takes Str s, Str needle, returns Int position or -1
        env.define(
            "aven/std/str::find".to_string(),
            Value::NativeFn {
                name: "aven/std/str::find".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(s), Value::Str(needle)) => {
                            if let Some(pos) = s.find(needle.as_str()) {
                                Ok(Value::Int(pos as i64))
                            } else {
                                Ok(Value::Int(-1))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_find requires two Str arguments".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_find".to_string(),
            Value::NativeFn {
                name: "str_find".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(s), Value::Str(needle)) => {
                            if let Some(pos) = s.find(needle.as_str()) {
                                Ok(Value::Int(pos as i64))
                            } else {
                                Ok(Value::Int(-1))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_find requires two Str arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/str::rest — arity 2, takes Str, Int start, returns substring from start to end
        env.define(
            "aven/std/str::rest".to_string(),
            Value::NativeFn {
                name: "aven/std/str::rest".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(s), Value::Int(start)) => {
                            if *start < 0 || *start as usize > s.len() {
                                Ok(Value::Str(String::new()))
                            } else {
                                let s_start = s.chars().take(*start as usize).count();
                                Ok(Value::Str(s[s_start..].to_string()))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_rest requires Str and Int arguments".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_rest".to_string(),
            Value::NativeFn {
                name: "str_rest".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(s), Value::Int(start)) => {
                            if *start < 0 || *start as usize > s.len() {
                                Ok(Value::Str(String::new()))
                            } else {
                                let s_start = s.chars().take(*start as usize).count();
                                Ok(Value::Str(s[s_start..].to_string()))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_rest requires Str and Int arguments".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_sub".to_string(),
            Value::NativeFn {
                name: "str_sub".to_string(),
                arity: 3,
                func: Arc::new(|args| {
                    if args.len() != 3 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 3 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1], &args[2]) {
                        (Value::Str(s), Value::Int(start), Value::Int(end)) => {
                            if *start < 0 || *start as usize > s.len() || *end < 0 || *end as usize > s.len() || *start >= *end {
                                Ok(Value::Str(String::new()))
                            } else {
                                let s_start = s.chars().take(*start as usize).count();
                                let s_end = s.chars().take(*end as usize).count();
                                Ok(Value::Str(s[s_start..s_end].to_string()))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_sub requires Str and two Int arguments".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_trim".to_string(),
            Value::NativeFn {
                name: "str_trim".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => Ok(Value::Str(s.trim().to_string())),
                        _ => Err(EvalError::TypeError("str_trim requires Str argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_to_int".to_string(),
            Value::NativeFn {
                name: "str_to_int".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Str(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() {
                                Ok(Value::Int(0))
                            } else if let Ok(n) = trimmed.parse::<i64>() {
                                Ok(Value::Int(n))
                            } else {
                                Ok(Value::Int(0))
                            }
                        }
                        _ => Err(EvalError::TypeError("str_to_int requires Str argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_from_int".to_string(),
            Value::NativeFn {
                name: "str_from_int".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Str(n.to_string())),
                        _ => Err(EvalError::TypeError("str_from_int requires Int argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "str_eq".to_string(),
            Value::NativeFn {
                name: "str_eq".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a == b)),
                        _ => Err(EvalError::TypeError("str_eq requires two Str arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::add — arity 2, takes two Ints, returns Int with overflow check
        env.define(
            "aven/std/math::add".to_string(),
            Value::NativeFn {
                name: "aven/std/math::add".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            match a.checked_add(*b) {
                                Some(result) => Ok(Value::Int(result)),
                                None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("add requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::sub — arity 2, takes two Ints, returns Int with overflow check
        env.define(
            "aven/std/math::sub".to_string(),
            Value::NativeFn {
                name: "aven/std/math::sub".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            match a.checked_sub(*b) {
                                Some(result) => Ok(Value::Int(result)),
                                None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("sub requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::mul — arity 2, takes two Ints, returns Int with overflow check
        env.define(
            "aven/std/math::mul".to_string(),
            Value::NativeFn {
                name: "aven/std/math::mul".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            match a.checked_mul(*b) {
                                Some(result) => Ok(Value::Int(result)),
                                None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("mul requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::div — arity 2, takes two Ints, returns Int with division by zero check
        env.define(
            "aven/std/math::div".to_string(),
            Value::NativeFn {
                name: "aven/std/math::div".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            if *b == 0 {
                                Err(EvalError::InvalidOperation("division by zero".to_string()))
                            } else {
                                match a.checked_div(*b) {
                                    Some(result) => Ok(Value::Int(result)),
                                    None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                                }
                            }
                        }
                        _ => Err(EvalError::TypeError("div requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::floor — arity 1, takes Int, returns Int (no-op)
        env.define(
            "aven/std/math::floor".to_string(),
            Value::NativeFn {
                name: "aven/std/math::floor".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Int(*n)),
                        _ => Err(EvalError::TypeError("floor requires Int argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::ceil — arity 1, takes Int, returns Int (no-op)
        env.define(
            "aven/std/math::ceil".to_string(),
            Value::NativeFn {
                name: "aven/std/math::ceil".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Int(*n)),
                        _ => Err(EvalError::TypeError("ceil requires Int argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/math::round — arity 1, takes Int, returns Int (no-op)
        env.define(
            "aven/std/math::round".to_string(),
            Value::NativeFn {
                name: "aven/std/math::round".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Int(*n)),
                        _ => Err(EvalError::TypeError("round requires Int argument".to_string())),
                    }
                }),
            },
        );

        // math_add alias
        env.define(
            "math_add".to_string(),
            Value::NativeFn {
                name: "math_add".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            match a.checked_add(*b) {
                                Some(result) => Ok(Value::Int(result)),
                                None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("add requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // math_sub alias
        env.define(
            "math_sub".to_string(),
            Value::NativeFn {
                name: "math_sub".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            match a.checked_sub(*b) {
                                Some(result) => Ok(Value::Int(result)),
                                None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("sub requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // math_mul alias
        env.define(
            "math_mul".to_string(),
            Value::NativeFn {
                name: "math_mul".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            match a.checked_mul(*b) {
                                Some(result) => Ok(Value::Int(result)),
                                None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                            }
                        }
                        _ => Err(EvalError::TypeError("mul requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // math_div alias
        env.define(
            "math_div".to_string(),
            Value::NativeFn {
                name: "math_div".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => {
                            if *b == 0 {
                                Err(EvalError::InvalidOperation("division by zero".to_string()))
                            } else {
                                match a.checked_div(*b) {
                                    Some(result) => Ok(Value::Int(result)),
                                    None => Err(EvalError::InvalidOperation("arithmetic overflow".to_string())),
                                }
                            }
                        }
                        _ => Err(EvalError::TypeError("div requires Int arguments".to_string())),
                    }
                }),
            },
        );

        // math_floor alias
        env.define(
            "math_floor".to_string(),
            Value::NativeFn {
                name: "math_floor".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Int(*n)),
                        _ => Err(EvalError::TypeError("floor requires Int argument".to_string())),
                    }
                }),
            },
        );

        // math_ceil alias
        env.define(
            "math_ceil".to_string(),
            Value::NativeFn {
                name: "math_ceil".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Int(*n)),
                        _ => Err(EvalError::TypeError("ceil requires Int argument".to_string())),
                    }
                }),
            },
        );

        // math_round alias
        env.define(
            "math_round".to_string(),
            Value::NativeFn {
                name: "math_round".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Int(n) => Ok(Value::Int(*n)),
                        _ => Err(EvalError::TypeError("round requires Int argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::list_new — arity 0, returns empty List
        env.define(
            "aven/std/collections::list_new".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::list_new".to_string(),
                arity: 0,
                func: Arc::new(|args| {
                    if args.len() != 0 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 0 args, got {}",
                            args.len()
                        )));
                    }
                    Ok(Value::List(vec![]))
                }),
            },
        );

        // aven/std/collections::list_push — arity 2, (List, Value) -> new List
        env.define(
            "aven/std/collections::list_push".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::list_push".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::List(list) => {
                            let mut new_list = list.clone();
                            new_list.push(args[1].clone());
                            Ok(Value::List(new_list))
                        }
                        _ => Err(EvalError::TypeError("list_push requires List as first argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::list_pop — arity 1, (List) -> new List without last element
        env.define(
            "aven/std/collections::list_pop".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::list_pop".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::List(list) => {
                            if list.is_empty() {
                                Err(EvalError::InvalidOperation("list_pop: list is empty".to_string()))
                            } else {
                                let mut new_list = list.clone();
                                new_list.pop();
                                Ok(Value::List(new_list))
                            }
                        }
                        _ => Err(EvalError::TypeError("list_pop requires List argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::list_get — arity 2, (List, Int idx) -> element or error
        env.define(
            "aven/std/collections::list_get".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::list_get".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::List(list), Value::Int(idx)) => {
                            if *idx < 0 || *idx as usize >= list.len() {
                                Err(EvalError::InvalidOperation(format!(
                                    "list_get: index {} out of bounds for list of length {}",
                                    idx, list.len()
                                )))
                            } else {
                                Ok(list[*idx as usize].clone())
                            }
                        }
                        (Value::List(_), _) => Err(EvalError::TypeError("list_get requires Int index".to_string())),
                        _ => Err(EvalError::TypeError("list_get requires List as first argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::list_len — arity 1, (List) -> Int length
        env.define(
            "aven/std/collections::list_len".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::list_len".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::List(list) => Ok(Value::Int(list.len() as i64)),
                        _ => Err(EvalError::TypeError("list_len requires List argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::map_new — arity 0, returns empty Map
        env.define(
            "aven/std/collections::map_new".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::map_new".to_string(),
                arity: 0,
                func: Arc::new(|args| {
                    if args.len() != 0 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 0 args, got {}",
                            args.len()
                        )));
                    }
                    Ok(Value::Map(vec![]))
                }),
            },
        );

        // aven/std/collections::map_set — arity 3, (Map, Str key, Value) -> new Map with upsert
        env.define(
            "aven/std/collections::map_set".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::map_set".to_string(),
                arity: 3,
                func: Arc::new(|args| {
                    if args.len() != 3 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 3 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Map(map), Value::Str(key)) => {
                            let mut new_map = map.clone();
                            if let Some(pos) = new_map.iter().position(|(k, _)| k == key) {
                                new_map[pos].1 = args[2].clone();
                            } else {
                                new_map.push((key.clone(), args[2].clone()));
                                new_map.sort_by(|a, b| a.0.cmp(&b.0));
                            }
                            Ok(Value::Map(new_map))
                        }
                        (Value::Map(_), _) => Err(EvalError::TypeError("map_set requires Str key".to_string())),
                        _ => Err(EvalError::TypeError("map_set requires Map as first argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::map_get — arity 2, (Map, Str key) -> Value or Nil
        env.define(
            "aven/std/collections::map_get".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::map_get".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Map(map), Value::Str(key)) => {
                            match map.iter().find(|(k, _)| k == key) {
                                Some((_, v)) => Ok(v.clone()),
                                None => Ok(Value::Nil),
                            }
                        }
                        (Value::Map(_), _) => Err(EvalError::TypeError("map_get requires Str key".to_string())),
                        _ => Err(EvalError::TypeError("map_get requires Map as first argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::map_has — arity 2, (Map, Str key) -> Bool
        env.define(
            "aven/std/collections::map_has".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::map_has".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Map(map), Value::Str(key)) => {
                            let has_key = map.iter().any(|(k, _)| k == key);
                            Ok(Value::Bool(has_key))
                        }
                        (Value::Map(_), _) => Err(EvalError::TypeError("map_has requires Str key".to_string())),
                        _ => Err(EvalError::TypeError("map_has requires Map as first argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::set_new — arity 0, returns empty Set
        env.define(
            "aven/std/collections::set_new".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::set_new".to_string(),
                arity: 0,
                func: Arc::new(|args| {
                    if args.len() != 0 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 0 args, got {}",
                            args.len()
                        )));
                    }
                    Ok(Value::Set(vec![]))
                }),
            },
        );

        // aven/std/collections::set_add — arity 2, (Set, Value) -> new Set with val added if not present
        env.define(
            "aven/std/collections::set_add".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::set_add".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match &args[1] {
                        Value::Fn { .. } | Value::NativeFn { .. } => {
                            return Err(EvalError::TypeError("set_add: function values are not comparable and cannot be added to sets".to_string()));
                        }
                        Value::Float(f) if f.is_nan() => {
                            return Err(EvalError::InvalidOperation("set_add: NaN is not allowed in sets".to_string()));
                        }
                        _ => {}
                    }
                    match &args[0] {
                        Value::Set(set) => {
                            let mut new_set = set.clone();
                            if !new_set.iter().any(|v| v == &args[1]) {
                                new_set.push(args[1].clone());
                            }
                            Ok(Value::Set(new_set))
                        }
                        _ => Err(EvalError::TypeError("set_add requires Set as first argument".to_string())),
                    }
                }),
            },
        );

        // aven/std/collections::set_has — arity 2, (Set, Value) -> Bool
        env.define(
            "aven/std/collections::set_has".to_string(),
            Value::NativeFn {
                name: "aven/std/collections::set_has".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Set(set) => {
                            let has_val = set.iter().any(|v| v == &args[1]);
                            Ok(Value::Bool(has_val))
                        }
                        _ => Err(EvalError::TypeError("set_has requires Set as first argument".to_string())),
                    }
                }),
            },
        );

        // Short aliases for testing
        env.define(
            "col_list_new".to_string(),
            Value::NativeFn {
                name: "col_list_new".to_string(),
                arity: 0,
                func: Arc::new(|args| {
                    if args.len() != 0 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 0 args, got {}",
                            args.len()
                        )));
                    }
                    Ok(Value::List(vec![]))
                }),
            },
        );

        env.define(
            "col_list_push".to_string(),
            Value::NativeFn {
                name: "col_list_push".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::List(list) => {
                            let mut new_list = list.clone();
                            new_list.push(args[1].clone());
                            Ok(Value::List(new_list))
                        }
                        _ => Err(EvalError::TypeError("col_list_push requires List as first argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_list_pop".to_string(),
            Value::NativeFn {
                name: "col_list_pop".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::List(list) => {
                            if list.is_empty() {
                                Err(EvalError::InvalidOperation("col_list_pop: list is empty".to_string()))
                            } else {
                                let mut new_list = list.clone();
                                new_list.pop();
                                Ok(Value::List(new_list))
                            }
                        }
                        _ => Err(EvalError::TypeError("col_list_pop requires List argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_list_get".to_string(),
            Value::NativeFn {
                name: "col_list_get".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::List(list), Value::Int(idx)) => {
                            if *idx < 0 || *idx as usize >= list.len() {
                                Err(EvalError::InvalidOperation(format!(
                                    "col_list_get: index {} out of bounds for list of length {}",
                                    idx, list.len()
                                )))
                            } else {
                                Ok(list[*idx as usize].clone())
                            }
                        }
                        (Value::List(_), _) => Err(EvalError::TypeError("col_list_get requires Int index".to_string())),
                        _ => Err(EvalError::TypeError("col_list_get requires List as first argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_list_len".to_string(),
            Value::NativeFn {
                name: "col_list_len".to_string(),
                arity: 1,
                func: Arc::new(|args| {
                    if args.len() != 1 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 1 arg, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::List(list) => Ok(Value::Int(list.len() as i64)),
                        _ => Err(EvalError::TypeError("col_list_len requires List argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_map_new".to_string(),
            Value::NativeFn {
                name: "col_map_new".to_string(),
                arity: 0,
                func: Arc::new(|args| {
                    if args.len() != 0 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 0 args, got {}",
                            args.len()
                        )));
                    }
                    Ok(Value::Map(vec![]))
                }),
            },
        );

        env.define(
            "col_map_set".to_string(),
            Value::NativeFn {
                name: "col_map_set".to_string(),
                arity: 3,
                func: Arc::new(|args| {
                    if args.len() != 3 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 3 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Map(map), Value::Str(key)) => {
                            let mut new_map = map.clone();
                            if let Some(pos) = new_map.iter().position(|(k, _)| k == key) {
                                new_map[pos].1 = args[2].clone();
                            } else {
                                new_map.push((key.clone(), args[2].clone()));
                                new_map.sort_by(|a, b| a.0.cmp(&b.0));
                            }
                            Ok(Value::Map(new_map))
                        }
                        (Value::Map(_), _) => Err(EvalError::TypeError("col_map_set requires Str key".to_string())),
                        _ => Err(EvalError::TypeError("col_map_set requires Map as first argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_map_get".to_string(),
            Value::NativeFn {
                name: "col_map_get".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Map(map), Value::Str(key)) => {
                            match map.iter().find(|(k, _)| k == key) {
                                Some((_, v)) => Ok(v.clone()),
                                None => Ok(Value::Nil),
                            }
                        }
                        (Value::Map(_), _) => Err(EvalError::TypeError("col_map_get requires Str key".to_string())),
                        _ => Err(EvalError::TypeError("col_map_get requires Map as first argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_map_has".to_string(),
            Value::NativeFn {
                name: "col_map_has".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Map(map), Value::Str(key)) => {
                            let has_key = map.iter().any(|(k, _)| k == key);
                            Ok(Value::Bool(has_key))
                        }
                        (Value::Map(_), _) => Err(EvalError::TypeError("col_map_has requires Str key".to_string())),
                        _ => Err(EvalError::TypeError("col_map_has requires Map as first argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_set_new".to_string(),
            Value::NativeFn {
                name: "col_set_new".to_string(),
                arity: 0,
                func: Arc::new(|args| {
                    if args.len() != 0 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 0 args, got {}",
                            args.len()
                        )));
                    }
                    Ok(Value::Set(vec![]))
                }),
            },
        );

        env.define(
            "col_set_add".to_string(),
            Value::NativeFn {
                name: "col_set_add".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    // Guard: reject Fn and NativeFn
                    match &args[1] {
                        Value::Fn { .. } | Value::NativeFn { .. } => {
                            return Err(EvalError::TypeError("col_set_add: function values are not comparable and cannot be added to sets".to_string()));
                        }
                        Value::Float(f) if f.is_nan() => {
                            return Err(EvalError::InvalidOperation("col_set_add: NaN is not allowed in sets".to_string()));
                        }
                        _ => {}
                    }
                    match &args[0] {
                        Value::Set(set) => {
                            let mut new_set = set.clone();
                            if !new_set.iter().any(|v| v == &args[1]) {
                                new_set.push(args[1].clone());
                            }
                            Ok(Value::Set(new_set))
                        }
                        _ => Err(EvalError::TypeError("col_set_add requires Set as first argument".to_string())),
                    }
                }),
            },
        );

        env.define(
            "col_set_has".to_string(),
            Value::NativeFn {
                name: "col_set_has".to_string(),
                arity: 2,
                func: Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected 2 args, got {}",
                            args.len()
                        )));
                    }
                    match &args[0] {
                        Value::Set(set) => {
                            let has_val = set.iter().any(|v| v == &args[1]);
                            Ok(Value::Bool(has_val))
                        }
                        _ => Err(EvalError::TypeError("col_set_has requires Set as first argument".to_string())),
                    }
                }),
            },
        );
    }

    pub fn with_parent(parent: Env) -> Self {
        Env {
            vars: HashMap::new(),
            context: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }
    
    pub fn define(&mut self, name: String, value: Value) {
        self.vars.insert(name, value);
    }
    
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(val) = self.vars.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    pub fn set_context(&mut self, key: String, value: Value) {
        // Walk parent chain to overwrite existing key; if not found, write to current env.
        if self.context.contains_key(&key) {
            self.context.insert(key, value);
        } else if let Some(parent) = &mut self.parent {
            parent.set_context(key, value);
        } else {
            self.context.insert(key, value);
        }
    }

    pub fn get_context(&self, key: &str) -> Option<Value> {
        if let Some(val) = self.context.get(key) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.get_context(key)
        } else {
            None
        }
    }
}

pub fn eval(expr: &Expr, env: &mut Env) -> Result<Value, EvalError> {
    match expr {
        Expr::Int(n, _, _) => Ok(Value::Int(*n)),
        Expr::Float(n, _, _) => Ok(Value::Float(*n)),
        Expr::Str(s, _, _) => Ok(Value::Str(s.clone())),
        Expr::Bool(b, _, _) => Ok(Value::Bool(*b)),
        Expr::Symbol(s, _, _) => Ok(Value::Symbol(s.clone())),
        Expr::Nil => Ok(Value::Nil),

        Expr::Var(name, _, _) => {
            env.get(name)
                .ok_or_else(|| EvalError::UndefinedVariable(name.clone()))
        }

        Expr::Let { name, value, .. } => {
            let val = eval(value, env)?;
            env.define(name.clone(), val.clone());
            Ok(val)
        }

        Expr::FnDef {
            name,
            params,
            body,
            return_type: _,
            effect_level: _,
            cap: _,
            ..
        } => {
            let fn_value = Value::Fn {
                params: params.clone(),
                body: (**body).clone(),
                closure_env: Env::new(),
            };
            env.define(name.clone(), fn_value.clone());
            Ok(fn_value)
        }

        Expr::FnCall { name, args, .. } => {
            let func = env.get(name)
                .ok_or_else(|| EvalError::UndefinedVariable(name.clone()))?;

            match func {
                Value::Fn {
                    params,
                    body,
                    closure_env: _,
                } => {
                    if params.len() != args.len() {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected {} args, got {}",
                            params.len(),
                            args.len()
                        )));
                    }

                    let mut call_env = Env::with_parent(env.clone());

                    for ((param_name, _), arg) in params.iter().zip(args.iter()) {
                        let arg_val = eval(arg, env)?;
                        call_env.define(param_name.clone(), arg_val);
                    }

                    eval(&body, &mut call_env)
                }
                Value::NativeFn { arity, func: native_func, .. } => {
                    if args.len() != arity {
                        return Err(EvalError::InvalidFunctionCall(format!(
                            "Expected {} args, got {}",
                            arity,
                            args.len()
                        )));
                    }

                    let mut arg_vals = Vec::new();
                    for arg in args {
                        arg_vals.push(eval(arg, env)?);
                    }

                    native_func(&arg_vals)
                }
                _ => Err(EvalError::InvalidFunctionCall(format!(
                    "{} is not a function",
                    name
                ))),
            }
        }

        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_val = eval(cond, env)?;
            match cond_val {
                Value::Bool(true) => eval(then_branch, env),
                Value::Bool(false) => eval(else_branch, env),
                _ => Err(EvalError::TypeError(
                    "Condition must be a boolean".to_string()
                )),
            }
        }

        Expr::Arithmetic {
            op,
            left,
            right,
            ..
        } => {
            let left_val = eval(left, env)?;
            let right_val = eval(right, env)?;

            match (left_val, right_val, op) {
                (Value::Int(l), Value::Int(r), ArithOp::Add) => Ok(Value::Int(l + r)),
                (Value::Int(l), Value::Int(r), ArithOp::Sub) => Ok(Value::Int(l - r)),
                (Value::Int(l), Value::Int(r), ArithOp::Mul) => Ok(Value::Int(l * r)),
                (Value::Int(l), Value::Int(r), ArithOp::Div) => {
                    if r == 0 {
                        Err(EvalError::DivisionByZero)
                    } else {
                        Ok(Value::Int(l / r))
                    }
                }
                (Value::Float(l), Value::Float(r), ArithOp::Add) => Ok(Value::Float(l + r)),
                (Value::Float(l), Value::Float(r), ArithOp::Sub) => Ok(Value::Float(l - r)),
                (Value::Float(l), Value::Float(r), ArithOp::Mul) => Ok(Value::Float(l * r)),
                (Value::Float(l), Value::Float(r), ArithOp::Div) => {
                    if r == 0.0 {
                        Err(EvalError::DivisionByZero)
                    } else {
                        Ok(Value::Float(l / r))
                    }
                }
                (Value::Str(l), Value::Str(r), ArithOp::Add) => {
                    Ok(Value::Str(format!("{}{}", l, r)))
                }
                _ => Err(EvalError::InvalidOperation(format!(
                    "Cannot apply {:?} to these types",
                    op
                ))),
            }
        }

        Expr::Ret(expr, _, _) => eval(expr, env),

        Expr::IoWrite(expr, _, _) => {
            let val = eval(expr, env)?;
            println!("{}", val);
            Ok(Value::Nil)
        }

        Expr::Block(exprs, _, _) => {
            let mut result = Value::Nil;
            for expr in exprs {
                result = eval(expr, env)?;
            }
            Ok(result)
        }

        Expr::Intent(_, _, _) => Ok(Value::Nil),

        Expr::Uncertain(inner, _, _) => eval(inner, env),

        Expr::Ctx { .. } => Ok(Value::Nil),

        Expr::CtxGet { ctx: _, key, .. } => {
            let key_val = eval(key, env)?;
            match key_val {
                Value::Str(key_str) => {
                    Ok(env.get_context(&key_str).unwrap_or(Value::Nil))
                }
                _ => Err(EvalError::TypeError("Context key must be a string".to_string())),
            }
        }

        Expr::CtxSet { ctx: _, key, value, .. } => {
            let key_val = eval(key, env)?;
            let val = eval(value, env)?;
            match key_val {
                Value::Str(key_str) => {
                    env.set_context(key_str, val.clone());
                    Ok(val)
                }
                _ => Err(EvalError::TypeError("Context key must be a string".to_string())),
            }
        }

        Expr::Diff { ops: _, .. } => {
            // M5.4: Diff evaluates to Nil. apply_diff is available as internal API; tested directly in unit tests.
            Ok(Value::Nil)
        }

        Expr::Use { .. } => Ok(Value::Nil),

        Expr::Mod { .. } => Ok(Value::Nil),

        Expr::Pub { .. } => Ok(Value::Nil),

        Expr::PubDecl { inner, .. } => eval(inner, env),

        Expr::TypeAlias { .. } => Ok(Value::Nil),

        Expr::Record { fields, .. } => {
            let mut record_fields = Vec::new();
            for (key, value_expr) in fields {
                let val = eval(value_expr, env)?;
                record_fields.push((key.clone(), val));
            }
            Ok(Value::Record(record_fields))
        }

        Expr::List { elements, .. } => {
            let mut list_values = Vec::new();
            for elem in elements {
                let val = eval(elem, env)?;
                list_values.push(val);
            }
            Ok(Value::List(list_values))
        }

        Expr::Match {
            scrutinee,
            patterns,
            ..
        } => {
            let scrutinee_val = eval(scrutinee, env)?;

            // Iterate through patterns to find a match
            for (pattern, body) in patterns {
                match pattern {
                    Pattern::Tag(tag) => {
                        // Match tagged values or symbols
                        match &scrutinee_val {
                            Value::Tagged(val_tag, _) => {
                                if val_tag == tag {
                                    return eval(body, env);
                                }
                            }
                            Value::Symbol(sym) => {
                                // Extract tag from symbol (e.g., "#admin" -> "admin")
                                if sym.starts_with('#') && &sym[1..] == tag {
                                    return eval(body, env);
                                }
                            }
                            _ => {}
                        }
                    }
                    Pattern::TagBind(tag, var) => {
                        if let Value::Tagged(val_tag, payload) = &scrutinee_val {
                            if val_tag == tag {
                                let mut child_env = Env::with_parent(env.clone());
                                let payload_val = payload.as_ref().map(|p| (**p).clone()).unwrap_or(Value::Nil);
                                child_env.define(var.clone(), payload_val);
                                return eval(body, &mut child_env);
                            }
                        }
                    }
                    Pattern::Wildcard => {
                        return eval(body, env);
                    }
                }
            }

            Err(EvalError::InvalidOperation(format!(
                "No matching pattern for value: {}",
                scrutinee_val
            )))
        }

        Expr::Tagged {
            tag,
            payload,
            ..
        } => {
            if let Some(payload_expr) = payload {
                let payload_val = eval(payload_expr, env)?;
                Ok(Value::Tagged(tag.clone(), Some(Box::new(payload_val))))
            } else {
                Ok(Value::Tagged(tag.clone(), None))
            }
        }
    }
}

/// Recursively find and return a mutable reference to a node by selector path matching NodeId.
fn find_node_by_selector<'a>(expr: &'a mut Expr, selector: &SelectorPath) -> Option<&'a mut Expr> {
    if selector.parts.is_empty() {
        return None;
    }

    let mut current = expr;

    for segment in &selector.parts {
        match segment {
            PathSegment::Named(name) => {
                // Walk into named fields based on the field name
                match current {
                    Expr::Let { value, .. } if name == "value" => {
                        current = value.as_mut();
                    }
                    Expr::Let { .. } => return None,

                    Expr::FnDef { body, .. } if name == "body" => {
                        current = body.as_mut();
                    }
                    Expr::FnDef { .. } => return None,

                    Expr::If { cond, .. } if name == "cond" => {
                        current = cond.as_mut();
                    }
                    Expr::If { then_branch, .. } if name == "then_branch" => {
                        current = then_branch.as_mut();
                    }
                    Expr::If { else_branch, .. } if name == "else_branch" => {
                        current = else_branch.as_mut();
                    }
                    Expr::If { .. } => return None,

                    Expr::Arithmetic { left, .. } if name == "left" => {
                        current = left.as_mut();
                    }
                    Expr::Arithmetic { right, .. } if name == "right" => {
                        current = right.as_mut();
                    }
                    Expr::Arithmetic { .. } => return None,

                    Expr::Ret(inner, _, _) if name == "ret" => {
                        current = inner.as_mut();
                    }
                    Expr::Ret(_, _, _) => return None,

                    Expr::IoWrite(inner, _, _) if name == "write" => {
                        current = inner.as_mut();
                    }
                    Expr::IoWrite(_, _, _) => return None,

                    Expr::Uncertain(inner, _, _) if name == "uncertain" => {
                        current = inner.as_mut();
                    }
                    Expr::Uncertain(_, _, _) => return None,

                    Expr::CtxGet { key, .. } if name == "key" => {
                        current = key.as_mut();
                    }
                    Expr::CtxGet { .. } => return None,

                    Expr::Match { scrutinee, .. } if name == "scrutinee" => {
                        current = scrutinee.as_mut();
                    }
                    Expr::Match { .. } => return None,

                    Expr::Tagged { payload, .. } if name == "payload" => {
                        if let Some(p) = payload {
                            current = p.as_mut();
                        } else {
                            return None;
                        }
                    }
                    Expr::Tagged { .. } => return None,

                    // Entity-name matching: when current is a Block, try to find a child with matching name
                    Expr::Block(ref mut exprs, _, _) => {
                        let matched_idx = exprs.iter().position(|e| match e {
                            Expr::FnDef { name: fn_name, .. } => fn_name == name,
                            Expr::Let { name: let_name, .. } => let_name == name,
                            Expr::Mod { name: mod_name, .. } => mod_name.to_string() == *name,
                            _ => false,
                        });
                        if let Some(idx) = matched_idx {
                            current = &mut exprs[idx];
                        } else {
                            return None;
                        }
                    }

                    _ => return None,
                };
            }

            PathSegment::Index(idx) => {
                // Index into a vector (for Block)
                match current {
                    Expr::Block(ref mut exprs, _, _) => {
                        if *idx < exprs.len() {
                            current = &mut exprs[*idx];
                        } else {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
        }
    }

    Some(current)
}

/// Apply a sequence of diff operations to an expression, mutating it in-place.
fn apply_diff(expr: &mut Expr, ops: &[DiffOp]) -> Result<(), EvalError> {
    for op in ops {
        match &op.kind {
            DiffKind::Replace => {
                if let Some(payload) = &op.payload {
                    if let Some(target) = find_node_by_selector(expr, &op.selector) {
                        *target = (**payload).clone();
                    } else {
                        return Err(EvalError::InvalidOperation(
                            format!("Cannot find node matching selector: {}", op.selector),
                        ));
                    }
                } else {
                    return Err(EvalError::InvalidOperation(
                        "Replace requires a payload".to_string(),
                    ));
                }
            }

            DiffKind::Delete => {
                if let Some(target) = find_node_by_selector(expr, &op.selector) {
                    *target = Expr::Nil;
                } else {
                    return Err(EvalError::InvalidOperation(
                        format!("Cannot find node matching selector: {}", op.selector),
                    ));
                }
            }

            DiffKind::Insert => {
                if let Some(payload) = &op.payload {
                    if let Some(insert_mode) = &op.insert_mode {
                        if let Some(target) = find_node_by_selector(expr, &op.selector) {
                            // Target must be a Block for Insert to work
                            if let Expr::Block(ref mut exprs, _, _) = target {
                                let payload_clone = (**payload).clone();
                                match insert_mode {
                                    crate::ast::InsertMode::First => {
                                        exprs.insert(0, payload_clone);
                                    }
                                    crate::ast::InsertMode::Last => {
                                        exprs.push(payload_clone);
                                    }
                                    crate::ast::InsertMode::Before(name) => {
                                        if let Ok(idx) = name.parse::<usize>() {
                                            if idx < exprs.len() {
                                                exprs.insert(idx, payload_clone);
                                            } else {
                                                return Err(EvalError::InvalidOperation(
                                                    format!("Index {} out of bounds", idx),
                                                ));
                                            }
                                        } else {
                                            return Err(EvalError::InvalidOperation(
                                                format!("Invalid index: {}", name),
                                            ));
                                        }
                                    }
                                    crate::ast::InsertMode::After(name) => {
                                        if let Ok(idx) = name.parse::<usize>() {
                                            if idx < exprs.len() {
                                                exprs.insert(idx + 1, payload_clone);
                                            } else {
                                                return Err(EvalError::InvalidOperation(
                                                    format!("Index {} out of bounds", idx),
                                                ));
                                            }
                                        } else {
                                            return Err(EvalError::InvalidOperation(
                                                format!("Invalid index: {}", name),
                                            ));
                                        }
                                    }
                                }
                            } else {
                                return Err(EvalError::InvalidOperation(
                                    "Insert target must be a Block".to_string(),
                                ));
                            }
                        } else {
                            return Err(EvalError::InvalidOperation(
                                format!("Cannot find node matching selector: {}", op.selector),
                            ));
                        }
                    } else {
                        return Err(EvalError::InvalidOperation(
                            "Insert requires an insert_mode".to_string(),
                        ));
                    }
                } else {
                    return Err(EvalError::InvalidOperation(
                        "Insert requires a payload".to_string(),
                    ));
                }
            }

            DiffKind::Move => {
                if let Some(insert_mode) = &op.insert_mode {
                    // Parse anchor from insert_mode (Before/After specify anchor, not destination)
                    let (anchor_path, is_after) = match insert_mode {
                        crate::ast::InsertMode::Before(path_str) => {
                            let normalized = if !path_str.starts_with('/') {
                                format!("/{}", path_str)
                            } else {
                                path_str.clone()
                            };
                            match crate::ast::SelectorPath::from_string(&normalized) {
                                Ok(p) => (p, false),
                                Err(e) => return Err(EvalError::InvalidOperation(e)),
                            }
                        }
                        crate::ast::InsertMode::After(path_str) => {
                            let normalized = if !path_str.starts_with('/') {
                                format!("/{}", path_str)
                            } else {
                                path_str.clone()
                            };
                            match crate::ast::SelectorPath::from_string(&normalized) {
                                Ok(p) => (p, true),
                                Err(e) => return Err(EvalError::InvalidOperation(e)),
                            }
                        }
                        _ => {
                            return Err(EvalError::InvalidOperation(
                                "Move requires Before or After mode with anchor path".to_string(),
                            ));
                        }
                    };

                    // Per spec §2.5: anchor must exist
                    if find_node_by_selector(expr, &anchor_path).is_none() {
                        return Err(EvalError::InvalidOperation(
                            format!("anchor node not found: {}", anchor_path),
                        ));
                    }

                    // Extract source
                    if let Some(source) = find_node_by_selector(expr, &op.selector) {
                        let source_clone = source.clone();
                        *source = Expr::Nil;

                        // Parse parent path by dropping the last segment of anchor_path
                        if anchor_path.parts.is_empty() {
                            return Err(EvalError::InvalidOperation(
                                "anchor path cannot be empty".to_string(),
                            ));
                        }

                        let parent_path = SelectorPath {
                            parts: anchor_path.parts[..anchor_path.parts.len() - 1].to_vec(),
                        };

                        // Find parent node
                        if parent_path.parts.is_empty() {
                            // Move to root-level Block
                            if let Expr::Block(ref mut exprs, _, _) = expr {
                                let mut insert_idx = if let PathSegment::Index(idx) = &anchor_path.parts[0] {
                                    *idx
                                } else {
                                    // For entity names at root level, append at end
                                    exprs.len()
                                };
                                // Per spec §2.5: After inserts at anchor_index + 1
                                if is_after {
                                    insert_idx += 1;
                                }
                                if insert_idx <= exprs.len() {
                                    exprs.insert(insert_idx, source_clone);
                                } else {
                                    return Err(EvalError::InvalidOperation(
                                        format!("insert index {} out of bounds", insert_idx),
                                    ));
                                }
                            } else {
                                return Err(EvalError::InvalidOperation(
                                    "anchor parent not found: root is not a Block".to_string(),
                                ));
                            }
                        } else if let Some(parent) = find_node_by_selector(expr, &parent_path) {
                            // Insert into parent's child list
                            if let Expr::Block(ref mut exprs, _, _) = parent {
                                let mut insert_idx = if let PathSegment::Index(idx) = &anchor_path.parts[anchor_path.parts.len() - 1] {
                                    *idx
                                } else {
                                    exprs.len()
                                };
                                // Per spec §2.5: After inserts at anchor_index + 1
                                if is_after {
                                    insert_idx += 1;
                                }
                                if insert_idx <= exprs.len() {
                                    exprs.insert(insert_idx, source_clone);
                                } else {
                                    return Err(EvalError::InvalidOperation(
                                        format!("insert index {} out of bounds", insert_idx),
                                    ));
                                }
                            } else {
                                return Err(EvalError::InvalidOperation(
                                    "anchor parent is not a Block".to_string(),
                                ));
                            }
                        } else {
                            return Err(EvalError::InvalidOperation(
                                format!("anchor parent not found: {}", parent_path),
                            ));
                        }
                    } else {
                        return Err(EvalError::InvalidOperation(
                            format!("Cannot find node matching selector: {}", op.selector),
                        ));
                    }
                } else {
                    return Err(EvalError::InvalidOperation(
                        "Move requires insert_mode with anchor".to_string(),
                    ));
                }
            }

            DiffKind::Copy => {
                if let Some(insert_mode) = &op.insert_mode {
                    // Parse anchor from insert_mode (Before/After specify anchor, not destination)
                    let (anchor_path, is_after) = match insert_mode {
                        crate::ast::InsertMode::Before(path_str) => {
                            let normalized = if !path_str.starts_with('/') {
                                format!("/{}", path_str)
                            } else {
                                path_str.clone()
                            };
                            match crate::ast::SelectorPath::from_string(&normalized) {
                                Ok(p) => (p, false),
                                Err(e) => return Err(EvalError::InvalidOperation(e)),
                            }
                        }
                        crate::ast::InsertMode::After(path_str) => {
                            let normalized = if !path_str.starts_with('/') {
                                format!("/{}", path_str)
                            } else {
                                path_str.clone()
                            };
                            match crate::ast::SelectorPath::from_string(&normalized) {
                                Ok(p) => (p, true),
                                Err(e) => return Err(EvalError::InvalidOperation(e)),
                            }
                        }
                        _ => {
                            return Err(EvalError::InvalidOperation(
                                "Copy requires Before or After mode with anchor path".to_string(),
                            ));
                        }
                    };

                    // Per spec §2.5: anchor must exist
                    if find_node_by_selector(expr, &anchor_path).is_none() {
                        return Err(EvalError::InvalidOperation(
                            format!("anchor node not found: {}", anchor_path),
                        ));
                    }

                    // Extract source (do not modify it)
                    if let Some(source) = find_node_by_selector(expr, &op.selector) {
                        let source_clone = source.clone();

                        // Parse parent path by dropping the last segment of anchor_path
                        if anchor_path.parts.is_empty() {
                            return Err(EvalError::InvalidOperation(
                                "anchor path cannot be empty".to_string(),
                            ));
                        }

                        let parent_path = SelectorPath {
                            parts: anchor_path.parts[..anchor_path.parts.len() - 1].to_vec(),
                        };

                        // Find parent node
                        if parent_path.parts.is_empty() {
                            // Copy to root-level Block
                            if let Expr::Block(ref mut exprs, _, _) = expr {
                                let mut insert_idx = if let PathSegment::Index(idx) = &anchor_path.parts[0] {
                                    *idx
                                } else {
                                    exprs.len()
                                };
                                // Per spec §2.5: After inserts at anchor_index + 1
                                if is_after {
                                    insert_idx += 1;
                                }
                                if insert_idx <= exprs.len() {
                                    exprs.insert(insert_idx, source_clone);
                                } else {
                                    return Err(EvalError::InvalidOperation(
                                        format!("insert index {} out of bounds", insert_idx),
                                    ));
                                }
                            } else {
                                return Err(EvalError::InvalidOperation(
                                    "anchor parent not found: root is not a Block".to_string(),
                                ));
                            }
                        } else if let Some(parent) = find_node_by_selector(expr, &parent_path) {
                            // Insert into parent's child list
                            if let Expr::Block(ref mut exprs, _, _) = parent {
                                let mut insert_idx = if let PathSegment::Index(idx) = &anchor_path.parts[anchor_path.parts.len() - 1] {
                                    *idx
                                } else {
                                    exprs.len()
                                };
                                // Per spec §2.5: After inserts at anchor_index + 1
                                if is_after {
                                    insert_idx += 1;
                                }
                                if insert_idx <= exprs.len() {
                                    exprs.insert(insert_idx, source_clone);
                                } else {
                                    return Err(EvalError::InvalidOperation(
                                        format!("insert index {} out of bounds", insert_idx),
                                    ));
                                }
                            } else {
                                return Err(EvalError::InvalidOperation(
                                    "anchor parent is not a Block".to_string(),
                                ));
                            }
                        } else {
                            return Err(EvalError::InvalidOperation(
                                format!("anchor parent not found: {}", parent_path),
                            ));
                        }
                    } else {
                        return Err(EvalError::InvalidOperation(
                            format!("Cannot find node matching selector: {}", op.selector),
                        ));
                    }
                } else {
                    return Err(EvalError::InvalidOperation(
                        "Copy requires insert_mode with anchor".to_string(),
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Apply a batch of diff operations atomically: clone the expression, apply all ops,
/// and only commit the result if all succeed. On any error, return the original expression unchanged.
/// After successful application, re-run type checking on the mutated AST; if it fails, return
/// an error and discard the clone (implicit rollback).
pub fn diffs_apply(root: &mut Expr, ops: &[DiffOp]) -> Result<(), EvalError> {
    let mut clone = root.clone();
    match apply_diff(&mut clone, ops) {
        Ok(()) => {
            // apply_diff succeeded; now typecheck the mutated clone
            match crate::typechecker::typecheck(&clone, &crate::typechecker::TypeEnv::new()) {
                Ok(_) => {
                    // Typecheck succeeded; commit the clone to root
                    *root = clone;
                    Ok(())
                }
                Err(type_err) => {
                    // Typecheck failed; discard clone (implicit rollback)
                    Err(EvalError::TypecheckFailed(type_err.message))
                }
            }
        }
        Err(e) => {
            // Rollback: leave root untouched, return error
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{SourceSpan, EffectSet};
    
    #[test]
    fn test_eval_int() {
        let expr = Expr::Int(42, 0, SourceSpan::zero());
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_eval_string() {
        let expr = Expr::Str("hello".to_string(), 0, SourceSpan::zero());
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Str("hello".to_string()));
    }

    #[test]
    fn test_eval_let() {
        let expr = Expr::Let {
            name: "x".to_string(),
            value: Box::new(Expr::Int(10, 0, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let mut env = Env::new();
        eval(&expr, &mut env).unwrap();
        assert_eq!(env.get("x"), Some(Value::Int(10)));
    }

    #[test]
    fn test_eval_var_lookup() {
        let mut env = Env::new();
        env.define("x".to_string(), Value::Int(42));
        let expr = Expr::Var("x".to_string(), 0, SourceSpan::zero());
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_eval_addition() {
        let expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(2, 0, SourceSpan::zero())),
            right: Box::new(Expr::Int(3, 0, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_eval_if_true() {
        let expr = Expr::If {
            cond: Box::new(Expr::Bool(true, 0, SourceSpan::zero())),
            then_branch: Box::new(Expr::Int(1, 0, SourceSpan::zero())),
            else_branch: Box::new(Expr::Int(0, 0, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_eval_intent_is_nil() {
        let expr = Expr::Intent(
            "annotate the surrounding code".to_string(),
            0,
            SourceSpan::zero(),
        );
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn test_eval_uncertain_pass_through() {
        let expr = Expr::Uncertain(
            Box::new(Expr::Int(42, 0, SourceSpan::zero())),
            0,
            SourceSpan::zero(),
        );
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_eval_uncertain_wraps_arithmetic() {
        let expr = Expr::Uncertain(
            Box::new(Expr::Arithmetic {
                op: ArithOp::Add,
                left: Box::new(Expr::Int(2, 0, SourceSpan::zero())),
                right: Box::new(Expr::Int(3, 0, SourceSpan::zero())),
                node_id: 0,
                span: SourceSpan::zero(),
            }),
            0,
            SourceSpan::zero(),
        );
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_eval_ctx_is_nil_placeholder() {
        let expr = Expr::Ctx {
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn test_eval_diff_is_nil_in_m1() {
        let expr = Expr::Diff {
            metadata: None,
            ops: vec![],
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn test_eval_function_def_and_call() {
        let mut env = Env::new();

        let fn_expr = Expr::FnDef {
            name: "double".to_string(),
            params: vec![("n".to_string(), Some(crate::ast::Type::Primitive(crate::ast::PrimitiveType::Int)))],
            body: Box::new(Expr::Arithmetic {
                op: ArithOp::Mul,
                left: Box::new(Expr::Var("n".to_string(), 0, SourceSpan::zero())),
                right: Box::new(Expr::Int(2, 0, SourceSpan::zero())),
                node_id: 0,
                span: SourceSpan::zero(),
            }),
            return_type: Some(crate::ast::Type::Primitive(crate::ast::PrimitiveType::Int)),
            effect_level: crate::ast::EffectSet::pure_(),
            cap: vec![],
            node_id: 0,
            span: SourceSpan::zero(),
        };
        eval(&fn_expr, &mut env).unwrap();

        let call_expr = Expr::FnCall {
            name: "double".to_string(),
            args: vec![Expr::Int(5, 0, SourceSpan::zero())],
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let result = eval(&call_expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(10));
    }

    #[test]
    fn test_eval_preserves_spans() {
        let expr = Expr::Int(42, 0, SourceSpan::new(3, 5));
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_eval_ignores_nodeid() {
        let expr1 = Expr::Int(42, 1, SourceSpan::zero());
        let expr2 = Expr::Int(42, 2, SourceSpan::zero());
        let mut env = Env::new();
        let result1 = eval(&expr1, &mut env).unwrap();
        let result2 = eval(&expr2, &mut env).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_apply_diff_replace_int() {
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(42, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(3, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let op = DiffOp {
            kind: DiffKind::Replace,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("right".to_string())],
            },
            payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
            insert_mode: None,
            node_id: 0,
            span: SourceSpan::zero(),
        };
        apply_diff(&mut expr, &[op]).unwrap();
        if let Expr::Arithmetic { right, .. } = expr {
            if let Expr::Int(val, _, _) = *right {
                assert_eq!(val, 99);
            } else {
                panic!("Expected Int after replace");
            }
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_apply_diff_delete_sets_nil() {
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(42, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(3, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let op = DiffOp {
            kind: DiffKind::Delete,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("right".to_string())],
            },
            payload: None,
            insert_mode: None,
            node_id: 0,
            span: SourceSpan::zero(),
        };
        apply_diff(&mut expr, &[op]).unwrap();
        if let Expr::Arithmetic { right, .. } = expr {
            assert!(matches!(*right, Expr::Nil));
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_apply_diff_selector_not_found_errors() {
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(42, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(3, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let op = DiffOp {
            kind: DiffKind::Replace,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("nonexistent".to_string())],
            },
            payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
            insert_mode: None,
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let result = apply_diff(&mut expr, &[op]);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_diff_multiple_ops() {
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(42, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(3, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };
        let ops = vec![
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("left".to_string())],
                },
                payload: Some(Box::new(Expr::Int(100, 3, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
            DiffOp {
                kind: DiffKind::Delete,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("right".to_string())],
                },
                payload: None,
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
        ];
        apply_diff(&mut expr, &ops).unwrap();
        if let Expr::Arithmetic { left, right, .. } = expr {
            if let Expr::Int(val, _, _) = *left {
                assert_eq!(val, 100);
            } else {
                panic!("Expected Int after first replace");
            }
            assert!(matches!(*right, Expr::Nil));
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_apply_diff_index_into_block() {
        let mut expr = Expr::Block(vec![
            Expr::Int(10, 0, SourceSpan::zero()),
            Expr::Int(20, 1, SourceSpan::zero()),
            Expr::Int(30, 2, SourceSpan::zero()),
        ], 0, SourceSpan::zero());

        let op = DiffOp {
            kind: DiffKind::Replace,
            selector: SelectorPath {
                parts: vec![PathSegment::Index(1)],
            },
            payload: Some(Box::new(Expr::Int(999, 3, SourceSpan::zero()))),
            insert_mode: None,
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::Block(children, _, _) = expr {
            if let Expr::Int(val, _, _) = children[1] {
                assert_eq!(val, 999);
            } else {
                panic!("Expected Int at index 1");
            }
        } else {
            panic!("Expected Block expr");
        }
    }

    #[test]
    fn test_apply_diff_index_out_of_bounds() {
        let mut expr = Expr::Block(vec![
            Expr::Int(10, 0, SourceSpan::zero()),
            Expr::Int(20, 1, SourceSpan::zero()),
            Expr::Int(30, 2, SourceSpan::zero()),
        ], 0, SourceSpan::zero());

        let op = DiffOp {
            kind: DiffKind::Replace,
            selector: SelectorPath {
                parts: vec![PathSegment::Index(10)],
            },
            payload: Some(Box::new(Expr::Int(999, 3, SourceSpan::zero()))),
            insert_mode: None,
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let result = apply_diff(&mut expr, &[op]);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_diff_insert_first() {
        let mut expr = Expr::If {
            cond: Box::new(Expr::Bool(true, 10, SourceSpan::zero())),
            then_branch: Box::new(Expr::Block(vec![
                Expr::Int(10, 0, SourceSpan::zero()),
                Expr::Int(20, 1, SourceSpan::zero()),
                Expr::Int(30, 2, SourceSpan::zero()),
            ], 5, SourceSpan::zero())),
            else_branch: Box::new(Expr::Nil),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let op = DiffOp {
            kind: DiffKind::Insert,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("then_branch".to_string())],
            },
            payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
            insert_mode: Some(crate::ast::InsertMode::First),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::If { then_branch, .. } = expr {
            if let Expr::Block(children, _, _) = *then_branch {
                assert_eq!(children.len(), 4);
                if let Expr::Int(val, _, _) = children[0] {
                    assert_eq!(val, 99);
                } else {
                    panic!("Expected Int at index 0");
                }
            } else {
                panic!("Expected Block in then_branch");
            }
        } else {
            panic!("Expected If expr");
        }
    }

    #[test]
    fn test_apply_diff_insert_last() {
        let mut expr = Expr::If {
            cond: Box::new(Expr::Bool(true, 10, SourceSpan::zero())),
            then_branch: Box::new(Expr::Block(vec![
                Expr::Int(10, 0, SourceSpan::zero()),
                Expr::Int(20, 1, SourceSpan::zero()),
                Expr::Int(30, 2, SourceSpan::zero()),
            ], 5, SourceSpan::zero())),
            else_branch: Box::new(Expr::Nil),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let op = DiffOp {
            kind: DiffKind::Insert,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("then_branch".to_string())],
            },
            payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
            insert_mode: Some(crate::ast::InsertMode::Last),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::If { then_branch, .. } = expr {
            if let Expr::Block(children, _, _) = *then_branch {
                assert_eq!(children.len(), 4);
                if let Expr::Int(val, _, _) = children[3] {
                    assert_eq!(val, 99);
                } else {
                    panic!("Expected Int at index 3");
                }
            } else {
                panic!("Expected Block in then_branch");
            }
        } else {
            panic!("Expected If expr");
        }
    }

    #[test]
    fn test_apply_diff_insert_before() {
        let mut expr = Expr::If {
            cond: Box::new(Expr::Bool(true, 10, SourceSpan::zero())),
            then_branch: Box::new(Expr::Block(vec![
                Expr::Int(10, 0, SourceSpan::zero()),
                Expr::Int(20, 1, SourceSpan::zero()),
                Expr::Int(30, 2, SourceSpan::zero()),
            ], 5, SourceSpan::zero())),
            else_branch: Box::new(Expr::Nil),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let op = DiffOp {
            kind: DiffKind::Insert,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("then_branch".to_string())],
            },
            payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
            insert_mode: Some(crate::ast::InsertMode::Before("1".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::If { then_branch, .. } = expr {
            if let Expr::Block(children, _, _) = *then_branch {
                assert_eq!(children.len(), 4);
                if let Expr::Int(val, _, _) = children[1] {
                    assert_eq!(val, 99);
                } else {
                    panic!("Expected Int(99) at index 1");
                }
                if let Expr::Int(val, _, _) = children[2] {
                    assert_eq!(val, 20);
                } else {
                    panic!("Expected Int(20) at index 2");
                }
            } else {
                panic!("Expected Block in then_branch");
            }
        } else {
            panic!("Expected If expr");
        }
    }

    #[test]
    fn test_apply_diff_insert_after() {
        let mut expr = Expr::If {
            cond: Box::new(Expr::Bool(true, 10, SourceSpan::zero())),
            then_branch: Box::new(Expr::Block(vec![
                Expr::Int(10, 0, SourceSpan::zero()),
                Expr::Int(20, 1, SourceSpan::zero()),
                Expr::Int(30, 2, SourceSpan::zero()),
            ], 5, SourceSpan::zero())),
            else_branch: Box::new(Expr::Nil),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let op = DiffOp {
            kind: DiffKind::Insert,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("then_branch".to_string())],
            },
            payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
            insert_mode: Some(crate::ast::InsertMode::After("1".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::If { then_branch, .. } = expr {
            if let Expr::Block(children, _, _) = *then_branch {
                assert_eq!(children.len(), 4);
                if let Expr::Int(val, _, _) = children[2] {
                    assert_eq!(val, 99);
                } else {
                    panic!("Expected Int(99) at index 2");
                }
                if let Expr::Int(val, _, _) = children[3] {
                    assert_eq!(val, 30);
                } else {
                    panic!("Expected Int(30) at index 3");
                }
            } else {
                panic!("Expected Block in then_branch");
            }
        } else {
            panic!("Expected If expr");
        }
    }

    #[test]
    fn test_apply_diff_move_source_to_nil() {
        // M5.6 change: Move now requires destination path to NOT exist and inserts into parent's Block child list.
        // This test verifies that when Move target is on an Arithmetic (non-Block), it errors appropriately.
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let op = DiffOp {
            kind: DiffKind::Move,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("right".to_string())],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::Before("/newfield".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        // This should fail because Arithmetic's parent path is empty and root is not a Block
        let result = apply_diff(&mut expr, &[op]);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_diff_copy_leaves_source() {
        // Copy now requires anchor to exist; this test verifies Copy leaves source unchanged.
        let mut expr = Expr::Block(
            vec![
                Expr::Let {
                    name: "x".to_string(),
                    value: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
                Expr::Let {
                    name: "y".to_string(),
                    value: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
                    node_id: 2,
                    span: SourceSpan::zero(),
                },
            ],
            0,
            SourceSpan::zero(),
        );

        // Copy x before anchor y at [1] — inserts at [1], shifting y right
        let op = DiffOp {
            kind: DiffKind::Copy,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("x".to_string())],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::Before("[1]".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::Block(children, _, _) = expr {
            assert_eq!(children.len(), 3);
            // x at [0] should still be intact (Copy leaves source)
            if let Expr::Let { name, .. } = &children[0] {
                assert_eq!(name, "x");
            } else {
                panic!("Expected Let 'x' at [0]");
            }
            // Copy of x should be at [1]
            if let Expr::Let { name, .. } = &children[1] {
                assert_eq!(name, "x");
            } else {
                panic!("Expected Let 'x' (copy) at [1]");
            }
            // y at [2] should still be intact but shifted
            if let Expr::Let { name, .. } = &children[2] {
                assert_eq!(name, "y");
            } else {
                panic!("Expected Let 'y' at [2]");
            }
        } else {
            panic!("Expected Block expr");
        }
    }

    #[test]
    fn test_diffs_apply_atomicity_rollback() {
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let ops = vec![
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("left".to_string())],
                },
                payload: Some(Box::new(Expr::Int(100, 3, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("nonexistent".to_string())],
                },
                payload: Some(Box::new(Expr::Int(200, 4, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
        ];

        let result = diffs_apply(&mut expr, &ops);
        assert!(result.is_err());

        // Verify original expr is unchanged
        if let Expr::Arithmetic { left, .. } = expr {
            if let Expr::Int(val, _, _) = *left {
                assert_eq!(val, 10);  // Original value preserved
            } else {
                panic!("Expected Int(10) at left");
            }
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_diffs_apply_atomicity_first_op_mutates_then_rollback() {
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let ops = vec![
            // First op: valid Replace on /right with Int(99) — this would succeed
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("right".to_string())],
                },
                payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
            // Second op: Replace on /nonexistent — this will fail
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("nonexistent".to_string())],
                },
                payload: Some(Box::new(Expr::Int(1, 4, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
        ];

        let result = diffs_apply(&mut expr, &ops);
        assert!(result.is_err());

        // Verify first op's mutation was rolled back: right should still be Int(20)
        if let Expr::Arithmetic { right, .. } = expr {
            if let Expr::Int(val, _, _) = *right {
                assert_eq!(val, 20);  // Unchanged — ops[0]'s mutation was rolled back
            } else {
                panic!("Expected Int(20) at right");
            }
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_diffs_apply_success_commits() {
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let ops = vec![
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("left".to_string())],
                },
                payload: Some(Box::new(Expr::Int(100, 3, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("right".to_string())],
                },
                payload: Some(Box::new(Expr::Int(200, 4, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
        ];

        diffs_apply(&mut expr, &ops).unwrap();

        // Verify mutations were committed
        if let Expr::Arithmetic { left, right, .. } = expr {
            if let Expr::Int(val, _, _) = *left {
                assert_eq!(val, 100);
            } else {
                panic!("Expected Int(100) at left");
            }
            if let Expr::Int(val, _, _) = *right {
                assert_eq!(val, 200);
            } else {
                panic!("Expected Int(200) at right");
            }
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_entity_name_selector_fn_greet() {
        // Create a Block containing a FnDef named "greet"
        let mut expr = Expr::Block(
            vec![
                Expr::FnDef {
                    name: "greet".to_string(),
                    params: vec![],
                    body: Box::new(Expr::Str("hello".to_string(), 1, SourceSpan::zero())),
                    return_type: None,
                    effect_level: EffectSet { err: false, io: false, async_: false },
                    cap: vec![],
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
                Expr::Int(42, 2, SourceSpan::zero()),
            ],
            0,
            SourceSpan::zero(),
        );

        // Selector /fn greet should resolve to the FnDef
        let selector = SelectorPath {
            parts: vec![PathSegment::Named("greet".to_string())],
        };

        let found = find_node_by_selector(&mut expr, &selector);
        assert!(found.is_some());
        if let Some(Expr::FnDef { name, .. }) = found {
            assert_eq!(name, "greet");
        } else {
            panic!("Expected FnDef named 'greet'");
        }
    }

    #[test]
    fn test_entity_name_selector_let_x() {
        // Create a Block containing a Let with name "x"
        let mut expr = Expr::Block(
            vec![
                Expr::Let {
                    name: "x".to_string(),
                    value: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
                Expr::Var("x".to_string(), 2, SourceSpan::zero()),
            ],
            0,
            SourceSpan::zero(),
        );

        // Selector /let x should resolve to the Let
        let selector = SelectorPath {
            parts: vec![PathSegment::Named("x".to_string())],
        };

        let found = find_node_by_selector(&mut expr, &selector);
        assert!(found.is_some());
        if let Some(Expr::Let { name, .. }) = found {
            assert_eq!(name, "x");
        } else {
            panic!("Expected Let named 'x'");
        }
    }

    #[test]
    fn test_entity_name_selector_nested() {
        // Create nested structure: Block -> FnDef "f1" -> Block -> Let "y"
        let mut expr = Expr::Block(
            vec![
                Expr::FnDef {
                    name: "f1".to_string(),
                    params: vec![],
                    body: Box::new(Expr::Block(
                        vec![
                            Expr::Let {
                                name: "y".to_string(),
                                value: Box::new(Expr::Int(99, 3, SourceSpan::zero())),
                                node_id: 2,
                                span: SourceSpan::zero(),
                            },
                        ],
                        1,
                        SourceSpan::zero(),
                    )),
                    return_type: None,
                    effect_level: EffectSet { err: false, io: false, async_: false },
                    cap: vec![],
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
            ],
            0,
            SourceSpan::zero(),
        );

        // Selector /f1/body/y should resolve to the Let "y" inside f1's body
        let selector = SelectorPath {
            parts: vec![
                PathSegment::Named("f1".to_string()),
                PathSegment::Named("body".to_string()),
                PathSegment::Named("y".to_string()),
            ],
        };

        let found = find_node_by_selector(&mut expr, &selector);
        assert!(found.is_some());
        if let Some(Expr::Let { name, .. }) = found {
            assert_eq!(name, "y");
        } else {
            panic!("Expected Let named 'y'");
        }
    }

    #[test]
    fn test_move_with_valid_anchor() {
        // Test that Move before an existing anchor works (anchor must exist)
        let mut expr = Expr::Block(
            vec![
                Expr::Let {
                    name: "x".to_string(),
                    value: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
                Expr::Let {
                    name: "y".to_string(),
                    value: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
                    node_id: 2,
                    span: SourceSpan::zero(),
                },
            ],
            0,
            SourceSpan::zero(),
        );

        // Move /x before anchor /y at [1] — anchor exists, so this is valid
        let op = DiffOp {
            kind: DiffKind::Move,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("x".to_string())],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::Before("[1]".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let result = apply_diff(&mut expr, &[op]);
        assert!(result.is_ok(), "Move with existing anchor should succeed");
    }

    #[test]
    fn test_move_inserts_into_parent_child_list() {
        // Create a Block with multiple Let bindings
        let mut expr = Expr::Block(
            vec![
                Expr::Let {
                    name: "x".to_string(),
                    value: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
                Expr::Let {
                    name: "y".to_string(),
                    value: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
                    node_id: 2,
                    span: SourceSpan::zero(),
                },
                Expr::Let {
                    name: "z".to_string(),
                    value: Box::new(Expr::Int(30, 3, SourceSpan::zero())),
                    node_id: 3,
                    span: SourceSpan::zero(),
                },
            ],
            0,
            SourceSpan::zero(),
        );

        // Move /x before anchor at [2] (z exists) — inserts at [2], shifting z right
        let op = DiffOp {
            kind: DiffKind::Move,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("x".to_string())],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::Before("[2]".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::Block(children, _, _) = expr {
            assert_eq!(children.len(), 4);
            // After moving x before [2] (z): [0]=Nil (was x), [1]=y, [2]=x (moved), [3]=z (shifted)
            if let Expr::Nil = &children[0] {
                // OK: source became Nil
            } else {
                panic!("Expected source to become Nil");
            }
            if let Expr::Let { name, .. } = &children[2] {
                assert_eq!(name, "x", "Expected Let 'x' at [2] after move before z");
            } else {
                panic!("Expected Let 'x' at [2] after move");
            }
            if let Expr::Let { name, .. } = &children[3] {
                assert_eq!(name, "z", "Expected Let 'z' at [3] after move");
            } else {
                panic!("Expected Let 'z' at [3] after move");
            }
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_copy_inserts_into_parent_child_list() {
        // Create a Block with multiple Let bindings
        let mut expr = Expr::Block(
            vec![
                Expr::Let {
                    name: "x".to_string(),
                    value: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
                Expr::Let {
                    name: "y".to_string(),
                    value: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
                    node_id: 2,
                    span: SourceSpan::zero(),
                },
            ],
            0,
            SourceSpan::zero(),
        );

        // Copy /x before anchor y at [1] — inserts at [1], shifting y right
        let op = DiffOp {
            kind: DiffKind::Copy,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("x".to_string())],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::Before("[1]".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::Block(children, _, _) = expr {
            assert_eq!(children.len(), 3);
            // x should still be intact at [0]
            if let Expr::Let { name, .. } = &children[0] {
                assert_eq!(name, "x");
            } else {
                panic!("Expected Let 'x' at [0]");
            }
            // Copy inserted at [1]
            if let Expr::Let { name, .. } = &children[1] {
                assert_eq!(name, "x");
            } else {
                panic!("Expected Let 'x' (copy) at [1]");
            }
            // y should be shifted right to [2]
            if let Expr::Let { name, .. } = &children[2] {
                assert_eq!(name, "y");
            } else {
                panic!("Expected Let 'y' at [2]");
            }
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_entity_name_with_move_copy() {
        // Create Block with FnDef and Let
        let mut expr = Expr::Block(
            vec![
                Expr::FnDef {
                    name: "greet".to_string(),
                    params: vec![],
                    body: Box::new(Expr::Str("hello".to_string(), 1, SourceSpan::zero())),
                    return_type: None,
                    effect_level: EffectSet { err: false, io: false, async_: false },
                    cap: vec![],
                    node_id: 1,
                    span: SourceSpan::zero(),
                },
                Expr::Let {
                    name: "z".to_string(),
                    value: Box::new(Expr::Int(99, 2, SourceSpan::zero())),
                    node_id: 2,
                    span: SourceSpan::zero(),
                },
            ],
            0,
            SourceSpan::zero(),
        );

        // Copy /greet before anchor z at [1] — inserts at [1], shifting z right
        let op = DiffOp {
            kind: DiffKind::Copy,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("greet".to_string())],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::Before("[1]".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::Block(children, _, _) = expr {
            assert_eq!(children.len(), 3);
            if let Expr::FnDef { name, .. } = &children[0] {
                assert_eq!(name, "greet");
            } else {
                panic!("Expected FnDef 'greet' at [0]");
            }
            if let Expr::FnDef { name, .. } = &children[1] {
                assert_eq!(name, "greet");
            } else {
                panic!("Expected FnDef 'greet' (copy) at [1]");
            }
            if let Expr::Let { name, .. } = &children[2] {
                assert_eq!(name, "z");
            } else {
                panic!("Expected Let 'z' at [2] after copy");
            }
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_move_source_not_found_errors() {
        let mut expr = Expr::Block(
            vec![
                Expr::Let {
                    name: "x".to_string(),
                    value: Box::new(Expr::Int(1, 0, SourceSpan::zero())),
                    node_id: 0,
                    span: SourceSpan::zero(),
                },
            ],
            0,
            SourceSpan::zero(),
        );

        let op = DiffOp {
            kind: DiffKind::Move,
            selector: SelectorPath {
                parts: vec![PathSegment::Named("nonexistent".to_string())],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::Before("[0]".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let result = apply_diff(&mut expr, &[op]);
        assert!(result.is_err());
        if let Err(EvalError::InvalidOperation(msg)) = result {
            assert!(msg.contains("Cannot find node matching selector"));
        } else {
            panic!("Expected InvalidOperation error with 'Cannot find node matching selector'");
        }
    }

    #[test]
    fn test_copy_insert_after_offsets_by_one() {
        // Test that After([anchor]) inserts at anchor_index + 1
        // Create Block with 3 children: [0]=Int 10, [1]=Int 20, [2]=Int 30
        // Copy selector [0] (Int 10) using After([0]) (anchor [0] exists)
        // Expected: copy inserted at [0]+1=[1], Block becomes [10, 10, 20, 30]
        let mut expr = Expr::Block(
            vec![
                Expr::Int(10, 0, SourceSpan::zero()),
                Expr::Int(20, 0, SourceSpan::zero()),
                Expr::Int(30, 0, SourceSpan::zero()),
            ],
            0,
            SourceSpan::zero(),
        );

        let op = DiffOp {
            kind: DiffKind::Copy,
            selector: SelectorPath {
                parts: vec![PathSegment::Index(0)],
            },
            payload: Some(Box::new(Expr::Nil)),
            insert_mode: Some(crate::ast::InsertMode::After("[0]".to_string())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        apply_diff(&mut expr, &[op]).unwrap();

        if let Expr::Block(children, _, _) = expr {
            assert_eq!(children.len(), 4, "Block should have 4 children after Copy After");
            match (&children[0], &children[1], &children[2], &children[3]) {
                (Expr::Int(10, _, _), Expr::Int(10, _, _), Expr::Int(20, _, _), Expr::Int(30, _, _)) => {
                    // After([0]) correctly inserted copy at [1]
                }
                _ => panic!("Expected [10, 10, 20, 30] but got {:?}",
                    children.iter().map(|e| {
                        if let Expr::Int(n, _, _) = e { Some(n) } else { None }
                    }).collect::<Vec<_>>()
                ),
            }
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_diffs_apply_type_check_success() {
        // Test that diffs_apply successfully applies a diff when typecheck passes
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let ops = vec![
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("right".to_string())],
                },
                payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
        ];

        // diffs_apply should succeed: diff applies OK, typecheck succeeds
        let result = diffs_apply(&mut expr, &ops);
        assert!(result.is_ok(), "diffs_apply should succeed when typecheck passes");

        // Verify mutation was committed
        if let Expr::Arithmetic { right, .. } = expr {
            if let Expr::Int(val, _, _) = *right {
                assert_eq!(val, 99);
            } else {
                panic!("Expected Int(99) at right");
            }
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_diffs_apply_type_check_fails_rejects() {
        // Test that diffs_apply rejects if typecheck fails after a valid diff.
        // The diff itself succeeds structurally (Bool is a valid Expr), but the result
        // is type-invalid (Arithmetic requires Int operands, not Bool).
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let ops = vec![
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("right".to_string())],
                },
                payload: Some(Box::new(Expr::Bool(true, 3, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
        ];

        // apply_diff succeeds (Bool is structurally valid at /right),
        // but typecheck fails (Arithmetic requires Int, not Bool).
        let result = diffs_apply(&mut expr, &ops);
        assert!(
            matches!(result, Err(EvalError::TypecheckFailed(_))),
            "diffs_apply should return TypecheckFailed when operand type is invalid"
        );

        // Verify original expr is unchanged (rollback occurred)
        if let Expr::Arithmetic { right, .. } = expr {
            if let Expr::Int(val, _, _) = *right {
                assert_eq!(val, 20);  // Original value preserved after rollback
            } else {
                panic!("Expected Int(20) at right after rollback");
            }
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_diffs_apply_type_check_fails_is_atomic() {
        // Test that if all ops succeed structurally on the clone but typecheck fails,
        // the entire batch is rolled back and root is unmodified (no partial commit).
        let mut expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(Expr::Int(10, 1, SourceSpan::zero())),
            right: Box::new(Expr::Int(20, 2, SourceSpan::zero())),
            node_id: 0,
            span: SourceSpan::zero(),
        };

        let ops = vec![
            // Op 1: Replace /right with Int(99) — succeeds, clone now has right=99
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("right".to_string())],
                },
                payload: Some(Box::new(Expr::Int(99, 3, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
            // Op 2: Replace /left with Bool(false) — succeeds on clone, but final AST fails typecheck
            DiffOp {
                kind: DiffKind::Replace,
                selector: SelectorPath {
                    parts: vec![PathSegment::Named("left".to_string())],
                },
                payload: Some(Box::new(Expr::Bool(false, 4, SourceSpan::zero()))),
                insert_mode: None,
                node_id: 0,
                span: SourceSpan::zero(),
            },
        ];

        // Both ops apply cleanly to the clone, but final typecheck fails
        // (Arithmetic with Bool left operand is invalid).
        let result = diffs_apply(&mut expr, &ops);
        assert!(
            matches!(result, Err(EvalError::TypecheckFailed(_))),
            "diffs_apply should return TypecheckFailed after multi-op failure"
        );

        // Verify entire batch was rolled back: both operands retain original values
        if let Expr::Arithmetic { left, right, .. } = expr {
            if let Expr::Int(lval, _, _) = *left {
                assert_eq!(lval, 10, "left should retain original value after rollback");
            } else {
                panic!("Expected Int(10) at left after rollback");
            }
            if let Expr::Int(rval, _, _) = *right {
                assert_eq!(rval, 20, "right should retain original value after rollback");
            } else {
                panic!("Expected Int(20) at right after rollback");
            }
        } else {
            panic!("Expected Arithmetic expr");
        }
    }

    #[test]
    fn test_native_fn_display_format() {
        // Test that NativeFn displays as <native:name>
        let val = Value::NativeFn {
            name: "abs".to_string(),
            arity: 1,
            func: Arc::new(|_| Err(EvalError::InvalidOperation("stub".to_string()))),
        };
        assert_eq!(format!("{}", val), "<native:abs>");
    }

    #[test]
    fn test_native_fn_equality_always_false() {
        // Test that two NativeFns are never equal (even with same name/arity)
        let fn1 = Value::NativeFn {
            name: "abs".to_string(),
            arity: 1,
            func: Arc::new(|_| Err(EvalError::InvalidOperation("stub1".to_string()))),
        };
        let fn2 = Value::NativeFn {
            name: "abs".to_string(),
            arity: 1,
            func: Arc::new(|_| Err(EvalError::InvalidOperation("stub2".to_string()))),
        };
        assert_ne!(fn1, fn2);
    }

    #[test]
    fn test_module_qualified_abs_lookup() {
        // Test that module-qualified abs can be fetched from Env::new()
        let env = Env::new();
        let abs_fn = env.get("aven/std/math::abs");
        assert!(abs_fn.is_some());
        if let Some(Value::NativeFn { name, arity, .. }) = abs_fn {
            assert_eq!(name, "aven/std/math::abs");
            assert_eq!(arity, 1);
        } else {
            panic!("Expected NativeFn for aven/std/math::abs");
        }
    }

    #[test]
    fn test_module_qualified_abs_invoke() {
        // Test that module-qualified abs can be invoked
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("aven/std/math::abs") {
            let result = func(&[Value::Int(5)]);
            assert_eq!(result, Ok(Value::Int(5)));
            let neg_result = func(&[Value::Int(-5)]);
            assert_eq!(neg_result, Ok(Value::Int(5)));
        } else {
            panic!("Expected NativeFn for aven/std/math::abs");
        }
    }

    #[test]
    fn test_abs_overflow_i64_min() {
        // Test that abs(i64::MIN) returns error instead of panicking
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("abs") {
            let result = func(&[Value::Int(i64::MIN)]);
            assert!(result.is_err());
            match result {
                Err(EvalError::InvalidOperation(msg)) if msg.contains("overflow") => {}
                _ => panic!("Expected InvalidOperation error for abs(i64::MIN)"),
            }
        } else {
            panic!("Expected abs function");
        }
    }

    #[test]
    fn test_pow_negative_exponent_error() {
        // Test that pow with negative exponent returns error
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("pow") {
            let result = func(&[Value::Int(2), Value::Int(-1)]);
            assert!(result.is_err());
            match result {
                Err(EvalError::InvalidOperation(msg)) if msg.contains("non-negative") => {}
                _ => panic!("Expected InvalidOperation error for pow with negative exponent"),
            }
        } else {
            panic!("Expected pow function");
        }
    }

    #[test]
    fn test_pow_exponent_too_large() {
        // Test that pow with exponent > u32::MAX returns error
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("pow") {
            let result = func(&[Value::Int(2), Value::Int(u32::MAX as i64 + 1)]);
            assert!(result.is_err());
            match result {
                Err(EvalError::InvalidOperation(msg)) if msg.contains("too large") => {}
                _ => panic!("Expected InvalidOperation error for pow exponent too large"),
            }
        } else {
            panic!("Expected pow function");
        }
    }

    #[test]
    fn test_sqrt_negative_argument_error() {
        // Test that sqrt with negative argument returns error
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("sqrt") {
            let result = func(&[Value::Int(-4)]);
            assert!(result.is_err());
            match result {
                Err(EvalError::InvalidOperation(msg)) if msg.contains("non-negative") => {}
                _ => panic!("Expected InvalidOperation error for sqrt with negative argument"),
            }
        } else {
            panic!("Expected sqrt function");
        }
    }

    #[test]
    fn test_sqrt_precision_exceeded() {
        // Test that sqrt with input > 2^53 returns error
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("sqrt") {
            let large_n = (1_i64 << 53) + 1;
            let result = func(&[Value::Int(large_n)]);
            assert!(result.is_err());
            match result {
                Err(EvalError::InvalidOperation(msg)) if msg.contains("f64 precision") => {}
                _ => panic!("Expected InvalidOperation error for sqrt exceeding f64 precision"),
            }
        } else {
            panic!("Expected sqrt function");
        }
    }

    #[test]
    fn test_abs_type_error() {
        // Test that abs with wrong type returns TypeError
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("abs") {
            let result = func(&[Value::Str("hello".to_string())]);
            assert!(result.is_err());
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("Expected TypeError for abs on Str"),
            }
        } else {
            panic!("Expected abs function");
        }
    }

    #[test]
    fn test_abs_arity_mismatch() {
        // Test that abs with wrong arity returns error
        let env = Env::new();
        if let Some(Value::NativeFn { func, .. }) = env.get("abs") {
            let result = func(&[Value::Int(5), Value::Int(3)]);
            assert!(result.is_err());
            match result {
                Err(EvalError::InvalidFunctionCall(msg)) if msg.contains("Expected 1 arg") => {}
                _ => panic!("Expected InvalidFunctionCall error for abs arity mismatch"),
            }
        } else {
            panic!("Expected abs function");
        }
    }

    #[test]
    fn test_read_line_from_lf() {
        use std::io::Cursor;
        let mut reader = Cursor::new("hello\n");
        let result = read_line_from(&mut reader);
        assert_eq!(result.unwrap(), Value::Str("hello".to_string()));
    }

    #[test]
    fn test_read_line_from_crlf() {
        use std::io::Cursor;
        let mut reader = Cursor::new("hello\r\n");
        let result = read_line_from(&mut reader);
        assert_eq!(result.unwrap(), Value::Str("hello".to_string()));
    }

    #[test]
    fn test_read_line_from_eof() {
        use std::io::Cursor;
        let mut reader = Cursor::new("");
        let result = read_line_from(&mut reader);
        assert_eq!(result.unwrap(), Value::Str("".to_string()));
    }

    #[test]
    fn test_list_display_string_elements() {
        use crate::parser::Parser;

        let code = r#"["a" "b"]"#;
        let mut parser = Parser::new(code).unwrap();
        let expr = parser.parse().unwrap();
        let mut env = Env::new();
        let result = eval(&expr, &mut env).unwrap();
        let display_str = format!("{}", result);
        assert_eq!(display_str, r#"["a" "b"]"#, "String elements in list should be quoted");
    }
}
