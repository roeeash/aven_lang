use crate::ast::{Expr, ArithOp, SourceSpan, NodeId, Type, PrimitiveType, EffectSet, Pattern, IntentEntry, IntentTable, ModulePath, SelectorPath, PathSegment, DiffOp, DiffKind, InsertMode, DiffMetadata};
use crate::lexer::{Token, Lexer};
use std::fmt;

#[derive(Debug, Clone)]
pub enum ParseError {
    UnexpectedToken(String),
    UnexpectedEof,
    InvalidSyntax(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken(msg) => write!(f, "Unexpected token: {}", msg),
            ParseError::UnexpectedEof => write!(f, "Unexpected end of file"),
            ParseError::InvalidSyntax(msg) => write!(f, "Invalid syntax: {}", msg),
        }
    }
}

pub struct Parser {
    tokens: Vec<(Token, SourceSpan)>,
    pos: usize,
    node_counter: u64,
    current_path: Vec<String>,
    intent_table: IntentTable,
    in_diff_context: bool,
}

impl Parser {
    pub fn new(input: &str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize_spanned();
        Ok(Parser { tokens, pos: 0, node_counter: 0, current_path: Vec::new(), intent_table: IntentTable { entries: Vec::new() }, in_diff_context: false })
    }

    fn next_node_id(&mut self) -> NodeId {
        let id = self.node_counter;
        self.node_counter += 1;
        id
    }

    fn current(&self) -> Token {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].0.clone()
        } else {
            Token::Eof
        }
    }

    /// Byte span of the token at the current position.
    pub fn current_span(&self) -> SourceSpan {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].1
        } else {
            SourceSpan::zero()
        }
    }

    fn advance(&mut self) -> Token {
        let tok = self.current();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }
    
    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let tok = self.current();
        if std::mem::discriminant(&tok) == std::mem::discriminant(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken(format!(
                "Expected {:?}, got {:?}",
                expected, tok
            )))
        }
    }
    
    pub fn parse(&mut self) -> Result<Expr, ParseError> {
        self.parse_program()
    }

    /// Parse the input and return the top-level expression together with the
    /// byte span `[start, end)` that it covers in the source string.
    pub fn parse_spanned(&mut self) -> Result<(Expr, SourceSpan), ParseError> {
        let start = self.current_span().start;
        let expr = self.parse_program()?;
        let end = if self.pos > 0 {
            self.tokens[self.pos - 1].1.end
        } else {
            self.current_span().end
        };
        Ok((expr, SourceSpan::new(start, end)))
    }

    /// Parse a `.avenpatch` file: `@patch-for path:"<file>" @diff ... @diff ...`
    /// Returns a vector of DiffOp nodes extracted from the patch file.
    pub fn parse_patch_file(&mut self) -> Result<Vec<DiffOp>, ParseError> {
        self.expect(Token::PatchFor)?;
        match self.current() {
            Token::Ident(ref s) if s == "path" => { self.advance(); }
            other => return Err(ParseError::UnexpectedToken(format!(
                "Expected 'path' keyword, got {:?}", other
            ))),
        }
        self.expect(Token::Colon)?;

        // Consume the file path string (we don't need to store it here)
        match self.current() {
            Token::String(_) => {
                self.advance();
            }
            _ => return Err(ParseError::UnexpectedToken(
                "Expected string literal for patch-for path".to_string(),
            )),
        }

        let mut ops = Vec::new();

        // Parse one or more @diff statements
        while self.current() != Token::Eof {
            match self.current() {
                Token::Diff => {
                    let expr = self.parse_diff()?;
                    // Extract DiffOp nodes from the Expr::Diff
                    if let Expr::Diff { ops: diff_ops, .. } = expr {
                        ops.extend(diff_ops);
                    } else {
                        // parse_diff should always return Expr::Diff; this is unreachable
                        return Err(ParseError::InvalidSyntax(
                            "parse_diff returned non-Diff expression".to_string(),
                        ));
                    }
                }
                Token::Eof => break,
                _ => {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected @diff or EOF, got {:?}",
                        self.current()
                    )));
                }
            }
        }

        Ok(ops)
    }

    fn span_from(&self, start: SourceSpan, end: SourceSpan) -> SourceSpan {
        SourceSpan::new(start.start, end.end)
    }

    fn push_scope(&mut self, name: String) {
        self.current_path.push(name);
    }

    fn pop_scope(&mut self) {
        self.current_path.pop();
    }

    pub fn get_intent_table(self) -> IntentTable {
        self.intent_table
    }

    fn is_expression_start(&self) -> bool {
        matches!(self.current(),
            Token::Let | Token::Fn | Token::Ret | Token::IoWrite | Token::Intent | Token::Use | Token::Mod | Token::Pub | Token::Match | Token::Ok | Token::Err | Token::Diff | Token::Diffs |
            Token::If | Token::Call | Token::Ident(_) | Token::String(_) | Token::Integer(_) | Token::True | Token::False | Token::LeftParen | Token::At | Token::Underscore
        )
    }

    fn parse_program(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        let mut exprs = Vec::new();

        while self.current() != Token::Eof {
            exprs.push(self.parse_statement()?);
        }

        if exprs.is_empty() {
            Ok(Expr::Nil)
        } else if exprs.len() == 1 {
            Ok(exprs.into_iter().next().unwrap())
        } else {
            let end_span = if self.pos > 0 {
                self.tokens[self.pos - 1].1
            } else {
                SourceSpan::zero()
            };
            Ok(Expr::Block(exprs, self.next_node_id(), self.span_from(start_span, end_span)))
        }
    }
    
    fn parse_statement(&mut self) -> Result<Expr, ParseError> {
        match self.current() {
            Token::Let => self.parse_let(),
            Token::Fn => self.parse_fn_def(),
            Token::Type => self.parse_type_alias(),
            Token::Ret => self.parse_ret(),
            Token::IoWrite => self.parse_io_write(),
            Token::Intent => self.parse_intent(),
            Token::Use => self.parse_use(),
            Token::Mod => self.parse_mod(),
            Token::Pub => self.parse_pub(),
            Token::Match => self.parse_match(),
            Token::Ok => self.parse_ok(),
            Token::Err => self.parse_err(),
            Token::Diff | Token::Diffs => self.parse_diff(),
            _ => self.parse_expression(),
        }
    }

    fn parse_ok(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Ok)?;
        let payload = Box::new(self.parse_expression()?);
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };
        Ok(Expr::Tagged {
            tag: "ok".to_string(),
            payload: Some(payload),
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_err(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Err)?;
        let payload = Box::new(self.parse_expression()?);
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };
        Ok(Expr::Tagged {
            tag: "err".to_string(),
            payload: Some(payload),
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    /// Parse optional `@meta { ... }` block at the head of a diff.
    /// Returns DiffMetadata with optional description, author, timestamp fields.
    /// If no @meta keyword is present, returns None.
    fn parse_meta(&mut self) -> Result<Option<DiffMetadata>, ParseError> {
        if self.current() != Token::Meta {
            return Ok(None);
        }
        self.advance(); // consume @meta
        self.expect(Token::LeftBrace)?;

        let mut description = None;
        let mut author = None;
        let mut timestamp = None;

        // Parse zero or more key-value pairs
        while self.current() != Token::RightBrace && self.current() != Token::Eof {
            match self.current() {
                Token::Ident(key) => {
                    let key_name = key.clone();
                    self.advance();
                    self.expect(Token::Colon)?;

                    match self.current() {
                        Token::String(s) => {
                            let value = s.clone();
                            self.advance();
                            match key_name.as_str() {
                                "description" => description = Some(value),
                                "author" => author = Some(value),
                                "timestamp" => timestamp = Some(value),
                                _ => {} // Ignore unknown keys
                            }
                        }
                        _ => return Err(ParseError::UnexpectedToken(
                            "Expected string value in @meta block".to_string()
                        )),
                    }
                }
                _ => return Err(ParseError::UnexpectedToken(
                    "Expected identifier in @meta block".to_string()
                )),
            }
        }

        self.expect(Token::RightBrace)?;
        Ok(Some(DiffMetadata { description, author, timestamp }))
    }

    /// Parse `@diff` or `@diffs` blocks. M1 stub: consume all remaining tokens
    /// Parse @diff or @diffs block with structured diff operations.
    /// Consumes operation keywords (@replace, @insert, @delete, @move, @copy)
    /// and builds a Vec<DiffOp> with selector paths and payloads.
    fn parse_diff(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.advance(); // consume @diff or @diffs

        // Parse optional @meta block
        let metadata = self.parse_meta()?;

        // Set diff context flag to disambiguate '/' as path separator
        let saved_diff_context = self.in_diff_context;
        self.in_diff_context = true;

        let mut ops = Vec::new();

        // Parse diff operations until we hit EOF or a non-diff token
        while self.current() != Token::Eof {
            match self.current() {
                Token::Replace => {
                    ops.push(self.parse_replace()?);
                }
                Token::Insert => {
                    ops.push(self.parse_insert()?);
                }
                Token::Delete => {
                    ops.push(self.parse_delete()?);
                }
                Token::Move => {
                    let move_ops = self.parse_move()?;
                    ops.extend(move_ops);
                }
                Token::Copy => {
                    let copy_ops = self.parse_copy()?;
                    ops.extend(copy_ops);
                }
                _ => {
                    // Stop on first non-diff token
                    break;
                }
            }
        }

        // Restore previous diff context
        self.in_diff_context = saved_diff_context;

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };
        Ok(Expr::Diff {
            metadata,
            ops,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    /// Parse @replace /path expr
    fn parse_replace(&mut self) -> Result<DiffOp, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Replace)?;
        let selector = self.parse_selector_path()?;

        // Parse payload expression
        let payload = Some(Box::new(self.parse_primary()?));

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(DiffOp {
            kind: DiffKind::Replace,
            selector,
            payload,
            insert_mode: None,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    /// Parse @insert [@first|@last|@before name|@after name] /path expr
    fn parse_insert(&mut self) -> Result<DiffOp, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Insert)?;

        let insert_mode = match self.current() {
            Token::First => {
                self.advance();
                Some(InsertMode::First)
            }
            Token::Last => {
                self.advance();
                Some(InsertMode::Last)
            }
            Token::Before => {
                self.advance();
                let name = match self.current() {
                    Token::Ident(n) => {
                        let name = n.clone();
                        self.advance();
                        name
                    }
                    _ => return Err(ParseError::UnexpectedToken(
                        "Expected identifier after @before".to_string(),
                    )),
                };
                Some(InsertMode::Before(name))
            }
            Token::After => {
                self.advance();
                let name = match self.current() {
                    Token::Ident(n) => {
                        let name = n.clone();
                        self.advance();
                        name
                    }
                    _ => return Err(ParseError::UnexpectedToken(
                        "Expected identifier after @after".to_string(),
                    )),
                };
                Some(InsertMode::After(name))
            }
            _ => None,
        };

        let selector = self.parse_selector_path()?;
        let payload = Some(Box::new(self.parse_primary()?));

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(DiffOp {
            kind: DiffKind::Insert,
            selector,
            payload,
            insert_mode,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    /// Parse @delete /path (no payload; error if payload follows)
    fn parse_delete(&mut self) -> Result<DiffOp, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Delete)?;
        let selector = self.parse_selector_path()?;

        // @delete does not take a payload; validate no expression follows
        if self.is_expression_start() {
            return Err(ParseError::InvalidSyntax(
                "@delete does not accept a payload; remove the expression after the selector path".to_string()
            ));
        }

        let payload = None;

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(DiffOp {
            kind: DiffKind::Delete,
            selector,
            payload,
            insert_mode: None,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    /// Parse @move /src @to /dst, returns two DiffOp nodes (source + destination)
    fn parse_move(&mut self) -> Result<Vec<DiffOp>, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Move)?;
        let src_selector = self.parse_selector_path()?;

        self.expect(Token::To)?;
        let dst_selector = self.parse_selector_path()?;

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        let span = self.span_from(start_span, end_span);

        // Create two DiffOp nodes: source and destination
        let src_op = DiffOp {
            kind: DiffKind::Move,
            selector: src_selector,
            payload: None,
            insert_mode: None,
            node_id: self.next_node_id(),
            span,
        };

        let dst_op = DiffOp {
            kind: DiffKind::Move,
            selector: dst_selector,
            payload: None,
            insert_mode: None,
            node_id: self.next_node_id(),
            span,
        };

        Ok(vec![src_op, dst_op])
    }

    /// Parse @copy /src @to /dst, returns two DiffOp nodes with Copy kind
    fn parse_copy(&mut self) -> Result<Vec<DiffOp>, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Copy)?;
        let src_selector = self.parse_selector_path()?;

        self.expect(Token::To)?;
        let dst_selector = self.parse_selector_path()?;

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        let span = self.span_from(start_span, end_span);

        // Create two DiffOp nodes: source and destination
        let src_op = DiffOp {
            kind: DiffKind::Copy,
            selector: src_selector,
            payload: None,
            insert_mode: None,
            node_id: self.next_node_id(),
            span,
        };

        let dst_op = DiffOp {
            kind: DiffKind::Copy,
            selector: dst_selector,
            payload: None,
            insert_mode: None,
            node_id: self.next_node_id(),
            span,
        };

        Ok(vec![src_op, dst_op])
    }

    fn parse_intent(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Intent)?;
        let s = match self.current() {
            Token::String(st) => st.clone(),
            tok => return Err(ParseError::UnexpectedToken(format!(
                "@intent must be followed by a string literal, got {:?}",
                tok
            ))),
        };
        self.advance();
        let end_span = if self.pos > 0 { self.tokens[self.pos - 1].1 } else { SourceSpan::zero() };
        let span = self.span_from(start_span, end_span);

        // Record intent in the table
        let selector = if self.current_path.is_empty() {
            "/".to_string()
        } else {
            "/".to_string() + &self.current_path.join("/")
        };
        self.intent_table.entries.push(IntentEntry {
            selector,
            intent_name: s.clone(),
            subtree_span: span,
        });

        Ok(Expr::Intent(s, self.next_node_id(), span))
    }

    fn parse_use(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Use)?;

        // Check for wildcard import: @use * @from module
        let mut caps = Vec::new();
        if self.current() == Token::Star {
            self.advance(); // consume *
            // Wildcard sentinel: store as vec![("*", None)]
            caps.push(("*".to_string(), None));
        } else {
            // Expect [ for explicit capability list
            self.expect(Token::LeftBracket)?;

            // Parse capability names with optional aliases until ]
            while self.current() != Token::RightBracket && self.current() != Token::Eof {
                if let Token::Ident(cap_name) = self.current() {
                    let cap = cap_name.clone();
                    self.advance();

                    // Check for optional "as" alias (as a bare identifier, not @as keyword)
                    let alias = if let Token::Ident(kw) = self.current() {
                        if kw == "as" {
                            self.advance(); // consume "as"
                            if let Token::Ident(alias_name) = self.current() {
                                let alias = alias_name.clone();
                                self.advance();
                                Some(alias)
                            } else {
                                return Err(ParseError::UnexpectedToken(
                                    "Expected identifier after as in @use".to_string(),
                                ));
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    caps.push((cap, alias));
                } else {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected capability name in @use list, got {:?}",
                        self.current()
                    )));
                }
            }
            self.expect(Token::RightBracket)?;
        }

        // Expect @from
        self.expect(Token::From)?;

        // Parse module path (dotted identifier)
        let mut path_parts = Vec::new();
        if let Token::Ident(name) = self.current() {
            path_parts.push(name.clone());
            self.advance();
        } else {
            return Err(ParseError::UnexpectedToken(format!(
                "Expected module name after @from, got {:?}",
                self.current()
            )));
        }

        // Parse additional path segments separated by /
        while self.current() == Token::Slash {
            self.advance();
            if let Token::Ident(seg) = self.current() {
                path_parts.push(seg.clone());
                self.advance();
            } else {
                return Err(ParseError::UnexpectedToken(format!(
                    "Expected identifier after / in module path, got {:?}",
                    self.current()
                )));
            }
        }

        let module = ModulePath::new(path_parts);

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::Use {
            caps,
            module,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_mod(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Mod)?;

        // Parse module path (dotted identifier)
        let mut path_parts = Vec::new();
        if let Token::Ident(n) = self.current() {
            path_parts.push(n.clone());
            self.advance();
        } else {
            return Err(ParseError::UnexpectedToken(format!(
                "Expected module name after @mod, got {:?}",
                self.current()
            )));
        }

        // Parse additional path segments separated by /
        while self.current() == Token::Slash {
            self.advance();
            if let Token::Ident(seg) = self.current() {
                path_parts.push(seg.clone());
                self.advance();
            } else {
                return Err(ParseError::UnexpectedToken(format!(
                    "Expected identifier after / in module path, got {:?}",
                    self.current()
                )));
            }
        }

        let name = ModulePath::new(path_parts);

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::Mod {
            name,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_pub(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Pub)?;

        // Peek at next token: if it's [, parse bracket-list form; otherwise parse per-declaration form
        match self.current() {
            Token::LeftBracket => {
                // Bracket form: @pub [read write]
                self.advance();

                // Parse capability names until ]
                let mut cap = Vec::new();
                while self.current() != Token::RightBracket && self.current() != Token::Eof {
                    if let Token::Ident(cap_name) = self.current() {
                        cap.push(cap_name.clone());
                        self.advance();
                    } else {
                        return Err(ParseError::UnexpectedToken(format!(
                            "Expected capability name in @pub list, got {:?}",
                            self.current()
                        )));
                    }
                }
                self.expect(Token::RightBracket)?;

                let end_span = if self.pos > 0 {
                    self.tokens[self.pos - 1].1
                } else {
                    SourceSpan::zero()
                };

                Ok(Expr::Pub {
                    cap,
                    node_id: self.next_node_id(),
                    span: self.span_from(start_span, end_span),
                })
            }
            Token::Fn | Token::Type | Token::Let => {
                // Per-declaration form: @pub @fn / @pub @type / @pub @let
                let inner = match self.current() {
                    Token::Fn => Box::new(self.parse_fn_def()?),
                    Token::Type => Box::new(self.parse_type_alias()?),
                    Token::Let => Box::new(self.parse_let()?),
                    _ => unreachable!(),
                };

                let end_span = if self.pos > 0 {
                    self.tokens[self.pos - 1].1
                } else {
                    SourceSpan::zero()
                };

                Ok(Expr::PubDecl {
                    inner,
                    node_id: self.next_node_id(),
                    span: self.span_from(start_span, end_span),
                })
            }
            tok => Err(ParseError::UnexpectedToken(format!(
                "Expected [ or declaration keyword after @pub, got {:?}",
                tok
            ))),
        }
    }

    fn parse_type_alias(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Type)?;

        let name = match self.advance() {
            Token::Ident(n) => n,
            tok => return Err(ParseError::UnexpectedToken(format!("Expected type alias name, got {:?}", tok))),
        };

        // Parse space-separated type parameters: any lowercase ident before =
        let mut type_params = Vec::new();
        loop {
            match self.current() {
                Token::Ident(ref p) if p.chars().next().map_or(false, |c| c.is_lowercase()) => {
                    let param = p.clone();
                    self.advance();
                    type_params.push(param);
                }
                _ => break,
            }
        }
        self.expect(Token::Equals)?;

        let ty = self.parse_type_expr()?;

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::TypeAlias {
            name,
            type_params,
            ty,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    /// Parse a selector path for @diff operations (e.g., `/fn greet/body/ret`).
    /// Precondition: a diff-op keyword (@replace, @insert, etc.) has just been consumed.
    /// Returns a SelectorPath with /- separated segments, each optionally indexed with [n].
    fn parse_selector_path(&mut self) -> Result<SelectorPath, ParseError> {
        // Expect leading /
        self.expect(Token::Slash)?;

        let mut parts = Vec::new();

        // Parse segments: each is an identifier, optionally followed by [index]
        loop {
            if self.current() == Token::Eof {
                break;
            }

            // We must have an identifier for the segment name
            let segment_name = match self.current() {
                Token::Ident(name) => {
                    let n = name.clone();
                    self.advance();
                    n
                }
                _ => {
                    // If no identifier, we're done with path segments
                    break;
                }
            };

            parts.push(PathSegment::Named(segment_name));

            // Check for optional [index] suffix
            if self.current() == Token::LeftBracket {
                self.advance();
                let index = match self.current() {
                    Token::Integer(n) => {
                        let idx = n as usize;
                        self.advance();
                        idx
                    }
                    _ => return Err(ParseError::UnexpectedToken(
                        "Expected integer index in [...]".to_string(),
                    )),
                };
                self.expect(Token::RightBracket)?;
                parts.push(PathSegment::Index(index));
            }

            // Check for continuation: next / means more segments
            if self.current() == Token::Slash {
                self.advance();
            } else {
                // No more segments
                break;
            }
        }

        if parts.is_empty() {
            return Err(ParseError::InvalidSyntax(
                "Selector path must have at least one segment".to_string(),
            ));
        }

        Ok(SelectorPath { parts })
    }

    fn parse_let(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Let)?;

        let name = match self.advance() {
            Token::Ident(n) => n,
            tok => return Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
        };

        self.expect(Token::DoubleColon)?;

        self.push_scope(format!("let {}", name));
        let value = Box::new(self.parse_expression()?);
        self.pop_scope();
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::Let {
            name,
            value,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    /// Parse a type expression (e.g., `Int`, `Str`, `#ok Int`).
    /// Handles primitives and basic union tags.
    /// Returns Err if the next token is not a recognized type.
    fn parse_type_expr(&mut self) -> Result<Type, ParseError> {
        let first_type = self.parse_type_expr_single()?;

        // Check for union syntax (Type | Type | ...)
        if self.current() == Token::Pipe {
            // This is a union type; first_type should have been a single union variant
            let mut union_variants = match first_type {
                Type::Union(vars) => vars,
                _ => return Err(ParseError::InvalidSyntax(
                    "Union syntax requires variant types (#tag field:Type)".to_string()
                )),
            };

            while self.current() == Token::Pipe {
                self.advance(); // consume |
                let next_type = self.parse_type_expr_single()?;
                match next_type {
                    Type::Union(mut vars) => union_variants.append(&mut vars),
                    _ => return Err(ParseError::InvalidSyntax(
                        "Union must contain only variant types (#tag field:Type)".to_string()
                    )),
                }
            }

            Ok(Type::Union(union_variants))
        } else {
            Ok(first_type)
        }
    }

    fn parse_type_expr_single(&mut self) -> Result<Type, ParseError> {
        match self.current() {
            Token::Question => {
                // Parse ?Type (option type)
                self.advance(); // consume ?
                let inner_type = self.parse_type_expr_single()?;
                Ok(Type::Option(Box::new(inner_type)))
            }
            Token::Ident(name) => {
                let name_str = name.as_str();
                // Check if this is a union variant (starts with #)
                if name_str.starts_with('#') {
                    let tag = name_str[1..].to_string(); // Remove the # prefix
                    self.advance();

                    // Parse optional payload type: field:Type or bare Type
                    let payload = if let Token::Ident(next_name) = self.current() {
                        // Check for field:Type syntax by looking ahead for colon
                        if !next_name.starts_with('#') {
                            // Save position to check for colon
                            let saved_pos = self.pos;
                            self.advance(); // consume the potential field name

                            if self.current() == Token::Colon {
                                // This is field:Type syntax; consume colon and parse type
                                self.advance();
                                Some(Box::new(self.parse_type_expr_single()?))
                            } else {
                                // No colon; backtrack and parse as bare type
                                self.pos = saved_pos;

                                // Try parsing bare type (primitive, TypeRef, or TypeParam)
                                if let Token::Ident(type_name) = self.current() {
                                    match type_name.as_str() {
                                        "Int" | "Bool" | "Str" | "Flt" | "Nil" => {
                                            Some(Box::new(self.parse_type_expr_single()?))
                                        }
                                        _ if type_name.len() == 1 && type_name.chars().next().map_or(false, |c| c.is_lowercase() && c.is_ascii()) => {
                                            // Type parameter
                                            Some(Box::new(self.parse_type_expr_single()?))
                                        }
                                        _ if type_name.chars().next().map_or(false, |c| c.is_uppercase()) => {
                                            // TypeRef payload (capitalized, e.g., Foo)
                                            Some(Box::new(self.parse_type_expr_single()?))
                                        }
                                        _ => None,
                                    }
                                } else {
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    Ok(Type::Union(vec![crate::ast::UnionVariant {
                        tag,
                        payload,
                    }]))
                } else {
                    // Check if it's a known primitive type or a type alias
                    match name_str {
                        "Int" | "Bool" | "Str" | "Flt" | "Nil" => {
                            let typ = match name_str {
                                "Int" => Type::Primitive(PrimitiveType::Int),
                                "Bool" => Type::Primitive(PrimitiveType::Bool),
                                "Str" => Type::Primitive(PrimitiveType::Str),
                                "Flt" => Type::Primitive(PrimitiveType::Flt),
                                "Nil" => Type::Primitive(PrimitiveType::Nil),
                                _ => unreachable!(),
                            };
                            self.advance();
                            Ok(typ)
                        }
                        _ => {
                            // Lowercase ident: type parameter (single or multi-letter per spec §1.6)
                            if name_str.chars().next().map_or(false, |c| c.is_lowercase() && c.is_ascii()) {
                                self.advance();
                                Ok(Type::TypeParam(name_str.to_string()))
                            } else if name_str.chars().next().map_or(false, |c| c.is_uppercase()) {
                                // Accept capitalized identifiers as potential type aliases/refs
                                let name_owned = name_str.to_string();
                                self.advance();
                                // Greedily collect type arguments (primitives, type params, upper-case refs, ?Type)
                                // Stop at union pipes, arrows, closing brackets, or EOF
                                let mut args = Vec::new();
                                loop {
                                    match self.current() {
                                        Token::Pipe | Token::Arrow | Token::RightParen | Token::RightBracket | Token::RightBrace | Token::Eof => break,
                                        Token::Ident(ref n) if matches!(n.as_str(), "Int"|"Bool"|"Str"|"Flt"|"Nil") => {
                                            args.push(self.parse_type_expr_single()?);
                                        }
                                        Token::Ident(ref n) if n.chars().next().map_or(false, |c| c.is_lowercase() && c.is_ascii()) => {
                                            args.push(self.parse_type_expr_single()?);
                                        }
                                        Token::Ident(ref n) if n.chars().next().map_or(false, |c| c.is_uppercase()) => {
                                            args.push(self.parse_type_expr_single()?);
                                        }
                                        Token::Question => {
                                            args.push(self.parse_type_expr_single()?);
                                        }
                                        _ => break,
                                    }
                                }
                                if args.is_empty() {
                                    Ok(Type::TypeRef(name_owned))
                                } else {
                                    Ok(Type::TypeApp(name_owned, args))
                                }
                            } else {
                                Err(ParseError::InvalidSyntax(format!("Unknown type: {}", name_str)))
                            }
                        }
                    }
                }
            }
            Token::LeftBracket => {
                self.advance(); // consume [
                let inner_type = self.parse_type_expr_single()?;
                if self.current() != Token::RightBracket {
                    return Err(ParseError::InvalidSyntax(
                        "Expected ] in list type".to_string()
                    ));
                }
                self.advance(); // consume ]
                Ok(Type::List(Box::new(inner_type)))
            }
            Token::LeftBrace => {
                // Parse record type: {field1:Type1 field2:Type2 ...} (space-separated, no commas)
                self.advance(); // consume {
                let mut fields = Vec::new();
                while self.current() != Token::RightBrace && self.current() != Token::Eof {
                    if let Token::Ident(field_name) = self.current() {
                        let fname = field_name.clone();
                        self.advance(); // consume field name
                        self.expect(Token::Colon)?;
                        let field_type = self.parse_type_expr_single()?;
                        fields.push((fname, field_type));
                        // In type expressions, fields are space-separated, not comma-separated
                    } else {
                        return Err(ParseError::InvalidSyntax("Expected field name in record type".to_string()));
                    }
                }
                self.expect(Token::RightBrace)?;
                Ok(Type::Record(fields))
            }
            Token::LeftParen => {
                // Parse function type: (Type -> Type) or (Type arrow Type)
                self.advance(); // consume (
                let first_type = self.parse_type_expr()?;

                // Check for effect arrow
                let effect_set = match self.current() {
                    Token::EffectArrow(es) => {
                        let es_copy = es.clone();
                        self.advance();
                        es_copy
                    }
                    Token::Arrow => {
                        self.advance();
                        EffectSet { err: false, io: false, async_: false }
                    }
                    _ => {
                        return Err(ParseError::InvalidSyntax(
                            "Expected arrow in function type expression".to_string()
                        ));
                    }
                };

                let return_type = self.parse_type_expr()?;

                if self.current() != Token::RightParen {
                    return Err(ParseError::InvalidSyntax(
                        "Expected ) in function type expression".to_string()
                    ));
                }
                self.advance(); // consume )

                Ok(Type::Fn {
                    params: vec![first_type],
                    return_type: Box::new(return_type),
                    effect: effect_set,
                    cap: None,
                })
            }
            _ => Err(ParseError::InvalidSyntax("Expected type expression".to_string())),
        }
    }


    fn parse_fn_def(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Fn)?;

        let name = match self.advance() {
            Token::Ident(n) => n,
            tok => return Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
        };

        self.expect(Token::DoubleColon)?;

        let mut params = Vec::new();

        while self.current() != Token::Arrow &&
              !matches!(self.current(), Token::EffectArrow(_)) &&
              self.current() != Token::Cap &&
              self.current() != Token::Eof {
            let param_name = match self.advance() {
                Token::Ident(n) => n,
                tok => return Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
            };

            let param_type = if self.current() == Token::Colon {
                // Type after : (a:Int syntax)
                self.advance();
                let saved_pos = self.pos;
                match self.parse_type_expr() {
                    Ok(t) => Some(t),
                    Err(_) => {
                        self.pos = saved_pos;
                        None
                    }
                }
            } else if self.current() == Token::DoubleColon {
                // Or after :: (a :: Int syntax)
                self.advance();
                let saved_pos = self.pos;
                match self.parse_type_expr() {
                    Ok(t) => Some(t),
                    Err(_) => {
                        self.pos = saved_pos;
                        None
                    }
                }
            } else {
                None
            };

            params.push((param_name, param_type));

            if self.current() == Token::Arrow ||
               matches!(self.current(), Token::EffectArrow(_)) ||
               self.current() == Token::Cap {
                break;
            }
        }

        // Parse optional @cap [ident ident ...] before the effect arrow
        let mut cap = Vec::new();
        if self.current() == Token::Cap {
            self.advance();
            // Expect [
            self.expect(Token::LeftBracket)?;
            // Parse capability names until ]
            while self.current() != Token::RightBracket && self.current() != Token::Eof {
                if let Token::Ident(cap_name) = self.current() {
                    cap.push(cap_name.clone());
                    self.advance();
                } else {
                    return Err(ParseError::UnexpectedToken(format!(
                        "Expected capability name in @cap list, got {:?}",
                        self.current()
                    )));
                }
            }
            self.expect(Token::RightBracket)?;
        }

        let mut return_type = None;
        let mut effect_level = EffectSet::pure_(); // Default to Pure
        if self.current() == Token::Arrow {
            self.advance();
            if let Ok(rt) = self.parse_type_expr() {
                return_type = Some(rt);
            }
        } else if let Token::EffectArrow(effect_set) = self.current() {
            effect_level = effect_set;
            self.advance();
            if let Ok(rt) = self.parse_type_expr() {
                return_type = Some(rt);
            }
        }

        self.push_scope(format!("fn {}", name));
        let body = Box::new(self.parse_statement()?);
        self.pop_scope();
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::FnDef {
            name,
            params,
            body,
            return_type,
            effect_level,
            cap,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }
    
    fn parse_ret(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Ret)?;
        let expr = Box::new(self.parse_expression()?);
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };
        Ok(Expr::Ret(expr, self.next_node_id(), self.span_from(start_span, end_span)))
    }
    
    fn parse_io_write(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::IoWrite)?;
        let expr = Box::new(self.parse_primary()?);
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };
        Ok(Expr::IoWrite(expr, self.next_node_id(), self.span_from(start_span, end_span)))
    }

    fn parse_ctx_get(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::CtxGet)?;
        let ctx = Box::new(self.parse_primary()?);
        let key = Box::new(self.parse_primary()?);
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };
        // Validate that key is a string literal at parse time
        match &*key {
            Expr::Str(_, _, _) => {}
            _ => return Err(ParseError::InvalidSyntax("@ctx.get key must be a string literal".to_string())),
        }
        Ok(Expr::CtxGet {
            ctx,
            key,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_ctx_set(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::CtxSet)?;
        let ctx = Box::new(self.parse_primary()?);
        let key = Box::new(self.parse_primary()?);
        let value = Box::new(self.parse_primary()?);
        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };
        // Validate that key is a string literal at parse time
        match &*key {
            Expr::Str(_, _, _) => {}
            _ => return Err(ParseError::InvalidSyntax("@ctx.set key must be a string literal".to_string())),
        }
        Ok(Expr::CtxSet {
            ctx,
            key,
            value,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_if()
    }
    
    fn parse_if(&mut self) -> Result<Expr, ParseError> {
        if self.current() == Token::If {
            let start_span = self.current_span();
            self.advance();

            let cond = Box::new(self.parse_comparison()?);

            self.expect(Token::Then)?;
            let then_branch = Box::new(self.parse_comparison()?);

            self.expect(Token::Else)?;
            let else_branch = Box::new(self.parse_comparison()?);

            let end_span = if self.pos > 0 {
                self.tokens[self.pos - 1].1
            } else {
                SourceSpan::zero()
            };

            Ok(Expr::If {
                cond,
                then_branch,
                else_branch,
                node_id: self.next_node_id(),
                span: self.span_from(start_span, end_span),
            })
        } else {
            self.parse_comparison()
        }
    }
    
    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        self.parse_arithmetic()
    }
    
    fn parse_arithmetic(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_primary()?;
        
        // Handle parenthesized arithmetic: (+ a b)
        if let Expr::Arithmetic { .. } = left {
            return Ok(left);
        }
        
        Ok(left)
    }
    
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.current() {
            Token::LeftParen => self.parse_parens(),
            Token::LeftBrace => self.parse_record(),
            Token::LeftBracket => self.parse_list(),
            Token::Integer(n) => {
                let span = self.current_span();
                let val = n;
                self.advance();
                Ok(Expr::Int(val, self.next_node_id(), span))
            }
            Token::Float(f) => {
                let span = self.current_span();
                let val = f;
                self.advance();
                Ok(Expr::Float(val, self.next_node_id(), span))
            }
            Token::String(s) => {
                let span = self.current_span();
                let val = s.clone();
                self.advance();
                Ok(Expr::Str(val, self.next_node_id(), span))
            }
            Token::True => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Bool(true, self.next_node_id(), span))
            }
            Token::False => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Bool(false, self.next_node_id(), span))
            }
            Token::Ident(name) => {
                let span = self.current_span();
                let n = name.clone();
                self.advance();

                if n.starts_with('#') {
                    if n.len() == 1 {
                        return Err(ParseError::UnexpectedToken("Symbol must have name after #".to_string()));
                    }
                    // Reject digit-leading symbol names
                    let sym_name = &n[1..];
                    if sym_name.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                        return Err(ParseError::InvalidSyntax(
                            format!("Symbol name cannot start with digit: {}", n)
                        ));
                    }
                    return Ok(Expr::Symbol(n, self.next_node_id(), span));
                }

                Ok(Expr::Var(n, self.next_node_id(), span))
            }
            Token::Call => self.parse_fn_call(),
            Token::Ok => self.parse_ok(),
            Token::Err => self.parse_err(),
            Token::Underscore => {
                self.advance();
                Ok(Expr::Nil)
            }
            Token::Uncertain => {
                let start_span = self.current_span();
                self.advance();
                let inner = Box::new(self.parse_primary()?);
                let end_span = if self.pos > 0 {
                    self.tokens[self.pos - 1].1
                } else {
                    SourceSpan::zero()
                };
                Ok(Expr::Uncertain(inner, self.next_node_id(), self.span_from(start_span, end_span)))
            }
            Token::CtxGet => self.parse_ctx_get(),
            Token::CtxSet => self.parse_ctx_set(),
            Token::Ctx => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Ctx { node_id: self.next_node_id(), span })
            }
            Token::Intent => self.parse_intent(),
            tok => Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
        }
    }
    
    fn parse_parens(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::LeftParen)?;

        match self.current() {
            Token::Plus => {
                self.advance();
                let left = Box::new(self.parse_arithmetic()?);
                let right = Box::new(self.parse_arithmetic()?);
                self.expect(Token::RightParen)?;
                let end_span = if self.pos > 0 {
                    self.tokens[self.pos - 1].1
                } else {
                    SourceSpan::zero()
                };
                Ok(Expr::Arithmetic {
                    op: ArithOp::Add,
                    left,
                    right,
                    node_id: self.next_node_id(),
                    span: self.span_from(start_span, end_span),
                })
            }
            Token::Minus => {
                self.advance();
                let left = Box::new(self.parse_arithmetic()?);
                let right = Box::new(self.parse_arithmetic()?);
                self.expect(Token::RightParen)?;
                let end_span = if self.pos > 0 {
                    self.tokens[self.pos - 1].1
                } else {
                    SourceSpan::zero()
                };
                Ok(Expr::Arithmetic {
                    op: ArithOp::Sub,
                    left,
                    right,
                    node_id: self.next_node_id(),
                    span: self.span_from(start_span, end_span),
                })
            }
            Token::Star => {
                self.advance();
                let left = Box::new(self.parse_arithmetic()?);
                let right = Box::new(self.parse_arithmetic()?);
                self.expect(Token::RightParen)?;
                let end_span = if self.pos > 0 {
                    self.tokens[self.pos - 1].1
                } else {
                    SourceSpan::zero()
                };
                Ok(Expr::Arithmetic {
                    op: ArithOp::Mul,
                    left,
                    right,
                    node_id: self.next_node_id(),
                    span: self.span_from(start_span, end_span),
                })
            }
            Token::Slash => {
                self.advance();
                let left = Box::new(self.parse_arithmetic()?);
                let right = Box::new(self.parse_arithmetic()?);
                self.expect(Token::RightParen)?;
                let end_span = if self.pos > 0 {
                    self.tokens[self.pos - 1].1
                } else {
                    SourceSpan::zero()
                };
                Ok(Expr::Arithmetic {
                    op: ArithOp::Div,
                    left,
                    right,
                    node_id: self.next_node_id(),
                    span: self.span_from(start_span, end_span),
                })
            }
            Token::Ident(name) if name.starts_with('#') => {
                // Tagged value: (#tag [payload])
                let tag = name[1..].to_string();
                self.advance();

                // Check if there's a payload expression
                if self.current() == Token::RightParen {
                    self.advance();
                    // Just a tag, no payload
                    Ok(Expr::Tagged {
                        tag,
                        payload: None,
                        node_id: self.next_node_id(),
                        span: start_span,
                    })
                } else {
                    // Has a payload
                    let payload = self.parse_expression()?;
                    self.expect(Token::RightParen)?;
                    let end_span = if self.pos > 0 {
                        self.tokens[self.pos - 1].1
                    } else {
                        SourceSpan::zero()
                    };
                    Ok(Expr::Tagged {
                        tag,
                        payload: Some(Box::new(payload)),
                        node_id: self.next_node_id(),
                        span: self.span_from(start_span, end_span),
                    })
                }
            }
            _ => {
                let expr = self.parse_expression()?;
                self.expect(Token::RightParen)?;
                Ok(expr)
            }
        }
    }

    fn parse_record(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::LeftBrace)?;

        let mut fields = Vec::new();

        // Parse field list: ident : expr , ... , ident : expr
        while self.current() != Token::RightBrace && self.current() != Token::Eof {
            // Expect field name (ident)
            let field_name = match self.current() {
                Token::Ident(name) => {
                    let n = name.clone();
                    // Reject hash-prefixed field names
                    if n.starts_with('#') {
                        return Err(ParseError::InvalidSyntax(
                            format!("Record field name cannot be a symbol: {}", n)
                        ));
                    }
                    self.advance();
                    n
                }
                tok => return Err(ParseError::UnexpectedToken(format!(
                    "Expected field name in record, got {:?}",
                    tok
                ))),
            };

            // Expect :
            self.expect(Token::Colon)?;

            // Parse field value
            let field_value = self.parse_expression()?;

            // Check for duplicate field names
            if fields.iter().any(|(k, _)| k == &field_name) {
                return Err(ParseError::InvalidSyntax(format!("Duplicate field name: {}", field_name)));
            }

            fields.push((field_name, field_value));

            // Comma is optional — AVEN records use space-separated fields
            if self.current() == Token::Comma {
                self.advance();
                // Allow trailing comma before }
                if self.current() == Token::RightBrace {
                    break;
                }
            }
        }

        if fields.is_empty() {
            return Err(ParseError::InvalidSyntax("Record must have at least one field".to_string()));
        }

        self.expect(Token::RightBrace)?;

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::Record {
            fields,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_list(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::LeftBracket)?;

        let mut elements = Vec::new();

        // Parse space-separated primary expressions until ]
        while self.current() != Token::RightBracket && self.current() != Token::Eof {
            elements.push(self.parse_primary()?);
        }

        self.expect(Token::RightBracket)?;

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::List {
            elements,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_fn_call(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Call)?;

        let name = match self.advance() {
            Token::Ident(n) => n,
            tok => return Err(ParseError::UnexpectedToken(format!("{:?}", tok))),
        };

        let mut args = Vec::new();

        while !matches!(self.current(), Token::Eof | Token::RightParen) {
            args.push(self.parse_primary()?);

            if matches!(self.current(), Token::Eof | Token::RightParen) {
                break;
            }
        }

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::FnCall {
            name,
            args,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_match(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.current_span();
        self.expect(Token::Match)?;

        // Parse scrutinee expression
        let scrutinee = Box::new(self.parse_expression()?);

        let mut patterns = Vec::new();

        // Parse pattern -> expression pairs
        loop {
            // Check if we're at the end or another statement
            if matches!(self.current(), Token::Eof | Token::Let | Token::Fn | Token::Ret | Token::IoWrite | Token::Intent | Token::Use | Token::Match | Token::Diff | Token::Diffs) {
                break;
            }

            let pattern = self.parse_pattern()?;

            // Expect ->
            self.expect(Token::Arrow)?;

            let body = self.parse_expression()?;
            patterns.push((pattern, body));
        }

        if patterns.is_empty() {
            return Err(ParseError::InvalidSyntax("Match must have at least one pattern".to_string()));
        }

        let end_span = if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            SourceSpan::zero()
        };

        Ok(Expr::Match {
            scrutinee,
            patterns,
            node_id: self.next_node_id(),
            span: self.span_from(start_span, end_span),
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        // Pattern can be:
        // - #tag (bare symbol, Pattern::Tag)
        // - #tag var (symbol with binding, Pattern::TagBind)
        // - _ (wildcard, Pattern::Wildcard)

        if self.current() == Token::Underscore {
            self.advance();
            return Ok(Pattern::Wildcard);
        }

        if let Token::Ident(name) = self.current() {
            if name.starts_with('#') {
                let tag = name[1..].to_string();
                self.advance();

                // Check if there's a binding variable
                if let Token::Ident(var_name) = self.current() {
                    // Not a symbol (doesn't start with #), so it's a binding variable
                    if !var_name.starts_with('#') {
                        let var = var_name.clone();
                        self.advance();
                        return Ok(Pattern::TagBind(tag, var));
                    }
                }

                // No binding, just the tag
                return Ok(Pattern::Tag(tag));
            }
        }

        Err(ParseError::InvalidSyntax(
            "Pattern must be #tag [var], bare identifier, or _".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_integer() {
        let mut parser = Parser::new("42").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Int(n, _, _) => assert_eq!(n, 42),
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn test_parse_string() {
        let mut parser = Parser::new(r#""hello""#).unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Str(s, _, _) => assert_eq!(s, "hello"),
            _ => panic!("Expected Str"),
        }
    }

    #[test]
    fn test_parse_boolean() {
        let mut parser = Parser::new("@true").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Bool(b, _, _) => assert!(b),
            _ => panic!("Expected Bool"),
        }
    }

    #[test]
    fn test_parse_let() {
        let mut parser = Parser::new("@let x :: 10").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Let { name, value, .. } => {
                assert_eq!(name, "x");
                match *value {
                    Expr::Int(n, _, _) => assert_eq!(n, 10),
                    _ => panic!("Expected Int value"),
                }
            }
            _ => panic!("Expected Let"),
        }
    }

    #[test]
    fn test_parse_arithmetic() {
        let mut parser = Parser::new("(+ 2 3)").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Arithmetic { op, left, right, .. } => {
                assert_eq!(op, ArithOp::Add);
                match (*left, *right) {
                    (Expr::Int(l, _, _), Expr::Int(r, _, _)) => {
                        assert_eq!(l, 2);
                        assert_eq!(r, 3);
                    }
                    _ => panic!("Expected Int operands"),
                }
            }
            _ => panic!("Expected Arithmetic"),
        }
    }

    #[test]
    fn test_parse_if_expr() {
        let mut parser = Parser::new("@if @true @then 1 @else 0").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                match *cond {
                    Expr::Bool(b, _, _) => assert!(b),
                    _ => panic!("Expected Bool cond"),
                }
                match *then_branch {
                    Expr::Int(n, _, _) => assert_eq!(n, 1),
                    _ => panic!("Expected Int then_branch"),
                }
                match *else_branch {
                    Expr::Int(n, _, _) => assert_eq!(n, 0),
                    _ => panic!("Expected Int else_branch"),
                }
            }
            _ => panic!("Expected If"),
        }
    }

    #[test]
    fn test_parse_intent() {
        let mut parser = Parser::new(r#"@intent "validate auth then dispatch""#).unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Intent(s, _, _) => assert_eq!(s, "validate auth then dispatch"),
            _ => panic!("Expected Intent"),
        }
    }

    #[test]
    fn test_parse_uncertain_wraps_primary() {
        let mut parser = Parser::new("@uncertain 42").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Uncertain(inner, _, _) => match *inner {
                Expr::Int(n, _, _) => assert_eq!(n, 42),
                _ => panic!("Expected Int inner"),
            },
            _ => panic!("Expected Uncertain"),
        }
    }

    #[test]
    fn test_parse_uncertain_wraps_arithmetic() {
        let mut parser = Parser::new("@uncertain (+ 1 2)").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Uncertain(inner, _, _) => match *inner {
                Expr::Arithmetic { op, .. } => assert_eq!(op, ArithOp::Add),
                _ => panic!("Expected wrapped arithmetic"),
            },
            _ => panic!("Expected Uncertain"),
        }
    }

    #[test]
    fn test_parse_ctx() {
        let mut parser = Parser::new("@ctx").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Ctx { .. } => {}
            _ => panic!("Expected Ctx"),
        }
    }

    #[test]
    fn test_parse_intent_rejects_non_string() {
        let mut parser = Parser::new("@intent 42").unwrap();
        assert!(parser.parse().is_err());
    }

    #[test]
    fn test_parse_diff_marker_only() {
        let mut parser = Parser::new("@diff").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { .. } => {}
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_parse_diff_with_multiple_operations() {
        let src = "@diff @replace /greet/body 99 @delete /unused";
        let mut parser = Parser::new(src).unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(ops[0].kind, DiffKind::Replace);
                assert_eq!(ops[1].kind, DiffKind::Delete);
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_parse_spanned_integer() {
        let mut parser = Parser::new("42").unwrap();
        let (expr, span) = parser.parse_spanned().unwrap();
        match expr {
            Expr::Int(n, _, _) => assert_eq!(n, 42),
            _ => panic!("Expected Int"),
        }
        assert_eq!(span, SourceSpan::new(0, 2));
    }

    #[test]
    fn test_parse_spanned_keyword_expr() {
        let mut parser = Parser::new("@true").unwrap();
        let (expr, span) = parser.parse_spanned().unwrap();
        match expr {
            Expr::Bool(b, _, _) => assert!(b),
            _ => panic!("Expected Bool"),
        }
        assert_eq!(span.start, 0);
        assert!(span.end > 0);
    }

    #[test]
    fn test_parse_spanned_leading_whitespace() {
        let mut parser = Parser::new("  99").unwrap();
        let (expr, span) = parser.parse_spanned().unwrap();
        match expr {
            Expr::Int(n, _, _) => assert_eq!(n, 99),
            _ => panic!("Expected Int"),
        }
        assert_eq!(span.start, 2);
        assert_eq!(span.end, 4);
    }

    #[test]
    fn test_parse_diffs_batch_keyword() {
        let mut parser = Parser::new("@diffs").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { .. } => {}
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_parser_span_on_simple_int() {
        let mut parser = Parser::new("42").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Int(n, _, span) => {
                assert_eq!(n, 42);
                assert_eq!(span.start, 0);
                assert_eq!(span.end, 2);
            }
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn test_parser_span_on_let_statement() {
        let mut parser = Parser::new("@let x :: 10").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Let { name, value, span, .. } => {
                assert_eq!(name, "x");
                match *value {
                    Expr::Int(n, _, _) => assert_eq!(n, 10),
                    _ => panic!("Expected Int value"),
                }
                assert_eq!(span.start, 0);
                assert!(span.end > 0);
            }
            _ => panic!("Expected Let"),
        }
    }

    #[test]
    fn test_parser_span_on_fn_def() {
        let mut parser = Parser::new("@fn greet :: -> Int @ret 42").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::FnDef {
                name, span, ..
            } => {
                assert_eq!(name, "greet");
                assert_eq!(span.start, 0);
                assert!(span.end > 0);
            }
            _ => panic!("Expected FnDef"),
        }
    }

    #[test]
    fn test_nodeid_unique_on_siblings() {
        let mut parser = Parser::new("42 99").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Block(exprs, _, _) => {
                assert_eq!(exprs.len(), 2);
                let id1 = match &exprs[0] {
                    Expr::Int(_, id, _) => *id,
                    _ => panic!("Expected Int"),
                };
                let id2 = match &exprs[1] {
                    Expr::Int(_, id, _) => *id,
                    _ => panic!("Expected Int"),
                };
                assert_ne!(id1, id2);
            }
            _ => panic!("Expected Block"),
        }
    }

    #[test]
    fn test_nodeid_same_for_identical_parse() {
        let mut parser1 = Parser::new("42").unwrap();
        let expr1 = parser1.parse().unwrap();
        let id1 = match expr1 {
            Expr::Int(_, id, _) => id,
            _ => panic!("Expected Int"),
        };

        let mut parser2 = Parser::new("42").unwrap();
        let expr2 = parser2.parse().unwrap();
        let id2 = match expr2 {
            Expr::Int(_, id, _) => id,
            _ => panic!("Expected Int"),
        };

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_nodeid_in_nested_expr() {
        let mut parser = Parser::new("(+ (* 2 3) 4)").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Arithmetic { left, right, node_id, .. } => {
                let left_id = match left.as_ref() {
                    Expr::Arithmetic { node_id, .. } => *node_id,
                    _ => panic!("Expected Arithmetic left"),
                };
                let right_id = match right.as_ref() {
                    Expr::Int(_, id, _) => *id,
                    _ => panic!("Expected Int right"),
                };
                // Outer node is constructed after its children, so its ID is higher.
                assert!(node_id > left_id);
                assert!(node_id > right_id);
                assert_ne!(left_id, right_id);
            }
            _ => panic!("Expected Arithmetic"),
        }
    }

    #[test]
    fn test_intent_empty_table() {
        let mut parser = Parser::new("42").unwrap();
        let _expr = parser.parse().unwrap();
        let table = parser.get_intent_table();
        assert!(table.entries.is_empty(), "Empty source should have no intent entries");
    }

    #[test]
    fn test_intent_single_fn_body() {
        let mut parser = Parser::new("@fn f :: -> @intent \"test\" 1").unwrap();
        let _expr = parser.parse().unwrap();
        let table = parser.get_intent_table();
        assert_eq!(table.entries.len(), 1, "Should have one intent entry");
        let entry = &table.entries[0];
        assert_eq!(entry.intent_name, "test");
        assert!(entry.selector.contains("fn f"), "Selector should contain function name");
    }

    #[test]
    fn test_intent_nested_path() {
        let mut parser = Parser::new("@fn f :: -> @let x :: @intent \"msg\" 5").unwrap();
        let _expr = parser.parse().unwrap();
        let table = parser.get_intent_table();
        assert_eq!(table.entries.len(), 1, "Should have one intent entry");
        let entry = &table.entries[0];
        assert_eq!(entry.intent_name, "msg");
        // Selector should reflect the nested structure
        assert!(entry.selector.contains("fn f"), "Should contain function scope");
        assert!(entry.selector.contains("let x"), "Should contain let scope");
    }

    #[test]
    fn test_intent_multiple_siblings() {
        let mut parser = Parser::new("@fn f :: -> @intent \"a\" @fn g :: -> @intent \"b\" 2").unwrap();
        let _expr = parser.parse().unwrap();
        let table = parser.get_intent_table();
        assert_eq!(table.entries.len(), 2, "Should have two intent entries");
        let names: Vec<_> = table.entries.iter().map(|e| e.intent_name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn test_intent_table_roundtrip() {
        // Test that intent_index from lib.rs works correctly
        use crate::intent_index;
        let source = "@fn test :: -> @intent \"roundtrip\" 42";
        let table = intent_index(source).unwrap();
        assert_eq!(table.entries.len(), 1);
        assert_eq!(table.entries[0].intent_name, "roundtrip");
    }

    #[test]
    fn test_diff_replace_simple() {
        let mut parser = Parser::new("@diff @replace /greet/body 42").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 1);
                assert_eq!(ops[0].kind, DiffKind::Replace);
                assert_eq!(ops[0].selector.parts.len(), 2);
                assert!(ops[0].payload.is_some());
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diff_insert_first() {
        let mut parser = Parser::new("@diff @insert @first /greet/body 100").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 1);
                assert_eq!(ops[0].kind, DiffKind::Insert);
                assert_eq!(ops[0].insert_mode, Some(InsertMode::First));
                assert!(ops[0].payload.is_some());
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diff_insert_before() {
        let mut parser = Parser::new("@diff @insert @before arg_name /greet/body 200").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 1);
                assert_eq!(ops[0].kind, DiffKind::Insert);
                match &ops[0].insert_mode {
                    Some(InsertMode::Before(name)) => assert_eq!(name, "arg_name"),
                    _ => panic!("Expected Before mode"),
                }
                assert!(ops[0].payload.is_some());
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diff_delete_no_payload() {
        let mut parser = Parser::new("@diff @delete /greet/body").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 1);
                assert_eq!(ops[0].kind, DiffKind::Delete);
                assert_eq!(ops[0].payload, None);
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diff_delete_rejects_payload() {
        let mut parser = Parser::new("@diff @delete /greet/body 100").unwrap();
        assert!(parser.parse().is_err());
    }

    #[test]
    fn test_diff_move_two_paths() {
        let mut parser = Parser::new("@diff @move /src @to /dst").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(ops[0].kind, DiffKind::Move);
                assert_eq!(ops[1].kind, DiffKind::Move);
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diff_copy_two_paths() {
        let mut parser = Parser::new("@diff @copy /src @to /dst").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(ops[0].kind, DiffKind::Copy);
                assert_eq!(ops[1].kind, DiffKind::Copy);
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diff_multiple_ops() {
        let mut parser = Parser::new("@diff @replace /a 10 @insert @last /b 20").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(ops[0].kind, DiffKind::Replace);
                assert_eq!(ops[1].kind, DiffKind::Insert);
                match &ops[1].insert_mode {
                    Some(InsertMode::Last) => {}
                    _ => panic!("Expected Last insert mode"),
                }
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diffs_batch() {
        let mut parser = Parser::new("@diffs @replace /a 10 @delete /b").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(ops[0].kind, DiffKind::Replace);
                assert_eq!(ops[1].kind, DiffKind::Delete);
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_diff_eval_returns_error_for_invalid_selector() {
        use crate::eval::{eval, Env};
        let mut parser = Parser::new("@diff @replace /a 5").unwrap();
        let expr = parser.parse().unwrap();
        let mut env = Env::new();
        // Standalone Diff expressions now evaluate to Ok(Value::Nil)
        let result = eval(&expr, &mut env);
        assert!(result.is_ok(), "Diff expression should evaluate to Ok");
    }

    #[test]
    fn test_selector_path_division_ambiguity() {
        // Verify that /func foo/bar parses as a selector path (not division)
        let mut parser = Parser::new("@diff @replace /func/foo/bar 42").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Diff { ops, .. } => {
                assert_eq!(ops.len(), 1);
                let selector = &ops[0].selector;
                // Path segments are split by /, so we get 3 parts
                assert_eq!(selector.parts.len(), 3);
            }
            _ => panic!("Expected Diff"),
        }
    }

    #[test]
    fn test_parse_float() {
        let mut parser = Parser::new("3.14").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Float(f, _, _) => {
                assert!((f - 3.14).abs() < 0.001);
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_parse_float_negative_literal() {
        let mut parser = Parser::new("-0.001").unwrap();
        let expr = parser.parse().unwrap();
        match expr {
            Expr::Float(f, _, _) => {
                assert!((f - (-0.001)).abs() < 0.0001);
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_list_type_annotation_in_fn_param() {
        let code = "@fn f :: xs:[Int] -> Nil _";
        let mut parser = Parser::new(code).unwrap();
        let expr = parser.parse();
        assert!(expr.is_ok(), "Failed to parse list type annotation in function parameter");
    }
}
