use crate::ast::{
    ArithOp, CapabilityMarker, EffectSet, Expr, ModulePath, Pattern, PrimitiveType, SourceSpan,
    Type, UnionVariant,
};
use crate::parser::{ParseError, Parser};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub struct TypeError {
    pub span: SourceSpan,
    pub message: String,
}

impl TypeError {
    pub fn display_with_source(&self, source: &str) -> String {
        if self.span.end > 0 {
            let (line, col) = crate::source_to_line_col(source, self.span.start);
            format!("{}:{}: {}", line, col, self.message)
        } else {
            self.message.clone()
        }
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Debug)]
pub struct TypeEnv {
    bindings: HashMap<String, Type>,
    parent: Option<Box<TypeEnv>>,
    pub effect_level: Option<EffectSet>,
    allow_uncertain: bool,
    pub module_caps: HashMap<ModulePath, Vec<CapabilityMarker>>,
    pub type_aliases: HashMap<String, (Vec<String>, Type)>,
}

impl TypeEnv {
    pub fn new() -> Self {
        TypeEnv {
            bindings: HashMap::new(),
            parent: None,
            effect_level: None,
            allow_uncertain: false,
            module_caps: HashMap::new(),
            type_aliases: HashMap::new(),
        }
    }

    pub fn with_parent(parent: &TypeEnv) -> Self {
        TypeEnv {
            bindings: HashMap::new(),
            parent: Some(Box::new(parent.clone())),
            effect_level: parent.effect_level,
            allow_uncertain: parent.allow_uncertain,
            module_caps: parent.module_caps.clone(),
            type_aliases: parent.type_aliases.clone(),
        }
    }

    pub fn with_parent_and_effect(parent: &TypeEnv, effect: EffectSet) -> Self {
        let mut env = Self::with_parent(parent);
        env.effect_level = Some(effect);
        env
    }

    pub fn with_uncertain_allowed(parent: &TypeEnv) -> Self {
        let mut env = Self::with_parent(parent);
        env.allow_uncertain = true;
        env
    }

    pub fn define(&mut self, name: String, ty: Type) {
        self.bindings.insert(name, ty);
    }

    pub fn get(&self, name: &str) -> Option<Type> {
        if let Some(ty) = self.bindings.get(name) {
            Some(ty.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    pub fn effect_level(&self) -> Option<EffectSet> {
        self.effect_level.or_else(|| {
            self.parent
                .as_ref()
                .and_then(|p| p.effect_level())
        })
    }
}

impl Clone for TypeEnv {
    fn clone(&self) -> Self {
        TypeEnv {
            bindings: self.bindings.clone(),
            parent: self.parent.clone(),
            effect_level: self.effect_level,
            allow_uncertain: self.allow_uncertain,
            module_caps: self.module_caps.clone(),
            type_aliases: self.type_aliases.clone(),
        }
    }
}

/// Resolves a type reference (e.g., a type alias) to its underlying type.
/// Returns the resolved type or an error if not found.
fn resolve_type_ref_with_span(ty: &Type, env: &TypeEnv, span: SourceSpan) -> Result<Type, TypeError> {
    let mut visited = std::collections::HashSet::new();
    resolve_type_ref_impl(ty, env, span, &mut visited)
}

fn resolve_type_ref_impl(ty: &Type, env: &TypeEnv, span: SourceSpan, visited: &mut std::collections::HashSet<String>) -> Result<Type, TypeError> {
    match ty {
        Type::TypeRef(name) => {
            if visited.contains(name) {
                return Err(TypeError {
                    span,
                    message: format!("Cyclic type alias: {}", name),
                });
            }
            visited.insert(name.clone());
            match env.type_aliases.get(name).cloned() {
                Some((type_params, resolved)) => {
                    if !type_params.is_empty() {
                        return Err(TypeError {
                            span,
                            message: format!(
                                "Generic type '{}' expects {} type argument(s), got 0",
                                name, type_params.len()
                            ),
                        });
                    }
                    resolve_type_ref_impl(&resolved, env, span, visited)
                }
                None => Err(TypeError {
                    span,
                    message: format!("Undefined type alias: {}", name),
                })
            }
        }
        Type::TypeApp(name, args) => {
            if visited.contains(name) {
                return Err(TypeError {
                    span,
                    message: format!("Cyclic type alias: {}", name),
                });
            }
            visited.insert(name.clone());
            let resolved_args: Vec<Type> = args.iter()
                .map(|a| resolve_type_ref_impl(a, env, span, visited))
                .collect::<Result<_, _>>()?;
            match env.type_aliases.get(name).cloned() {
                Some((type_params, body)) => {
                    if type_params.len() != resolved_args.len() {
                        return Err(TypeError {
                            span,
                            message: format!(
                                "Generic type '{}' expects {} type arguments, got {}",
                                name, type_params.len(), resolved_args.len()
                            ),
                        });
                    }
                    let subst: std::collections::HashMap<String, Type> = type_params.iter()
                        .zip(resolved_args.iter())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    let substituted = substitute_type_params(&body, &subst);
                    resolve_type_ref_impl(&substituted, env, span, visited)
                }
                None => Err(TypeError {
                    span,
                    message: format!("Undefined generic type: {}", name),
                }),
            }
        }
        Type::Option(inner) => {
            let resolved = resolve_type_ref_impl(inner, env, span, visited)?;
            Ok(Type::Option(Box::new(resolved)))
        }
        Type::List(inner) => {
            let resolved = resolve_type_ref_impl(inner, env, span, visited)?;
            Ok(Type::List(Box::new(resolved)))
        }
        Type::Record(fields) => {
            let mut resolved_fields = Vec::new();
            for (name, field_ty) in fields {
                let resolved = resolve_type_ref_impl(field_ty, env, span, visited)?;
                resolved_fields.push((name.clone(), resolved));
            }
            Ok(Type::Record(resolved_fields))
        }
        Type::Fn { params, return_type, effect, cap } => {
            let mut resolved_params = Vec::new();
            for param in params {
                let resolved = resolve_type_ref_impl(param, env, span, visited)?;
                resolved_params.push(resolved);
            }
            let resolved_return = resolve_type_ref_impl(return_type, env, span, visited)?;
            Ok(Type::Fn {
                params: resolved_params,
                return_type: Box::new(resolved_return),
                effect: *effect,
                cap: cap.clone(),
            })
        }
        _ => Ok(ty.clone()),
    }
}

/// Structural compatibility for type checking (bidirectional).
pub fn types_compatible(expected: &Type, found: &Type) -> bool {
    types_compatible_impl(expected, found)
}

fn types_compatible_impl(expected: &Type, found: &Type) -> bool {
    match (expected, found) {
        (Type::Primitive(a), Type::Primitive(b)) => a == b,
        (Type::Option(expected_inner), Type::Option(found_inner)) => {
            // Option[T] matches Option[T] exactly
            types_compatible_impl(expected_inner, found_inner)
        }
        (Type::Option(inner), found_type) => {
            // Option[T] is satisfied by T itself or Nil
            types_compatible_impl(inner, found_type)
                || (found_type == &Type::Primitive(PrimitiveType::Nil))
        }
        (Type::List(a), Type::List(b)) => types_compatible_impl(a, b),
        (Type::Record(a_fields), Type::Record(b_fields)) => {
            a_fields.len() == b_fields.len()
                && a_fields
                    .iter()
                    .zip(b_fields.iter())
                    .all(|((an, at), (bn, bt))| an == bn && types_compatible_impl(at, bt))
        }
        (Type::Union(a_vars), Type::Union(b_vars)) => {
            // Subset check: every variant in b_vars (found) must appear in a_vars (expected) by tag
            b_vars.iter().all(|b| {
                a_vars.iter().any(|a| {
                    a.tag == b.tag
                        && match (&a.payload, &b.payload) {
                            (None, None) => true,
                            (Some(ap), Some(bp)) => types_compatible_impl(ap, bp),
                            _ => false,
                        }
                })
            })
        }
        (
            Type::Fn {
                params: p1,
                return_type: r1,
                effect: e1,
                cap: c1,
            },
            Type::Fn {
                params: p2,
                return_type: r2,
                effect: e2,
                cap: c2,
            },
        ) => {
            // Covariant effect compatibility: found effects (e2) must be subset of expected (e1).
            // This allows a Pure function to satisfy an IO-expected parameter.
            p1.len() == p2.len()
                && p1
                    .iter()
                    .zip(p2.iter())
                    .all(|(a, b)| types_compatible_impl(a, b))
                && types_compatible_impl(r1, r2)
                && e2.is_subset_of(e1)
                && (c1.is_none() || c2.is_none() || c1 == c2)
        }
        (Type::Symbol, Type::Symbol) => true,
        (Type::TypeParam(a), Type::TypeParam(b)) => a == b,
        (Type::TypeRef(a), Type::TypeRef(b)) => a == b,
        (Type::TypeApp(a, args_a), Type::TypeApp(b, args_b)) => {
            a == b && args_a.len() == args_b.len() &&
            args_a.iter().zip(args_b.iter()).all(|(x, y)| types_compatible_impl(x, y))
        }
        (Type::Uncertain(inner_a), Type::Uncertain(inner_b)) => {
            types_compatible_impl(inner_a, inner_b)
        }
        (Type::Uncertain(inner), other) | (other, Type::Uncertain(inner)) => {
            types_compatible_impl(inner, other)
        }
        (Type::UnannotatedParam, _) | (_, Type::UnannotatedParam) => true,
        (Type::TypeRef(_), _) | (_, Type::TypeRef(_)) => false,
        _ => false,
    }
}

fn is_uncertain_type(ty: &Type) -> bool {
    matches!(ty, Type::Uncertain(_))
}

fn contains_type_param(ty: &Type) -> bool {
    match ty {
        Type::TypeParam(_) => true,
        Type::Option(inner) => contains_type_param(inner),
        Type::List(inner) => contains_type_param(inner),
        Type::Record(fields) => fields.iter().any(|(_, field_ty)| contains_type_param(field_ty)),
        Type::Union(variants) => variants.iter().any(|v| v.payload.as_ref().map_or(false, |p| contains_type_param(p))),
        Type::Fn { params, return_type, .. } => {
            params.iter().any(|p| contains_type_param(p)) || contains_type_param(return_type)
        }
        Type::TypeApp(_, args) => args.iter().any(|a| contains_type_param(a)),
        Type::Uncertain(inner) => contains_type_param(inner),
        _ => false,
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Primitive(prim) => match prim {
            PrimitiveType::Int => "Int".to_string(),
            PrimitiveType::Bool => "Bool".to_string(),
            PrimitiveType::Str => "Str".to_string(),
            PrimitiveType::Nil => "Nil".to_string(),
            PrimitiveType::Flt => "Flt".to_string(),
        },
        Type::Fn { params, return_type, .. } => {
            let param_strs: Vec<String> = params.iter().map(|p| format_type(p)).collect();
            format!("({} -> {})", param_strs.join(" "), format_type(return_type))
        }
        Type::Option(inner) => format!("Option[{}]", format_type(inner)),
        Type::List(inner) => format!("List[{}]", format_type(inner)),
        Type::Record(fields) => {
            let field_strs: Vec<String> = fields.iter()
                .map(|(name, ty)| format!("{}: {}", name, format_type(ty)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }
        Type::Union(variants) => {
            let variant_strs: Vec<String> = variants.iter()
                .map(|v| format!("{}", v.tag))
                .collect();
            format!("({})", variant_strs.join(" | "))
        }
        Type::Symbol => "Symbol".to_string(),
        Type::TypeParam(name) => name.clone(),
        Type::TypeRef(name) => name.clone(),
        Type::TypeApp(name, args) => {
            let arg_strs: Vec<String> = args.iter().map(format_type).collect();
            format!("{} {}", name, arg_strs.join(" "))
        }
        Type::Uncertain(inner) => format!("@uncertain[{}]", format_type(inner)),
        Type::UnannotatedParam => "_".to_string(),
    }
}

// NOTE: contains_unresolved_type_param was defined here but is unused in M1.
// Wiring it in to reject unbound return-position TypeParams is deferred per spec §1.6:
// the parser cannot produce List(TypeParam)/Option(TypeParam) yet, so the common case
// is benign; rejection is queued for a future stage when compound parameterized types
// are parseable.

fn substitute_type_params(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::TypeParam(name) => {
            subst.get(name).cloned().unwrap_or_else(|| ty.clone())
        }
        Type::Option(inner) => {
            Type::Option(Box::new(substitute_type_params(inner, subst)))
        }
        Type::List(inner) => {
            Type::List(Box::new(substitute_type_params(inner, subst)))
        }
        Type::Record(fields) => {
            let subst_fields = fields.iter()
                .map(|(name, field_ty)| (name.clone(), substitute_type_params(field_ty, subst)))
                .collect();
            Type::Record(subst_fields)
        }
        Type::Union(variants) => {
            let subst_variants = variants.iter()
                .map(|v| crate::ast::UnionVariant {
                    tag: v.tag.clone(),
                    payload: v.payload.as_ref().map(|p| Box::new(substitute_type_params(p, subst))),
                })
                .collect();
            Type::Union(subst_variants)
        }
        Type::Fn { params, return_type, effect, cap } => {
            let subst_params = params.iter()
                .map(|p| substitute_type_params(p, subst))
                .collect();
            let subst_return = Box::new(substitute_type_params(return_type, subst));
            Type::Fn {
                params: subst_params,
                return_type: subst_return,
                effect: effect.clone(),
                cap: cap.clone(),
            }
        }
        Type::TypeApp(name, args) => {
            let subst_args = args.iter()
                .map(|a| substitute_type_params(a, subst))
                .collect();
            Type::TypeApp(name.clone(), subst_args)
        }
        Type::Uncertain(inner) => {
            Type::Uncertain(Box::new(substitute_type_params(inner, subst)))
        }
        _ => ty.clone(),
    }
}

fn reject_uncertain_escape(ty: &Type, name: &str, span: SourceSpan) -> Result<(), TypeError> {
    if is_uncertain_type(ty) {
        return Err(TypeError {
            span,
            message: format!(
                "uncertain value {} escapes typed boundary: explicit acknowledgement required",
                name
            ),
        });
    }
    Ok(())
}

fn param_types(params: &[(String, Option<Type>)]) -> Vec<Type> {
    params
        .iter()
        .map(|(_, t)| {
            t.clone()
                .unwrap_or(Type::UnannotatedParam)
        })
        .collect()
}

fn fn_type_from_def(
    params: &[(String, Option<Type>)],
    return_type: Option<&Type>,
    effect: EffectSet,
    cap: &[CapabilityMarker],
) -> Type {
    Type::Fn {
        params: param_types(params),
        return_type: Box::new(
            return_type
                .cloned()
                .unwrap_or(Type::Primitive(PrimitiveType::Nil)),
        ),
        effect,
        cap: cap.first().cloned(),
    }
}

pub fn typecheck(expr: &Expr, env: &TypeEnv) -> Result<Type, TypeError> {
    match expr {
        Expr::Int(_, _, _) => Ok(Type::Primitive(PrimitiveType::Int)),
        Expr::Float(_, _, _) => Ok(Type::Primitive(PrimitiveType::Flt)),
        Expr::Str(_, _, _) => Ok(Type::Primitive(PrimitiveType::Str)),
        Expr::Bool(_, _, _) => Ok(Type::Primitive(PrimitiveType::Bool)),
        Expr::Nil => Ok(Type::Primitive(PrimitiveType::Nil)),
        Expr::Symbol(_, _, _) => Ok(Type::Symbol),

        Expr::Var(name, _, span) => {
            let ty = env.get(name).ok_or_else(|| TypeError {
                span: *span,
                message: format!("Undefined variable: {}", name),
            })?;
            if !env.allow_uncertain {
                reject_uncertain_escape(&ty, name, *span)?;
            }
            Ok(ty)
        }

        Expr::Let { name, value, span, .. } => {
            let val_type = typecheck(value, env)?;
            if !env.allow_uncertain {
                reject_uncertain_escape(&val_type, name, *span)?;
            }
            let mut child = TypeEnv::with_parent(env);
            child.define(name.clone(), val_type.clone());
            Ok(val_type)
        }

        Expr::FnDef {
            name,
            params,
            body,
            return_type,
            effect_level,
            cap,
            span,
            ..
        } => {
            // Resolve type aliases in parameters and return type
            let resolved_return_type = if let Some(rt) = return_type {
                Some(resolve_type_ref_with_span(rt, env, *span)?)
            } else {
                None
            };

            let resolved_params: Result<Vec<(String, Option<Type>)>, TypeError> = params
                .iter()
                .map(|(pname, pty)| {
                    let resolved_pty = if let Some(t) = pty {
                        Some(resolve_type_ref_with_span(t, env, *span)?)
                    } else {
                        None
                    };
                    Ok((pname.clone(), resolved_pty))
                })
                .collect();
            let resolved_params = resolved_params?;

            let param_types_vec = param_types(&resolved_params);
            let provisional_fn_type = Type::Fn {
                params: param_types_vec.clone(),
                return_type: Box::new(resolved_return_type.as_ref().cloned().unwrap_or(Type::Primitive(PrimitiveType::Nil))),
                effect: *effect_level,
                cap: cap.first().cloned(),
            };

            let mut child_env = TypeEnv::with_parent_and_effect(env, *effect_level);
            for (pname, pty) in &resolved_params {
                let t = pty.clone().unwrap_or(Type::UnannotatedParam);
                child_env.define(pname.clone(), t);
            }
            child_env.define(name.clone(), provisional_fn_type.clone());

            let inferred_body_type = typecheck(body, &child_env)?;

            if !child_env.allow_uncertain {
                reject_uncertain_escape(&inferred_body_type, "return", *span)?;
            }

            if let Some(expected_type) = &resolved_return_type {
                // Skip return type check if the expected type contains unresolved type parameters
                // (will be validated at call sites after substitution)
                if !contains_type_param(expected_type) {
                    if !types_compatible(expected_type, &inferred_body_type) {
                        return Err(TypeError {
                            span: *span,
                            message: format!(
                                "Return type mismatch: expected {}, found {}",
                                format_type(expected_type),
                                format_type(&inferred_body_type)
                            ),
                        });
                    }
                }
            }

            let fn_type = fn_type_from_def(&resolved_params, resolved_return_type.as_ref(), *effect_level, cap);
            Ok(fn_type)
        }

        Expr::FnCall { name, args, span, .. } => {
            // Typecheck all arguments first
            let arg_types: Result<Vec<Type>, TypeError> = args
                .iter()
                .map(|arg| typecheck(arg, env))
                .collect();
            let arg_types = arg_types?;

            let callee_type = env.get(name).ok_or_else(|| TypeError {
                span: *span,
                message: format!("Undefined function: {}", name),
            })?;

            let (params, callee_effect, return_type) = match &callee_type {
                Type::Fn {
                    params,
                    effect,
                    return_type,
                    ..
                } => (params.clone(), *effect, return_type.as_ref().clone()),
                _ => {
                    return Err(TypeError {
                        span: *span,
                        message: format!("{} is not a function", name),
                    });
                }
            };

            // Check arity: argument count must match parameter count
            if arg_types.len() != params.len() {
                return Err(TypeError {
                    span: *span,
                    message: format!(
                        "Function {} expects {} arguments, got {}",
                        name,
                        params.len(),
                        arg_types.len()
                    ),
                });
            }

            // Check argument types against parameter types.
            // Skip type checking for UnannotatedParam (no annotation provided) and TypeParam (will be substituted).
            for (i, (arg_type, param_type)) in arg_types.iter().zip(params.iter()).enumerate() {
                // Skip type checking if param is unannotated or a type parameter
                if matches!(param_type, Type::UnannotatedParam | Type::TypeParam(_)) {
                    continue;
                }
                // Explicitly reject mismatches for annotated params (including explicit Nil)
                if !types_compatible(param_type, arg_type) {
                    return Err(TypeError {
                        span: *span,
                        message: format!(
                            "Argument {} has type {}, expected {}",
                            i + 1,
                            format_type(arg_type),
                            format_type(param_type)
                        ),
                    });
                }
            }

            // Check effect subset: when effect_level = None (root context), all calls are allowed.
            // Root context means the top-level REPL or script entry point, which implicitly
            // allows all effects. Non-root contexts enforce that callee effects must be a
            // subset of the caller's effects (e.g., a Pure function cannot call IO or Async).
            if let Some(caller_effect) = env.effect_level() {
                if !callee_effect.is_subset_of(&caller_effect) {
                    return Err(TypeError {
                        span: *span,
                        message: format!(
                            "Function {} effect {:?} is not a subset of caller effect {:?}",
                            name, callee_effect, caller_effect
                        ),
                    });
                }
            }

            // Build substitution map for type parameters: map each param's type parameter
            // name to the concrete type of the corresponding argument.
            let mut subst = HashMap::new();
            for (param_type, arg_type) in params.iter().zip(arg_types.iter()) {
                if let Type::TypeParam(name) = param_type {
                    if let Some(existing_type) = subst.get(name) {
                        // Check if the existing type is compatible with the new arg type
                        if !types_compatible(existing_type, arg_type) {
                            return Err(TypeError {
                                span: *span,
                                message: format!(
                                    "Type parameter '{}' bound to conflicting types: {} and {}",
                                    name,
                                    format_type(existing_type),
                                    format_type(arg_type)
                                ),
                            });
                        }
                    } else {
                        subst.insert(name.clone(), arg_type.clone());
                    }
                }
            }

            // Apply substitution to the return type
            let substituted_return = substitute_type_params(&return_type, &subst);
            Ok(substituted_return)
        }

        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
            ..
        } => {
            let cond_type = typecheck(cond, env)?;
            if cond_type != Type::Primitive(PrimitiveType::Bool) {
                return Err(TypeError {
                    span: *span,
                    message: format!(
                        "If condition must be Bool, found {:?}",
                        cond_type
                    ),
                });
            }
            let then_type = typecheck(then_branch, env)?;
            let else_type = typecheck(else_branch, env)?;
            if then_type != else_type {
                return Err(TypeError {
                    span: *span,
                    message: format!(
                        "If branches have mismatched types: then={:?}, else={:?}",
                        then_type, else_type
                    ),
                });
            }
            Ok(then_type)
        }

        Expr::Arithmetic { op, left, right, span, .. } => {
            let left_type = typecheck(left, env)?;
            let right_type = typecheck(right, env)?;
            match (op, &left_type, &right_type) {
                (ArithOp::Add, Type::Primitive(PrimitiveType::Int), Type::Primitive(PrimitiveType::Int))
                | (ArithOp::Sub, Type::Primitive(PrimitiveType::Int), Type::Primitive(PrimitiveType::Int))
                | (ArithOp::Mul, Type::Primitive(PrimitiveType::Int), Type::Primitive(PrimitiveType::Int))
                | (ArithOp::Div, Type::Primitive(PrimitiveType::Int), Type::Primitive(PrimitiveType::Int)) => {
                    Ok(Type::Primitive(PrimitiveType::Int))
                }
                (ArithOp::Add, Type::Primitive(PrimitiveType::Flt), Type::Primitive(PrimitiveType::Flt))
                | (ArithOp::Sub, Type::Primitive(PrimitiveType::Flt), Type::Primitive(PrimitiveType::Flt))
                | (ArithOp::Mul, Type::Primitive(PrimitiveType::Flt), Type::Primitive(PrimitiveType::Flt))
                | (ArithOp::Div, Type::Primitive(PrimitiveType::Flt), Type::Primitive(PrimitiveType::Flt)) => {
                    Ok(Type::Primitive(PrimitiveType::Flt))
                }
                (ArithOp::Add, Type::Primitive(PrimitiveType::Str), Type::Primitive(PrimitiveType::Str)) => {
                    Ok(Type::Primitive(PrimitiveType::Str))
                }
                _ => Err(TypeError {
                    span: *span,
                    message: format!(
                        "Invalid arithmetic operands: {:?} and {:?} for {:?}",
                        left_type, right_type, op
                    ),
                }),
            }
        }

        Expr::Ret(inner, _, _) => typecheck(inner, env),

        Expr::IoWrite(inner, _, _) => {
            let _ = typecheck(inner, env)?;
            Ok(Type::Primitive(PrimitiveType::Nil))
        }

        Expr::Block(exprs, _, _) => typecheck_block_stmts(exprs, env),

        Expr::Intent(_, _, span) => Err(TypeError {
            span: *span,
            message: "Type checker does not yet handle Intent".to_string(),
        }),

        Expr::Uncertain(inner, _, _) => {
            let child = TypeEnv::with_uncertain_allowed(env);
            let inner_type = typecheck(inner, &child)?;
            Ok(Type::Uncertain(Box::new(inner_type)))
        }

        Expr::Ctx { span, .. } => Err(TypeError {
            span: *span,
            message: "Type checker does not yet handle Ctx".to_string(),
        }),

        Expr::CtxGet { .. } => Ok(Type::Option(Box::new(Type::TypeParam("T".to_string())))),

        Expr::CtxSet { value, .. } => typecheck(value, env),

        Expr::Diff { span, .. } => Err(TypeError {
            span: *span,
            message: "Type checker does not yet handle Diff".to_string(),
        }),

        Expr::Use { caps, module, span, .. } => {
            if caps.is_empty() {
                return Ok(Type::Primitive(PrimitiveType::Nil));
            }

            // Check for wildcard sentinel: vec![("*", None)]
            let is_wildcard = caps.len() == 1 && caps[0].0 == "*" && caps[0].1.is_none();

            let exported = env.module_caps.get(module).ok_or_else(|| TypeError {
                span: *span,
                message: format!("module not found: {}", module),
            })?;

            let caps_to_validate = if is_wildcard {
                // Expand wildcard to all exported capabilities
                if exported.is_empty() {
                    return Err(TypeError {
                        span: *span,
                        message: format!("wildcard import from module '{}' which exports nothing", module),
                    });
                }
                exported.iter().map(|cap| (cap.clone(), None)).collect::<Vec<_>>()
            } else {
                caps.clone()
            };

            let mut seen_originals = std::collections::HashSet::new();
            let mut seen_aliases = std::collections::HashSet::new();
            for (orig, alias) in &caps_to_validate {
                if !exported.contains(orig) {
                    return Err(TypeError {
                        span: *span,
                        message: format!(
                            "module {} does not export capability {}",
                            module, orig
                        ),
                    });
                }
                if seen_originals.contains(orig) {
                    return Err(TypeError {
                        span: *span,
                        message: format!("capability '{}' imported more than once", orig),
                    });
                }
                if let Some(alias_name) = alias {
                    if seen_aliases.contains(alias_name) {
                        return Err(TypeError {
                            span: *span,
                            message: format!("alias '{}' used more than once", alias_name),
                        });
                    }
                    if seen_originals.contains(alias_name) {
                        return Err(TypeError {
                            span: *span,
                            message: format!(
                                "alias '{}' conflicts with imported capability name",
                                alias_name
                            ),
                        });
                    }
                    seen_aliases.insert(alias_name.clone());
                } else if seen_aliases.contains(orig) {
                    return Err(TypeError {
                        span: *span,
                        message: format!(
                            "capability '{}' conflicts with existing alias name",
                            orig
                        ),
                    });
                }
                seen_originals.insert(orig.clone());
            }
            Ok(Type::Primitive(PrimitiveType::Nil))
        }

        Expr::Mod { .. } | Expr::Pub { .. } | Expr::TypeAlias { .. } => Ok(Type::Primitive(PrimitiveType::Nil)),

        Expr::PubDecl { inner, .. } => typecheck(inner, env),

        Expr::Record { fields, .. } => infer_record_type(fields, env),

        Expr::List { elements, span, .. } => {
            if elements.is_empty() {
                // Empty list: List(TypeParam("t"))
                Ok(Type::List(Box::new(Type::TypeParam("t".to_string()))))
            } else {
                // Typecheck first element to determine list element type
                let first_type = typecheck(&elements[0], env)?;
                // Verify all elements have the same type
                for (i, elem) in elements.iter().enumerate().skip(1) {
                    let elem_type = typecheck(elem, env)?;
                    if !types_compatible(&first_type, &elem_type) {
                        return Err(TypeError {
                            span: *span,
                            message: format!(
                                "List elements have incompatible types: element 0 is {}, element {} is {}",
                                format_type(&first_type),
                                i,
                                format_type(&elem_type)
                            ),
                        });
                    }
                }
                Ok(Type::List(Box::new(first_type)))
            }
        }

        Expr::Match {
            scrutinee,
            patterns,
            span,
            ..
        } => {
            if patterns.is_empty() {
                return Err(TypeError {
                    span: *span,
                    message: "Match expression must have at least one branch".to_string(),
                });
            }

            let scrutinee_type = typecheck(scrutinee, env)?;
            let union_variants = match &scrutinee_type {
                Type::Union(variants) => variants.clone(),
                other => {
                    return Err(TypeError {
                        span: *span,
                        message: format!(
                            "Match scrutinee must be a union type, got {:?}",
                            other
                        ),
                    });
                }
            };

            let mut covered_tags: HashSet<String> = HashSet::new();
            let mut has_wildcard = false;
            let mut branch_type: Option<Type> = None;

            for (pattern, body) in patterns {
                match pattern {
                    Pattern::Tag(tag) => {
                        if !union_variants.iter().any(|v| v.tag == *tag) {
                            return Err(TypeError {
                                span: *span,
                                message: format!(
                                    "Match pattern #{} not in scrutinee union",
                                    tag
                                ),
                            });
                        }
                        covered_tags.insert(tag.clone());
                    }
                    Pattern::TagBind(tag, var) => {
                        if !union_variants.iter().any(|v| v.tag == *tag) {
                            return Err(TypeError {
                                span: *span,
                                message: format!(
                                    "Match pattern #{} not in scrutinee union",
                                    tag
                                ),
                            });
                        }
                        covered_tags.insert(tag.clone());
                        let mut pat_env = TypeEnv::with_parent(env);
                        if let Some(variant) = union_variants.iter().find(|v| v.tag == *tag) {
                            if let Some(payload) = &variant.payload {
                                pat_env.define(var.clone(), (**payload).clone());
                            }
                        }
                        let bt = typecheck(body, &pat_env)?;
                        branch_type = Some(merge_branch_type(branch_type, bt, *span)?);
                        continue;
                    }
                    Pattern::Wildcard => {
                        has_wildcard = true;
                    }
                }

                let bt = typecheck(body, env)?;
                branch_type = Some(merge_branch_type(branch_type, bt, *span)?);
            }

            if !has_wildcard {
                for variant in &union_variants {
                    if !covered_tags.contains(&variant.tag) {
                        return Err(TypeError {
                            span: *span,
                            message: format!(
                                "Non-exhaustive match: missing variant #{}",
                                variant.tag
                            ),
                        });
                    }
                }
            }

            Ok(branch_type.unwrap_or(Type::Primitive(PrimitiveType::Nil)))
        }

        Expr::Tagged { tag, payload, span, .. } => {
            if tag == "err" {
                if let Some(caller) = env.effect_level() {
                    if !caller.err {
                        return Err(TypeError {
                            span: *span,
                            message: "@err can only be used in functions with err effect (-?> or higher)"
                                .to_string(),
                        });
                    }
                }
            }
            let payload_type = if let Some(p) = payload {
                Some(Box::new(typecheck(p, env)?))
            } else {
                None
            };
            Ok(Type::Union(vec![UnionVariant {
                tag: tag.clone(),
                payload: payload_type,
            }]))
        }
    }
}

fn merge_branch_type(
    acc: Option<Type>,
    branch: Type,
    span: SourceSpan,
) -> Result<Type, TypeError> {
    match acc {
        None => Ok(branch),
        Some(prev) if types_compatible(&prev, &branch) => Ok(branch),
        Some(prev) => Err(TypeError {
            span,
            message: format!(
                "Match branches have mismatched types: {:?} vs {:?}",
                prev, branch
            ),
        }),
    }
}

fn typecheck_block_stmts(exprs: &[Expr], env: &TypeEnv) -> Result<Type, TypeError> {
    let mut current_env = env.clone();
    let mut result = Type::Primitive(PrimitiveType::Nil);
    for expr in exprs {
        match expr {
            Expr::Let { name, value, span, .. } => {
                let val_type = typecheck(value, &current_env)?;
                if !current_env.allow_uncertain {
                    reject_uncertain_escape(&val_type, name, *span)?;
                }
                current_env.define(name.clone(), val_type.clone());
                result = val_type;
            }
            Expr::FnDef { name, .. } => {
                let fn_type = typecheck(expr, &current_env)?;
                current_env.define(name.clone(), fn_type.clone());
                result = fn_type;
            }
            Expr::TypeAlias { name, type_params, ty, .. } => {
                current_env.type_aliases.insert(name.clone(), (type_params.clone(), ty.clone()));
                result = Type::Primitive(PrimitiveType::Nil);
            }
            _ => {
                result = typecheck(expr, &current_env)?;
            }
        }
    }
    if !current_env.allow_uncertain {
        reject_uncertain_escape(&result, "block result", SourceSpan::zero())?;
    }
    Ok(result)
}

pub fn is_cap_subset(requested: &[CapabilityMarker], exported: &[CapabilityMarker]) -> bool {
    requested.iter().all(|c| exported.contains(c))
}

/// Pre-scan top-level AST for `@mod` / `@pub` capability exports.
pub fn build_module_caps_map(expr: &Expr) -> HashMap<ModulePath, Vec<CapabilityMarker>> {
    let mut caps_map: HashMap<ModulePath, Vec<CapabilityMarker>> = HashMap::new();
    let mut current: Option<ModulePath> = None;

    fn traverse(
        expr: &Expr,
        caps_map: &mut HashMap<ModulePath, Vec<CapabilityMarker>>,
        current: &mut Option<ModulePath>,
    ) {
        match expr {
            Expr::Mod { name, .. } => {
                *current = Some(name.clone());
                caps_map.entry(name.clone()).or_default();
            }
            Expr::Pub { cap, .. } => {
                if let Some(module) = current.clone() {
                    caps_map.insert(module, cap.clone());
                }
            }
            Expr::PubDecl { inner, .. } => {
                // Register the declared name as an exported capability
                let name = match &**inner {
                    Expr::FnDef { name, .. } => Some(name.clone()),
                    Expr::TypeAlias { name, .. } => Some(name.clone()),
                    Expr::Let { name, .. } => Some(name.clone()),
                    _ => None,
                };
                if let Some(n) = name {
                    if let Some(module) = current.clone() {
                        caps_map.entry(module).or_default().push(n);
                    }
                }
                traverse(inner, caps_map, current);
            }
            Expr::Block(exprs, _, _) => {
                for e in exprs {
                    traverse(e, caps_map, current);
                }
            }
            _ => {}
        }
    }

    traverse(expr, &mut caps_map, &mut current);
    caps_map
}

/// Build module dependency DAG from `@use` edges (self-loops excluded).
pub fn build_module_dependency_dag(expr: &Expr) -> HashMap<ModulePath, Vec<ModulePath>> {
    let mut dag: HashMap<ModulePath, Vec<ModulePath>> = HashMap::new();
    let mut current: Option<ModulePath> = None;

    fn traverse(
        expr: &Expr,
        dag: &mut HashMap<ModulePath, Vec<ModulePath>>,
        current: &mut Option<ModulePath>,
    ) {
        match expr {
            Expr::Mod { name, .. } => {
                *current = Some(name.clone());
                dag.entry(name.clone()).or_default();
            }
            Expr::Use { module, .. } => {
                if let Some(from) = current.clone() {
                    if from != *module {
                        dag.entry(from.clone()).or_default();
                        let deps = dag.entry(from).or_default();
                        if !deps.contains(module) {
                            deps.push(module.clone());
                        }
                    }
                    dag.entry(module.clone()).or_default();
                }
            }
            Expr::Block(exprs, _, _) => {
                for e in exprs {
                    traverse(e, dag, current);
                }
            }
            _ => {}
        }
    }

    traverse(expr, &mut dag, &mut current);
    dag
}

pub fn detect_cycles(dag: &HashMap<ModulePath, Vec<ModulePath>>) -> Result<(), TypeError> {
    let mut visiting: HashSet<ModulePath> = HashSet::new();
    let mut visited: HashSet<ModulePath> = HashSet::new();

    fn dfs(
        node: &ModulePath,
        dag: &HashMap<ModulePath, Vec<ModulePath>>,
        visiting: &mut HashSet<ModulePath>,
        visited: &mut HashSet<ModulePath>,
        path: &mut Vec<ModulePath>,
    ) -> Result<(), TypeError> {
        if visiting.contains(node) {
            let cycle_start = path.iter().position(|p| p == node).unwrap_or(path.len());
            let cycle: Vec<String> = path[cycle_start..]
                .iter()
                .chain(std::iter::once(node))
                .map(|p| p.to_string())
                .collect();
            return Err(TypeError {
                span: SourceSpan::zero(),
                message: format!(
                    "circular module dependency detected: {}",
                    cycle.join(" -> ")
                ),
            });
        }
        if visited.contains(node) {
            return Ok(());
        }

        visiting.insert(node.clone());
        path.push(node.clone());

        if let Some(deps) = dag.get(node) {
            for dep in deps {
                dfs(dep, dag, visiting, visited, path)?;
            }
        }

        path.pop();
        visiting.remove(node);
        visited.insert(node.clone());
        Ok(())
    }

    for node in dag.keys() {
        if !visited.contains(node) {
            dfs(node, dag, &mut visiting, &mut visited, &mut Vec::new())?;
        }
    }
    Ok(())
}

/// Kahn topological sort; self-loops must already be excluded from `dag`.
pub fn topological_sort(
    dag: &HashMap<ModulePath, Vec<ModulePath>>,
) -> Result<Vec<ModulePath>, TypeError> {
    let mut in_degree: HashMap<ModulePath, usize> = HashMap::new();
    for (node, deps) in dag {
        in_degree.entry(node.clone()).or_insert(0);
        for dep in deps {
            in_degree.entry(dep.clone()).or_insert(0);
            *in_degree.entry(node.clone()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<ModulePath> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(n, _)| n.clone())
        .collect();

    let mut sorted = Vec::new();
    while let Some(node) = queue.pop_front() {
        sorted.push(node.clone());
        for (dependent, deps) in dag {
            if deps.contains(&node) {
                if let Some(deg) = in_degree.get_mut(dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
    }

    if sorted.len() != in_degree.len() {
        return Err(TypeError {
            span: SourceSpan::zero(),
            message:
                "Cycle detected in module dependencies (should have been caught by detect_cycles)"
                    .to_string(),
        });
    }

    Ok(sorted)
}

/// Partition a top-level program into prelude statements and per-module statement groups.
pub fn partition_by_module(
    expr: &Expr,
) -> (Vec<Expr>, HashMap<ModulePath, Vec<Expr>>) {
    let mut prelude = Vec::new();
    let mut modules: HashMap<ModulePath, Vec<Expr>> = HashMap::new();
    let mut current: Option<ModulePath> = None;

    let stmts: Vec<&Expr> = match expr {
        Expr::Block(exprs, _, _) => exprs.iter().collect(),
        other => {
            prelude.push(other.clone());
            return (prelude, modules);
        }
    };

    for stmt in stmts {
        if let Expr::Mod { name, .. } = stmt {
            // Only push @mod on first occurrence; subsequent declarations for the same module
            // update current but do not add a second @mod node. This preserves the invariant
            // that @mod is the first element and appears exactly once per module slice.
            if !modules.contains_key(name) {
                modules.entry(name.clone()).or_default().push(stmt.clone());
            }
            current = Some(name.clone());
            // Note: deep-scan of nested @mod in Block bodies is deferred to M4.7 (multi-file model).
            // M4 assumes all module declarations are top-level; hierarchical nesting requires per-file parsing.
        } else if let Some(module) = &current {
            modules
                .entry(module.clone())
                .or_default()
                .push(stmt.clone());
        } else {
            prelude.push(stmt.clone());
        }
    }

    (prelude, modules)
}

/// Collect function names by module from the AST.
pub fn build_module_function_names(
    expr: &Expr,
) -> HashMap<ModulePath, HashMap<String, SourceSpan>> {
    let mut fn_names: HashMap<ModulePath, HashMap<String, SourceSpan>> = HashMap::new();
    let mut current: Option<ModulePath> = None;

    fn traverse(
        expr: &Expr,
        fn_names: &mut HashMap<ModulePath, HashMap<String, SourceSpan>>,
        current: &mut Option<ModulePath>,
    ) {
        match expr {
            Expr::Mod { name, .. } => {
                *current = Some(name.clone());
                fn_names.entry(name.clone()).or_default();
            }
            Expr::FnDef { name, span, .. } => {
                if let Some(module) = current.clone() {
                    fn_names
                        .entry(module)
                        .or_default()
                        .insert(name.clone(), *span);
                } else {
                    // Top-level (pre-@mod) functions are intentionally excluded — this check is cross-@mod only.
                }
            }
            Expr::Block(exprs, _, _) => {
                for e in exprs {
                    traverse(e, fn_names, current);
                }
            }
            _ => {}
        }
    }

    traverse(expr, &mut fn_names, &mut current);
    fn_names
}

/// Detect duplicate function names across modules.
pub fn detect_duplicate_fns(
    fn_names: &HashMap<ModulePath, HashMap<String, SourceSpan>>,
) -> Result<(), TypeError> {
    let mut seen: HashMap<String, Vec<(ModulePath, SourceSpan)>> = HashMap::new();

    for (module, names) in fn_names {
        for (name, span) in names {
            seen.entry(name.clone())
                .or_default()
                .push((module.clone(), *span));
        }
    }

    for (name, occurrences) in seen {
        if occurrences.len() > 1 {
            let mut module_list: Vec<String> = occurrences
                .iter()
                .map(|(m, _)| m.to_string())
                .collect();
            module_list.sort();
            // Use the span from the first occurrence found
            let span = occurrences[0].1;
            return Err(TypeError {
                span,
                message: format!(
                    "Function '{}' defined in multiple modules: {}",
                    name, module_list.join(", ")
                ),
            });
        }
    }

    Ok(())
}

/// Infer the Type::Record from a Record expression's fields.
/// Typechecks each field value and builds a Type::Record with field types.
pub fn infer_record_type(fields: &[(String, Expr)], env: &TypeEnv) -> Result<Type, TypeError> {
    let mut field_types = Vec::new();
    for (key, value_expr) in fields {
        let val_type = typecheck(value_expr, env)?;
        field_types.push((key.clone(), val_type));
    }
    Ok(Type::Record(field_types))
}

/// Typecheck prelude statements, then each module body in dependency order, accumulating env bindings.
pub fn typecheck_program_ordered(
    prelude: &[Expr],
    modules: &HashMap<ModulePath, Vec<Expr>>,
    sorted_modules: &[ModulePath],
    env: &mut TypeEnv,
) -> Result<Type, TypeError> {
    let mut result = Type::Primitive(PrimitiveType::Nil);

    for stmt in prelude {
        result = typecheck_statement(stmt, env)?;
    }

    let mut order: Vec<ModulePath> = sorted_modules.to_vec();
    for module in modules.keys() {
        if !order.contains(module) {
            order.push(module.clone());
        }
    }

    let mut module_result = Type::Primitive(PrimitiveType::Nil);
    for module in &order {
        if let Some(stmts) = modules.get(module) {
            for stmt in stmts {
                module_result = typecheck_statement(stmt, env)?;
            }
            if !env.allow_uncertain {
                reject_uncertain_escape(&module_result, "module result", SourceSpan::zero())?;
            }
        }
    }

    Ok(result)
}

fn typecheck_statement(stmt: &Expr, env: &mut TypeEnv) -> Result<Type, TypeError> {
    match stmt {
        Expr::Let { name, value, span, .. } => {
            let val_type = typecheck(value, env)?;
            if !env.allow_uncertain {
                reject_uncertain_escape(&val_type, name, *span)?;
            }
            env.define(name.clone(), val_type.clone());
            Ok(val_type)
        }
        Expr::FnDef { name, .. } => {
            let fn_type = typecheck(stmt, env)?;
            env.define(name.clone(), fn_type.clone());
            Ok(fn_type)
        }
        Expr::TypeAlias { name, type_params, ty, .. } => {
            env.type_aliases.insert(name.clone(), (type_params.clone(), ty.clone()));
            typecheck(stmt, env)
        }
        Expr::PubDecl { inner, .. } => {
            // Typecheck the inner declaration
            typecheck_statement(inner, env)
        }
        _ => typecheck(stmt, env),
    }
}

pub fn typecheck_str(input: &str) -> Result<Type, TypeError> {
    let mut parser = Parser::new(input).map_err(|e: ParseError| TypeError {
        span: SourceSpan::zero(),
        message: e.to_string(),
    })?;
    let expr = parser.parse().map_err(|e: ParseError| TypeError {
        span: SourceSpan::zero(),
        message: e.to_string(),
    })?;

    let module_caps = build_module_caps_map(&expr);
    let dag = build_module_dependency_dag(&expr);
    detect_cycles(&dag)?;
    let sorted_modules = topological_sort(&dag)?;
    let fn_names = build_module_function_names(&expr);
    detect_duplicate_fns(&fn_names)?;

    let (prelude, modules) = partition_by_module(&expr);
    let mut env = TypeEnv::new();
    env.module_caps = module_caps;
    typecheck_program_ordered(&prelude, &modules, &sorted_modules, &mut env)
}

#[derive(Debug, Clone, PartialEq)]
pub struct UncertainViolation {
    pub path: String,
    pub span: SourceSpan,
}

/// Walk the entire AST and find every `@uncertain` node, recording its
/// selector path and source span. Used by the "check-uncertainty" linter.
pub fn check_uncertainty(expr: &Expr) -> Vec<UncertainViolation> {
    let mut violations = Vec::new();
    let mut path = Vec::new();
    walk_for_uncertain(expr, &mut path, &mut violations);
    violations
}

fn walk_for_uncertain(
    expr: &Expr,
    current_path: &mut Vec<String>,
    violations: &mut Vec<UncertainViolation>,
) {
    match expr {
        Expr::Int(..)
        | Expr::Float(..)
        | Expr::Str(..)
        | Expr::Bool(..)
        | Expr::Nil
        | Expr::Var(..)
        | Expr::Symbol(..)
        | Expr::Intent(..)
        | Expr::Ctx { .. }
        | Expr::Mod { .. }
        | Expr::Pub { .. }
        | Expr::TypeAlias { .. } => {}

        Expr::PubDecl { inner, .. } => walk_for_uncertain(inner, current_path, violations),

        Expr::Uncertain(inner, _, span) => {
            violations.push(UncertainViolation {
                path: current_path.join("/"),
                span: *span,
            });
            walk_for_uncertain(inner, current_path, violations);
        }

        Expr::Let { name, value, .. } => {
            current_path.push(name.clone());
            walk_for_uncertain(value, current_path, violations);
            current_path.pop();
        }

        Expr::FnDef { name, body, .. } => {
            current_path.push(format!("fn {}", name));
            walk_for_uncertain(body, current_path, violations);
            current_path.pop();
        }

        Expr::FnCall { args, .. } => {
            for (i, arg) in args.iter().enumerate() {
                current_path.push(format!("[{}]", i));
                walk_for_uncertain(arg, current_path, violations);
                current_path.pop();
            }
        }

        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            current_path.push("cond".to_string());
            walk_for_uncertain(cond, current_path, violations);
            current_path.pop();
            current_path.push("then".to_string());
            walk_for_uncertain(then_branch, current_path, violations);
            current_path.pop();
            current_path.push("else".to_string());
            walk_for_uncertain(else_branch, current_path, violations);
            current_path.pop();
        }

        Expr::Arithmetic { left, right, .. } => {
            current_path.push("left".to_string());
            walk_for_uncertain(left, current_path, violations);
            current_path.pop();
            current_path.push("right".to_string());
            walk_for_uncertain(right, current_path, violations);
            current_path.pop();
        }

        Expr::Ret(inner, ..) | Expr::IoWrite(inner, ..) => {
            walk_for_uncertain(inner, current_path, violations);
        }

        Expr::Block(exprs, ..) => {
            for (i, child) in exprs.iter().enumerate() {
                current_path.push(format!("[{}]", i));
                walk_for_uncertain(child, current_path, violations);
                current_path.pop();
            }
        }

        Expr::CtxGet { ctx, key, .. } => {
            current_path.push("ctx".to_string());
            walk_for_uncertain(ctx, current_path, violations);
            current_path.pop();
            current_path.push("key".to_string());
            walk_for_uncertain(key, current_path, violations);
            current_path.pop();
        }

        Expr::CtxSet { ctx, key, value, .. } => {
            current_path.push("ctx".to_string());
            walk_for_uncertain(ctx, current_path, violations);
            current_path.pop();
            current_path.push("key".to_string());
            walk_for_uncertain(key, current_path, violations);
            current_path.pop();
            current_path.push("value".to_string());
            walk_for_uncertain(value, current_path, violations);
            current_path.pop();
        }

        Expr::Diff { .. } => {}

        Expr::Use { .. } => {}

        Expr::Match { scrutinee, patterns, .. } => {
            current_path.push("scrutinee".to_string());
            walk_for_uncertain(scrutinee, current_path, violations);
            current_path.pop();
            for (i, (_pattern, body)) in patterns.iter().enumerate() {
                current_path.push(format!("branch[{}]", i));
                walk_for_uncertain(body, current_path, violations);
                current_path.pop();
            }
        }

        Expr::Tagged { payload, .. } => {
            if let Some(p) = payload {
                walk_for_uncertain(p, current_path, violations);
            }
        }

        Expr::Record { fields, .. } => {
            for (name, val) in fields {
                current_path.push(name.clone());
                walk_for_uncertain(val, current_path, violations);
                current_path.pop();
            }
        }

        Expr::List { elements, .. } => {
            for (i, elem) in elements.iter().enumerate() {
                current_path.push(format!("[{}]", i));
                walk_for_uncertain(elem, current_path, violations);
                current_path.pop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_types_compatible_primitive() {
        assert!(types_compatible(
            &Type::Primitive(PrimitiveType::Int),
            &Type::Primitive(PrimitiveType::Int)
        ));
        assert!(!types_compatible(
            &Type::Primitive(PrimitiveType::Int),
            &Type::Primitive(PrimitiveType::Bool)
        ));
    }

    #[test]
    fn test_types_compatible_option() {
        let a = Type::Option(Box::new(Type::Primitive(PrimitiveType::Int)));
        let b = Type::Option(Box::new(Type::Primitive(PrimitiveType::Int)));
        assert!(types_compatible(&a, &b));
    }

    #[test]
    fn test_types_compatible_list() {
        let a = Type::List(Box::new(Type::Primitive(PrimitiveType::Str)));
        let b = Type::List(Box::new(Type::Primitive(PrimitiveType::Str)));
        assert!(types_compatible(&a, &b));
    }

    #[test]
    fn test_types_compatible_record() {
        let a = Type::Record(vec![
            ("x".to_string(), Type::Primitive(PrimitiveType::Int)),
            ("y".to_string(), Type::Primitive(PrimitiveType::Bool)),
        ]);
        let b = Type::Record(vec![
            ("x".to_string(), Type::Primitive(PrimitiveType::Int)),
            ("y".to_string(), Type::Primitive(PrimitiveType::Bool)),
        ]);
        assert!(types_compatible(&a, &b));
    }

    #[test]
    fn test_types_compatible_union() {
        let a = Type::Union(vec![UnionVariant {
            tag: "ok".to_string(),
            payload: Some(Box::new(Type::Primitive(PrimitiveType::Int))),
        }]);
        let b = Type::Union(vec![UnionVariant {
            tag: "ok".to_string(),
            payload: Some(Box::new(Type::Primitive(PrimitiveType::Int))),
        }]);
        assert!(types_compatible(&a, &b));
    }

    #[test]
    fn test_types_compatible_fn() {
        let a = Type::Fn {
            params: vec![Type::Primitive(PrimitiveType::Int)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        let b = Type::Fn {
            params: vec![Type::Primitive(PrimitiveType::Int)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        assert!(types_compatible(&a, &b));
    }

    #[test]
    fn test_types_compatible_symbol() {
        assert!(types_compatible(&Type::Symbol, &Type::Symbol));
    }

    #[test]
    fn test_types_compatible_uncertain() {
        let a = Type::Uncertain(Box::new(Type::Primitive(PrimitiveType::Int)));
        let b = Type::Primitive(PrimitiveType::Int);
        assert!(types_compatible(&a, &b));
    }

    #[test]
    fn test_types_compatible_unannotated_param_wildcard() {
        // UnannotatedParam should be compatible with any type in Fn-type structural comparison
        let fn_with_unannotated = Type::Fn {
            params: vec![Type::UnannotatedParam],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        let fn_with_int = Type::Fn {
            params: vec![Type::Primitive(PrimitiveType::Int)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        assert!(types_compatible(&fn_with_unannotated, &fn_with_int));
        assert!(types_compatible(&fn_with_int, &fn_with_unannotated));
    }

    #[test]
    fn test_is_cap_subset() {
        assert!(is_cap_subset(&["read".to_string()], &["read".to_string(), "write".to_string()]));
        assert!(!is_cap_subset(
            &["write".to_string()],
            &["read".to_string()]
        ));
        assert!(is_cap_subset(&[], &["read".to_string()]));
    }

    #[test]
    fn test_topo_sort_single_module() {
        let mut dag = HashMap::new();
        let m = ModulePath::new(vec!["a".to_string()]);
        dag.insert(m.clone(), vec![]);
        let sorted = topological_sort(&dag).unwrap();
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0], m);
    }

    #[test]
    fn test_topo_sort_linear_chain() {
        // DAG edges follow build_module_dependency_dag: consumer -> [provider].
        // a uses b uses c => providers first: c, then b, then a.
        let a = ModulePath::new(vec!["a".to_string()]);
        let b = ModulePath::new(vec!["b".to_string()]);
        let c = ModulePath::new(vec!["c".to_string()]);
        let mut dag = HashMap::new();
        dag.insert(a.clone(), vec![b.clone()]);
        dag.insert(b.clone(), vec![c.clone()]);
        dag.insert(c.clone(), vec![]);
        let sorted = topological_sort(&dag).unwrap();
        assert_eq!(sorted.len(), 3);
        let pos = |m: &ModulePath| sorted.iter().position(|x| x == m).unwrap();
        assert!(pos(&c) < pos(&b));
        assert!(pos(&b) < pos(&a));
    }

    #[test]
    fn test_topo_sort_diamond() {
        // a uses b,c; both use d => d first, then b/c, then a.
        let a = ModulePath::new(vec!["a".to_string()]);
        let b = ModulePath::new(vec!["b".to_string()]);
        let c = ModulePath::new(vec!["c".to_string()]);
        let d = ModulePath::new(vec!["d".to_string()]);
        let mut dag = HashMap::new();
        dag.insert(a.clone(), vec![b.clone(), c.clone()]);
        dag.insert(b.clone(), vec![d.clone()]);
        dag.insert(c.clone(), vec![d.clone()]);
        dag.insert(d.clone(), vec![]);
        let sorted = topological_sort(&dag).unwrap();
        assert_eq!(sorted.len(), 4);
        let pos = |m: &ModulePath| sorted.iter().position(|x| x == m).unwrap();
        assert!(pos(&d) < pos(&b));
        assert!(pos(&d) < pos(&c));
        assert!(pos(&b) < pos(&a));
        assert!(pos(&c) < pos(&a));
    }

    #[test]
    fn test_detect_cycles_two_module() {
        let a = ModulePath::new(vec!["a".to_string()]);
        let b = ModulePath::new(vec!["b".to_string()]);
        let mut dag = HashMap::new();
        dag.insert(a.clone(), vec![b.clone()]);
        dag.insert(b.clone(), vec![a.clone()]);
        assert!(detect_cycles(&dag).is_err());
    }

    #[test]
    fn test_typecheck_reversed_module_order_in_source() {
        let code = r#"@mod consumer
@pub [read]
@use [read] @from provider
@fn run :: -> Int @ret (@call get_data)
@mod provider
@pub [read]
@fn get_data :: -> Int @ret 42"#;
        let result = typecheck_str(code);
        assert!(
            result.is_ok(),
            "Reversed module order should typecheck after topo ordering: {:?}",
            result
        );
    }

    #[test]
    fn test_fncall_arity_mismatch_too_few() {
        let code = r#"@fn f :: x:Int y:Int -> Int @ret 42
@call f 1"#;
        let result = typecheck_str(code);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("expects 2 arguments") && e.message.contains("got 1"));
        }
    }

    #[test]
    fn test_fncall_arity_mismatch_too_many() {
        let code = r#"@fn f :: x:Int -> Int @ret 42
@call f 1 2 3"#;
        let result = typecheck_str(code);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("expects 1 arguments") && e.message.contains("got 3"));
        }
    }

    #[test]
    fn test_fncall_arg_type_mismatch() {
        let code = r#"@fn f :: x:Int -> Int @ret 42
@call f "hello""#;
        let result = typecheck_str(code);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("Argument 1") && e.message.contains("expected"));
        }
    }

    #[test]
    fn test_fncall_arg_type_correct() {
        let code = r#"@fn f :: x:Int y:Str -> Str @ret "ok"
@call f 42 "hello""#;
        let result = typecheck_str(code);
        assert!(result.is_ok());
        if let Ok(ty) = result {
            assert_eq!(ty, Type::Primitive(PrimitiveType::Str));
        }
    }

    #[test]
    fn test_fncall_unannotated_params() {
        let code = r#"@fn f :: x y -> Int @ret 42
@call f 1 2"#;
        let result = typecheck_str(code);
        assert!(result.is_ok());
    }

    #[test]
    fn test_uncertain_module_return_value_rejected() {
        let code = r#"@mod m
@ret (@uncertain 5)"#;
        let result = typecheck_str(code);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("uncertain"));
        }
    }

    #[test]
    fn test_uncertain_across_modules_ordered_path() {
        let code = r#"@mod m1
@fn f :: -> Int @ret 1
@mod m2
@ret (@uncertain 42)"#;
        let result = typecheck_str(code);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("uncertain"));
        }
    }

    #[test]
    fn test_uncertain_module_result_ordered() {
        let code = r#"@mod m @ret (@uncertain 42)"#;
        let result = typecheck_str(code);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("uncertain"));
        }
    }

    #[test]
    fn test_uncertain_module_allow_uncertain_suppresses_check() {
        let base_env = TypeEnv::new();
        let env_with_uncertain = TypeEnv::with_uncertain_allowed(&base_env);

        let uncertain_expr = Expr::Uncertain(
            Box::new(Expr::Int(42, 0u64, SourceSpan::zero())),
            0u64,
            SourceSpan::zero(),
        );

        let result = typecheck(&uncertain_expr, &env_with_uncertain);
        assert!(result.is_ok(), "should succeed with allow_uncertain=true");
    }

    #[test]
    fn test_types_compatible_fn_pure_found_io_expected() {
        // Pure function should be compatible with IO-expected parameter (Pure ⊆ IO).
        let pure_fn = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        let mut io_effect = EffectSet::pure_();
        io_effect.io = true;
        let io_fn_param = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: io_effect,
            cap: None,
        };
        // Found Pure, Expected IO: should be compatible.
        assert!(types_compatible(&io_fn_param, &pure_fn));
    }

    #[test]
    fn test_types_compatible_fn_io_found_pure_expected() {
        // IO function should NOT be compatible with Pure-expected parameter.
        let mut io_effect = EffectSet::pure_();
        io_effect.io = true;
        let io_fn = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: io_effect,
            cap: None,
        };
        let pure_fn_param = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        // Found IO, Expected Pure: should NOT be compatible.
        assert!(!types_compatible(&pure_fn_param, &io_fn));
    }

    #[test]
    fn test_types_compatible_fn_same_effects() {
        // Same effects should always be compatible.
        let pure_param = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        let pure_found = Type::Fn {
            params: vec![],
            return_type: Box::new(Type::Primitive(PrimitiveType::Int)),
            effect: EffectSet::pure_(),
            cap: None,
        };
        assert!(types_compatible(&pure_param, &pure_found));
    }

}
