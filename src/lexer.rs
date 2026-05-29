use crate::ast::{SourceSpan, EffectSet};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Sigils
    At,                  // @
    Hash,                // #
    Underscore,          // _
    
    // Keywords (sigil-prefixed)
    Let,                 // @let
    Fn,                  // @fn
    Type,                // @type
    Ret,                 // @ret
    If,                  // @if
    Then,                // @then
    Else,                // @else
    True,                // @true
    False,               // @false
    IoWrite,             // @io.write
    Call,                // @call
    Cap,                 // @cap
    Use,                 // @use
    From,                // @from
    Mod,                 // @mod
    Pub,                 // @pub
    Match,               // @match
    Ok,                  // @ok
    Err,                 // @err
    As,                  // @as

    // AI-native annotation keywords (parse-only in M1)
    Intent,              // @intent
    Uncertain,           // @uncertain
    Ctx,                 // @ctx
    CtxGet,              // @ctx.get
    CtxSet,              // @ctx.set

    // @diff sub-grammar keywords. Reserved in M1; full parsing in M5.
    // These tokens cannot appear as identifiers — the lexer always promotes them
    // to keyword tokens. The parser only acts on @diff/@diffs; the operation
    // tokens are reserved so M5 can give them grammar without breaking M1 code.
    Diff,                // @diff
    Diffs,               // @diffs
    Replace,             // @replace
    Insert,              // @insert
    Delete,              // @delete
    Move,                // @move
    Copy,                // @copy
    Meta,                // @meta
    To,                  // @to
    First,               // @first
    Last,                // @last
    Before,              // @before
    After,               // @after
    PatchFor,            // @patch-for
    
    // Separators
    Colon,               // :
    DoubleColon,         // ::
    Equals,              // =
    Arrow,               // ->
    EffectArrow(EffectSet), // -?>, -!>, -~>, etc.
    Comma,
    LeftParen,           // (
    RightParen,          // )
    LeftBrace,           // {
    RightBrace,          // }
    LeftBracket,         // [
    RightBracket,        // ]
    Pipe,                // |
    
    // Operators
    Plus,                // +
    Minus,               // -
    Star,                // *
    Slash,               // /
    Question,            // ?

    // Literals
    Ident(String),
    Integer(i64),
    Float(f64),
    String(String),
    
    // Special
    Eof,
    Newline,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
        }
    }
    
    fn current(&self) -> Option<char> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }
    
    fn advance(&mut self) -> Option<char> {
        let ch = self.current();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }
    
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current() {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }
    
    fn skip_comment(&mut self) {
        if self.current() == Some(';') {
            while let Some(ch) = self.current() {
                if ch == '\n' {
                    break;
                }
                self.advance();
            }
        }
    }
    
    fn read_string(&mut self) -> String {
        let mut result = String::new();
        self.advance(); // skip opening "
        
        while let Some(ch) = self.current() {
            if ch == '"' {
                self.advance();
                break;
            }
            if ch == '\\' {
                self.advance();
                if let Some(escaped) = self.current() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        '\\' => result.push('\\'),
                        '"' => result.push('"'),
                        _ => result.push(escaped),
                    }
                    self.advance();
                }
            } else {
                result.push(ch);
                self.advance();
            }
        }
        
        result
    }
    
    fn read_number(&mut self) -> (f64, bool) {
        let mut num_str = String::new();
        let mut is_float = false;

        // Read integer part
        while let Some(ch) = self.current() {
            if ch.is_ascii_digit() || ch == '_' {
                if ch != '_' {
                    num_str.push(ch);
                }
                self.advance();
            } else {
                break;
            }
        }

        // Check for decimal point followed by digit(s)
        if self.current() == Some('.') {
            let saved_pos = self.pos;
            self.advance(); // consume '.'
            if let Some(ch) = self.current() {
                if ch.is_ascii_digit() {
                    // This is a float
                    is_float = true;
                    num_str.push('.');
                    while let Some(ch) = self.current() {
                        if ch.is_ascii_digit() || ch == '_' {
                            if ch != '_' {
                                num_str.push(ch);
                            }
                            self.advance();
                        } else {
                            break;
                        }
                    }
                } else {
                    // Not a float, backtrack
                    self.pos = saved_pos;
                }
            } else {
                // Not a float, backtrack
                self.pos = saved_pos;
            }
        }

        // Check for exponent
        if (self.current() == Some('e') || self.current() == Some('E')) && !num_str.is_empty() {
            let saved_pos = self.pos;
            self.advance(); // consume 'e'/'E'
            let mut exp_str = String::from("e");

            // Optional sign
            if self.current() == Some('+') || self.current() == Some('-') {
                exp_str.push(self.current().unwrap());
                self.advance();
            }

            // Exponent digits
            let mut has_digits = false;
            while let Some(ch) = self.current() {
                if ch.is_ascii_digit() {
                    exp_str.push(ch);
                    has_digits = true;
                    self.advance();
                } else {
                    break;
                }
            }

            if has_digits {
                is_float = true;
                num_str.push_str(&exp_str);
            } else {
                // No exponent digits, backtrack
                self.pos = saved_pos;
            }
        }

        let val = num_str.parse::<f64>().unwrap_or(0.0);
        (val, is_float)
    }
    
    fn read_ident(&mut self) -> String {
        let mut ident = String::new();

        while let Some(ch) = self.current() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        ident
    }

    /// Read a symbol name (for #name syntax): only alphanumeric + underscore, no dots or dashes.
    fn read_symbol_name(&mut self) -> String {
        let mut name = String::new();

        while let Some(ch) = self.current() {
            if ch.is_alphanumeric() || ch == '_' {
                name.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        name
    }
    
    /// Core tokeniser: returns the next `(Token, start_byte_offset)`.
    /// The caller computes `end = self.pos` after this returns to form a span.
    fn next_token_impl(&mut self) -> (Token, usize) {
        loop {
            self.skip_whitespace();

            match self.current() {
                Some(';') => {
                    self.skip_comment();
                    continue;
                }
                Some('\n') => {
                    let start = self.pos;
                    self.advance();
                    return (Token::Newline, start);
                }
                Some('@') => {
                    let start = self.pos;
                    self.advance();
                    let ident = self.read_ident();
                    let tok = match ident.as_str() {
                        "let" => Token::Let,
                        "fn" => Token::Fn,
                        "type" => Token::Type,
                        "ret" => Token::Ret,
                        "if" => Token::If,
                        "then" => Token::Then,
                        "else" => Token::Else,
                        "true" => Token::True,
                        "false" => Token::False,
                        "call" => Token::Call,
                        "cap" => Token::Cap,
                        "use" => Token::Use,
                        "from" => Token::From,
                        "mod" => Token::Mod,
                        "pub" => Token::Pub,
                        "match" => Token::Match,
                        "ok" => Token::Ok,
                        "err" => Token::Err,
                        "as" => Token::As,
                        "io.write" => Token::IoWrite,
                        "intent" => Token::Intent,
                        "uncertain" => Token::Uncertain,
                        "ctx.get" => Token::CtxGet,
                        "ctx.set" => Token::CtxSet,
                        "ctx" => Token::Ctx,
                        "diff" => Token::Diff,
                        "diffs" => Token::Diffs,
                        "replace" => Token::Replace,
                        "insert" => Token::Insert,
                        "delete" => Token::Delete,
                        "move" => Token::Move,
                        "copy" => Token::Copy,
                        "meta" => Token::Meta,
                        "to" => Token::To,
                        "first" => Token::First,
                        "last" => Token::Last,
                        "before" => Token::Before,
                        "after" => Token::After,
                        "patch-for" => Token::PatchFor,
                        _ => Token::Ident(format!("@{}", ident)),
                    };
                    return (tok, start);
                }
                Some('#') => {
                    let start = self.pos;
                    self.advance();
                    let sym = self.read_symbol_name();
                    // Validate: symbol name must not be empty and must not start with digit
                    if sym.is_empty() {
                        return (Token::Ident("#".to_string()), start);
                    }
                    if sym.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                        // Reject digit-leading symbol; emit Error token conceptually, but lexer doesn't have error token.
                        // For now, emit it as Ident and let parser handle the error.
                        return (Token::Ident(format!("#{}", sym)), start);
                    }
                    return (Token::Ident(format!("#{}", sym)), start);
                }
                Some('"') => {
                    let start = self.pos;
                    let s = self.read_string();
                    return (Token::String(s), start);
                }
                Some(':') => {
                    let start = self.pos;
                    self.advance();
                    if self.current() == Some(':') {
                        self.advance();
                        return (Token::DoubleColon, start);
                    } else {
                        return (Token::Colon, start);
                    }
                }
                Some('-') => {
                    let start = self.pos;
                    self.advance();
                    if self.current() == Some('>') {
                        self.advance();
                        return (Token::Arrow, start);
                    } else if self.current() == Some('?') || self.current() == Some('!') || self.current() == Some('~') {
                        let mut err = false;
                        let mut io = false;
                        let mut async_ = false;
                        if self.current() == Some('?') {
                            err = true;
                            self.advance();
                        }
                        if self.current() == Some('!') {
                            io = true;
                            self.advance();
                        }
                        if self.current() == Some('~') {
                            async_ = true;
                            self.advance();
                        }
                        if self.current() == Some('>') {
                            self.advance();
                            return (Token::EffectArrow(EffectSet { err, io, async_ }), start);
                        }
                        // Not a valid effect arrow, return Minus and re-process
                        return (Token::Minus, start);
                    } else if self.current().map_or(false, |c| c.is_ascii_digit()) {
                        let (num, is_float) = self.read_number();
                        if is_float {
                            return (Token::Float(-num), start);
                        } else {
                            return (Token::Integer(-(num as i64)), start);
                        }
                    } else {
                        return (Token::Minus, start);
                    }
                }
                Some('~') => {
                    // Lone ~ (not part of -?!~> syntax) is invalid; skip and continue parsing.
                    self.advance();
                    continue;
                }
                Some('=') => { let s = self.pos; self.advance(); return (Token::Equals, s); }
                Some('+') => { let s = self.pos; self.advance(); return (Token::Plus, s); }
                Some('*') => { let s = self.pos; self.advance(); return (Token::Star, s); }
                Some('/') => { let s = self.pos; self.advance(); return (Token::Slash, s); }
                Some('?') => { let s = self.pos; self.advance(); return (Token::Question, s); }
                Some('(') => { let s = self.pos; self.advance(); return (Token::LeftParen, s); }
                Some(')') => { let s = self.pos; self.advance(); return (Token::RightParen, s); }
                Some('[') => { let s = self.pos; self.advance(); return (Token::LeftBracket, s); }
                Some(']') => { let s = self.pos; self.advance(); return (Token::RightBracket, s); }
                Some('{') => { let s = self.pos; self.advance(); return (Token::LeftBrace, s); }
                Some('}') => { let s = self.pos; self.advance(); return (Token::RightBrace, s); }
                Some('|') => { let s = self.pos; self.advance(); return (Token::Pipe, s); }
                Some(',') => { let s = self.pos; self.advance(); return (Token::Comma, s); }
                Some('_') => { let s = self.pos; self.advance(); return (Token::Underscore, s); }
                Some(ch) if ch.is_ascii_digit() => {
                    let start = self.pos;
                    let (num, is_float) = self.read_number();
                    if is_float {
                        return (Token::Float(num), start);
                    } else {
                        return (Token::Integer(num as i64), start);
                    }
                }
                Some(ch) if ch.is_alphabetic() => {
                    let start = self.pos;
                    let ident = self.read_ident();
                    return (Token::Ident(ident), start);
                }
                None => return (Token::Eof, self.pos),
                Some(_) => {
                    self.advance();
                    continue;
                }
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.next_token_impl().0
    }

    /// Like `next_token` but also returns the byte span `[start, end)` of the token.
    pub fn next_token_spanned(&mut self) -> (Token, SourceSpan) {
        let (tok, start) = self.next_token_impl();
        (tok, SourceSpan::new(start, self.pos))
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token();
            if tok == Token::Eof {
                tokens.push(tok);
                break;
            }
            if tok != Token::Newline {
                tokens.push(tok);
            }
        }
        tokens
    }

    /// Like `tokenize` but preserves byte spans for each token.
    pub fn tokenize_spanned(&mut self) -> Vec<(Token, SourceSpan)> {
        let mut tokens = Vec::new();
        loop {
            let (tok, span) = self.next_token_spanned();
            if tok == Token::Eof {
                tokens.push((tok, span));
                break;
            }
            if tok != Token::Newline {
                tokens.push((tok, span));
            }
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keywords() {
        let mut lexer = Lexer::new("@let @fn @ret @if @then @else @true @false @call @io.write");
        let tokens = lexer.tokenize();
        assert!(tokens.contains(&Token::Let));
        assert!(tokens.contains(&Token::Fn));
        assert!(tokens.contains(&Token::Ret));
        assert!(tokens.contains(&Token::If));
        assert!(tokens.contains(&Token::Then));
        assert!(tokens.contains(&Token::Else));
        assert!(tokens.contains(&Token::True));
        assert!(tokens.contains(&Token::False));
        assert!(tokens.contains(&Token::Call));
        assert!(tokens.contains(&Token::IoWrite));
    }
    
    #[test]
    fn test_integers() {
        let mut lexer = Lexer::new("42 -7 1_000");
        let tokens = lexer.tokenize();
        assert_eq!(tokens[0], Token::Integer(42));
        assert_eq!(tokens[1], Token::Integer(-7));
        assert_eq!(tokens[2], Token::Integer(1000));
    }
    
    #[test]
    fn test_strings() {
        let mut lexer = Lexer::new(r#""hello" "world""#);
        let tokens = lexer.tokenize();
        assert_eq!(tokens[0], Token::String("hello".to_string()));
        assert_eq!(tokens[1], Token::String("world".to_string()));
    }
    
    #[test]
    fn test_symbols() {
        let mut lexer = Lexer::new("#admin #get #pending");
        let tokens = lexer.tokenize();
        assert_eq!(tokens[0], Token::Ident("#admin".to_string()));
        assert_eq!(tokens[1], Token::Ident("#get".to_string()));
        assert_eq!(tokens[2], Token::Ident("#pending".to_string()));
    }
    
    #[test]
    fn test_comments() {
        let mut lexer = Lexer::new("42 ; this is a comment\n 100");
        let tokens = lexer.tokenize();
        assert_eq!(tokens[0], Token::Integer(42));
        assert_eq!(tokens[1], Token::Integer(100));
    }
    
    #[test]
    fn test_operators() {
        let mut lexer = Lexer::new("+ - * /");
        let tokens = lexer.tokenize();
        assert_eq!(tokens[0], Token::Plus);
        assert_eq!(tokens[1], Token::Minus);
        assert_eq!(tokens[2], Token::Star);
        assert_eq!(tokens[3], Token::Slash);
    }

    #[test]
    fn test_annotation_keywords() {
        let mut lexer = Lexer::new("@intent @uncertain @ctx");
        let tokens = lexer.tokenize();
        assert_eq!(tokens[0], Token::Intent);
        assert_eq!(tokens[1], Token::Uncertain);
        assert_eq!(tokens[2], Token::Ctx);
    }

    #[test]
    fn test_token_spans_integers() {
        let mut lexer = Lexer::new("42 100");
        let spanned = lexer.tokenize_spanned();
        // "42" → bytes 0..2, "100" → bytes 3..6
        assert_eq!(spanned[0], (Token::Integer(42),  SourceSpan::new(0, 2)));
        assert_eq!(spanned[1], (Token::Integer(100), SourceSpan::new(3, 6)));
    }

    #[test]
    fn test_token_spans_keyword() {
        let mut lexer = Lexer::new("@let");
        let spanned = lexer.tokenize_spanned();
        assert_eq!(spanned[0], (Token::Let, SourceSpan::new(0, 4)));
    }

    #[test]
    fn test_token_spans_string_literal() {
        let mut lexer = Lexer::new(r#""hi""#);
        let spanned = lexer.tokenize_spanned();
        // The span covers the opening quote through the closing quote: bytes 0..4
        assert_eq!(spanned[0], (Token::String("hi".to_string()), SourceSpan::new(0, 4)));
    }

    #[test]
    fn test_token_spans_skip_whitespace() {
        // Leading spaces shift the start offset.
        let mut lexer = Lexer::new("   99");
        let spanned = lexer.tokenize_spanned();
        assert_eq!(spanned[0], (Token::Integer(99), SourceSpan::new(3, 5)));
    }

    #[test]
    fn test_diff_keywords() {
        // All @diff sub-grammar keywords lex as distinct tokens — not generic
        // identifiers — so M5 can act on them without breaking M1 code.
        let mut lexer = Lexer::new("@diff @diffs @replace @insert @delete @move @copy @meta");
        let tokens = lexer.tokenize();
        assert_eq!(tokens[0], Token::Diff);
        assert_eq!(tokens[1], Token::Diffs);
        assert_eq!(tokens[2], Token::Replace);
        assert_eq!(tokens[3], Token::Insert);
        assert_eq!(tokens[4], Token::Delete);
        assert_eq!(tokens[5], Token::Move);
        assert_eq!(tokens[6], Token::Copy);
        assert_eq!(tokens[7], Token::Meta);
    }

    #[test]
    fn test_effect_arrow_io_only() {
        // Tokenize -!> and verify it yields EffectArrow with IO flag
        let mut lexer = Lexer::new("-!>");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::EffectArrow(es) => {
                assert!(!es.err && es.io && !es.async_);
            }
            _ => panic!("Expected EffectArrow"),
        }
    }

    #[test]
    fn test_effect_arrow_async_only() {
        // Tokenize -~> and verify it yields EffectArrow with async flag
        let mut lexer = Lexer::new("-~>");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::EffectArrow(es) => {
                assert!(!es.err && !es.io && es.async_);
            }
            _ => panic!("Expected EffectArrow"),
        }
    }

    #[test]
    fn test_effect_arrow_err_async() {
        // Tokenize -?~> and verify it yields EffectArrow with err and async flags
        let mut lexer = Lexer::new("-?~>");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::EffectArrow(es) => {
                assert!(es.err && !es.io && es.async_);
            }
            _ => panic!("Expected EffectArrow"),
        }
    }

    #[test]
    fn test_effect_arrow_io_async() {
        // Tokenize -!~> and verify it yields EffectArrow with IO and async flags
        let mut lexer = Lexer::new("-!~>");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::EffectArrow(es) => {
                assert!(!es.err && es.io && es.async_);
            }
            _ => panic!("Expected EffectArrow"),
        }
    }

    #[test]
    fn test_float_simple() {
        let mut lexer = Lexer::new("3.14");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::Float(f) => {
                assert!((f - 3.14).abs() < 0.001);
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_float_negative() {
        let mut lexer = Lexer::new("-0.001");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::Float(f) => {
                assert!((f - (-0.001)).abs() < 0.0001);
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_float_with_exponent() {
        let mut lexer = Lexer::new("1.5e2");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::Float(f) => {
                assert!((f - 150.0).abs() < 0.001);
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_integer_not_float() {
        let mut lexer = Lexer::new("42");
        let tokens = lexer.tokenize();
        match &tokens[0] {
            Token::Integer(i) => assert_eq!(*i, 42),
            _ => panic!("Expected Integer"),
        }
    }
}
