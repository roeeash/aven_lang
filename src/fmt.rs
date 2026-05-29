use crate::ast::{Expr, Type, PrimitiveType, EffectSet, ArithOp};

/// Format an expression into deterministic AVEN syntax.
pub fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Int(n, ..) => n.to_string(),
        Expr::Bool(b, ..) => if *b { "@true".to_string() } else { "@false".to_string() },
        Expr::Str(s, ..) => {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        }
        Expr::Nil => "_".to_string(),
        Expr::Symbol(s, ..) => s.clone(),
        Expr::Var(name, ..) => name.clone(),
        Expr::Let { name, value, .. } => {
            format!("@let {} :: {}", name, format_expr(value))
        }
        Expr::FnDef { name, params, body, return_type, effect_level, .. } => {
            let params_str = format_params(params);
            let effect_str = format_effect_set(effect_level);
            let return_type_str = match return_type {
                Some(ty) => format!(" {}", format_type(ty)),
                None => String::new(),
            };
            format!("@fn {} :: {}{}{} {}", name, params_str, effect_str, return_type_str, format_expr(body))
        }
        Expr::FnCall { name, args, .. } => {
            let args_str = args.iter().map(format_expr).collect::<Vec<_>>().join(" ");
            if args.is_empty() {
                format!("(@call {})", name)
            } else {
                format!("(@call {} {})", name, args_str)
            }
        }
        Expr::If { cond, then_branch, else_branch, .. } => {
            format!("(@if {} @then {} @else {})",
                format_expr(cond),
                format_expr(then_branch),
                format_expr(else_branch))
        }
        Expr::Arithmetic { op, left, right, .. } => {
            let op_str = match op {
                ArithOp::Add => "+",
                ArithOp::Sub => "-",
                ArithOp::Mul => "*",
                ArithOp::Div => "/",
            };
            format!("({} {} {})", op_str, format_expr(left), format_expr(right))
        }
        Expr::Block(exprs, ..) => {
            exprs.iter().map(format_expr).collect::<Vec<_>>().join("\n")
        }
        Expr::Ret(expr, ..) => {
            format!("@ret {}", format_expr(expr))
        }
        Expr::IoWrite(expr, ..) => {
            format!("@io.write {}", format_expr(expr))
        }
        Expr::TypeAlias { name, type_params, ty, .. } => {
            if type_params.is_empty() {
                format!("@type {} = {}", name, format_type(ty))
            } else {
                format!("@type {} {} = {}", name, type_params.join(" "), format_type(ty))
            }
        }
        Expr::Float(f, ..) => {
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') || s.contains('E') {
                s
            } else {
                format!("{}.0", s)
            }
        }
        Expr::Record { fields, .. } => {
            let inner = fields.iter()
                .map(|(k, v)| format!("{}:{}", k, format_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{}}}", inner)
        }
        Expr::PubDecl { inner, .. } => {
            format!("@pub {}", format_expr(inner))
        }
        Expr::CtxGet { ctx, key, .. } => {
            format!("@ctx.get {} {}", format_expr(ctx), format_expr(key))
        }
        Expr::CtxSet { ctx, key, value, .. } => {
            format!("@ctx.set {} {} {}", format_expr(ctx), format_expr(key), format_expr(value))
        }
        // Out of scope for M7.3
        _ => "<expr>".to_string(),
    }
}

/// Format a type into AVEN syntax.
pub fn format_type(ty: &Type) -> String {
    match ty {
        Type::Primitive(prim) => match prim {
            PrimitiveType::Int => "Int".to_string(),
            PrimitiveType::Bool => "Bool".to_string(),
            PrimitiveType::Str => "Str".to_string(),
            PrimitiveType::Flt => "Flt".to_string(),
            PrimitiveType::Nil => "Nil".to_string(),
        },
        Type::Option(inner) => format!("?{}", format_type(inner)),
        Type::List(inner) => format!("[{}]", format_type(inner)),
        Type::Record(_) | Type::Union(_) => "<type>".to_string(),
        Type::Symbol => "Symbol".to_string(),
        Type::TypeParam(name) => name.clone(),
        Type::TypeRef(name) => name.clone(),
        Type::TypeApp(name, args) => {
            let arg_strs: Vec<String> = args.iter().map(format_type).collect();
            format!("{} {}", name, arg_strs.join(" "))
        }
        Type::Fn { params, return_type, effect, .. } => {
            if params.len() == 1 {
                let param_str = format_type(&params[0]);
                let ret_str = format_type(return_type);
                // Guard: if either format contains an unformattable placeholder, emit "<type>"
                if param_str.contains("<type>") || param_str.contains("<uncertain>") || param_str.contains("<unannotated>")
                    || ret_str.contains("<type>") || ret_str.contains("<uncertain>") || ret_str.contains("<unannotated>") {
                    "<type>".to_string()
                } else {
                    format!("({} {} {})", param_str, effect.arrow_symbol(), ret_str)
                }
            } else {
                "<type>".to_string()
            }
        }
        Type::Uncertain(_) => {
            "<uncertain>".to_string()
        }
        Type::UnannotatedParam => "<unannotated>".to_string(),
    }
}

/// Format function parameters as a string.
fn format_params(params: &[(String, Option<Type>)]) -> String {
    params.iter()
        .map(|(name, ty_opt)| match ty_opt {
            Some(ty) => format!("{}:{}", name, format_type(ty)),
            None => name.clone(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format an effect set as an arrow symbol.
fn format_effect_set(effect: &EffectSet) -> String {
    format!(" {}", effect.arrow_symbol())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_int() {
        let expr = Expr::Int(42, 0, crate::ast::SourceSpan::zero());
        assert_eq!(format_expr(&expr), "42");
    }

    #[test]
    fn test_format_bool_true() {
        let expr = Expr::Bool(true, 0, crate::ast::SourceSpan::zero());
        assert_eq!(format_expr(&expr), "@true");
    }

    #[test]
    fn test_format_bool_false() {
        let expr = Expr::Bool(false, 0, crate::ast::SourceSpan::zero());
        assert_eq!(format_expr(&expr), "@false");
    }

    #[test]
    fn test_format_nil() {
        let expr = Expr::Nil;
        assert_eq!(format_expr(&expr), "_");
    }

    #[test]
    fn test_format_string() {
        let expr = Expr::Str("hello".to_string(), 0, crate::ast::SourceSpan::zero());
        assert_eq!(format_expr(&expr), "\"hello\"");
    }

    #[test]
    fn test_format_string_with_escape() {
        let expr = Expr::Str("hello\\world".to_string(), 0, crate::ast::SourceSpan::zero());
        assert_eq!(format_expr(&expr), "\"hello\\\\world\"");
    }

    #[test]
    fn test_format_var() {
        let expr = Expr::Var("x".to_string(), 0, crate::ast::SourceSpan::zero());
        assert_eq!(format_expr(&expr), "x");
    }

    #[test]
    fn test_format_arithmetic_add() {
        let left = Box::new(Expr::Int(3, 0, crate::ast::SourceSpan::zero()));
        let right = Box::new(Expr::Int(5, 1, crate::ast::SourceSpan::zero()));
        let expr = Expr::Arithmetic {
            op: ArithOp::Add,
            left,
            right,
            node_id: 2,
            span: crate::ast::SourceSpan::zero(),
        };
        assert_eq!(format_expr(&expr), "(+ 3 5)");
    }

    #[test]
    fn test_format_type_int() {
        let ty = Type::Primitive(PrimitiveType::Int);
        assert_eq!(format_type(&ty), "Int");
    }

    #[test]
    fn test_format_type_str() {
        let ty = Type::Primitive(PrimitiveType::Str);
        assert_eq!(format_type(&ty), "Str");
    }

    #[test]
    fn test_format_record() {
        let fields = vec![
            ("x".to_string(), Expr::Int(1, 0, crate::ast::SourceSpan::zero())),
            ("y".to_string(), Expr::Int(2, 1, crate::ast::SourceSpan::zero())),
        ];
        let expr = Expr::Record {
            fields,
            node_id: 2,
            span: crate::ast::SourceSpan::zero(),
        };
        assert_eq!(format_expr(&expr), "{x:1, y:2}");
    }
}
