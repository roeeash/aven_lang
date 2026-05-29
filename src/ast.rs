pub type NodeId = u64;

/// Byte-offset span within a source string. `start` is inclusive, `end` is
/// exclusive (one past the last byte of the token/expression).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

/// A dotted module path, e.g., "aven/std/io" or "app/services/auth".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModulePath {
    pub parts: Vec<String>,
}

impl ModulePath {
    pub fn new(parts: Vec<String>) -> Self {
        ModulePath { parts }
    }

    pub fn to_string(&self) -> String {
        self.parts.join("/")
    }
}

impl std::fmt::Display for ModulePath {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// A path segment in a selector (e.g., "fn", "let[0]").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Named(String),
    Index(usize),
}

/// A selector path for @diff operations (e.g., "/fn greet/body/ret").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorPath {
    pub parts: Vec<PathSegment>,
}

/// Insert mode for @insert operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertMode {
    First,
    Last,
    Before(String),
    After(String),
}

/// Kind of diff operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffKind {
    Replace,
    Insert,
    Delete,
    Move,
    Copy,
}

/// A single diff operation: selector path + operation kind + optional payload.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffOp {
    pub kind: DiffKind,
    pub selector: SelectorPath,
    pub payload: Option<Box<Expr>>,
    pub insert_mode: Option<InsertMode>,
    pub node_id: NodeId,
    pub span: SourceSpan,
}

/// Metadata for diff blocks: description, author, timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffMetadata {
    pub description: Option<String>,
    pub author: Option<String>,
    pub timestamp: Option<String>,
}

impl std::fmt::Display for SelectorPath {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "/")?;
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            match part {
                PathSegment::Named(s) => write!(f, "{}", s)?,
                PathSegment::Index(idx) => write!(f, "[{}]", idx)?,
            }
        }
        Ok(())
    }
}

impl SelectorPath {
    /// Parse a selector path from a string (for tests).
    pub fn from_string(s: &str) -> Result<Self, String> {
        if !s.starts_with('/') {
            return Err("Selector path must start with /".to_string());
        }
        let s = &s[1..]; // Skip leading /
        if s.is_empty() {
            return Err("Selector path cannot be empty".to_string());
        }
        let mut parts = Vec::new();
        for segment in s.split('/') {
            if segment.is_empty() {
                return Err("Empty path segment".to_string());
            }
            // Check if segment has an index suffix [n]
            if let Some(bracket_idx) = segment.find('[') {
                let name = &segment[..bracket_idx];
                let index_part = &segment[bracket_idx + 1..];
                if !index_part.ends_with(']') {
                    return Err("Malformed index".to_string());
                }
                let idx_str = &index_part[..index_part.len() - 1];
                let idx: usize = idx_str.parse().map_err(|_| "Invalid index".to_string())?;
                if !name.is_empty() {
                    parts.push(PathSegment::Named(name.to_string()));
                }
                parts.push(PathSegment::Index(idx));
            } else {
                parts.push(PathSegment::Named(segment.to_string()));
            }
        }
        Ok(SelectorPath { parts })
    }
}

/// An entry in the intent index table: selector path, intent name, and subtree span.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentEntry {
    pub selector: String,
    pub intent_name: String,
    pub subtree_span: SourceSpan,
}

/// A table of all `@intent` annotations found during parsing, indexed by selector path.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentTable {
    pub entries: Vec<IntentEntry>,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize) -> Self {
        SourceSpan { start, end }
    }
    pub fn zero() -> Self {
        SourceSpan { start: 0, end: 0 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64, NodeId, SourceSpan),
    Float(f64, NodeId, SourceSpan),
    Str(String, NodeId, SourceSpan),
    Bool(bool, NodeId, SourceSpan),
    Nil,
    Symbol(String, NodeId, SourceSpan),
    Var(String, NodeId, SourceSpan),

    Let {
        name: String,
        value: Box<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },

    FnDef {
        name: String,
        params: Vec<(String, Option<Type>)>,
        body: Box<Expr>,
        return_type: Option<Type>,
        effect_level: EffectSet,
        cap: Vec<CapabilityMarker>,
        node_id: NodeId,
        span: SourceSpan,
    },

    FnCall {
        name: String,
        args: Vec<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },

    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },

    Arithmetic {
        op: ArithOp,
        left: Box<Expr>,
        right: Box<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },

    Ret(Box<Expr>, NodeId, SourceSpan),

    IoWrite(Box<Expr>, NodeId, SourceSpan),

    Block(Vec<Expr>, NodeId, SourceSpan),

    Intent(String, NodeId, SourceSpan),

    Uncertain(Box<Expr>, NodeId, SourceSpan),

    Ctx {
        node_id: NodeId,
        span: SourceSpan,
    },

    CtxGet {
        ctx: Box<Expr>,
        key: Box<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },

    CtxSet {
        ctx: Box<Expr>,
        key: Box<Expr>,
        value: Box<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },

    Diff {
        metadata: Option<DiffMetadata>,
        ops: Vec<DiffOp>,
        node_id: NodeId,
        span: SourceSpan,
    },

    Use {
        caps: Vec<(String, Option<String>)>,
        module: ModulePath,
        node_id: NodeId,
        span: SourceSpan,
    },

    Match {
        scrutinee: Box<Expr>,
        patterns: Vec<(Pattern, Expr)>,
        node_id: NodeId,
        span: SourceSpan,
    },

    Tagged {
        tag: String,
        payload: Option<Box<Expr>>,
        node_id: NodeId,
        span: SourceSpan,
    },

    Mod {
        name: ModulePath,
        node_id: NodeId,
        span: SourceSpan,
    },

    Pub {
        cap: Vec<CapabilityMarker>,
        node_id: NodeId,
        span: SourceSpan,
    },

    PubDecl {
        inner: Box<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },

    TypeAlias {
        name: String,
        type_params: Vec<String>,
        ty: Type,
        node_id: NodeId,
        span: SourceSpan,
    },

    Record {
        fields: Vec<(String, Expr)>,
        node_id: NodeId,
        span: SourceSpan,
    },

    List {
        elements: Vec<Expr>,
        node_id: NodeId,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Tag(String),                    // Matches bare #tag symbol
    TagBind(String, String),        // Matches #tag var, binds payload to var
    Wildcard,                       // Matches anything, no binding
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Capability marker for function types (e.g., "read", "write").
pub type CapabilityMarker = String;

/// Primitive types in AVEN.
#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveType {
    Int,
    Bool,
    Str,
    Flt,
    Nil,
}

/// Orthogonal effect flags for function arrows: error, IO, and async.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectSet {
    pub err: bool,
    pub io: bool,
    pub async_: bool,
}

impl EffectSet {
    pub fn pure_() -> Self {
        Self { err: false, io: false, async_: false }
    }
    pub fn is_pure(&self) -> bool {
        !self.err && !self.io && !self.async_
    }
    pub fn is_subset_of(&self, other: &EffectSet) -> bool {
        (!self.err || other.err) && (!self.io || other.io) && (!self.async_ || other.async_)
    }
    pub fn arrow_symbol(&self) -> &'static str {
        match (self.err, self.io, self.async_) {
            (false, false, false) => "->",
            (true,  false, false) => "-?>",
            (false, true,  false) => "-!>",
            (false, false, true)  => "-~>",
            (true,  true,  false) => "-?!>",
            (true,  false, true)  => "-?~>",
            (false, true,  true)  => "-!~>",
            (true,  true,  true)  => "-?!~>",
        }
    }
}

/// A variant in a union type.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionVariant {
    pub tag: String,
    pub payload: Option<Box<Type>>,
}

/// Types in AVEN covering primitives, functions, options, records, unions, and parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Primitive(PrimitiveType),
    Fn {
        params: Vec<Type>,
        return_type: Box<Type>,
        effect: EffectSet,
        cap: Option<CapabilityMarker>,
    },
    Option(Box<Type>),
    List(Box<Type>),
    Record(Vec<(String, Type)>),
    Union(Vec<UnionVariant>),
    Symbol,
    TypeParam(String),
    TypeRef(String),
    TypeApp(String, Vec<Type>),  // Generic type application, e.g., Pair Int Str
    Uncertain(Box<Type>),  // Wraps a type with uncertainty flag for M3.3
    UnannotatedParam,      // Sentinel for unannotated function parameters (M2 TC-R04)
}

impl Type {
    pub fn is_pure(&self) -> bool {
        match self {
            Type::Fn { effect, .. } => effect.is_pure(),
            _ => false,
        }
    }

    pub fn is_io(&self) -> bool {
        match self {
            Type::Fn { effect, .. } => effect.io,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_primitive_int() {
        let t = Type::Primitive(PrimitiveType::Int);
        assert_eq!(t, Type::Primitive(PrimitiveType::Int));
    }

    #[test]
    fn test_type_primitive_bool() {
        let t = Type::Primitive(PrimitiveType::Bool);
        assert_eq!(t, Type::Primitive(PrimitiveType::Bool));
    }

    #[test]
    fn test_type_fn_pure() {
        let t = Type::Fn {
            params: vec![Type::Primitive(PrimitiveType::Int)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        assert!(t.is_pure());
    }

    #[test]
    fn test_type_fn_io() {
        let t = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Nil)),
            effect: EffectSet { err: false, io: true, async_: false },
            cap: None,
        };
        assert!(t.is_io());
    }

    #[test]
    fn test_type_fn_with_cap() {
        let t = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Nil)),
            effect: EffectSet::pure_(),
            cap: Some("write".to_string()),
        };
        if let Type::Fn { cap, .. } = t {
            assert_eq!(cap, Some("write".to_string()));
        } else {
            panic!("Expected Fn variant");
        }
    }

    #[test]
    fn test_type_option() {
        let t = Type::Option(Box::new(Type::Primitive(PrimitiveType::Str)));
        if let Type::Option(inner) = t {
            assert_eq!(*inner, Type::Primitive(PrimitiveType::Str));
        } else {
            panic!("Expected Option variant");
        }
    }

    #[test]
    fn test_type_record() {
        let t = Type::Record(vec![
            ("name".to_string(), Type::Primitive(PrimitiveType::Str)),
            ("age".to_string(), Type::Primitive(PrimitiveType::Int)),
        ]);
        if let Type::Record(fields) = t {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "name");
            assert_eq!(fields[1].0, "age");
        } else {
            panic!("Expected Record variant");
        }
    }

    #[test]
    fn test_type_union() {
        let t = Type::Union(vec![
            UnionVariant {
                tag: "ok".to_string(),
                payload: None,
            },
            UnionVariant {
                tag: "err".to_string(),
                payload: None,
            },
        ]);
        if let Type::Union(variants) = t {
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].tag, "ok");
            assert_eq!(variants[1].tag, "err");
        } else {
            panic!("Expected Union variant");
        }
    }

    #[test]
    fn test_type_param() {
        let t = Type::TypeParam("a".to_string());
        if let Type::TypeParam(name) = t {
            assert_eq!(name, "a");
        } else {
            panic!("Expected TypeParam variant");
        }
    }

    #[test]
    fn test_effect_set_pure_arrow() {
        assert_eq!(EffectSet::pure_().arrow_symbol(), "->");
    }

    #[test]
    fn test_effect_set_io_arrow() {
        assert_eq!(EffectSet { err: false, io: true, async_: false }.arrow_symbol(), "-!>");
    }

    #[test]
    fn test_effect_set_is_subset_of() {
        let pure = EffectSet::pure_();
        let io = EffectSet { err: false, io: true, async_: false };
        let err_io = EffectSet { err: true, io: true, async_: false };
        assert!(pure.is_subset_of(&io));
        assert!(io.is_subset_of(&err_io));
        assert!(!err_io.is_subset_of(&io));
    }

    #[test]
    fn test_type_uncertain_wraps_int() {
        let t = Type::Uncertain(Box::new(Type::Primitive(PrimitiveType::Int)));
        if let Type::Uncertain(inner) = t {
            assert_eq!(*inner, Type::Primitive(PrimitiveType::Int));
        } else {
            panic!("Expected Uncertain variant");
        }
    }
}
