pub mod ast;
pub mod lexer;
pub mod parser;
pub mod eval;
pub mod typechecker;
pub mod fmt;

pub use parser::{Parser, ParseError};
pub use ast::{Expr, SourceSpan, IntentTable, IntentEntry, SelectorPath, PathSegment, DiffOp, DiffKind, InsertMode, DiffMetadata, Type, PrimitiveType};
pub use eval::{eval, Env, Value, EvalError};
pub use lexer::Token;
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

pub fn emit_tokens_str(input: &str) -> Vec<Token> {
    let mut lexer = lexer::Lexer::new(input);
    lexer.tokenize()
}

pub fn emit_ast_str(input: &str) -> Result<Expr, ParseError> {
    parse_str(input)
}

pub fn canonical_token_dump(tokens: &[Token]) -> String {
    tokens.iter().map(|token| {
        match token {
            Token::At => "At".to_string(),
            Token::Hash => "Hash".to_string(),
            Token::Underscore => "Underscore".to_string(),
            Token::Let => "Let".to_string(),
            Token::Fn => "Fn".to_string(),
            Token::Type => "Type".to_string(),
            Token::Ret => "Ret".to_string(),
            Token::If => "If".to_string(),
            Token::Then => "Then".to_string(),
            Token::Else => "Else".to_string(),
            Token::True => "True".to_string(),
            Token::False => "False".to_string(),
            Token::IoWrite => "IoWrite".to_string(),
            Token::Call => "Call".to_string(),
            Token::Cap => "Cap".to_string(),
            Token::Use => "Use".to_string(),
            Token::From => "From".to_string(),
            Token::Mod => "Mod".to_string(),
            Token::Pub => "Pub".to_string(),
            Token::Match => "Match".to_string(),
            Token::Ok => "Ok".to_string(),
            Token::Err => "Err".to_string(),
            Token::As => "As".to_string(),
            Token::Intent => "Intent".to_string(),
            Token::Uncertain => "Uncertain".to_string(),
            Token::Ctx => "Ctx".to_string(),
            Token::CtxGet => "CtxGet".to_string(),
            Token::CtxSet => "CtxSet".to_string(),
            Token::Diff => "Diff".to_string(),
            Token::Diffs => "Diffs".to_string(),
            Token::Replace => "Replace".to_string(),
            Token::Insert => "Insert".to_string(),
            Token::Delete => "Delete".to_string(),
            Token::Move => "Move".to_string(),
            Token::Copy => "Copy".to_string(),
            Token::Meta => "Meta".to_string(),
            Token::To => "To".to_string(),
            Token::First => "First".to_string(),
            Token::Last => "Last".to_string(),
            Token::Before => "Before".to_string(),
            Token::After => "After".to_string(),
            Token::PatchFor => "PatchFor".to_string(),
            Token::Colon => "Colon".to_string(),
            Token::DoubleColon => "DoubleColon".to_string(),
            Token::Equals => "Equals".to_string(),
            Token::Arrow => "Arrow".to_string(),
            Token::EffectArrow(effect_set) => {
                let sigils = format_effect_sigils(effect_set);
                format!("EffectArrow {}", sigils)
            },
            Token::Comma => "Comma".to_string(),
            Token::LeftParen => "LeftParen".to_string(),
            Token::RightParen => "RightParen".to_string(),
            Token::LeftBrace => "LeftBrace".to_string(),
            Token::RightBrace => "RightBrace".to_string(),
            Token::LeftBracket => "LeftBracket".to_string(),
            Token::RightBracket => "RightBracket".to_string(),
            Token::Pipe => "Pipe".to_string(),
            Token::Plus => "Plus".to_string(),
            Token::Minus => "Minus".to_string(),
            Token::Star => "Star".to_string(),
            Token::Slash => "Slash".to_string(),
            Token::Question => "Question".to_string(),
            Token::Ident(s) => format!("Ident {}", s),
            Token::Integer(n) => format!("Integer {}", n),
            Token::Float(f) => format!("Float {}", f),
            Token::String(s) => format!("String \"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Token::Eof => "Eof".to_string(),
            Token::Newline => "Newline".to_string(),
        }
    }).collect::<Vec<_>>().join("\n")
}

fn format_effect_sigils(effect_set: &ast::EffectSet) -> String {
    let mut sigils = String::new();
    if effect_set.err {
        sigils.push('?');
    }
    if effect_set.io {
        sigils.push('!');
    }
    if effect_set.async_ {
        sigils.push('~');
    }
    if sigils.is_empty() {
        "[]".to_string()
    } else {
        sigils
    }
}

fn format_type_canonical(ty: &ast::Type) -> String {
    match ty {
        ast::Type::Primitive(prim) => match prim {
            ast::PrimitiveType::Int => "Int".to_string(),
            ast::PrimitiveType::Bool => "Bool".to_string(),
            ast::PrimitiveType::Str => "Str".to_string(),
            ast::PrimitiveType::Flt => "Flt".to_string(),
            ast::PrimitiveType::Nil => "Nil".to_string(),
        },
        ast::Type::Fn { params, return_type, effect, cap } => {
            let param_strs: Vec<String> = params.iter().map(format_type_canonical).collect();
            let ret = format_type_canonical(return_type);
            let effect_str = format_effect_sigils(effect);
            let cap_str = cap.as_ref().map(|c| c.as_str()).unwrap_or("");
            format!("(Fn ({}) {} {}{})", param_strs.join(" "), ret, effect_str,
                    if cap_str.is_empty() { String::new() } else { format!(" {}", cap_str) })
        },
        ast::Type::Option(inner) => format!("(Option {})", format_type_canonical(inner)),
        ast::Type::List(inner) => format!("(List {})", format_type_canonical(inner)),
        ast::Type::Record(fields) => {
            let field_strs: Vec<String> = fields.iter()
                .map(|(name, ty)| format!("({} {})", name, format_type_canonical(ty)))
                .collect();
            format!("(Record {})", field_strs.join(" "))
        },
        ast::Type::Union(variants) => {
            let variant_strs: Vec<String> = variants.iter()
                .map(|v| {
                    if let Some(payload) = &v.payload {
                        format!("({} {})", v.tag, format_type_canonical(payload))
                    } else {
                        format!("({})", v.tag)
                    }
                })
                .collect();
            format!("(Union {})", variant_strs.join(" "))
        },
        ast::Type::Symbol => "Symbol".to_string(),
        ast::Type::TypeParam(name) => format!("TypeParam {}", name),
        ast::Type::TypeRef(name) => format!("TypeRef {}", name),
        ast::Type::TypeApp(name, args) => {
            let arg_strs: Vec<String> = args.iter().map(format_type_canonical).collect();
            format!("(TypeApp {} {})", name, arg_strs.join(" "))
        },
        ast::Type::Uncertain(inner) => format!("(Uncertain {})", format_type_canonical(inner)),
        ast::Type::UnannotatedParam => "UnannotatedParam".to_string(),
    }
}

pub fn canonical_ast_dump(expr: &Expr) -> String {
    dump_expr(expr)
}

fn dump_expr(expr: &Expr) -> String {
    match expr {
        Expr::Int(n, _, _) => format!("(Int {})", n),
        Expr::Float(f, _, _) => format!("(Float {})", f),
        Expr::Str(s, _, _) => format!("(Str \"{}\")", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Expr::Bool(b, _, _) => format!("(Bool {})", b),
        Expr::Nil => "(Nil)".to_string(),
        Expr::Symbol(s, _, _) => format!("(Symbol {})", s),
        Expr::Var(s, _, _) => format!("(Var {})", s),
        Expr::Let { name, value, .. } => {
            format!("(Let {} {})", name, dump_expr(value))
        },
        Expr::FnDef { name, params, body, return_type, effect_level, cap, .. } => {
            let param_strs: Vec<String> = params.iter()
                .map(|(pname, ptype)| {
                    match ptype {
                        Some(t) => format!("({} {})", pname, format_type_canonical(t)),
                        None => format!("({})", pname),
                    }
                })
                .collect();
            let ret_type = return_type.as_ref()
                .map(|t| format_type_canonical(t))
                .unwrap_or_else(|| "(Nil)".to_string());
            let effects = format_effect_sigils(effect_level);
            let caps_str = cap.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(" ");
            format!("(FnDef {} ({}) {} {} {}{})",
                name,
                param_strs.join(" "),
                dump_expr(body),
                ret_type,
                effects,
                if caps_str.is_empty() { String::new() } else { format!(" {}", caps_str) })
        },
        Expr::FnCall { name, args, .. } => {
            let args_strs: Vec<String> = args.iter().map(dump_expr).collect();
            format!("(FnCall {} {})", name, args_strs.join(" "))
        },
        Expr::If { cond, then_branch, else_branch, .. } => {
            format!("(If {} {} {})",
                dump_expr(cond), dump_expr(then_branch), dump_expr(else_branch))
        },
        Expr::Arithmetic { op, left, right, .. } => {
            let op_str = match op {
                ast::ArithOp::Add => "Add",
                ast::ArithOp::Sub => "Sub",
                ast::ArithOp::Mul => "Mul",
                ast::ArithOp::Div => "Div",
            };
            format!("(Arithmetic {} {} {})", op_str, dump_expr(left), dump_expr(right))
        },
        Expr::Ret(e, _, _) => format!("(Ret {})", dump_expr(e)),
        Expr::IoWrite(e, _, _) => format!("(IoWrite {})", dump_expr(e)),
        Expr::Block(exprs, _, _) => {
            let exprs_strs: Vec<String> = exprs.iter().map(dump_expr).collect();
            format!("(Block {})", exprs_strs.join(" "))
        },
        Expr::Intent(s, _, _) => format!("(Intent {})", s),
        Expr::Uncertain(e, _, _) => format!("(Uncertain {})", dump_expr(e)),
        Expr::Ctx { .. } => "(Ctx)".to_string(),
        Expr::CtxGet { ctx, key, .. } => {
            format!("(CtxGet {} {})", dump_expr(ctx), dump_expr(key))
        },
        Expr::CtxSet { ctx, key, value, .. } => {
            format!("(CtxSet {} {} {})", dump_expr(ctx), dump_expr(key), dump_expr(value))
        },
        Expr::Diff { metadata, ops, .. } => {
            let meta_str = if let Some(m) = metadata {
                format!("(Meta {}{}{})",
                    m.description.as_ref().map(|d| format!("desc:\"{}\" ", d)).unwrap_or_default(),
                    m.author.as_ref().map(|a| format!("author:\"{}\" ", a)).unwrap_or_default(),
                    m.timestamp.as_ref().map(|t| format!("time:\"{}\"", t)).unwrap_or_default())
            } else {
                "(Meta)".to_string()
            };
            let ops_strs: Vec<String> = ops.iter().map(dump_diff_op).collect();
            format!("(Diff {} {})", meta_str, ops_strs.join(" "))
        },
        Expr::Use { caps, module, .. } => {
            let caps_strs: Vec<String> = caps.iter().map(|(c, alias)| {
                match alias {
                    Some(a) => format!("({} as {})", c, a),
                    None => format!("({})", c),
                }
            }).collect();
            let module_path = module.to_string();
            format!("(Use {} {})", module_path, caps_strs.join(" "))
        },
        Expr::Match { scrutinee, patterns, .. } => {
            let pattern_strs: Vec<String> = patterns.iter().map(|(pat, expr)| {
                let pat_str = match pat {
                    ast::Pattern::Tag(t) => format!("(Tag {})", t),
                    ast::Pattern::TagBind(t, v) => format!("(TagBind {} {})", t, v),
                    ast::Pattern::Wildcard => "(Wildcard)".to_string(),
                };
                format!("({} {})", pat_str, dump_expr(expr))
            }).collect();
            format!("(Match {} {})", dump_expr(scrutinee), pattern_strs.join(" "))
        },
        Expr::Tagged { tag, payload, .. } => {
            let payload_str = payload.as_ref()
                .map(|p| dump_expr(p))
                .unwrap_or_else(|| "(Nil)".to_string());
            format!("(Tagged {} {})", tag, payload_str)
        },
        Expr::Mod { name, .. } => format!("(Mod {})", name.to_string()),
        Expr::Pub { cap, .. } => {
            let cap_strs: Vec<String> = cap.iter().map(|c| format!("({})", c)).collect();
            format!("(Pub {})", cap_strs.join(" "))
        },
        Expr::PubDecl { inner, .. } => format!("(PubDecl {})", dump_expr(inner)),
        Expr::TypeAlias { name, type_params, ty, .. } => {
            let param_strs: Vec<String> = type_params.iter().map(|p| format!("({})", p)).collect();
            let type_str = format_type_canonical(ty);
            format!("(TypeAlias {} ({}) {})", name, param_strs.join(" "), type_str)
        },
        Expr::Record { fields, .. } => {
            let field_strs: Vec<String> = fields.iter()
                .map(|(fname, fexpr)| format!("({} {})", fname, dump_expr(fexpr)))
                .collect();
            format!("(Record {})", field_strs.join(" "))
        },
        Expr::List { elements, .. } => {
            let elems_strs: Vec<String> = elements.iter().map(dump_expr).collect();
            format!("(List {})", elems_strs.join(" "))
        },
    }
}

fn dump_diff_op(op: &ast::DiffOp) -> String {
    let kind_str = match op.kind {
        ast::DiffKind::Replace => "Replace",
        ast::DiffKind::Insert => "Insert",
        ast::DiffKind::Delete => "Delete",
        ast::DiffKind::Move => "Move",
        ast::DiffKind::Copy => "Copy",
    };
    let selector_str = op.selector.to_string();
    let payload_str = op.payload.as_ref()
        .map(|p| dump_expr(p))
        .unwrap_or_else(|| "(Nil)".to_string());
    format!("(DiffOp {} {} {})", kind_str, selector_str, payload_str)
}
