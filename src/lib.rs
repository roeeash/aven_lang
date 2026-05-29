pub mod ast;
pub mod lexer;
pub mod parser;
pub mod eval;
pub mod typechecker;
pub mod fmt;

pub use parser::{Parser, ParseError};
pub use ast::{Expr, SourceSpan, IntentTable, IntentEntry, SelectorPath, PathSegment, DiffOp, DiffKind, InsertMode, DiffMetadata, Type, PrimitiveType};
pub use eval::{eval, Env, Value, EvalError};
pub use typechecker::{typecheck, typecheck_str, TypeError, TypeEnv, partition_by_module, build_module_caps_map, build_module_dependency_dag, detect_cycles, topological_sort, typecheck_program_ordered, check_uncertainty, UncertainViolation};
pub use fmt::format_expr;
pub use std::sync::Arc;
use std::collections::HashMap;

pub fn source_to_line_col(source: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 0usize;
    let mut current_offset = 0usize;

    for ch in source.chars() {
        if current_offset >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        current_offset += ch.len_utf8();
    }

    (line, col)
}

#[derive(Debug)]
pub enum RunError {
    Parse(ParseError),
    Eval(EvalError),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RunError::Parse(e) => write!(f, "Parse error: {}", e),
            RunError::Eval(e) => write!(f, "Eval error: {}", e),
        }
    }
}

pub fn parse_str(input: &str) -> Result<ast::Expr, ParseError> {
    let mut parser = Parser::new(input)?;
    parser.parse()
}

pub fn run_str(input: &str) -> Result<Value, RunError> {
    let mut parser = Parser::new(input).map_err(RunError::Parse)?;
    let expr = parser.parse().map_err(RunError::Parse)?;
    let mut env = Env::new();
    eval(&expr, &mut env).map_err(RunError::Eval)
}

pub fn run_str_with_context(input: &str, context: HashMap<String, Value>) -> Result<Value, RunError> {
    let mut parser = Parser::new(input).map_err(RunError::Parse)?;
    let expr = parser.parse().map_err(RunError::Parse)?;
    let mut env = Env::new();
    for (key, value) in context {
        env.set_context(key, value);
    }
    eval(&expr, &mut env).map_err(RunError::Eval)
}

pub fn run_str_with_env(input: &str, env: &mut Env) -> Result<Value, RunError> {
    let mut parser = Parser::new(input).map_err(RunError::Parse)?;
    let expr = parser.parse().map_err(RunError::Parse)?;
    eval(&expr, env).map_err(RunError::Eval)
}

pub fn intent_index(source: &str) -> Result<IntentTable, ParseError> {
    let mut parser = Parser::new(source)?;
    let _expr = parser.parse()?;
    Ok(parser.get_intent_table())
}

pub fn format_intent_output(table: &IntentTable, source: &str) -> Vec<String> {
    let mut entries = table.entries.clone();
    // Stable sort: first by selector, then by source position to ensure deterministic output.
    entries.sort_by(|a, b| {
        a.selector.cmp(&b.selector)
            .then_with(|| a.subtree_span.start.cmp(&b.subtree_span.start))
    });

    entries.iter().map(|entry| {
        let (line, col) = source_to_line_col(source, entry.subtree_span.start);
        format!("{} {} {}:{}", entry.selector, entry.intent_name, line, col + 1)
    }).collect()
}

pub fn patch_file_to_diffs(text: &str) -> Result<Vec<DiffOp>, ParseError> {
    let mut parser = Parser::new(text)?;
    parser.parse_patch_file()
}

pub fn diffs_to_avenpatch_string(ops: &[DiffOp], target_path: &str) -> String {
    let mut result = format!("@patch-for path:\"{}\"\n", target_path);

    let mut i = 0;
    while i < ops.len() {
        let op = &ops[i];
        result.push_str("@diff ");

        // Check if this is a Move/Copy pair (two consecutive ops with same kind)
        if (op.kind == DiffKind::Move || op.kind == DiffKind::Copy) &&
           i + 1 < ops.len() &&
           ops[i + 1].kind == op.kind {
            let keyword = if op.kind == DiffKind::Move { "@move" } else { "@copy" };
            result.push_str(keyword);
            result.push_str(" ");
            result.push_str(&format!("{}", op.selector));
            result.push_str(" @to ");
            result.push_str(&format!("{}", ops[i + 1].selector));
            result.push('\n');
            i += 2; // Skip both ops
        } else {
            match &op.kind {
                DiffKind::Replace => {
                    result.push_str("@replace ");
                    result.push_str(&format!("{} ", op.selector));
                    if let Some(payload) = &op.payload {
                        result.push_str(&format!("{}", expr_to_string(payload)));
                    }
                }
                DiffKind::Insert => {
                    result.push_str("@insert ");
                    if let Some(insert_mode) = &op.insert_mode {
                        match insert_mode {
                            InsertMode::First => result.push_str("@first "),
                            InsertMode::Last => result.push_str("@last "),
                            InsertMode::Before(name) => result.push_str(&format!("@before {} ", name)),
                            InsertMode::After(name) => result.push_str(&format!("@after {} ", name)),
                        }
                    }
                    result.push_str(&format!("{} ", op.selector));
                    if let Some(payload) = &op.payload {
                        result.push_str(&format!("{}", expr_to_string(payload)));
                    }
                }
                DiffKind::Delete => {
                    result.push_str("@delete ");
                    result.push_str(&format!("{}", op.selector));
                }
                DiffKind::Move | DiffKind::Copy => {
                    // Unpaired Move/Copy (shouldn't happen in well-formed input, but handle it)
                    let keyword = if op.kind == DiffKind::Move { "@move" } else { "@copy" };
                    result.push_str(keyword);
                    result.push_str(" ");
                    result.push_str(&format!("{}", op.selector));
                    result.push_str(" @to ");
                    result.push_str("(missing destination)");
                }
            }
            result.push('\n');
            i += 1;
        }
    }

    result
}

fn expr_to_string(expr: &ast::Expr) -> String {
    match expr {
        ast::Expr::Int(n, ..) => n.to_string(),
        ast::Expr::Bool(b, ..) => format!("@{}", b),
        ast::Expr::Str(s, ..) => {
            // Escape backslashes and quotes
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        }
        ast::Expr::Float(f, ..) => {
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') || s.contains('E') { s } else { format!("{}.0", s) }
        }
        ast::Expr::Symbol(s, ..) => format!("#{}", s),
        ast::Expr::Nil => "@nil".to_string(),
        ast::Expr::Arithmetic { left, op, right, .. } => {
            let op_str = match op {
                ast::ArithOp::Add => "+",
                ast::ArithOp::Sub => "-",
                ast::ArithOp::Mul => "*",
                ast::ArithOp::Div => "/",
            };
            format!("({} {} {})", expr_to_string(left), op_str, expr_to_string(right))
        }
        ast::Expr::Block(exprs, ..) => {
            let inner = exprs.iter().map(expr_to_string).collect::<Vec<_>>().join("; ");
            format!("{{ {} }}", inner)
        }
        _ => "<expr>".to_string(),
    }
}
