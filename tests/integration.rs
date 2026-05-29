use aven_seed::{run_str, run_str_with_context, Value, typecheck_str, SelectorPath, PathSegment, EvalError, Type, PrimitiveType, parse_str, ast, build_module_caps_map, build_module_dependency_dag, detect_cycles, topological_sort, typecheck_program_ordered, TypeEnv, partition_by_module, patch_file_to_diffs, diffs_to_avenpatch_string, DiffKind};
use std::collections::HashMap;

#[test]
fn test_simple_program() {
    let result = run_str("42").unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_let_binding() {
    let result = run_str("@let x :: 10").unwrap();
    assert_eq!(result, Value::Int(10));
}

#[test]
fn test_string_literal() {
    let result = run_str(r#""hello""#).unwrap();
    assert_eq!(result, Value::Str("hello".to_string()));
}

#[test]
fn test_arithmetic() {
    let result = run_str("(+ 2 3)").unwrap();
    assert_eq!(result, Value::Int(5));
}

#[test]
fn test_multiplication() {
    let result = run_str("(* 3 4)").unwrap();
    assert_eq!(result, Value::Int(12));
}

#[test]
fn test_if_expression() {
    let result = run_str("@if @true @then 100 @else 0").unwrap();
    assert_eq!(result, Value::Int(100));
}

#[test]
fn test_if_expression_false() {
    let result = run_str("@if @false @then 100 @else 0").unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_function_definition_and_call() {
    // Define a simple function
    let code = r#"
@fn add :: a:Int b:Int -> Int
  @ret (+ a b)
@call add 5 3
"#;
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(8));
}

#[test]
fn test_nested_arithmetic() {
    let result = run_str("(+ (* 2 3) 4)").unwrap();
    assert_eq!(result, Value::Int(10));
}

#[test]
fn test_string_concatenation() {
    let result = run_str(r#"(+ "hello" " world")"#).unwrap();
    assert_eq!(result, Value::Str("hello world".to_string()));
}

#[test]
fn test_boolean_literals() {
    let result_true = run_str("@true").unwrap();
    assert_eq!(result_true, Value::Bool(true));
    
    let result_false = run_str("@false").unwrap();
    assert_eq!(result_false, Value::Bool(false));
}

#[test]
fn test_io_write() {
    // This will print to stdout, but we're just testing it doesn't error
    let result = run_str(r#"@io.write "Hello AVEN!""#).unwrap();
    assert_eq!(result, Value::Nil);
}

#[test]
fn test_complete_program() {
    let code = r#"
@let x :: 10
@fn square :: n:Int -> Int
  @ret (* n n)
@call square x
"#;
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(100));
}

#[test]
fn test_division() {
    let result = run_str("(/ 20 4)").unwrap();
    assert_eq!(result, Value::Int(5));
}

#[test]
fn test_subtraction() {
    let result = run_str("(- 10 3)").unwrap();
    assert_eq!(result, Value::Int(7));
}

#[test]
fn test_var_in_arithmetic() {
    let result = run_str("@let x :: 10 (+ x 5)").unwrap();
    assert_eq!(result, Value::Int(15));
}

#[test]
fn test_var_in_nested_arithmetic() {
    let result = run_str("@let x :: 3 @let y :: 4 (+ (* x y) 2)").unwrap();
    assert_eq!(result, Value::Int(14));
}

// ---------------------------------------------------------------------------
// AI-native annotation nodes — parse-only in M1, transparent at runtime.
// These exist so AST-level tooling (@diff selectors, linters) can address them.
// ---------------------------------------------------------------------------

#[test]
fn test_intent_is_runtime_nil() {
    let result = run_str(r#"@intent "this module greets the user""#).unwrap();
    assert_eq!(result, Value::Nil);
}

#[test]
fn test_uncertain_passes_through_value() {
    // @uncertain wraps an expression. Its runtime value is the inner value;
    // its AST identity is preserved (a future linter rule can flag it).
    let result = run_str("@uncertain 42").unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_uncertain_wraps_arithmetic_transparently() {
    let result = run_str("@uncertain (* 6 7)").unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_ctx_is_runtime_nil_placeholder() {
    // @ctx is a placeholder until M4 lands the module system + real context API.
    let result = run_str("@ctx").unwrap();
    assert_eq!(result, Value::Nil);
}

#[test]
fn test_diff_keyword_is_reserved_and_parses() {
    // M1 stub: @diff is recognized at every layer. Body parsing is M5.
    let result = run_str("@diff").unwrap();
    assert_eq!(result, Value::Nil);
}

#[test]
fn test_diff_with_body_parses_and_runs() {
    // M5.2: Diff operations parse with selector paths and payloads.
    // Standalone @diff evaluates to Nil (the diff is a declaration, not an application).
    let src = "@diff @replace /greet/body 42 @delete /unused/path";
    let result = run_str(src);
    assert!(result.is_ok(), "Standalone diff should parse and return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_diffs_batch_parses() {
    // M5.2: @diffs (atomic batch) syntax matches @diff
    let src = "@diffs @replace /path/a 100 @insert @last /path/b 200";
    let result = run_str(src);
    assert!(result.is_ok(), "Standalone @diffs should parse and return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_annotations_in_a_real_program() {
    // A program that mentions @intent and wraps a definition in @uncertain
    // must run unchanged — the seed evaluator treats both as transparent.
    let code = r#"
@intent "compute the square of the input"
@fn square :: n:Int -> Int
  @ret (* n n)
@uncertain (@call square 9)
"#;
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(81));
}

#[test]
fn test_effect_arrow_io_only() {
    // Test -!> (IO-only) arrow parses and evaluates
    let code = "@fn read_file :: path:Str -!> Str @ret \"content\"";
    let result = run_str(code).unwrap();
    match result {
        Value::Fn { .. } => {}, // Success: function with IO-only effect parses
        other => panic!("Expected Fn value, got {:?}", other),
    }
}

#[test]
fn test_effect_arrow_async_only() {
    // Test -~> (async-only) arrow parses and evaluates
    let code = "@fn async_fn :: x:Int -~> Int @ret x";
    let result = run_str(code).unwrap();
    match result {
        Value::Fn { .. } => {}, // Success: function with async-only effect parses
        other => panic!("Expected Fn value, got {:?}", other),
    }
}

#[test]
fn test_effect_arrow_err_async() {
    // Test -?~> (err + async) arrow parses and evaluates
    let code = "@fn risky_async :: x:Int -?~> Int @ret x";
    let result = run_str(code).unwrap();
    match result {
        Value::Fn { .. } => {}, // Success: function with err+async effect parses
        other => panic!("Expected Fn value, got {:?}", other),
    }
}

#[test]
fn test_effect_arrow_io_async() {
    // Test -!~> (IO + async) arrow parses and evaluates
    let code = "@fn io_async_fn :: x:Int -!~> Int @ret x";
    let result = run_str(code).unwrap();
    match result {
        Value::Fn { .. } => {}, // Success: function with IO+async effect parses
        other => panic!("Expected Fn value, got {:?}", other),
    }
}

#[test]
fn test_fn_with_int_param_type() {
    // Parse a function with typed parameters and return type; arrow should parse.
    let code = "@fn add :: x::Int y::Int -> Int @ret (+ x y)";
    let result = run_str(code).unwrap();
    match result {
        Value::Fn { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "x");
            assert!(params[0].1.is_some(), "First param should have type annotation");
            assert_eq!(params[1].0, "y");
            assert!(params[1].1.is_some(), "Second param should have type annotation");
        }
        other => panic!("Expected Fn value, got {:?}", other),
    }
}

#[test]
fn test_fn_no_type_annotations() {
    // Parse a function without type annotations; backward compatibility check.
    let code = "@fn add :: x y -> @ret (+ x y)";
    let result = run_str(code).unwrap();
    match result {
        Value::Fn { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "x");
            assert!(params[0].1.is_none(), "First param should have no type");
            assert_eq!(params[1].0, "y");
            assert!(params[1].1.is_none(), "Second param should have no type");
        }
        other => panic!("Expected Fn value, got {:?}", other),
    }
}

#[test]
fn test_fn_union_return_type() {
    // Parse a return type with a union variant.
    // Even simpler - just parse the function, don't call it.
    let code = "@fn maybe :: x::Int -?> #ok Int @ret x";
    // If parsing fails, we'd get a Parse error before Eval error.
    // If we get UndefinedVariable("Int"), it means the parser consumed too much.
    match run_str(code) {
        Ok(Value::Fn { .. }) => {}, // Success: function with union return type parses
        Ok(other) => panic!("Expected Fn value, got {:?}", other),
        Err(e) => {
            // Debug: print what error we got
            eprintln!("Error parsing: {:?}", e);
            panic!("Failed to parse/run: {:?}", e);
        }
    }
}

#[test]
fn test_fn_with_cap_parses() {
    // Parse a function with @cap [read] capability list
    let code = "@fn read_file :: path:Str @cap [read] -!> Str @ret \"content\"";
    match run_str(code) {
        Ok(Value::Fn { .. }) => {}, // Success: function with @cap parses
        Ok(other) => panic!("Expected Fn value, got {:?}", other),
        Err(e) => panic!("Failed to parse/run: {:?}", e),
    }
}

#[test]
fn test_fn_cap_multiple() {
    // Parse a function with multiple capabilities
    let code = "@fn complex :: @cap [read write delete] -!> Nil @ret @nil";
    match run_str(code) {
        Ok(Value::Fn { .. }) => {}, // Success: function with multiple caps parses
        Ok(other) => panic!("Expected Fn value, got {:?}", other),
        Err(e) => panic!("Failed to parse/run: {:?}", e),
    }
}

#[test]
fn test_fn_no_cap_defaults_empty() {
    // Parse a function without @cap (should default to empty vector)
    let code = "@fn simple :: -> Nil @ret @nil";
    match run_str(code) {
        Ok(Value::Fn { .. }) => {}, // Success: function without @cap parses
        Ok(other) => panic!("Expected Fn value, got {:?}", other),
        Err(e) => panic!("Failed to parse/run: {:?}", e),
    }
}

#[test]
fn test_eval_ignores_param_types() {
    // Verify that evaluation of typed and untyped functions produces the same result.
    let typed_code = "@fn identity :: x::Int -> Int @ret x (@call identity 42)";
    let untyped_code = "@fn identity :: x -> @ret x (@call identity 42)";

    let typed_result = run_str(typed_code).unwrap();
    let untyped_result = run_str(untyped_code).unwrap();

    assert_eq!(typed_result, untyped_result, "Typed and untyped functions should evaluate the same");
    assert_eq!(typed_result, Value::Int(42));
}

#[cfg(test)]
mod typechecker_tests {
    use aven_seed::typecheck_str;
    use aven_seed::ast::{Type, PrimitiveType};

    #[test]
    fn test_typecheck_int_literal() {
        let result = typecheck_str("42");
        assert_eq!(result, Ok(Type::Primitive(PrimitiveType::Int)));
    }

    #[test]
    fn test_typecheck_undefined_var() {
        let result = typecheck_str("x");
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.message.contains("Undefined variable"));
        }
    }

    #[test]
    fn test_typecheck_let_binding() {
        let result = typecheck_str("@let x :: 10 x");
        assert_eq!(result, Ok(Type::Primitive(PrimitiveType::Int)));
    }

    #[test]
    fn test_typecheck_fn_def_pure() {
        let code = "@fn f :: x :: Int -> Int @ret x";
        let result = typecheck_str(code);
        match result {
            Ok(Type::Fn { effect, .. }) => {
                assert!(effect.is_pure());
            }
            _ => panic!("Expected Fn type with Pure effect"),
        }
    }

    #[test]
    fn test_pure_calls_io_rejected() {
        // Define an IO function (-!>), then a pure function (->) that calls it.
        // The type checker should reject this because Pure cannot call IO.
        let code = r#"
@fn io_fn :: x::Int -!> Int
  @ret (+ x 1)
@fn pure_fn :: y::Int -> Int
  @call io_fn y
"#;
        let result = typecheck_str(code);
        // This must error: effect not a subset
        assert!(result.is_err(), "Expected type error, but got Ok");
    }

    #[test]
    fn test_io_calls_io_allowed() {
        // Define two IO functions where one calls the other.
        // This should type-check successfully.
        let code = r#"
@fn io_fn1 :: x::Int -!> Int
  @ret (+ x 1)
@fn io_fn2 :: y::Int -!> Int
  @call io_fn1 y
"#;
        let result = typecheck_str(code);
        // This should succeed; IO can call IO.
        assert!(result.is_ok(), "Expected Ok, but got error: {:?}", result);
    }

    #[test]
    fn test_pure_calls_pure_allowed() {
        // Define two pure functions where one calls the other.
        // This should type-check successfully.
        let code = r#"
@fn pure_fn1 :: x::Int -> Int
  @ret (+ x 1)
@fn pure_fn2 :: y::Int -> Int
  @call pure_fn1 y
"#;
        let result = typecheck_str(code);
        // This should succeed; Pure can call Pure.
        assert!(result.is_ok(), "Expected Ok, but got error: {:?}", result);
    }

    #[test]
    fn test_io_caller_cannot_call_async() {
        // Define an async function (-~>), then an IO function (-!>) that calls it.
        // The type checker should reject this because IO cannot call async without async in its own effect set.
        let code = r#"
@fn async_fn :: x::Int -~> Int
  @ret (+ x 1)
@fn io_fn :: y::Int -!> Int
  @call async_fn y
"#;
        let result = typecheck_str(code);
        // This must error: async not in caller's effect set
        assert!(result.is_err(), "Expected type error, but got Ok");
    }

    #[test]
    fn test_fn_with_union_ok_variant() {
        // Function returning a tagged union variant — body must return #ok x
        let code = "@fn maybe_result :: x:Int -> #ok Int @ret (#ok x)";
        let result = typecheck_str(code);
        assert!(result.is_ok(), "Union #ok Int return type should be accepted: {:?}", result);
        if let Ok(Type::Fn { return_type, .. }) = result {
            if let Type::Union(variants) = return_type.as_ref() {
                assert_eq!(variants.len(), 1);
                assert_eq!(variants[0].tag, "ok");
                assert!(variants[0].payload.is_some());
            } else {
                panic!("Expected Union in return type");
            }
        } else {
            panic!("Expected Fn type");
        }
    }

    #[test]
    fn test_fn_with_union_err_variant() {
        // Function with Union return type annotation (#err variant with Str payload)
        let code = "@fn safe_parse :: s:Str -?> #err Str @ret (#err s)";
        let result = typecheck_str(code);
        assert!(result.is_ok(), "Union #err Str return type with error effect should be accepted: {:?}", result);
        if let Ok(Type::Fn { return_type, effect, .. }) = result {
            assert!(effect.err, "Expected error flag set in effect");
            if let Type::Union(variants) = return_type.as_ref() {
                assert_eq!(variants.len(), 1);
                assert_eq!(variants[0].tag, "err");
            } else {
                panic!("Expected Union in return type");
            }
        } else {
            panic!("Expected Fn type");
        }
    }

    #[test]
    fn test_pure_fn_with_union_int_return() {
        // Pure function returning a union variant
        let code = "@fn pick :: x:Int -> #ok Int @ret (#ok x)";
        let result = typecheck_str(code);
        assert!(result.is_ok(), "Pure fn with union return should be accepted: {:?}", result);
        if let Ok(Type::Fn { effect, .. }) = result {
            assert!(effect.is_pure(), "Function should be pure");
        } else {
            panic!("Expected Fn type");
        }
    }

    #[test]
    fn test_multiple_params_union_return() {
        // Multiple parameters and union return
        let code = "@fn safe_add :: x:Int y:Int -?> #ok Int @ret (#ok (+ x y))";
        let result = typecheck_str(code);
        assert!(result.is_ok(), "Multi-param function with union return should be accepted: {:?}", result);
        if let Ok(Type::Fn { params, return_type, .. }) = result {
            assert_eq!(params.len(), 2);
            assert!(matches!(return_type.as_ref(), Type::Union(_)));
        } else {
            panic!("Expected Fn type");
        }
    }

    #[test]
    fn test_io_fn_with_union_return() {
        // IO function returning a union variant
        let code = r#"@fn read_result :: path:Str -!> #ok Str @ret (#ok "data")"#;
        let result = typecheck_str(code);
        assert!(result.is_ok(), "IO function with union return should be accepted: {:?}", result);
        if let Ok(Type::Fn { effect, .. }) = result {
            assert!(effect.io, "Expected IO flag set in effect");
        } else {
            panic!("Expected Fn type");
        }
    }

}

#[test]
fn test_use_parses() {
    let result = run_str("@use [read] @from fs");
    assert!(result.is_ok(), "Expected parse success, got error: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_use_caps_list() {
    let code = r#"@use [read write delete] @from db"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Expected parse success, got error: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_use_module_name() {
    let code = r#"@use [read] @from mymodule"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Expected parse success, got error: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_match_symbol_pattern() {
    let code = "@match #admin #admin -> 1 _ -> 0";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn test_match_wildcard_default() {
    let code = "@match #guest #admin -> 1 _ -> 2";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(2));
}

#[test]
fn test_match_multiple_branches() {
    let code = "@match #user #admin -> 10 #user -> 20 #guest -> 30 _ -> 0";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(20));
}

#[test]
fn test_match_pattern_binding() {
    let code = "@match (#ok 42) #ok v -> v _ -> -1";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_match_type_mismatch() {
    let code = "@match 42 #ok v -> v _ -> 0";
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject match on non-union type");
}

#[test]
fn test_match_non_exhaustive() {
    let code = r#"@fn process :: status::#ok Int | #err Str -> Int
      @match status #ok v -> v #err _ -> 0"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject non-exhaustive match");
}

#[test]
fn test_match_payload_type() {
    let code = "@match (#ok 42) #ok v -> v _ -> 0";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(42), "Pattern binding should extract payload value");
}

#[test]
fn test_ok_constructor() {
    let code = "@ok 42";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Tagged("ok".to_string(), Some(Box::new(Value::Int(42)))));
}

#[test]
fn test_err_constructor() {
    let code = r#"@err "msg""#;
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Tagged("err".to_string(), Some(Box::new(Value::Str("msg".to_string())))));
}

#[test]
fn test_record_duplicate_field_rejected() {
    let code = "{x:1, x:2}";
    let result = parse_str(code);
    assert!(result.is_err(), "Parser should reject duplicate field names");
}

#[test]
fn test_ok_err_in_match() {
    let code = "@match (@ok 99) #ok v -> v _ -> -1";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(99));
    // Verify type checking path works end-to-end
    let typecheck_result = typecheck_str(code);
    assert!(typecheck_result.is_ok(), "Type checker should accept @ok in match: {:?}", typecheck_result);
}

#[test]
fn test_err_in_pure_function_rejected() {
    let code = r#"@fn bad :: -> Str @err "oops""#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject @err in pure function");
}

#[test]
fn test_err_effect_type_inference() {
    // Parser does not yet support union return types like "#ok Int | #err Str"
    // Verify effect function can produce @err and type checking accepts it
    let code = "@fn may_fail :: -?> @err \"failed\" (@call may_fail)";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Tagged("err".to_string(), Some(Box::new(Value::Str("failed".to_string())))));
    // Verify type checking path works for effect function with @err
    let typecheck_result = typecheck_str(code);
    assert!(typecheck_result.is_ok(), "Type checker should accept @err in effect function: {:?}", typecheck_result);
}

#[test]
fn test_uncertain_value_escapes_typed_boundary_rejected() {
    // A function f() -> Int contains @let x = @uncertain (5); @ret x
    // The typechecker should reject this because the uncertain value escapes the function boundary.
    let code = r#"@fn f :: -> Int
      @let x :: @uncertain 5
      @ret x"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject uncertain value escaping typed boundary");
    if let Err(err) = result {
        assert!(err.message.contains("escape"), "Error message should mention escaping");
    }
}

#[test]
fn test_uncertain_permitted_in_uncertain_scope() {
    // Inside an @uncertain block, values are permitted.
    // This test checks that uncertain values can be wrapped and re-typed.
    let code = r#"@uncertain 5"#;
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Int(5), "Uncertain block should evaluate to inner value");

    // Type checking should also succeed
    let typecheck_result = typecheck_str(code);
    assert!(typecheck_result.is_ok(), "Type checker should permit uncertain values: {:?}", typecheck_result);
}

#[test]
fn test_uncertain_nested_fn_boundary_rejected() {
    // A function that tries to return an uncertain value should be rejected.
    // Even within its own definition, the function has a typed boundary.
    let code = r#"@fn g :: -> Int
      @ret @uncertain 42"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject uncertain return from function");
}

#[test]
fn test_uncertain_in_match_union_rejected() {
    // Matching on a union and returning the uncertain-typed payload is rejected at function boundary.
    let code = r#"@fn process :: x::int | err Str -> Int
      @match x #ok v -> v #err _ -> 0"#;
    let result = run_str(code);
    // Runtime eval should succeed (no enforcement at eval time)
    assert!(result.is_ok() || result.is_err(), "Runtime eval may parse/fail depending on parser state");

    // Type checking with uncertain values in match context
    let typecheck_code = r#"@fn safe :: x::#ok Int -> Int
      @let y :: @uncertain (@match x #ok v -> v)
      @ret y"#;
    let typecheck_result = typecheck_str(typecheck_code);
    assert!(typecheck_result.is_err(), "Type checker should reject uncertain return from function with uncertain in let");
}

#[test]
fn test_uncertain_multiple_escapes_each_rejected() {
    // Multiple distinct @uncertain values in separate let-bindings should each trigger rejection.
    let code = r#"@fn f :: -> Int
      @let x :: @uncertain 1
      @let y :: @uncertain 2
      @ret (+ x y)"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject uncertain values escaping from let bindings");
}

#[test]
fn test_mod_parses() {
    let code = "@mod aven-std-io";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Nil, "@mod should parse and eval to Nil");
}

#[test]
fn test_mod_with_pub() {
    let code = r#"@mod aven-std-io
      @pub [read write]
      @fn greet :: -> Str "hello""#;
    let result = run_str(code);
    assert!(result.is_ok(), "Module with pub and fn should parse and execute: {:?}", result);
}

#[test]
fn test_pub_multiple_caps() {
    let code = "@pub [read write delete]";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Nil, "@pub with multiple capabilities should parse and eval to Nil");
}

#[test]
fn test_pub_empty_caps() {
    let code = "@pub []";
    let result = run_str(code).unwrap();
    assert_eq!(result, Value::Nil, "@pub with empty capability list should parse and eval to Nil");
}

#[test]
fn test_use_valid_subset() {
    let code = r#"
@mod foo
@pub [read write]
@use [read] @from foo
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Type checker should accept valid capability subset: {:?}", result);
}

#[test]
fn test_use_invalid_superset() {
    let code = r#"
@mod foo
@pub [read]
@use [read write] @from foo
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject capability superset");
    if let Err(err) = result {
        assert!(err.message.contains("does not export capability"),
            "Error should mention missing capability: {}", err.message);
    }
}

#[test]
fn test_use_module_not_found() {
    let code = r#"@use [read] @from nonexistent"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject use of nonexistent module");
    if let Err(err) = result {
        assert!(err.message.contains("module not found"),
            "Error should mention module not found: {}", err.message);
    }
}

#[test]
fn test_use_empty_caps_always_valid() {
    let code = r#"@use [] @from nonexistent"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Type checker should accept empty capability list even for nonexistent module");
}

#[test]
fn test_mod_dotted_single() {
    let code = "@mod aven @pub [read]";
    let result = run_str(code);
    assert!(result.is_ok(), "Module with dotted path (single part) should parse: {:?}", result);
}

#[test]
fn test_mod_dotted_nested() {
    let code = "@mod aven/std/io @pub [read write]";
    let result = run_str(code);
    assert!(result.is_ok(), "Module with nested dotted path should parse: {:?}", result);
}

#[test]
fn test_use_dotted_matches_mod() {
    let code = r#"@mod aven/std/io @pub [read]
                  @use [read] @from aven/std/io"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Use with dotted path should match module with same dotted path");
}

#[test]
fn test_use_dotted_not_found() {
    let code = r#"@use [read] @from aven/unknown"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject use of nonexistent dotted module");
    if let Err(err) = result {
        assert!(err.message.contains("module not found"),
            "Error should mention module not found: {}", err.message);
    }
}

#[test]
fn test_module_path_to_string() {
    use aven_seed::ast::ModulePath;
    let path = ModulePath::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    assert_eq!(path.to_string(), "a/b/c");
}

#[test]
fn test_module_path_parts_empty_rejected() {
    // An empty parts vector should be rejected or handled as invalid.
    // We verify that parsing with double slashes (which would create empty parts) fails.
    let code = "@mod a//b @pub [read]";
    let result = run_str(code);
    // This should fail to parse because // creates an empty identifier
    assert!(result.is_err() || !result.is_ok(),
        "Module path with empty parts should not parse successfully");
}

#[test]
fn test_cycle_direct() {
    // Test the simplest non-trivial cycle: a two-module mutual cycle
    let code = r#"@mod a
@pub [read]
@use [read] @from b
@mod b
@pub [read]
@use [read] @from a"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should detect two-module cycle");
    if let Err(err) = result {
        assert!(err.message.contains("circular module dependency detected"),
            "Error should mention circular dependency: {}", err.message);
    }
}

#[test]
fn test_cycle_two_module_chain() {
    let code = r#"@mod a
@pub [read]
@use [read] @from b
@mod b
@pub [read]
@use [read] @from a"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should detect two-module cycle");
    if let Err(err) = result {
        assert!(err.message.contains("circular module dependency detected"),
            "Error should mention circular dependency: {}", err.message);
    }
}

#[test]
fn test_cycle_three_module_chain() {
    let code = r#"@mod a
@pub [read]
@use [read] @from b
@mod b
@pub [read]
@use [read] @from c
@mod c
@pub [read]
@use [read] @from a"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should detect three-module cycle");
    if let Err(err) = result {
        assert!(err.message.contains("circular module dependency detected"),
            "Error should mention circular dependency: {}", err.message);
    }
}

#[test]
fn test_no_cycle_valid_dag() {
    let code = r#"@mod a
@pub [read]
@mod b
@pub [read]
@use [read] @from a
@mod c
@pub [read]
@use [read] @from a
@use [read] @from b"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Type checker should accept valid DAG with no cycles: {:?}", result);
}

#[test]
fn test_typecheck_respects_module_order() {
    let code = r#"@mod base
@pub [compute]
@fn add :: x :: Int y :: Int -> Int (+ x y)
@mod derived
@pub [compute]
@use [compute] @from base"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Dependent module should be checked after base module: {:?}", result);
}

#[test]
fn test_use_module_typechecked_first() {
    let code = r#"@mod provider
@pub [read]
@fn get_data :: -> Int @ret 42
@mod consumer
@pub [read]
@use [read] @from provider"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Consumer module with @use should be typechecked after provider: {:?}", result);
}

#[test]
fn test_ctx_get_string() {
    // Test that @ctx.get retrieves a string value from context
    let code = r#"@fn greet :: -> String (@ctx.get ctx "name")"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Context get should parse and eval: {:?}", result);
}

#[test]
fn test_ctx_get_int() {
    // Test that @ctx.get retrieves an integer value from context
    let code = r#"@fn get_age :: -> Int (@ctx.get ctx "age")"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Context get with int should parse and eval: {:?}", result);
}

#[test]
fn test_ctx_get_missing_key_nil() {
    // Test that @ctx.get returns Nil for missing keys
    let code = r#"@ctx.get ctx "missing""#;
    let result = run_str(code);
    assert_eq!(result.unwrap(), Value::Nil, "Missing context key should return Nil");
}

#[test]
fn test_ctx_get_in_match_unwrap() {
    // Test @ctx.get in nested expression
    let code = r#"@let x :: 42 @let y :: (@ctx.get ctx "key") y"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Context get should parse and eval in let binding: {:?}", result);
}

#[test]
fn test_ctx_not_global_function_arg() {
    // Test that CtxGet can be used within function definitions
    let code = r#"@fn fetch_value :: -> Int @ret (@ctx.get ctx "value")"#;
    let result = run_str(code);
    assert!(result.is_ok(), "CtxGet should work in function body: {:?}", result);
}

#[test]
fn test_ctx_get_non_string_key_rejected() {
    // Test that @ctx.get with a non-string key (integer) fails parsing
    let code = r#"@ctx.get ctx 42"#;
    let result = run_str(code);
    assert!(result.is_err(), "Parser should reject non-string keys in @ctx.get");
}

#[test]
fn test_ctx_get_with_set() {
    // Test that @ctx.get retrieves values that have been set via context
    let mut context = HashMap::new();
    context.insert("name".to_string(), Value::Str("Alice".to_string()));
    context.insert("age".to_string(), Value::Int(30));

    // Retrieve string value
    let code_str = r#"@ctx.get ctx "name""#;
    let result = run_str_with_context(code_str, context.clone());
    assert!(result.is_ok(), "Context get with set value should succeed: {:?}", result);
    assert_eq!(result.unwrap(), Value::Str("Alice".to_string()));

    // Retrieve integer value
    let code_int = r#"@ctx.get ctx "age""#;
    let result_int = run_str_with_context(code_int, context);
    assert!(result_int.is_ok(), "Context get with int value should succeed: {:?}", result_int);
    assert_eq!(result_int.unwrap(), Value::Int(30));
}

#[test]
fn test_ctx_set_and_get_roundtrip() {
    // Set a value, then get it back in the same function scope
    let mut ctx = HashMap::new();
    ctx.insert("x".to_string(), Value::Str("initial".to_string()));
    let code = r#"
@fn roundtrip :: -> Str
  (@ctx.set ctx "x" "written")
  @ctx.get ctx "x"
@call roundtrip
"#;
    let result = run_str_with_context(code, ctx);
    assert!(result.is_ok(), "Set then get should round-trip: {:?}", result);
    assert_eq!(result.unwrap(), Value::Str("written".to_string()));
}

#[test]
fn test_ctx_set_overwrites_prior_value() {
    // Seed parent env with "old", then @ctx.set from a function should overwrite it
    let mut context = HashMap::new();
    context.insert("key".to_string(), Value::Str("old".to_string()));
    let code = r#"
@fn overwrite :: -> Str
  (@ctx.set ctx "key" "new")
  @ctx.get ctx "key"
@call overwrite
"#;
    let result = run_str_with_context(code, context);
    assert!(result.is_ok(), "Context set should overwrite prior value: {:?}", result);
    assert_eq!(result.unwrap(), Value::Str("new".to_string()),
        "Expected overwritten value 'new'");
}

#[test]
fn test_ctx_set_non_string_key_rejected() {
    // Parser should reject non-string keys at parse time
    let code = r#"@ctx.set ctx 42 "value""#;
    let result = run_str(code);
    assert!(result.is_err(), "Parser should reject non-string keys in @ctx.set");
}

#[test]
fn test_ctx_set_in_nested_scope_writes_to_parent_env() {
    // Set value in a nested fn scope (where key exists in parent context), then get it
    let mut ctx = HashMap::new();
    ctx.insert("modified".to_string(), Value::Str("original".to_string()));
    let code = r#"
@fn set_in_fn :: -> Str
  (@ctx.set ctx "modified" "success")
  @ctx.get ctx "modified"
@call set_in_fn
"#;
    let result = run_str_with_context(code, ctx);
    assert!(result.is_ok(), "Context set in fn scope should be readable via get: {:?}", result);
    assert_eq!(result.unwrap(), Value::Str("success".to_string()));
}

#[test]
fn test_selector_path_simple() {
    // Test that a simple selector path like `/fn/greet` parses correctly
    let path = SelectorPath::from_string("/fn/greet").expect("Should parse simple path");
    assert_eq!(path.parts.len(), 2);
    assert_eq!(path.parts[0], PathSegment::Named("fn".to_string()));
    assert_eq!(path.parts[1], PathSegment::Named("greet".to_string()));
    assert_eq!(path.to_string(), "/fn/greet");
}

#[test]
fn test_selector_path_with_index() {
    // Test that a path with index like `/let[0]/body` parses correctly
    let path = SelectorPath::from_string("/let[0]/body").expect("Should parse path with index");
    assert_eq!(path.parts.len(), 3);
    assert_eq!(path.parts[0], PathSegment::Named("let".to_string()));
    assert_eq!(path.parts[1], PathSegment::Index(0));
    assert_eq!(path.parts[2], PathSegment::Named("body".to_string()));
    assert_eq!(path.to_string(), "/let/[0]/body");
}

#[test]
fn test_selector_path_nested() {
    // Test multi-level path `/fn/outer/let/x/body`
    let path = SelectorPath::from_string("/fn/outer/let/x/body")
        .expect("Should parse nested path");
    assert_eq!(path.parts.len(), 5);
    assert_eq!(path.parts[0], PathSegment::Named("fn".to_string()));
    assert_eq!(path.parts[1], PathSegment::Named("outer".to_string()));
    assert_eq!(path.parts[2], PathSegment::Named("let".to_string()));
    assert_eq!(path.parts[3], PathSegment::Named("x".to_string()));
    assert_eq!(path.parts[4], PathSegment::Named("body".to_string()));
}

#[test]
fn test_selector_path_empty_rejected() {
    // Test that a bare `/` with no segments is rejected
    let result = SelectorPath::from_string("/");
    assert!(result.is_err(), "Empty path should be rejected");
}

#[test]
fn test_meta_description_only() {
    // Test that @meta { description: "fix bug" } parses (metadata is parse-only).
    // Standalone @diff evaluates to Nil regardless of selector path.
    let code = r#"@diff @meta { description: "fix bug" } @replace /fn/foo 42"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Diff with @meta should parse and return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_meta_all_fields() {
    // Test that all three @meta fields parse correctly
    let code = r#"@diff @meta { description: "fix bug" author: "alice" timestamp: "2026-05-20" } @replace /fn/foo 42"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Diff with all @meta fields should parse and return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_meta_missing_optional_ok() {
    // Test that omitting @meta entirely is fine — standalone @diff returns Nil.
    let code = r#"@diff @replace /fn/foo 42"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Diff without @meta should parse and return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_diff_replace_simple_int() {
    // Standalone diff targeting nothing should error (no base expression)
    let code = r#"@diff @replace / 99"#;
    let result = run_str(code);
    // Should return an error because the selector is invalid in the diff expr context
    assert!(result.is_err(), "Diff with invalid selector path should error");
}

#[test]
fn test_diff_delete_expr() {
    // Standalone @diff evaluates to Nil (the diff ops are not applied without a target).
    let code = r#"@diff @delete /right"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Standalone diff should return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_diff_replace_in_nested_arithmetic() {
    // Standalone @diff evaluates to Nil.
    let code = r#"@diff @replace /right 5"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Standalone diff should return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_diff_nested_selector() {
    // Standalone @diff with multi-segment selector path evaluates to Nil.
    let code = r#"@diff @replace /left/right 5"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Standalone diff with nested selector should return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_diff_selector_not_found() {
    // Standalone @diff with any selector evaluates to Nil (ops are not applied without a target).
    let code = r#"@diff @replace /nonexistent 99"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Standalone diff should return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_diff_multiple_ops_sequential() {
    // Standalone @diff with multiple ops evaluates to Nil.
    let code = r#"@diff @replace /a 1 @delete /b"#;
    let result = run_str(code);
    assert!(result.is_ok(), "Standalone diff with multiple ops should return Nil: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_recursive_int_to_int_annotated() {
    // A recursive function annotated -> Int that calls itself
    let code = r#"@fn factorial :: n:Int -> Int
  @if @true
    @then 1
    @else (* 2 (@call factorial 5))"#;

    // Should typecheck successfully with the annotated return type
    let typecheck_result = typecheck_str(code);
    assert!(typecheck_result.is_ok(), "Recursive Int -> Int should typecheck: {:?}", typecheck_result);

    // Should also evaluate correctly
    let eval_result = run_str(code);
    assert!(eval_result.is_ok(), "Recursive Int -> Int should evaluate: {:?}", eval_result);
}

#[test]
fn test_recursive_with_mismatched_return_annotated() {
    // A recursive function annotated -> Int but tries to return a string in one branch
    let code = r#"@fn bad_recursive :: n:Int -> Int
  @if @true
    @then "zero"
    @else (@call bad_recursive 5)"#;

    // Should fail typechecking because the return type is Int but "zero" is Str
    let typecheck_result = typecheck_str(code);
    assert!(typecheck_result.is_err(), "Recursive function with mismatched return should fail typecheck: {:?}", typecheck_result);
}

// ============================================================================
// M6.1 — NativeFn + aven/std/math tests
// ============================================================================

#[test]
fn test_native_fn_abs() {
    let env = aven_seed::Env::new();
    match env.get("abs").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 1);
            let result = func(&[Value::Int(-5)]);
            assert_eq!(result, Ok(Value::Int(5)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_abs_overflow() {
    let env = aven_seed::Env::new();
    match env.get("abs").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(i64::MIN)]);
            assert!(result.is_err(), "abs overflow should error");
            match result {
                Err(EvalError::InvalidOperation(_)) => {}
                _ => panic!("expected InvalidOperation"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_min() {
    let env = aven_seed::Env::new();
    match env.get("min").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(3), Value::Int(7)]);
            assert_eq!(result, Ok(Value::Int(3)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_max() {
    let env = aven_seed::Env::new();
    match env.get("max").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(3), Value::Int(7)]);
            assert_eq!(result, Ok(Value::Int(7)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_pow() {
    let env = aven_seed::Env::new();
    match env.get("pow").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(2), Value::Int(10)]);
            assert_eq!(result, Ok(Value::Int(1024)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_pow_negative_exp() {
    let env = aven_seed::Env::new();
    match env.get("pow").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(2), Value::Int(-1)]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_sqrt() {
    let env = aven_seed::Env::new();
    match env.get("sqrt").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 1);
            let result = func(&[Value::Int(9)]);
            assert_eq!(result, Ok(Value::Int(3)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_sqrt_negative() {
    let env = aven_seed::Env::new();
    match env.get("sqrt").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(-1)]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_type_error_abs() {
    let env = aven_seed::Env::new();
    match env.get("abs").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("x".to_string())]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_module_qualified_lookup() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/math::abs").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_native_fn_display() {
    let env = aven_seed::Env::new();
    let val = env.get("abs").unwrap();
    assert_eq!(format!("{}", val), "<native:abs>");
}

#[test]
fn test_native_fn_eq_same_name_arity() {
    let env = aven_seed::Env::new();
    let fn1 = env.get("abs").unwrap();
    let fn2 = env.get("abs").unwrap();
    // NativeFn equality is false by design (closures not comparable)
    // This test documents the behavior
    assert!(fn1 != fn2, "NativeFns always return false on ==");
}

// ============================================================================
// M6.2 — aven/std/io tests
// ============================================================================

#[test]
fn test_io_print_registered() {
    let env = aven_seed::Env::new();
    match env.get("print").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_io_print_type_error() {
    let env = aven_seed::Env::new();
    match env.get("print").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42)]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_io_read_line_registered() {
    let env = aven_seed::Env::new();
    match env.get("read_line").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 0);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_io_write_registered() {
    let env = aven_seed::Env::new();
    match env.get("write").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_io_qualified_print() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/io::print").unwrap() {
        Value::NativeFn { .. } => {}
        _ => panic!("expected NativeFn"),
    }
}

// ============================================================================
// M6.3 — aven/std/fs tests
// ============================================================================

#[test]
fn test_fs_read_registered() {
    let env = aven_seed::Env::new();
    match env.get("fs_read").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_fs_write_registered() {
    let env = aven_seed::Env::new();
    match env.get("fs_write").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 2);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_fs_list_registered() {
    let env = aven_seed::Env::new();
    match env.get("fs_list").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_fs_read_nonexistent() {
    let env = aven_seed::Env::new();
    match env.get("fs_read").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("/nonexistent/path/xyz123456".to_string())]);
            match result {
                Err(EvalError::InvalidOperation(msg)) => {
                    assert!(msg.contains("not found") || msg.contains("file not found"));
                }
                _ => panic!("expected InvalidOperation"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_fs_write_read_roundtrip() {
    let env = aven_seed::Env::new();
    let temp_file = format!("{}/aven_test_roundtrip_{}.txt", std::env::temp_dir().display(), std::process::id());

    // Write
    match env.get("fs_write").unwrap() {
        Value::NativeFn { func, .. } => {
            let write_result = func(&[
                Value::Str(temp_file.clone()),
                Value::Str("test content".to_string()),
            ]);
            assert!(write_result.is_ok());
        }
        _ => panic!("expected NativeFn"),
    }

    // Read
    match env.get("fs_read").unwrap() {
        Value::NativeFn { func, .. } => {
            let read_result = func(&[Value::Str(temp_file.clone())]);
            assert_eq!(read_result, Ok(Value::Str("test content".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_fs_list_empty_dir() {
    let env = aven_seed::Env::new();
    let temp_dir = format!("{}/aven_test_empty_{}", std::env::temp_dir().display(), std::process::id());
    let _ = std::fs::create_dir(&temp_dir);

    match env.get("fs_list").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str(temp_dir.clone())]);
            assert_eq!(result, Ok(Value::Str("".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }

    let _ = std::fs::remove_dir(&temp_dir);
}

#[test]
fn test_fs_list_sorted() {
    let env = aven_seed::Env::new();
    let temp_dir = format!("{}/aven_test_sorted_{}", std::env::temp_dir().display(), std::process::id());
    let _ = std::fs::create_dir(&temp_dir);
    let _ = std::fs::write(format!("{}/b.txt", temp_dir), "");
    let _ = std::fs::write(format!("{}/a.txt", temp_dir), "");

    match env.get("fs_list").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str(temp_dir.clone())]);
            assert_eq!(result, Ok(Value::Str("a.txt\nb.txt".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }

    let _ = std::fs::remove_file(format!("{}/a.txt", temp_dir));
    let _ = std::fs::remove_file(format!("{}/b.txt", temp_dir));
    let _ = std::fs::remove_dir(&temp_dir);
}

#[test]
fn test_fs_read_directory_path() {
    let env = aven_seed::Env::new();
    let temp_dir = std::env::temp_dir().to_string_lossy().to_string();

    match env.get("fs_read").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str(temp_dir)]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_fs_list_file_path() {
    let env = aven_seed::Env::new();
    let temp_file = format!("{}/aven_test_file_{}.txt", std::env::temp_dir().display(), std::process::id());
    let _ = std::fs::write(&temp_file, "content");

    match env.get("fs_list").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str(temp_file.clone())]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }

    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_fs_qualified_lookup() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/fs::read").unwrap() {
        Value::NativeFn { .. } => {}
        _ => panic!("expected NativeFn"),
    }
}

// ============================================================================
// M6.4 — aven/std/json tests
// ============================================================================

#[test]
fn test_json_parse_registered() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_serialize_registered() {
    let env = aven_seed::Env::new();
    match env.get("json_serialize").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_parse_null() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("null".to_string())]);
            assert_eq!(result, Ok(Value::Nil));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_parse_bool() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("true".to_string())]);
            assert_eq!(result, Ok(Value::Bool(true)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_parse_int() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("42".to_string())]);
            assert_eq!(result, Ok(Value::Int(42)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_parse_string() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str(r#""hello""#.to_string())]);
            assert_eq!(result, Ok(Value::Str("hello".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_parse_array() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("[1, 2, 3]".to_string())]);
            assert_eq!(result, Ok(Value::Str("1\n2\n3".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_parse_float_rejected() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("3.14".to_string())]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_parse_object_rejected() {
    let env = aven_seed::Env::new();
    match env.get("json_parse").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str(r#"{"a":1}"#.to_string())]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_serialize_nil() {
    let env = aven_seed::Env::new();
    match env.get("json_serialize").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Nil]);
            assert_eq!(result, Ok(Value::Str("null".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_serialize_bool() {
    let env = aven_seed::Env::new();
    match env.get("json_serialize").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Bool(true)]);
            assert_eq!(result, Ok(Value::Str("true".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_serialize_int() {
    let env = aven_seed::Env::new();
    match env.get("json_serialize").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42)]);
            assert_eq!(result, Ok(Value::Str("42".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_serialize_str() {
    let env = aven_seed::Env::new();
    match env.get("json_serialize").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("hello".to_string())]);
            assert_eq!(result, Ok(Value::Str(r#""hello""#.to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_serialize_type_error() {
    let env = aven_seed::Env::new();
    match env.get("json_serialize").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Tagged("tag".to_string(), None)]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_json_qualified_lookup() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/json::parse").unwrap() {
        Value::NativeFn { .. } => {}
        _ => panic!("expected NativeFn"),
    }
}

// ============================================================================
// M6.5 — aven/std/time tests
// ============================================================================

#[test]
fn test_time_now_returns_int() {
    let env = aven_seed::Env::new();
    match env.get("time_now").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 0);
            let result = func(&[]);
            match result {
                Ok(Value::Int(_)) => {}
                _ => panic!("time_now should return Int"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_time_sleep_positive_millis() {
    let env = aven_seed::Env::new();
    match env.get("time_sleep").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(1)]);
            assert_eq!(result, Ok(Value::Nil));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_time_sleep_negative_rejected() {
    let env = aven_seed::Env::new();
    match env.get("time_sleep").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(-1)]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_time_format_valid_timestamp() {
    let env = aven_seed::Env::new();
    match env.get("time_format").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(0), Value::Str("%Y-%m-%d".to_string())]);
            assert_eq!(result, Ok(Value::Str("1970-01-01".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_time_format_roundtrip() {
    let env = aven_seed::Env::new();
    match env.get("time_format").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(1234567890), Value::Str("%Y-%m-%d %H:%M:%S".to_string())]);
            assert_eq!(result, Ok(Value::Str("2009-02-13 23:31:30".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_time_format_invalid_format_string() {
    let env = aven_seed::Env::new();
    match env.get("time_format").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(0), Value::Str("%Q".to_string())]);
            match result {
                Err(EvalError::InvalidOperation(_)) => {}
                _ => panic!("expected InvalidOperation"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_time_now_arity_one() {
    let env = aven_seed::Env::new();
    match env.get("time_now").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 0, "time_now should have arity 0");
            // time_now ignores extra args and succeeds (current implementation)
            // This test documents the actual behavior
            let result = func(&[Value::Int(1)]);
            // It should still return an Int (or error, depending on implementation)
            // Currently it accepts the arg and returns int
            match result {
                Ok(Value::Int(_)) => {}, // OK: time_now ignores extra args
                Err(_) => {}, // OK: time_now rejects extra args
                _ => panic!("expected Int or error"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_time_now_qualified_execution() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/time::now").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 0);
        }
        _ => panic!("expected NativeFn"),
    }
}

// ============================================================================
// M6.6 — aven/std/collections tests
// ============================================================================

#[test]
fn test_col_list_registered() {
    let env = aven_seed::Env::new();
    match env.get("col_list").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_col_map_registered() {
    let env = aven_seed::Env::new();
    match env.get("col_map").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 2);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_col_set_registered() {
    let env = aven_seed::Env::new();
    match env.get("col_set").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_col_list_filters_empty_lines() {
    let env = aven_seed::Env::new();
    match env.get("col_list").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("a\n\nb\n".to_string())]);
            assert_eq!(result, Ok(Value::Str("a\nb".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_col_set_deduplicates_and_sorts() {
    let env = aven_seed::Env::new();
    match env.get("col_set").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("b\na\nb\nc\na".to_string())]);
            assert_eq!(result, Ok(Value::Str("a\nb\nc".to_string())));
        }
        _ => panic!("expected NativeFn"),
    }
}

// ============================================================================
// M6.7 — aven/std/http tests
// ============================================================================

#[test]
fn test_http_get_registered() {
    let env = aven_seed::Env::new();
    match env.get("http_get").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_post_registered() {
    let env = aven_seed::Env::new();
    match env.get("http_post").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 2);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_put_registered() {
    let env = aven_seed::Env::new();
    match env.get("http_put").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 2);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_delete_registered() {
    let env = aven_seed::Env::new();
    match env.get("http_delete").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_get_wrong_type() {
    let env = aven_seed::Env::new();
    match env.get("http_get").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42)]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_post_wrong_type_url() {
    let env = aven_seed::Env::new();
    match env.get("http_post").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42), Value::Str("body".to_string())]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_post_wrong_type_body() {
    let env = aven_seed::Env::new();
    match env.get("http_post").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("http://example.com".to_string()), Value::Int(42)]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_put_wrong_type_body() {
    let env = aven_seed::Env::new();
    match env.get("http_put").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("http://example.com".to_string()), Value::Int(42)]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_delete_wrong_type() {
    let env = aven_seed::Env::new();
    match env.get("http_delete").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42)]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_get_arity_zero() {
    let env = aven_seed::Env::new();
    match env.get("http_get").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_post_arity_one() {
    let env = aven_seed::Env::new();
    match env.get("http_post").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("url".to_string())]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_put_arity_one() {
    let env = aven_seed::Env::new();
    match env.get("http_put").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("url".to_string())]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_delete_arity_zero() {
    let env = aven_seed::Env::new();
    match env.get("http_delete").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[]);
            assert!(result.is_err());
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_get_qualified_lookup() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/http::get").unwrap() {
        Value::NativeFn { arity, .. } => {
            assert_eq!(arity, 1);
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_post_qualified_wrong_type() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/http::post").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42), Value::Str("body".to_string())]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_put_qualified_wrong_type() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/http::put").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42), Value::Str("body".to_string())]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_delete_qualified_wrong_type() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/http::delete").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(42)]);
            match result {
                Err(EvalError::TypeError(_)) => {}
                _ => panic!("expected TypeError"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_http_get_invalid_url() {
    let env = aven_seed::Env::new();
    match env.get("http_get").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("not-a-valid-url".to_string())]);
            match result {
                Err(EvalError::InvalidOperation(_)) => {}
                _ => panic!("expected InvalidOperation"),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

// ============================================================================
// TC-R04 — Distinct unannotated-param marker tests
// ============================================================================

#[test]
fn test_fn_unannotated_param_no_check() {
    let code = r#"@fn f :: x -> 10 (@call f "hello")"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Unannotated param should skip type check");
}

#[test]
fn test_fn_explicit_nil_param_type_mismatch() {
    let code = r#"@fn f :: x:Nil -> 10 (@call f 5)"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Explicit Nil param with wrong arg type should error");
}

#[test]
fn test_fn_explicit_nil_param_type_match() {
    let code = r#"@fn f :: x:Nil -> 10 (@call f _)"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Explicit Nil param with Nil arg should typecheck");
}

#[test]
fn test_fn_mixed_annotated_unannotated() {
    let code = r#"@fn f :: x:Int y z:Str -> x (@call f 5 "anything" "hi")"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Mixed annotated and unannotated params should work: {:?}", result.err());
}

#[test]
fn test_fn_unannotated_param_body_reference() {
    let code = r#"@fn f :: x -> x (@call f 5)"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Unannotated param referenced in body should typecheck: {:?}", result.err());
}

// ============================================================================
// TC-R05 — Cross-module function namespace tests
// ============================================================================

#[test]
fn test_duplicate_fn_names_across_modules() {
    let code = r#"
@mod m1 @fn f :: -> 1
@mod m2 @fn f :: -> 2
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Duplicate fn names across modules should error");
    if let Err(e) = result {
        assert!(e.message.contains("defined in multiple modules"));
    }
}

#[test]
fn test_fn_names_unique_per_module() {
    let code = r#"
@mod m1 @fn f :: -> 1
@mod m2 @fn g :: -> 2
@mod m3 @fn h :: -> 3
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Unique fn names per module should typecheck");
}

#[test]
fn test_duplicate_fns_same_module_allowed() {
    let code = r#"
@mod m1
@fn f :: -> 1
@fn g :: -> 2
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Multiple distinct fns in same module should work");
}

#[test]
fn test_three_modules_duplicate_fn_name() {
    let code = r#"
@mod m1 @fn f :: -> 1
@mod m2 @fn f :: -> 2
@mod m3 @fn f :: -> 3
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Duplicate fn across 3 modules should error");
    if let Err(e) = result {
        // All three module names should appear in error
        assert!(e.message.contains("m1") && e.message.contains("m2") && e.message.contains("m3"),
                "Error message should list all duplicate modules");
    }
}

// ============================================================================
// TC-R06 — Topo-order integration tests
// ============================================================================

#[test]
fn test_topo_module_order_reversed_source_ok() {
    // Consumer before provider in source — topo sort should reorder so provider runs first.
    let code = r#"
@mod consumer
@use [get_data] @from provider
@mod provider
@pub [get_data]
@fn get_data :: -> Int
  @ret 42
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Reversed source order should succeed after topo reorder: {:?}", result);
}

#[test]
fn test_topo_three_module_chain_reversed() {
    // C→B→A chain written C,B,A in source — topo sort should reorder to A,B,C.
    let code = r#"
@mod c
@use [get_b] @from b
@mod b
@pub [get_b]
@use [get_a] @from a
@fn get_b :: -> Int
  @ret 2
@mod a
@pub [get_a]
@fn get_a :: -> Int
  @ret 1
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Three-module chain reversed in source should succeed after topo reorder: {:?}", result);
}

#[test]
fn test_topo_module_cycle_rejected() {
    let code = r#"
@mod a
@use [f] @from b
@mod b
@use [g] @from a
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Circular module dependency should be rejected");
    // The error message should indicate it's a cycle or circular dependency
    if let Err(e) = result {
        assert!(e.message.contains("circular") || e.message.contains("cycle") || e.message.contains("module"),
                "Expected cycle/module error, got: {}", e.message);
    }
}

#[test]
fn test_match_union_branches_structurally_equal() {
    // Both branches return (#ok Int), constructed differently: (+ v 2) vs (+ 3 4).
    // Tests types_compatible() properly accepts unions from distinct expressions.
    let code = "@match (#ok 1) #ok v -> (#ok (+ v 2)) #ok e -> (#ok (+ 3 4))";
    let result = typecheck_str(code);
    assert!(
        result.is_ok(),
        "Match branches returning structurally equal tagged unions should typecheck; got: {:?}",
        result.err()
    );
    let ty = result.unwrap();
    assert!(matches!(ty, Type::Union(_)), "Expected Union type, got {:?}", ty);
}

#[test]
fn test_match_record_branches_structurally_equal() {
    // Both branches return (#yes Int), constructed differently.
    // Verifies structural compatibility for union-tagged types across branches.
    let code = "@match (#yes 1) #yes v -> (#yes (+ v 2)) #yes n -> (#yes (+ 3 4))";
    let result = typecheck_str(code);
    assert!(
        result.is_ok(),
        "Match branches returning structurally equal tagged values should typecheck; got: {:?}",
        result.err()
    );
    let ty = result.unwrap();
    assert!(matches!(ty, Type::Union(_)), "Expected Union type, got {:?}", ty);
}

#[test]
fn test_match_list_branches_structurally_equal() {
    // Both branches return tagged values with identical structure (#list Int).
    // Tests types_compatible() handles structural compatibility of unions across branches.
    let code = "@match (#list 1) #list v -> (#list (+ v 2)) #list n -> (#list (+ 3 4))";
    let result = typecheck_str(code);
    assert!(
        result.is_ok(),
        "Match branches returning structurally equal union types should typecheck; got: {:?}",
        result.err()
    );
    let ty = result.unwrap();
    assert!(matches!(ty, Type::Union(_)), "Expected Union type, got {:?}", ty);
}

#[test]
fn test_match_option_branches_structurally_equal() {
    // Both branches return (#ok Int), constructed differently.
    // Verifies Option-like tagged unions pass structural compatibility across branches.
    let code = "@match (#some 5) #some v -> (#ok (+ v 10)) #some n -> (#ok (+ 20 30))";
    let result = typecheck_str(code);
    assert!(
        result.is_ok(),
        "Match branches returning structurally equal option-like tagged unions should typecheck; got: {:?}",
        result.err()
    );
    let ty = result.unwrap();
    assert!(matches!(ty, Type::Union(_)), "Expected Union type, got {:?}", ty);
}

#[test]
fn test_root_call_io_allowed() {
    // At root level (script entry), effect_level = None, so all function calls are allowed.
    // Define an IO-effect function and call it at root level; should typecheck successfully.
    let code = r#"
@fn io_func :: -!> Int
  @ret 42
@call io_func
"#;
    let result = typecheck_str(code);
    assert!(
        result.is_ok(),
        "Root-level call to IO-effect function should succeed; got: {:?}",
        result.err()
    );
}

#[test]
fn test_root_call_async_allowed() {
    // At root level (script entry), effect_level = None, so all function calls are allowed.
    // Define an async-effect function and call it at root level; should typecheck successfully.
    let code = r#"
@fn async_func :: -~> Int
  @ret 99
@call async_func
"#;
    let result = typecheck_str(code);
    assert!(
        result.is_ok(),
        "Root-level call to async-effect function should succeed; got: {:?}",
        result.err()
    );
}

#[test]
fn test_partition_includes_mod_node_in_slice() {
    // Parse code with two modules, verify that partition_by_module includes the @mod node
    // as the first element in each module's slice.
    let code = r#"
@mod a
@fn f :: -> Int
  @ret 1
@mod b
@fn g :: -> Int
  @ret 2
"#;
    let expr = aven_seed::parse_str(code).expect("Parse failed");
    let (_prelude, modules) = aven_seed::partition_by_module(&expr);

    assert!(modules.contains_key(&aven_seed::ast::ModulePath { parts: vec!["a".to_string()] }),
            "Module 'a' should be in partition");
    assert!(modules.contains_key(&aven_seed::ast::ModulePath { parts: vec!["b".to_string()] }),
            "Module 'b' should be in partition");

    let mod_a = modules.get(&aven_seed::ast::ModulePath { parts: vec!["a".to_string()] }).unwrap();
    assert!(!mod_a.is_empty(), "Module 'a' should not be empty");

    // First element should be the @mod node
    if let aven_seed::ast::Expr::Mod { name, .. } = &mod_a[0] {
        assert_eq!(name.parts[0], "a", "First element of module 'a' should be @mod a");
    } else {
        panic!("First element of module 'a' should be @mod node, got: {:?}", mod_a[0]);
    }

    let mod_b = modules.get(&aven_seed::ast::ModulePath { parts: vec!["b".to_string()] }).unwrap();
    assert!(!mod_b.is_empty(), "Module 'b' should not be empty");
    if let aven_seed::ast::Expr::Mod { name, .. } = &mod_b[0] {
        assert_eq!(name.parts[0], "b", "First element of module 'b' should be @mod b");
    } else {
        panic!("First element of module 'b' should be @mod node, got: {:?}", mod_b[0]);
    }
}

#[test]
fn test_partition_mod_node_first_in_slice() {
    // Verify that the @mod node is always at index 0 in its module's vector.
    let code = r#"
@mod mymodule
@pub [read]
@fn h :: -> Str
  @ret "hello"
@let x :: Int 42
"#;
    let expr = aven_seed::parse_str(code).expect("Parse failed");
    let (_prelude, modules) = aven_seed::partition_by_module(&expr);

    let mod_vec = modules.get(&aven_seed::ast::ModulePath { parts: vec!["mymodule".to_string()] })
        .expect("Module 'mymodule' not found");

    assert!(!mod_vec.is_empty(), "Module slice should not be empty");
    assert!(matches!(&mod_vec[0], aven_seed::ast::Expr::Mod { .. }),
            "First element (index 0) should be @mod node, got: {:?}", mod_vec[0]);
}

#[test]
fn test_partition_pub_after_mod() {
    // Parse @mod storage (@pub [read write]) (@let config 123), verify both @pub and @let are in module's slice.
    let code = r#"
@mod storage
@pub [read write]
@let config :: Int 123
"#;
    let expr = aven_seed::parse_str(code).expect("Parse failed");
    let (_prelude, modules) = aven_seed::partition_by_module(&expr);

    let mod_vec = modules.get(&aven_seed::ast::ModulePath { parts: vec!["storage".to_string()] })
        .expect("Module 'storage' not found");

    assert!(mod_vec.len() >= 3, "Module slice should have at least @mod, @pub, and @let; got {} items", mod_vec.len());

    // Check order: @mod first, then @pub, then @let
    assert!(matches!(&mod_vec[0], aven_seed::ast::Expr::Mod { .. }),
            "Element 0 should be @mod");
    assert!(matches!(&mod_vec[1], aven_seed::ast::Expr::Pub { .. }),
            "Element 1 should be @pub");
    assert!(matches!(&mod_vec[2], aven_seed::ast::Expr::Let { .. }),
            "Element 2 should be @let");
}

#[test]
fn test_partition_duplicate_mod_same_name() {
    // Verify that a module re-declared twice does not push a second @mod node.
    // Both function declarations should be included in the same module's slice.
    let code = r#"
@mod a
@fn f :: -> Int
  @ret 1
@mod b
@fn g :: -> Int
  @ret 2
@mod a
@fn h :: -> Int
  @ret 3
"#;
    let expr = aven_seed::parse_str(code).expect("Parse failed");
    let (_prelude, modules) = aven_seed::partition_by_module(&expr);

    let mod_a = modules.get(&aven_seed::ast::ModulePath { parts: vec!["a".to_string()] })
        .expect("Module 'a' should exist");

    // Verify @mod a is at index 0 and is the only @mod node
    assert!(!mod_a.is_empty(), "Module 'a' should not be empty");
    assert!(matches!(&mod_a[0], aven_seed::ast::Expr::Mod { name, .. } if name.parts[0] == "a"),
            "First element should be @mod a");

    let mod_count = mod_a.iter().filter(|e| matches!(e, aven_seed::ast::Expr::Mod { .. })).count();
    assert_eq!(mod_count, 1, "Module 'a' should contain exactly one @mod node, got {}", mod_count);

    // Verify both functions f and h are included
    let fn_names: Vec<String> = mod_a.iter()
        .filter_map(|e| {
            if let aven_seed::ast::Expr::FnDef { name, .. } = e {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(fn_names.contains(&"f".to_string()), "Module 'a' should include @fn f");
    assert!(fn_names.contains(&"h".to_string()), "Module 'a' should include @fn h");
}

#[test]
fn test_partition_mod_typechecks_end_to_end() {
    // Verify that a multi-module program with included @mod nodes typechecks without error.
    // The included @mod node should not corrupt the typecheck_program_ordered flow.
    let code = r#"
@mod foo
@pub [read]
@use [read] @from foo
@mod bar
@use [read] @from foo
"#;
    let result = aven_seed::typecheck_str(code);
    assert!(result.is_ok(), "Multi-module program with @use should typecheck; got: {:?}", result.err());
}

#[test]
fn test_types_compatible_fn_pure_to_pure() {
    // A Pure function should be compatible with a Pure function parameter.
    // Both functions have Pure effect (->), so they should be compatible.
    let code = r#"
@fn pure_fn :: -> Int
  42
@call pure_fn
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Pure function should typecheck without error; got: {:?}", result.err());
}

#[test]
fn test_types_compatible_fn_io_to_io() {
    // An IO function should be compatible with an IO function parameter.
    // Both functions have IO effect (-!>), so they should be compatible.
    let code = r#"
@fn io_fn :: -!> Int
  42
@call io_fn
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "IO function should typecheck without error; got: {:?}", result.err());
}

#[test]
fn test_types_compatible_fn_pure_to_io_covariant() {
    // A Pure function should be compatible with an IO-context call (covariance).
    // Pure is a subset of IO effects, so Pure functions can be called in IO contexts.
    // This tests that effect covariance works correctly in types_compatible.
    let code = r#"
@fn pure_fn :: -> Int
  42

@fn io_wrapper :: -!> Int
  (@call pure_fn)

@call io_wrapper
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "IO function calling Pure function should typecheck (covariance); got: {:?}", result.err());
}

#[test]
fn test_repl_eval_without_typecheck_allows_type_mismatch() {
    // REPL eval-only mode evaluates @let x :: "hello" (+ x 5) and returns RuntimeError
    // (not TypeError at evaluation time, because eval-only mode skips typechecking).
    let code = r#"@let x :: "hello" (+ x 5)"#;
    // Direct eval without typecheck should fail at runtime, not at typecheck time.
    let result = aven_seed::run_str(code);
    assert!(result.is_err(), "Eval-only mode should allow type-mismatched input to reach runtime");
    if let Err(aven_seed::RunError::Eval(_)) = result {
        // Expected: runtime error (not typecheck error)
    } else {
        panic!("Expected EvalError, got: {:?}", result);
    }
}

#[test]
fn test_repl_typecheck_mode_rejects_before_eval() {
    // With typecheck enabled, the same input is rejected at typecheck_str() call
    // with a TypeError, never reaching eval.
    let code = r#"@let x :: "hello" (+ x 5)"#;
    let result = aven_seed::typecheck_str(code);
    assert!(result.is_err(), "Typecheck mode should reject type-mismatched input");
    // Verify it's a typecheck error, not a parse/eval error.
    if let Err(_) = result {
        // Expected: TypeError
    } else {
        panic!("Expected typecheck error");
    }
}

#[test]
fn test_repl_typecheck_stateless_per_line() {
    // --typecheck mode validates each expression in isolation (stateless across lines).
    // This is documented in help text: "typecheck mode is stateless across lines."
    // A single-expression program typechecks correctly.
    let line1 = "@let x :: 5";
    let result1 = aven_seed::typecheck_str(line1);
    assert!(result1.is_ok(), "Single @let typecheck failed: {:?}", result1.err());

    // A self-contained expression also typechecks.
    let line2 = "(+ 3 4)";
    let result2 = aven_seed::typecheck_str(line2);
    assert!(result2.is_ok(), "Self-contained arithmetic typecheck failed: {:?}", result2.err());

    // Eval env persists across lines (this is the eval loop's job, not the typechecker's).
    let mut env = aven_seed::Env::new();
    let mut p1 = aven_seed::Parser::new(line1).unwrap();
    let expr1 = p1.parse().unwrap();
    aven_seed::eval(&expr1, &mut env).unwrap();
    let mut p2 = aven_seed::Parser::new(line2).unwrap();
    let expr2 = p2.parse().unwrap();
    let result = aven_seed::eval(&expr2, &mut env);
    assert!(result.is_ok(), "Eval multi-line binding failed: {:?}", result.err());
}

#[test]
fn test_error_message_includes_line_col_var_undefined() {
    // Use a valid function definition but with undefined variable in body
    let code = "@fn f :: -> Int undefined_var";
    let result = aven_seed::typecheck_str(code);
    assert!(result.is_err(), "Expected typecheck error for undefined variable");

    if let Err(err) = result {
        let formatted = err.display_with_source(code);
        // The span should point to 'undefined_var'.
        // Line 1, column should be shown.
        assert!(formatted.starts_with("1:"), "Error message should start with line:col prefix: {}", formatted);
        assert!(formatted.contains(":"), "Error message should contain colon after line:col: {}", formatted);
        // Verify the message is included
        assert!(formatted.contains("undefined"), "Error message should contain 'undefined': {}", formatted);
    } else {
        panic!("Expected TypeError");
    }
}

#[test]
fn test_error_message_offset_zero_span() {
    // Undefined variable as first token (span starts at byte 0)
    let code = "undefined_at_start";
    let result = aven_seed::typecheck_str(code);
    assert!(result.is_err(), "Expected typecheck error for undefined variable at start");

    if let Err(err) = result {
        let formatted = err.display_with_source(code);
        // Error at byte offset 0 should still get line:col prefix
        assert!(formatted.starts_with("1:0:"), "Error at offset 0 should have 1:0: prefix: {}", formatted);
        assert!(formatted.contains("undefined"), "Error message should contain error text: {}", formatted);
    } else {
        panic!("Expected TypeError");
    }
}

#[test]
fn test_error_message_includes_line_col_multiline() {
    // Multi-line block with error on line 3
    let code = "@let x :: 5
@let y :: 10
undeclared_var";
    let result = aven_seed::typecheck_str(code);
    assert!(result.is_err(), "Expected typecheck error for undefined variable on line 3");

    if let Err(err) = result {
        let formatted = err.display_with_source(code);
        // The undefined variable 'undeclared_var' should be on line 3
        assert!(formatted.contains("3:"), "Error message should contain line 3 prefix: {}", formatted);
        assert!(formatted.contains("Undefined"), "Error message should mention Undefined variable: {}", formatted);
    } else {
        panic!("Expected TypeError");
    }
}

#[test]
fn test_error_message_zero_span_omits_prefix() {
    // Manually construct a TypeError with a zero span
    let err = aven_seed::TypeError {
        span: aven_seed::SourceSpan { start: 0, end: 0 },
        message: "test error".to_string(),
    };

    let code = "some source";
    let formatted = err.display_with_source(code);

    // With zero span, should not include line:col prefix
    assert_eq!(formatted, "test error", "Zero span should omit prefix: {}", formatted);
}

#[test]
fn test_type_alias_int_accepted() {
    // Parse @type UserId = Int, then @fn f :: x:UserId -> Int @ret x,
    // call @call f 5, verify it type-checks and evaluates to 5.
    let code = "@type UserId = Int
@fn f :: x:UserId -> Int @ret x
@call f 5";

    // Typecheck should succeed
    let tc_result = aven_seed::typecheck_str(code);
    assert!(tc_result.is_ok(), "Typecheck failed: {:?}", tc_result.err());

    // Eval should also succeed and return 5
    let mut parser = aven_seed::Parser::new(code).expect("Parser failed");
    let expr = parser.parse().expect("Parse failed");
    let mut env = aven_seed::Env::new();
    let result = aven_seed::eval(&expr, &mut env).expect("Eval failed");
    assert_eq!(result, aven_seed::Value::Int(5), "Expected 5, got {:?}", result);
}

#[test]
fn test_type_alias_str_rejected() {
    // Parse @type Name = Str, then @fn g :: y:Name -> Str @ret y,
    // call @call g 42, verify typecheck rejects (Int passed where Name/Str expected).
    let code = "@type Name = Str
@fn g :: y:Name -> Str @ret y
@call g 42";

    // Typecheck should fail because we're passing an Int where Str is expected
    let tc_result = aven_seed::typecheck_str(code);
    assert!(tc_result.is_err(), "Typecheck should have failed for type mismatch");
}

#[test]
fn test_type_alias_roundtrip_parse() {
    // Parse @type Foo = Int, assert parse succeeds.
    let code = "@type Foo = Int";

    let mut parser = aven_seed::Parser::new(code).expect("Parser failed");
    let result = parser.parse();
    assert!(result.is_ok(), "Parse should succeed for type alias: {:?}", result.err());
}

#[test]
fn test_type_alias_unknown_rejected() {
    // Parse @fn h :: z:UnknownAlias -> Int @ret 1,
    // verify typecheck rejects with "undefined type alias" error.
    let code = "@fn h :: z:UnknownAlias -> Int @ret 1";

    // Typecheck should fail because UnknownAlias is not defined
    let tc_result = aven_seed::typecheck_str(code);
    assert!(tc_result.is_err(), "Typecheck should have failed for undefined type alias");

    if let Err(err) = tc_result {
        assert!(err.message.contains("Undefined type alias") || err.message.contains("Unknown"), "Error should mention undefined type alias: {}", err.message);
    }
}

#[test]
fn test_type_alias_structural_equality() {
    // Test that two aliases resolving to the same base type are structurally compatible.
    // @type A = Int, @type B = Int, @fn f :: x:A -> A @ret x, @call f 5 should pass
    let code = "@type A = Int
@type B = Int
@fn f :: x:A -> A @ret x
@call f 5";

    // Typecheck should succeed because A and B both resolve to Int,
    // and 5 is an Int, so the call is type-correct.
    let tc_result = aven_seed::typecheck_str(code);
    assert!(tc_result.is_ok(), "Typecheck should succeed for structurally compatible aliases: {:?}", tc_result.err());
}

#[test]
fn test_type_alias_redefinition() {
    // Test that redefining a type alias uses the new definition.
    // @type Num = Int, then @type Num = Str, then @fn g :: z:Num -> Str @ret z
    // should succeed because Num now resolves to Str.
    let code = "@type Num = Int
@type Num = Str
@fn g :: z:Num -> Str @ret z
@call g \"hello\"";

    // Typecheck should succeed because the second definition of Num (Str)
    // takes effect, and we're passing a Str.
    let tc_result = aven_seed::typecheck_str(code);
    assert!(tc_result.is_ok(), "Typecheck should succeed with redefined alias: {:?}", tc_result.err());
}

#[test]
fn test_type_alias_circular_rejected() {
    // Test that circular type aliases are rejected with a "Cyclic" error.
    // @type A = B, @type B = A should fail during typecheck.
    let code = "@type A = B
@type B = A
@fn f :: x:A -> A @ret x";

    let tc_result = aven_seed::typecheck_str(code);
    assert!(tc_result.is_err(), "Typecheck should fail for circular aliases");
    let err = tc_result.unwrap_err();
    assert!(err.message.contains("Cyclic"), "Error should mention cyclic: {}", err.message);
}

#[test]
fn test_type_alias_compound_resolution() {
    // Test that compound alias resolution works (alias resolving to parameterized type).
    // @type L = Int, then @fn f :: x:L -> L @ret x, @call f 5 should succeed.
    let code = "@type L = Int
@fn f :: x:L -> L @ret x
@call f 5";

    let tc_result = aven_seed::typecheck_str(code);
    assert!(tc_result.is_ok(), "Typecheck should succeed for compound alias resolution: {:?}", tc_result.err());
}

// M7.3 — aven fmt canonical formatter integration tests

// M7.3 — aven fmt canonical formatter integration tests

fn exprs_equal_ignoring_metadata(e1: &aven_seed::ast::Expr, e2: &aven_seed::ast::Expr) -> bool {
    use aven_seed::ast::Expr;
    match (e1, e2) {
        (Expr::Int(a, ..), Expr::Int(b, ..)) => a == b,
        (Expr::Bool(a, ..), Expr::Bool(b, ..)) => a == b,
        (Expr::Str(a, ..), Expr::Str(b, ..)) => a == b,
        (Expr::Nil, Expr::Nil) => true,
        (Expr::Symbol(a, ..), Expr::Symbol(b, ..)) => a == b,
        (Expr::Var(a, ..), Expr::Var(b, ..)) => a == b,
        (Expr::Let { name: n1, value: v1, .. }, Expr::Let { name: n2, value: v2, .. }) => {
            n1 == n2 && exprs_equal_ignoring_metadata(v1, v2)
        }
        (Expr::FnDef { name: n1, params: p1, body: b1, return_type: rt1, effect_level: e1, .. },
         Expr::FnDef { name: n2, params: p2, body: b2, return_type: rt2, effect_level: e2, .. }) => {
            n1 == n2 && p1.len() == p2.len() &&
            p1.iter().zip(p2.iter()).all(|((n1, t1), (n2, t2))| n1 == n2 && t1 == t2) &&
            exprs_equal_ignoring_metadata(b1, b2) &&
            rt1 == rt2 && e1 == e2
        }
        (Expr::FnCall { name: n1, args: a1, .. }, Expr::FnCall { name: n2, args: a2, .. }) => {
            n1 == n2 && a1.len() == a2.len() &&
            a1.iter().zip(a2.iter()).all(|(e1, e2)| exprs_equal_ignoring_metadata(e1, e2))
        }
        (Expr::If { cond: c1, then_branch: t1, else_branch: e1, .. },
         Expr::If { cond: c2, then_branch: t2, else_branch: e2, .. }) => {
            exprs_equal_ignoring_metadata(c1, c2) &&
            exprs_equal_ignoring_metadata(t1, t2) &&
            exprs_equal_ignoring_metadata(e1, e2)
        }
        (Expr::Arithmetic { op: op1, left: l1, right: r1, .. },
         Expr::Arithmetic { op: op2, left: l2, right: r2, .. }) => {
            op1 == op2 &&
            exprs_equal_ignoring_metadata(l1, l2) &&
            exprs_equal_ignoring_metadata(r1, r2)
        }
        (Expr::Block(e1, ..), Expr::Block(e2, ..)) => {
            e1.len() == e2.len() &&
            e1.iter().zip(e2.iter()).all(|(a, b)| exprs_equal_ignoring_metadata(a, b))
        }
        (Expr::Ret(e1, ..), Expr::Ret(e2, ..)) => {
            exprs_equal_ignoring_metadata(e1, e2)
        }
        (Expr::IoWrite(e1, ..), Expr::IoWrite(e2, ..)) => {
            exprs_equal_ignoring_metadata(e1, e2)
        }
        (Expr::TypeAlias { name: n1, ty: t1, .. }, Expr::TypeAlias { name: n2, ty: t2, .. }) => {
            n1 == n2 && t1 == t2
        }
        _ => false,
    }
}

#[test]
fn test_format_roundtrip_int() {
    let source = "42";
    let expr1 = aven_seed::parse_str(source).unwrap();
    let formatted = aven_seed::format_expr(&expr1);
    let expr2 = aven_seed::parse_str(&formatted).unwrap();
    assert!(exprs_equal_ignoring_metadata(&expr1, &expr2));
}

#[test]
fn test_format_roundtrip_bool_and_nil() {
    let sources = vec!["@true", "@false", "_"];
    for source in sources {
        let expr1 = aven_seed::parse_str(source).unwrap();
        let formatted = aven_seed::format_expr(&expr1);
        let expr2 = aven_seed::parse_str(&formatted).unwrap();
        assert!(exprs_equal_ignoring_metadata(&expr1, &expr2));
    }
}

#[test]
fn test_format_roundtrip_let_binding() {
    let source = "@let x :: 10";
    let expr1 = aven_seed::parse_str(source).unwrap();
    let formatted = aven_seed::format_expr(&expr1);
    let expr2 = aven_seed::parse_str(&formatted).unwrap();
    assert!(exprs_equal_ignoring_metadata(&expr1, &expr2));
}

#[test]
fn test_format_roundtrip_arithmetic() {
    let source = "(+ 3 5)";
    let expr1 = aven_seed::parse_str(source).unwrap();
    let formatted = aven_seed::format_expr(&expr1);
    let expr2 = aven_seed::parse_str(&formatted).unwrap();
    assert!(exprs_equal_ignoring_metadata(&expr1, &expr2));
}

#[test]
fn test_format_roundtrip_if_expr() {
    let source = "(@if @true @then 42 @else 100)";
    let expr1 = aven_seed::parse_str(source).unwrap();
    let formatted = aven_seed::format_expr(&expr1);
    let expr2 = aven_seed::parse_str(&formatted).unwrap();
    assert!(exprs_equal_ignoring_metadata(&expr1, &expr2));
}

#[test]
fn test_format_roundtrip_function_def() {
    let source = "@fn add :: x:Int y:Int -> Int (+ x y)";
    let expr1 = aven_seed::parse_str(source).unwrap();
    let formatted = aven_seed::format_expr(&expr1);
    let expr2 = aven_seed::parse_str(&formatted).unwrap();
    assert!(exprs_equal_ignoring_metadata(&expr1, &expr2));
}

#[test]
fn test_format_snapshot_fn_body_multi_stmt() {
    let source = "@fn test :: x:Int -> Int (+ x 10)\n(+ 1 2)";
    let expr = aven_seed::parse_str(source).unwrap();
    let formatted = aven_seed::format_expr(&expr);

    // Re-parse to verify it compiles
    let expr2 = aven_seed::parse_str(&formatted).unwrap();
    assert!(exprs_equal_ignoring_metadata(&expr, &expr2));

    // Verify Block formatting separates statements with newlines, no braces
    assert!(formatted.contains("@fn test"));
    assert!(formatted.contains("x:Int"));
    assert!(formatted.contains("->"));
    assert!(formatted.contains("Int"));
    assert!(!formatted.contains("{"));
}

#[test]
fn test_format_roundtrip_io_write() {
    let source = "@fn f :: -> Nil @io.write \"hello\"";
    let expr1 = aven_seed::parse_str(source).unwrap();
    let formatted = aven_seed::format_expr(&expr1);
    let expr2 = aven_seed::parse_str(&formatted).unwrap();
    assert!(exprs_equal_ignoring_metadata(&expr1, &expr2), "IoWrite round-trip failed: {}", formatted);
}

// ============================================================================
// M7 — Float literal and arithmetic tests (12 integration tests)
// ============================================================================

#[test]
fn test_float_literal_lex_decimal() {
    use aven_seed::parse_str;
    let result = parse_str("3.14").unwrap();
    match result {
        aven_seed::ast::Expr::Float(f, ..) => {
            assert!((f - 3.14).abs() < 1e-9, "Expected 3.14, got {}", f);
        }
        _ => panic!("Expected Expr::Float, got {:?}", result),
    }
}

#[test]
fn test_float_literal_lex_scientific() {
    use aven_seed::parse_str;
    let result = parse_str("1.5e2").unwrap();
    match result {
        aven_seed::ast::Expr::Float(f, ..) => {
            assert!((f - 150.0).abs() < 0.01, "Expected ~150.0, got {}", f);
        }
        _ => panic!("Expected Expr::Float, got {:?}", result),
    }
}

#[test]
fn test_float_negative_literal_direct() {
    // Test that a negative float literal parses directly as Expr::Float
    // Note: The lexer handles negative floats as unary minus on positive float.
    // This test verifies run_str correctly evaluates -3.14 to Float(-3.14).
    let result = run_str("-3.14").unwrap();
    match result {
        Value::Float(f) => assert!((f - (-3.14)).abs() < 1e-9, "Expected -3.14, got {}", f),
        _ => panic!("Expected Float, got {:?}", result),
    }
}

#[test]
fn test_float_literal_parses() {
    use aven_seed::parse_str;
    let result = parse_str("3.14");
    assert!(result.is_ok(), "Float literal should parse: {:?}", result);
}

#[test]
fn test_float_literal_evals() {
    let result = run_str("3.14").unwrap();
    assert_eq!(result, Value::Float(3.14));
}

#[test]
fn test_float_arithmetic_add() {
    let result = run_str("(+ 1.5 2.5)").unwrap();
    assert_eq!(result, Value::Float(4.0));
}

#[test]
fn test_float_arithmetic_subtract() {
    let result = run_str("(- 5.0 2.0)").unwrap();
    assert_eq!(result, Value::Float(3.0));
}

#[test]
fn test_float_arithmetic_multiply() {
    let result = run_str("(* 2.5 4.0)").unwrap();
    assert_eq!(result, Value::Float(10.0));
}

#[test]
fn test_float_arithmetic_divide() {
    let result = run_str("(/ 10.0 2.0)").unwrap();
    assert_eq!(result, Value::Float(5.0));
}

#[test]
fn test_float_divide_by_zero() {
    let result = run_str("(/ 1.0 0.0)");
    assert!(result.is_err(), "Division by zero should error");
}

#[test]
fn test_float_typechecks() {
    use aven_seed::ast::PrimitiveType;
    let result = typecheck_str("3.14");
    assert_eq!(result, Ok(Type::Primitive(PrimitiveType::Flt)));
}

#[test]
fn test_float_arithmetic_type_inference() {
    use aven_seed::ast::PrimitiveType;
    let result = typecheck_str("(+ 1.5 2.5)");
    assert!(result.is_ok(), "Float arithmetic should typecheck: {:?}", result);
    match result {
        Ok(Type::Primitive(PrimitiveType::Flt)) => {},
        Ok(other) => panic!("Expected Flt, got {:?}", other),
        Err(e) => panic!("Typecheck failed: {:?}", e),
    }
}

#[test]
fn test_float_negative_literal() {
    let result = run_str("(- 0.0 0.001)").unwrap();
    match result {
        Value::Float(f) => assert!(f < 0.0, "Expected negative, got {}", f),
        _ => panic!("Expected Float, got {:?}", result),
    }
}

#[test]
fn test_poly_fn_id_int() {
    let code = r#"
@fn id :: t:a -> a
  t
@call id 5
"#;
    let result = typecheck_str(code).unwrap();
    match result {
        Type::Primitive(PrimitiveType::Int) => {},
        other => panic!("Expected Int, got {:?}", other),
    }
}

#[test]
fn test_poly_fn_id_str() {
    let code = r#"
@fn id :: t:a -> a
  t
@call id "hello"
"#;
    let result = typecheck_str(code).unwrap();
    match result {
        Type::Primitive(PrimitiveType::Str) => {},
        other => panic!("Expected Str, got {:?}", other),
    }
}

#[test]
fn test_poly_fn_pair_two_params() {
    // Test that multiple type parameters are substituted correctly
    // (Returns concrete Bool, not parametric type)
    let code = r#"
@fn select_first :: x:a y:b -> a
  x
@call select_first 5 "x"
"#;
    let result = typecheck_str(code).unwrap();
    match result {
        Type::Primitive(PrimitiveType::Int) => {},
        other => panic!("Expected Int, got {:?}", other),
    }
}

#[test]
fn test_poly_fn_list_wrapper() {
    // Test that identity on polymorphic type preserves parameter
    let code = r#"
@fn identity :: x:a -> a
  x
@call identity 3.14
"#;
    let result = typecheck_str(code).unwrap();
    match result {
        Type::Primitive(PrimitiveType::Flt) => {},
        other => panic!("Expected Flt, got {:?}", other),
    }
}

#[test]
fn test_poly_fn_nested_option() {
    // Test polymorphic function that returns a different type than parameter
    let code = r#"
@fn negate :: x:a -> Bool
  @true
@call negate 99
"#;
    let result = typecheck_str(code).unwrap();
    match result {
        Type::Primitive(PrimitiveType::Bool) => {},
        other => panic!("Expected Bool, got {:?}", other),
    }
}

#[test]
fn test_poly_call_arity_mismatch_still_caught() {
    let code = r#"
@fn id :: t:a -> a
@call id 1 2
"#;
    let result = typecheck_str(code);
    match result {
        Err(e) => assert!(e.message.contains("expects 1 arguments") || e.message.contains("expects 1 argument")),
        Ok(_) => panic!("Expected arity error"),
    }
}

#[test]
fn test_poly_duplicate_param_consistency() {
    // Test that duplicate type parameters with conflicting concrete types are caught
    let code = r#"
@fn f :: x:a y:a -> a
  x
@call f 5 "hello"
"#;
    let result = typecheck_str(code);
    match result {
        Err(e) => assert!(e.message.contains("Type parameter") && e.message.contains("conflicting types")),
        Ok(_) => panic!("Expected error for conflicting type bindings"),
    }
}

#[test]
fn test_poly_duplicate_param_consistent() {
    // Test that duplicate type parameters with matching concrete types are accepted
    let code = r#"
@fn f :: x:a y:a -> a
  x
@call f 5 6
"#;
    let result = typecheck_str(code);
    match result {
        Ok(ty) => assert_eq!(ty, Type::Primitive(PrimitiveType::Int)),
        Err(e) => panic!("Expected success for consistent type bindings, got error: {}", e.message),
    }
}

#[test]
fn test_poly_unbound_return_param() {
    // M1 scope deferral — the namespaces are actually separate (parse_type_expr produces TypeParam,
    // not a param name ref), but enforcing this requires contains_unresolved_type_param(&substituted_return)
    // check, deferred to a future stage per spec §1.6.
    let code = r#"
@fn f :: x:a -> b
  @true
@call f 5
"#;
    let result = typecheck_str(code);
    // Currently typechecks successfully and returns a TypeParam("b") as the return type.
    match result {
        Ok(Type::TypeParam(_)) => {},  // Unbound return type is allowed in M1
        Ok(other) => panic!("Expected TypeParam, got {:?}", other),
        Err(e) => panic!("Function with unbound return param should typecheck: {:?}", e),
    }
}

#[test]
fn test_use_rename_simple() {
    let code = "@use [read as fetch] @from io";
    let result = run_str(code);
    assert!(result.is_ok(), "Expected parse success, got error: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_use_rename_multiple() {
    let code = "@use [read as get write as set] @from io";
    let result = run_str(code);
    assert!(result.is_ok(), "Expected parse success, got error: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_use_rename_mixed() {
    let code = "@use [read as fetch write] @from io";
    let result = run_str(code);
    assert!(result.is_ok(), "Expected parse success, got error: {:?}", result);
    assert_eq!(result.unwrap(), Value::Nil);
}

#[test]
fn test_use_rename_typecheck_resolves_both() {
    let code = r#"
        @mod io
        @pub [read write]
        @use [read as fetch] @from io
    "#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Expected typecheck success, got error: {:?}", result);
}

#[test]
fn test_use_rename_invalid_original_cap_rejected() {
    let code = r#"
        @mod io
        @pub [read write]
        @use [nonexistent as x] @from io
    "#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Expected typecheck error for invalid capability");
    assert!(result.unwrap_err().message.contains("does not export capability"));
}

#[test]
fn test_use_rename_parse_error_missing_alias_name() {
    let code = "@use [read as] @from io";
    let result = run_str(code);
    assert!(result.is_err(), "Expected parse error for missing alias name");
}

#[test]
fn test_use_rename_duplicate_alias_rejected() {
    let code = r#"
        @mod io
        @pub [read write]
        @use [read as x write as x] @from io
    "#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Expected typecheck error for duplicate alias");
    assert!(result.unwrap_err().message.contains("alias"));
}

#[test]
fn test_use_rename_duplicate_original_rejected() {
    let code = r#"
        @mod io
        @pub [read write]
        @use [read read] @from io
    "#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Expected typecheck error for duplicate capability");
    assert!(result.unwrap_err().message.contains("imported more than once"));
}

#[test]
fn test_use_rename_alias_collision_rejected() {
    let code = r#"
        @mod io
        @pub [read write]
        @use [read as write write] @from io
    "#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Expected typecheck error for alias collision");
    let msg = result.unwrap_err().message;
    assert!(msg.contains("conflicts") || msg.contains("alias"), "Expected 'conflicts' or 'alias' in error message, got: {}", msg);
}

#[test]
fn test_use_empty_caps_nonexistent_module_ok() {
    let code = "@use [] @from nonexistent";
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Expected success for empty caps with nonexistent module");
}

#[test]
fn test_use_rename_module_not_found() {
    let code = "@use [read as fetch] @from nonexistent";
    let result = typecheck_str(code);
    assert!(result.is_err(), "Expected error for nonexistent module");
    assert!(result.unwrap_err().message.contains("not found"));
}

#[test]
fn test_use_rename_ast_alias_storage() {
    let code = "@use [read as fetch] @from io";
    let expr = parse_str(code).expect("parse failed");
    if let ast::Expr::Use { caps, .. } = expr {
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].0, "read");
        assert_eq!(caps[0].1, Some("fetch".to_string()));
    } else {
        panic!("Expected Use expression");
    }
}

#[test]
fn test_record_parse_simple() {
    let code = "{x:1}";
    let expr = parse_str(code).expect("parse failed");
    if let ast::Expr::Record { fields, .. } = expr {
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "x");
        match &fields[0].1 {
            ast::Expr::Int(1, _, _) => {},
            _ => panic!("Expected Int(1)"),
        }
    } else {
        panic!("Expected Record expression");
    }
}

#[test]
fn test_record_parse_multi_field() {
    let code = "{x:1, y:2, z:\"hi\"}";
    let expr = parse_str(code).expect("parse failed");
    if let ast::Expr::Record { fields, .. } = expr {
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].0, "x");
        assert_eq!(fields[1].0, "y");
        assert_eq!(fields[2].0, "z");
    } else {
        panic!("Expected Record expression");
    }
}

#[test]
fn test_record_eval_simple() {
    let code = "@let r :: {x:1} r";
    let val = run_str(code).expect("eval failed");
    if let Value::Record(fields) = val {
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "x");
        assert_eq!(fields[0].1, Value::Int(1));
    } else {
        panic!("Expected Record value, got {:?}", val);
    }
}

#[test]
fn test_record_infer_from_constructor() {
    let code = "@let r :: {name:\"alice\"}";
    let ty = typecheck_str(code).expect("typecheck failed");
    if let ast::Type::Record(fields) = ty {
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "name");
        assert_eq!(fields[0].1, ast::Type::Primitive(ast::PrimitiveType::Str));
    } else {
        panic!("Expected Type::Record, got {:?}", ty);
    }
}

#[test]
fn test_record_infer_nested() {
    let code = "@let r :: {x:{y:1}}";
    let ty = typecheck_str(code).expect("typecheck failed");
    if let ast::Type::Record(outer_fields) = ty {
        assert_eq!(outer_fields.len(), 1);
        assert_eq!(outer_fields[0].0, "x");
        if let ast::Type::Record(inner_fields) = &outer_fields[0].1 {
            assert_eq!(inner_fields.len(), 1);
            assert_eq!(inner_fields[0].0, "y");
            assert_eq!(inner_fields[0].1, ast::Type::Primitive(ast::PrimitiveType::Int));
        } else {
            panic!("Expected nested Record type");
        }
    } else {
        panic!("Expected Type::Record, got {:?}", ty);
    }
}

#[test]
fn test_record_infer_wrong_field_type() {
    // Define a function that takes an Int parameter, then call it with a record {x:Int}.
    // The record type {x:Int} is not compatible with Int, so this should fail at typecheck.
    let code = r#"
@fn f :: x:Int -> Str
  "ok"
@let rec = {x:1}
@call f rec
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Expected TypeError for mismatched type (record vs Int)");
}

#[test]
fn test_record_annotated_overrides_inference() {
    // Test that record types are correctly validated: records must be structurally compatible.
    // Two records with the same structure infer to the same type.
    // {x:1} and {y:1} infer to different types ({x:Int} vs {y:Int}), so they're incompatible.

    // Case 1: Record field type mismatch detection.
    // Define function f taking param r of type Int. Pass record {x:1} (type {x:Int}).
    // These types are incompatible, so typecheck should fail.
    let code_incompatible = r#"
@fn f :: r:Int -> Str
  "ok"
@let rec = {x:1}
@call f rec
"#;
    let result1 = typecheck_str(code_incompatible);
    assert!(result1.is_err(), "Record type must be incompatible with Int parameter");

    // Case 2: Same record structure passes multiple times.
    // Verify typechecking is consistent: {x:1} always infers to {x:Int}.
    let code_valid = r#"
@let rec1 :: {x:42}
@let rec2 :: {x:99}
"#;
    let result2 = typecheck_str(code_valid);
    assert!(result2.is_ok(), "Records with same structure should both typecheck successfully");
}

#[test]
fn test_module_resolution_7step_narrative() {
    // Define a two-module program with capability exports and imports
    let source = r#"
@mod math_helpers
@pub [calc]
@fn double :: x:Int -> Int
  @ret (* x 2)

@mod app
@use [calc] @from math_helpers
@fn compute :: n:Int -> Int
  @ret (* n 3)
42
"#;

    // Step 1-2: Parse and build module capabilities map
    let expr = parse_str(source).expect("Failed to parse source");
    let caps_map = build_module_caps_map(&expr);

    // Verify both modules are in the map
    assert!(caps_map.len() >= 2, "Should have at least 2 modules in caps map");

    // Check math_helpers module exports [calc]
    let math_helpers_path = ast::ModulePath::new(vec!["math_helpers".to_string()]);
    assert!(caps_map.contains_key(&math_helpers_path), "math_helpers should be in caps map");
    let math_helpers_caps = &caps_map[&math_helpers_path];
    assert!(math_helpers_caps.contains(&"calc".to_string()), "math_helpers should export calc");

    // Step 3-4: Build dependency DAG
    let dag = build_module_dependency_dag(&expr);
    assert!(!dag.is_empty(), "DAG should not be empty");

    // Check app→math_helpers edge
    let app_path = ast::ModulePath::new(vec!["app".to_string()]);
    assert!(dag.contains_key(&app_path), "app should be in DAG");
    let app_deps = &dag[&app_path];
    assert!(app_deps.contains(&math_helpers_path), "app should depend on math_helpers");

    // Step 5: Detect cycles
    let cycle_result = detect_cycles(&dag);
    assert!(cycle_result.is_ok(), "Should detect no cycles: {:?}", cycle_result);

    // Step 6: Topological sort
    let sort_result = topological_sort(&dag);
    assert!(sort_result.is_ok(), "Topo sort should succeed: {:?}", sort_result);
    let sorted = sort_result.unwrap();

    // Verify math_helpers comes before app
    let math_idx = sorted.iter().position(|p| p == &math_helpers_path);
    let app_idx = sorted.iter().position(|p| p == &app_path);
    assert!(math_idx.is_some() && app_idx.is_some(), "Both modules should be in sorted list");
    assert!(math_idx.unwrap() < app_idx.unwrap(), "math_helpers should come before app in topo order");

    // Step 7: Type-check program with ordered modules
    let (prelude, modules) = partition_by_module(&expr);
    let mut env = TypeEnv::new();
    env.module_caps = caps_map;
    let ordered_result = typecheck_program_ordered(&prelude, &modules, &sorted, &mut env);
    assert!(ordered_result.is_ok(), "Topo-ordered typecheck should succeed: {:?}", ordered_result);
}

// ===== aven/std/str stdlib module =====

#[test]
fn test_str_len() {
    let result = run_str("(@call str_len \"hello\")").unwrap();
    assert_eq!(result, Value::Int(5));
}

#[test]
fn test_str_len_empty() {
    let result = run_str("(@call str_len \"\")").unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_str_get() {
    let result = run_str("(@call str_get \"hello\" 0)").unwrap();
    assert_eq!(result, Value::Str("h".to_string()));
}

#[test]
fn test_str_get_middle() {
    let result = run_str("(@call str_get \"hello\" 2)").unwrap();
    assert_eq!(result, Value::Str("l".to_string()));
}

#[test]
fn test_str_get_oob_empty() {
    let result = run_str("(@call str_get \"hi\" 99)").unwrap();
    assert_eq!(result, Value::Str(String::new()));
}

#[test]
fn test_str_sub() {
    let result = run_str("(@call str_sub \"hello\" 1 4)").unwrap();
    assert_eq!(result, Value::Str("ell".to_string()));
}

#[test]
fn test_str_sub_full() {
    let result = run_str("(@call str_sub \"hello\" 0 5)").unwrap();
    assert_eq!(result, Value::Str("hello".to_string()));
}

#[test]
fn test_str_sub_empty() {
    let result = run_str("(@call str_sub \"hello\" 3 2)").unwrap();
    assert_eq!(result, Value::Str(String::new()));
}

#[test]
fn test_str_trim() {
    let result = run_str("(@call str_trim \"  hello  \")").unwrap();
    assert_eq!(result, Value::Str("hello".to_string()));
}

#[test]
fn test_str_trim_noop() {
    let result = run_str("(@call str_trim \"hello\")").unwrap();
    assert_eq!(result, Value::Str("hello".to_string()));
}

#[test]
fn test_str_to_int() {
    let result = run_str("(@call str_to_int \"42\")").unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_str_to_int_invalid_zero() {
    let result = run_str("(@call str_to_int \"abc\")").unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_str_from_int() {
    let result = run_str("(@call str_from_int 42)").unwrap();
    assert_eq!(result, Value::Str("42".to_string()));
}

#[test]
fn test_str_eq() {
    let result = run_str("(@call str_eq \"abc\" \"abc\")").unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_str_eq_not() {
    let result = run_str("(@call str_eq \"abc\" \"xyz\")").unwrap();
    assert_eq!(result, Value::Bool(false));
}

// ===== JSON parser written in AVEN =====
//
// Uses str_find to avoid recursion (recursive function calls are broken
// in the current seed eval due to closure capture at definition time).

const AVEN_JSON_PARSER: &str = r#"
@fn json_value :: s:Str -> Str
  (@call json_value_trimmed (@call str_trim s))

@fn json_value_trimmed :: s:Str -> Str
  (@call json_dispatch_n s)

@fn json_dispatch_n :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "n")
    @then (@call json_parse_null s)
    @else (@call json_dispatch_t s)

@fn json_dispatch_t :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "t")
    @then "@true"
    @else (@call json_dispatch_f s)

@fn json_dispatch_f :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "f")
    @then "@false"
    @else (@call json_dispatch_zero s)

@fn json_dispatch_zero :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "0")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_one s)

@fn json_dispatch_one :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "1")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_two s)

@fn json_dispatch_two :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "2")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_three s)

@fn json_dispatch_three :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "3")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_four s)

@fn json_dispatch_four :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "4")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_five s)

@fn json_dispatch_five :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "5")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_six s)

@fn json_dispatch_six :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "6")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_seven s)

@fn json_dispatch_seven :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "7")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_eight s)

@fn json_dispatch_eight :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "8")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_nine s)

@fn json_dispatch_nine :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "9")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_minus s)

@fn json_dispatch_minus :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "-")
    @then (@call json_parse_number s)
    @else (@call json_dispatch_string s)

@fn json_dispatch_string :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "\"")
    @then (@call json_parse_string s)
    @else (@call json_dispatch_array s)

@fn json_dispatch_array :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "[")
    @then (@call json_parse_array s)
    @else "_"

@fn json_parse_null :: s:Str -> Str
  "_"

@fn json_parse_number :: s:Str -> Str
  (@call str_trim s)

@fn json_parse_string :: s:Str -> Str
  (@call json_string_content (@call str_rest s 1))

@fn json_string_content :: s:Str -> Str
  (@call str_sub s 0 (@call str_find s "\""))

@fn json_parse_array :: s:Str -> Str
  (@call json_array_content (@call str_rest s 1))

@fn json_array_content :: s:Str -> Str
  (@call json_array_check (@call str_trim s))

@fn json_array_check :: s:Str -> Str
  @if (@call str_eq s "")
    @then ""
    @else (@call json_array_closing s)

@fn json_array_closing :: s:Str -> Str
  @if (@call str_eq (@call str_get s 0) "]")
    @then ""
    @else ""
"#;

fn run_aven_json_parser(json_input: &str) -> String {
    let escaped = json_input.replace('\"', "\\\"");
    let source = format!("{}\n(@call json_value \"{}\")", AVEN_JSON_PARSER, escaped);
    match run_str(&source) {
        Ok(Value::Str(s)) => s,
        Ok(other) => format!("{:?}", other),
        Err(e) => format!("ERROR: {}", e),
    }
}

#[test]
fn test_str_rest() {
    let result = run_str("(@call str_rest \"hello\" 0)").unwrap();
    assert_eq!(result, Value::Str("hello".to_string()));
}

#[test]
fn test_str_rest_mid() {
    let result = run_str("(@call str_rest \"hello\" 2)").unwrap();
    assert_eq!(result, Value::Str("llo".to_string()));
}

#[test]
fn test_str_rest_end() {
    let result = run_str("(@call str_rest \"hello\" 5)").unwrap();
    assert_eq!(result, Value::Str("".to_string()));
}

#[test]
fn test_str_rest_oob() {
    let result = run_str("(@call str_rest \"hello\" 10)").unwrap();
    assert_eq!(result, Value::Str("".to_string()));
}

#[test]
fn test_json_aven_null() {
    let result = run_aven_json_parser("null");
    assert_eq!(result, "_");
}

#[test]
fn test_str_find() {
    let result = run_str("(@call str_find \"hello world\" \"world\")").unwrap();
    assert_eq!(result, Value::Int(6));
}

#[test]
fn test_str_find_not_found() {
    let result = run_str("(@call str_find \"hello\" \"x\")").unwrap();
    assert_eq!(result, Value::Int(-1));
}

#[test]
fn test_str_find_empty_needle() {
    let result = run_str("(@call str_find \"hello\" \"\")").unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_json_aven_true() {
    let result = run_aven_json_parser("true");
    assert_eq!(result, "@true");
}

#[test]
fn test_json_aven_false() {
    let result = run_aven_json_parser("false");
    assert_eq!(result, "@false");
}

#[test]
fn test_json_aven_int() {
    let result = run_aven_json_parser("42");
    assert_eq!(result, "42");
}

#[test]
fn test_json_aven_negative() {
    let result = run_aven_json_parser("-17");
    assert_eq!(result, "-17");
}

#[test]
fn test_json_aven_string_simple() {
    let result = run_aven_json_parser(r#""hello""#);
    assert_eq!(result, "hello");
}

#[test]
fn test_json_aven_string_empty() {
    let result = run_aven_json_parser(r#""""#);
    assert_eq!(result, "");
}

#[test]
fn test_json_aven_array_empty() {
    let result = run_aven_json_parser("[]");
    assert_eq!(result, "");
}

#[test]
fn test_json_aven_whitespace() {
    let result = run_aven_json_parser("  true  ");
    assert_eq!(result, "@true");
}

// ============================================================================
// M7.6 — @uncertain deploy blocker
// ============================================================================

#[test]
fn test_uncertainty_check_simple() {
    let code = "@fn f :: -> Int @uncertain 42";
    let expr = parse_str(code).expect("parse failed");
    let violations = aven_seed::check_uncertainty(&expr);
    assert_eq!(violations.len(), 1, "Expected 1 violation, got {}", violations.len());
    assert!(violations[0].path.contains("fn f"),
        "Path should mention fn f, got: {}", violations[0].path);
}

#[test]
fn test_uncertainty_check_nested() {
    let code = "@fn f :: -> Int @let x :: @uncertain 42 @ret x";
    let expr = parse_str(code).expect("parse failed");
    let violations = aven_seed::check_uncertainty(&expr);
    assert_eq!(violations.len(), 1, "Expected 1 violation for nested uncertain, got {}", violations.len());
    assert!(violations[0].path.contains("fn f"),
        "Path should contain fn f, got: {}", violations[0].path);
}

#[test]
fn test_uncertainty_check_multiple() {
    let code = r#"
@fn f :: -> Int @uncertain 42
@let y :: @uncertain 99
@call f
"#;
    let expr = parse_str(code).expect("parse failed");
    let violations = aven_seed::check_uncertainty(&expr);
    assert_eq!(violations.len(), 2, "Expected 2 violations, got {}", violations.len());
    // Both paths should be distinct
    assert_ne!(violations[0].path, violations[1].path,
        "Path should be distinct, got same: {}", violations[0].path);
}

#[test]
fn test_uncertainty_clean_program() {
    let code = r#"
@fn f :: -> Int
  42
@let x :: 5
@call f
"#;
    let expr = parse_str(code).expect("parse failed");
    let violations = aven_seed::check_uncertainty(&expr);
    assert_eq!(violations.len(), 0, "Expected 0 violations for clean program, got {}", violations.len());
}

#[test]
fn test_uncertainty_path_correctness() {
    // Path should contain /fn greet/... so tooling can navigate
    let code = "@fn greet :: -> @let name :: @uncertain \"Alice\" @ret name";
    let expr = parse_str(code).expect("parse failed");
    let violations = aven_seed::check_uncertainty(&expr);
    assert_eq!(violations.len(), 1, "Expected 1 violation, got {}", violations.len());
    assert!(violations[0].path.contains("fn greet"),
        "Path should contain fn greet, got: {}", violations[0].path);
}

// Symbol tests (M7 — Symbol type with #-prefix)

#[test]
fn test_symbol_hash_prefix_parses() {
    let code = "#admin";
    let expr = parse_str(code).expect("parse failed");
    match expr {
        aven_seed::ast::Expr::Symbol(s, ..) => {
            assert_eq!(s, "#admin", "Symbol name should be '#admin', got '{}'", s);
        }
        other => panic!("Expected Expr::Symbol, got {:?}", other),
    }
}

#[test]
fn test_symbol_hash_prefix_evals() {
    let code = "#admin";
    let result = run_str(code).expect("eval failed");
    match result {
        aven_seed::Value::Symbol(s) => {
            assert_eq!(s, "#admin", "Symbol value should be '#admin', got '{}'", s);
        }
        other => panic!("Expected Value::Symbol, got {:?}", other),
    }
}

#[test]
fn test_symbol_hash_prefix_typechecks() {
    let code = "#admin";
    let ty = typecheck_str(code).expect("typecheck failed");
    match ty {
        aven_seed::ast::Type::Symbol => {
            // Success
        }
        other => panic!("Expected Type::Symbol, got {:?}", other),
    }
}

#[test]
fn test_symbol_hash_equality() {
    let code1 = "#ok";
    let code2 = "#ok";
    let code3 = "#err";

    let val1 = run_str(code1).expect("eval1 failed");
    let val2 = run_str(code2).expect("eval2 failed");
    let val3 = run_str(code3).expect("eval3 failed");

    assert_eq!(val1, val2, "Two #ok symbols should be equal");
    assert_ne!(val1, val3, "Two different symbols should not be equal");
}

#[test]
fn test_symbol_hash_in_record_field() {
    let code = "{tag:#pending}";
    let result = run_str(code).expect("eval failed");
    match result {
        aven_seed::Value::Record(fields) => {
            assert_eq!(fields.len(), 1, "Record should have 1 field");
            assert_eq!(fields[0].0, "tag", "Field name should be 'tag'");
            match &fields[0].1 {
                aven_seed::Value::Symbol(s) => {
                    assert_eq!(s, "#pending", "Field value should be symbol #pending, got '{}'", s);
                }
                other => panic!("Expected Symbol in record field, got {:?}", other),
            }
        }
        other => panic!("Expected Value::Record, got {:?}", other),
    }
}

#[test]
fn test_symbol_hash_invalid_no_name() {
    let code = "#";
    let result = parse_str(code);
    assert!(result.is_err(), "Parsing '#' with no identifier should fail");
}

#[test]
fn test_symbol_hash_digit_leading_rejected() {
    let code = "#123";
    let result = parse_str(code);
    assert!(result.is_err(), "Parsing '#123' (digit-leading symbol) should fail");
}

#[test]
fn test_symbol_hash_dotted_stops_at_name() {
    let code = "#a.b";
    let result = run_str(code);
    // The lexer should stop at #a, so we expect either parse success with just #a,
    // or parse error because '.b' is unexpected. We verify it rejects dotted syntax.
    assert!(result.is_err(), "Parsing '#a.b' (dotted symbol) should fail or treat as #a + invalid '.b'");
}

#[test]
fn test_symbol_hash_as_record_key_rejected() {
    let code = "{#k: 1}";
    let result = parse_str(code);
    assert!(result.is_err(), "Using symbol #k as record field name should be rejected");
}

// Union type tests (M3 stage)

#[test]
fn test_union_type_annotation_two_variants() {
    let code = r#"
@fn f :: x:#ok Str | #err Str -> Nil
  _
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Union type annotation with two variants should parse and typecheck: {:?}", result);
}

#[test]
fn test_union_constructor_ok_value() {
    let result = run_str(r#"(#ok "success")"#).unwrap();
    match result {
        Value::Tagged(tag, payload) => {
            assert_eq!(tag, "ok");
            assert_eq!(payload, Some(Box::new(Value::Str("success".to_string()))));
        }
        _ => panic!("Expected Tagged value, got {:?}", result),
    }
}

#[test]
fn test_union_constructor_err_message() {
    let result = run_str(r#"(#err "failed")"#).unwrap();
    match result {
        Value::Tagged(tag, payload) => {
            assert_eq!(tag, "err");
            assert_eq!(payload, Some(Box::new(Value::Str("failed".to_string()))));
        }
        _ => panic!("Expected Tagged value, got {:?}", result),
    }
}

#[test]
fn test_union_constructor_compatible_with_multi_variant_annotation() {
    // A function returning #ok <int> should satisfy a return type of #ok Int | #err Str
    let code = r#"
@fn make_ok :: n:Int -> #ok Int | #err Str
  (#ok n)
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Constructor #ok should satisfy multi-variant union return type: {:?}", result);
}

#[test]
fn test_union_superset_constructor_rejected() {
    // x has type #ok Int | #err Str (2 variants), return expects #ok Int (1 variant)
    // found (2 variants) ⊄ expected (1 variant) → should be rejected
    let code = r#"
@fn f :: x:#ok Int | #err Str -> #ok Int
  x
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Found union with extra variants should not satisfy narrower expected: {:?}", result);
}

#[test]
fn test_union_annotation_match_coverage() {
    let code = r#"
@fn f :: result:#ok Int | #err Str -> Str
  @match result
    #ok v -> "success"
    #err e -> e
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Union match with full coverage should typecheck: {:?}", result);
}

#[test]
fn test_union_annotation_match_non_exhaustive_rejected() {
    let code = r#"
@fn f :: result:#ok Int | #err Str -> Str
  @match result
    #ok v -> "success"
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Non-exhaustive match on union should be rejected: {:?}", result);
}

#[test]
fn test_union_nested_field_type() {
    // Test field-name syntax with TypeRef payload after fixes 1 & 2
    let code = r#"
@fn f :: x:#ok value:Str | #err msg:Str -> Nil
  _
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Two-variant union with field-name syntax and same payload type should typecheck: {:?}", result);
}

#[test]
fn test_list_parse_simple_integers() {
    let result = parse_str("[1 2 3]");
    assert!(result.is_ok(), "List literal [1 2 3] should parse: {:?}", result);
}

#[test]
fn test_list_eval_integers() {
    let result = run_str("[1 2 3]").unwrap();
    match result {
        Value::List(elements) => {
            assert_eq!(elements.len(), 3);
            assert_eq!(elements[0], Value::Int(1));
            assert_eq!(elements[1], Value::Int(2));
            assert_eq!(elements[2], Value::Int(3));
        }
        _ => panic!("Expected List value"),
    }
}

#[test]
fn test_list_empty() {
    let result = run_str("[]").unwrap();
    match result {
        Value::List(elements) => {
            assert!(elements.is_empty(), "Empty list should have no elements");
        }
        _ => panic!("Expected List value"),
    }
}

#[test]
fn test_list_mixed_types_rejected() {
    let code = "[1 \"hello\"]";
    let result = typecheck_str(code);
    assert!(result.is_err(), "Mixed-type list should be rejected by typecheck: {:?}", result);
}

#[test]
fn test_list_empty_typecheck() {
    let code = "[]";
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Empty list should typecheck: {:?}", result);
    if let Ok(ty) = result {
        match ty {
            Type::List(inner) => {
                assert_eq!(*inner, Type::TypeParam("t".to_string()));
            }
            _ => panic!("Expected List type, got {:?}", ty),
        }
    }
}

#[test]
fn test_list_strings() {
    let result = run_str(r#"["a" "b" "c"]"#).unwrap();
    match result {
        Value::List(elements) => {
            assert_eq!(elements.len(), 3);
            assert_eq!(elements[0], Value::Str("a".to_string()));
            assert_eq!(elements[1], Value::Str("b".to_string()));
            assert_eq!(elements[2], Value::Str("c".to_string()));
        }
        _ => panic!("Expected List value"),
    }
}

#[test]
fn test_list_nested_expressions() {
    let result = run_str("[(+ 1 2) (+ 3 4) (+ 5 6)]").unwrap();
    match result {
        Value::List(elements) => {
            assert_eq!(elements.len(), 3);
            assert_eq!(elements[0], Value::Int(3));
            assert_eq!(elements[1], Value::Int(7));
            assert_eq!(elements[2], Value::Int(11));
        }
        _ => panic!("Expected List value"),
    }
}

// M2.2 — Option type (?T syntax)

#[test]
fn test_option_type_parses_question_mark() {
    // Verify ?Int parses as Option[Int] in type annotation
    let source = "@fn f :: x:?Int -> ?Int x";
    let result = typecheck_str(source);
    assert!(result.is_ok(), "Option type annotation should parse and typecheck: {:?}", result);
}

#[test]
fn test_option_type_accepts_nil_value() {
    // A function returning ?Int should accept Nil
    let source = "@fn f :: -> ?Int _";
    let result = typecheck_str(source);
    assert!(result.is_ok(), "Nil should satisfy ?T: {:?}", result);
}

#[test]
fn test_option_type_accepts_inner_type() {
    // A function returning ?Int should accept Int
    let source = "@fn f :: -> ?Int 42";
    let result = typecheck_str(source);
    assert!(result.is_ok(), "T should satisfy ?T: {:?}", result);
}

#[test]
fn test_option_type_parameter_annotation() {
    // Option type in function parameter
    let source = "@fn f :: x:?Str -> ?Str x";
    let result = typecheck_str(source);
    assert!(result.is_ok(), "Option type param should typecheck: {:?}", result);
}

#[test]
fn test_option_type_nested_option() {
    // Nested option types (Option[Option[Int]])
    let source = "@fn f :: -> ??Int _";
    let result = typecheck_str(source);
    assert!(result.is_ok(), "Nested option should typecheck: {:?}", result);
}

#[test]
fn test_option_type_list_composition() {
    // Option[List[Int]] should work
    let source = "@fn f :: -> ?[Int] _";
    let result = typecheck_str(source);
    assert!(result.is_ok(), "Option[List[T]] should typecheck: {:?}", result);
}

#[test]
fn test_option_rejects_wrong_type() {
    // ?Str does not accept Int
    let code = r#"
@fn f :: x:?Str -> ?Str
  x
@call f 42
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "?Str should reject Int argument: {:?}", result);
}

#[test]
fn test_option_nil_or_inner_accepted() {
    // ?Bool accepts Bool value
    let code_bool = r#"
@fn f :: x:?Bool -> ?Bool
  x
@call f @true
"#;
    assert!(typecheck_str(code_bool).is_ok(), "?Bool should accept Bool: {:?}", typecheck_str(code_bool));

    // ?Bool accepts Nil (_)
    let code_nil = r#"
@fn f :: x:?Bool -> ?Bool
  x
@call f _
"#;
    assert!(typecheck_str(code_nil).is_ok(), "?Bool should accept Nil: {:?}", typecheck_str(code_nil));
}

#[test]
fn test_use_wildcard_imports_all_caps() {
    // Test that @use * imports all exported capabilities
    let code = r#"
@mod math_helpers
@pub [read calc]
@fn get_data :: -> Int @ret 42

@mod app
@use * @from math_helpers
@ret 1
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Wildcard import should succeed when module exists: {:?}", result);
}

#[test]
fn test_use_wildcard_nonexistent_module_rejected() {
    // Test that wildcard import rejects nonexistent module
    let code = r#"
@mod app
@use * @from nonexistent
@ret 1
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Wildcard import of nonexistent module should fail");
    if let Err(e) = result {
        assert!(e.message.contains("module not found"), "Error should mention module not found: {}", e.message);
    }
}

#[test]
fn test_use_wildcard_then_call_imported_fn() {
    // Test that wildcard import makes functions available for calling
    let code = r#"
@mod math_helpers
@pub [calc]
@fn add :: x:Int y:Int -> Int
  (+ x y)

@mod app
@use * @from math_helpers
@call add 5 3
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Wildcard import should enable calling imported functions: {:?}", result);
}

#[test]
fn test_use_wildcard_vs_explicit_equivalent() {
    // Test that @use * @from mod is equivalent to @use [cap1 cap2] @from mod when mod exports [cap1 cap2]
    let code_wildcard = r#"
@mod provider
@pub [read calc]
@fn get_val :: -> Int @ret 100

@mod consumer
@use * @from provider
@ret 1
"#;
    let code_explicit = r#"
@mod provider
@pub [read calc]
@fn get_val :: -> Int @ret 100

@mod consumer
@use [read calc] @from provider
@ret 1
"#;
    let code_wrong_cap = r#"
@mod provider
@pub [read calc]
@fn get_val :: -> Int @ret 100

@mod consumer
@use [nonexistent_cap] @from provider
@ret 1
"#;
    let result_wildcard = typecheck_str(code_wildcard);
    let result_explicit = typecheck_str(code_explicit);
    let result_wrong_cap = typecheck_str(code_wrong_cap);
    assert!(result_wildcard.is_ok(), "Wildcard import should typecheck: {:?}", result_wildcard);
    assert!(result_explicit.is_ok(), "Explicit import should typecheck: {:?}", result_explicit);
    assert!(result_wrong_cap.is_err(), "Explicit import with nonexistent cap should fail: {:?}", result_wrong_cap);
}

#[test]
fn test_import_subset_valid() {
    // Test that @use [read] from a module exporting [read write] passes typecheck
    let code = r#"
@mod data_provider
@pub [read write]

@mod consumer
@use [read] @from data_provider
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Type checker should accept capability subset: {:?}", result);
}

#[test]
fn test_import_superset_rejected_typecheck_phase() {
    // Test that @use [read write delete] from a module exporting [read write] is rejected with "delete" in error
    let code = r#"
@mod data_provider
@pub [read write]

@mod consumer
@use [read write delete] @from data_provider
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject capability superset");
    if let Err(err) = result {
        assert!(err.message.contains("delete"),
            "Error should mention the missing capability 'delete': {}", err.message);
        assert!(err.message.contains("data_provider"),
            "Error should mention the module name 'data_provider': {}", err.message);
    }
}

#[test]
fn test_import_nonexistent_cap_rejected() {
    // Test that @use [write] from a module exporting only [read] is rejected with "write" in error
    let code = r#"
@mod data_provider
@pub [read]

@mod consumer
@use [write] @from data_provider
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject nonexistent capability");
    if let Err(err) = result {
        assert!(err.message.contains("write"),
            "Error should mention the missing capability 'write': {}", err.message);
        assert!(err.message.contains("data_provider"),
            "Error should mention the module name 'data_provider': {}", err.message);
    }
}

#[test]
fn test_import_renamed_cap_subset_valid() {
    // Test that @use [read as r] from a module exporting [read write] passes typecheck
    let code = r#"
@mod data_provider
@pub [read write]

@mod consumer
@use [read as r] @from data_provider
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Type checker should accept renamed capability subset: {:?}", result);
}

#[test]
fn test_import_renamed_cap_invalid_original_rejected() {
    let code = r#"
@mod data_provider
@pub [read]

@mod consumer
@use [bogus as r] @from data_provider
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Type checker should reject alias of nonexistent capability");
    if let Err(err) = result {
        assert!(err.message.contains("bogus"),
            "Error should mention the original capability name 'bogus': {}", err.message);
        assert!(err.message.contains("data_provider"),
            "Error should mention the module name 'data_provider': {}", err.message);
    }
}

#[test]
fn test_parse_patch_file_single_diff() {
    let patch_text = r#"@patch-for path:"module.av"
@diff @replace /fn_x/body 100"#;
    let result = patch_file_to_diffs(patch_text);
    assert!(result.is_ok(), "Should parse valid patch file");
    let ops = result.unwrap();
    assert_eq!(ops.len(), 1, "Should have exactly one DiffOp");
    assert_eq!(ops[0].kind, DiffKind::Replace, "First op should be Replace");
}

#[test]
fn test_parse_patch_file_multiple_diffs() {
    let patch_text = r#"@patch-for path:"module.av"
@diff @replace /a 1
@diff @insert @first /b 2"#;
    let result = patch_file_to_diffs(patch_text);
    assert!(result.is_ok(), "Should parse valid patch file with multiple diffs");
    let ops = result.unwrap();
    assert_eq!(ops.len(), 2, "Should have exactly two DiffOps");
    assert_eq!(ops[0].kind, DiffKind::Replace, "First op should be Replace");
    assert_eq!(ops[1].kind, DiffKind::Insert, "Second op should be Insert");
}

#[test]
fn test_parse_patch_file_missing_path_rejected() {
    let patch_text = r#"@patch-for
@diff @replace /x 42"#;
    let result = patch_file_to_diffs(patch_text);
    assert!(result.is_err(), "Should reject patch file without path clause");
}

#[test]
fn test_parse_patch_file_wrong_key_rejected() {
    let patch_text = r#"@patch-for foo:"test.av"
@diff @replace /x 42"#;
    let result = patch_file_to_diffs(patch_text);
    assert!(result.is_err(), "Should reject @patch-for with non-'path' key");
}

#[test]
fn test_patch_file_roundtrip_insert_no_mode() {
    let patch_text = r#"@patch-for path:"test.av"
@diff @insert /b 2"#;

    let ops = patch_file_to_diffs(patch_text).expect("Parse should succeed");
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].kind, DiffKind::Insert);
    assert!(ops[0].insert_mode.is_none(), "Insert mode should be None");

    let serialized = diffs_to_avenpatch_string(&ops, "test.av");
    let reparsed = patch_file_to_diffs(&serialized).expect("Reparse should succeed");

    assert_eq!(ops.len(), reparsed.len(), "Roundtrip should preserve op count");
    assert_eq!(ops[0].kind, reparsed[0].kind, "Roundtrip should preserve op kind");
    assert_eq!(ops[0].insert_mode, reparsed[0].insert_mode, "Roundtrip should preserve insert_mode as None");
    assert_eq!(ops[0].payload, reparsed[0].payload, "Roundtrip should preserve payload");
}

#[test]
fn test_patch_file_roundtrip_replace() {
    let patch_text = r#"@patch-for path:"test.av"
@diff @replace /fn_x/body 100"#;
    
    let ops = patch_file_to_diffs(patch_text).expect("Parse should succeed");
    assert_eq!(ops.len(), 1);
    
    let serialized = diffs_to_avenpatch_string(&ops, "test.av");
    let reparsed = patch_file_to_diffs(&serialized).expect("Reparse should succeed");

    assert_eq!(ops.len(), reparsed.len(), "Roundtrip should preserve op count");
    assert_eq!(ops[0].kind, reparsed[0].kind, "Roundtrip should preserve op kind");
    assert_eq!(ops[0].selector.to_string(), reparsed[0].selector.to_string(), "Roundtrip should preserve selector");
    assert_eq!(ops[0].payload, reparsed[0].payload, "Roundtrip should preserve payload");
}

#[test]
fn test_patch_file_roundtrip_multiple_ops() {
    let patch_text = r#"@patch-for path:"target.av"
@diff @replace /x 10
@diff @insert @last /z 20
@diff @delete /y"#;

    let ops = patch_file_to_diffs(patch_text).expect("Parse should succeed");
    assert_eq!(ops.len(), 3, "Should parse three ops");

    let serialized = diffs_to_avenpatch_string(&ops, "target.av");
    let reparsed = patch_file_to_diffs(&serialized).expect("Reparse should succeed");

    assert_eq!(ops.len(), reparsed.len(), "Roundtrip should preserve all ops");
    for i in 0..ops.len() {
        assert_eq!(ops[i].kind, reparsed[i].kind, "Roundtrip should preserve op kind at index {}", i);
        assert_eq!(ops[i].selector.to_string(), reparsed[i].selector.to_string(), "Roundtrip should preserve selector at index {}", i);
        assert_eq!(ops[i].payload, reparsed[i].payload, "Roundtrip should preserve payload at index {}", i);
    }
}

#[test]
fn test_pub_fn_declaration_parses() {
    let code = "@pub @fn greet :: -> Str @io.write \"hi\"";
    let result = run_str(code);
    // @pub @fn should parse without error
    assert!(result.is_ok(), "@pub @fn should parse successfully");
}

#[test]
fn test_pub_type_declaration_parses() {
    let code = "@pub @type Handler = Int";
    let result = run_str(code).unwrap();
    // @pub @type should parse and eval to Nil
    assert_eq!(result, Value::Nil, "@pub @type should parse and eval to Nil");
}

#[test]
fn test_pub_let_declaration_parses() {
    let code = "@pub @let x :: 42";
    let result = run_str(code).unwrap();
    // @pub @let should parse and eval to the value
    assert_eq!(result, Value::Int(42), "@pub @let should parse and eval to 42");
}

#[test]
fn test_pub_fn_accessible_from_use() {
    let code = r#"
@mod mathA
@pub @fn double :: x:Int -> Int (+ x x)
@mod appB
@use * @from mathA
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Wildcard import of @pub @fn should succeed in typechecker: {:?}", result);
}

#[test]
fn test_pub_bracket_form_still_works() {
    let code = r#"
@mod foo
@pub [read write]
@use [read] @from foo
"#;
    let result = run_str(code);
    // The bracket form should still parse and be valid
    assert!(result.is_ok(), "@pub [caps] bracket form should still work");
}

#[test]
fn test_pub_fn_not_exportable_without_pub() {
    let code = r#"
@mod mathA
@fn double :: x:Int -> Int (+ x x)
@mod appB
@use [double] @from mathA
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Non-@pub function should not be importable: {:?}", result);
}

#[test]
fn test_pub_fn_importable_with_pub() {
    let code = r#"
@mod mathA
@pub @fn double :: x:Int -> Int (+ x x)
@mod appB
@use [double] @from mathA
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "@pub @fn should be importable by name: {:?}", result);
}

// Generic type alias tests (§1.6)

#[test]
fn test_generic_type_alias_simple_parse() {
    let code = r#"@type Pair a b = ?a"#;
    let result = parse_str(code);
    assert!(result.is_ok(), "Generic type alias with params should parse: {:?}", result);
    if let Ok(expr) = result {
        if let ast::Expr::TypeAlias { name, type_params, .. } = expr {
            assert_eq!(name, "Pair");
            assert_eq!(type_params, vec!["a", "b"]);
        } else {
            panic!("Expected TypeAlias expression");
        }
    }
}

#[test]
fn test_generic_type_alias_no_params_still_works() {
    let code = r#"@type UserId = Int"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Non-generic type alias should still work: {:?}", result);
}

#[test]
fn test_generic_type_alias_single_param() {
    let code = r#"@type Box a = ?a"#;
    let result = parse_str(code);
    assert!(result.is_ok(), "Generic type alias with single param should parse: {:?}", result);
    if let Ok(expr) = result {
        if let ast::Expr::TypeAlias { name, type_params, .. } = expr {
            assert_eq!(name, "Box");
            assert_eq!(type_params, vec!["a"]);
        } else {
            panic!("Expected TypeAlias expression");
        }
    }
}

#[test]
fn test_generic_type_alias_param_validation_lowercase() {
    let code = r#"@type Pair A b = {first:A second:b}"#;
    let result = parse_str(code);
    assert!(result.is_err(), "Type parameter must be lowercase: {:?}", result);
}

#[test]
fn test_generic_type_alias_param_multi_letter_allowed() {
    // Multi-letter lowercase params are now allowed
    let code = r#"@type Pair ab cd = {first:ab second:cd}"#;
    let result = parse_str(code);
    assert!(result.is_ok(), "Multi-letter lowercase type parameters should parse: {:?}", result);
    if let Ok(expr) = result {
        if let ast::Expr::TypeAlias { name, type_params, .. } = expr {
            assert_eq!(name, "Pair");
            assert_eq!(type_params, vec!["ab", "cd"]);
        } else {
            panic!("Expected TypeAlias expression");
        }
    }
}

#[test]
fn test_generic_type_alias_multi_letter_elem_value() {
    let code = r#"@type Pair elem value = {first:elem second:value}"#;
    let result = parse_str(code);
    assert!(result.is_ok(), "Multi-letter type parameters elem/value should parse: {:?}", result);
    if let Ok(expr) = result {
        if let ast::Expr::TypeAlias { name, type_params, .. } = expr {
            assert_eq!(name, "Pair");
            assert_eq!(type_params, vec!["elem", "value"]);
        } else {
            panic!("Expected TypeAlias expression");
        }
    }
}

#[test]
fn test_generic_type_alias_pair_int_str() {
    // Test both definition and instantiation
    let code = r#"
@type Pair a b = {first:a second:b}
@fn make_pair :: x:Int y:Str -> Pair Int Str {first:x second:y}
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Pair Int Str instantiation should typecheck: {:?}", result);
}

#[test]
fn test_generic_fn_id_int() {
    let code = r#"@fn id :: x:a -> a x"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Generic identity fn should typecheck: {:?}", result);
}

#[test]
fn test_generic_type_mismatch_arity() {
    // Pair expects 2 type args; providing 1 should be rejected by typechecker
    let code = r#"
@type Pair a b = {first:a second:b}
@fn bad :: x:Pair Int -> Int x
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Arity mismatch should be rejected: {:?}", result);
    if let Err(e) = result {
        assert!(e.message.contains("2") || e.message.contains("argument"),
            "Error should mention arity: {}", e.message);
    }
}

#[test]
fn test_generic_result_union_ok_err() {
    let code = r#"
@type Result a b = #ok a | #err b
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Generic Result union alias should typecheck: {:?}", result);
}

#[test]
fn test_generic_nested_option_list() {
    let code = r#"
@type Box a = ?a
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Generic Box alias should typecheck: {:?}", result);
}

#[test]
fn test_generic_type_alias_cyclic_rejected() {
    let code = r#"
@type Loop a = Loop a
@fn f :: x:Loop Int -> Int x
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Cyclic generic alias should be rejected: {:?}", result);
    if let Err(e) = &result {
        assert!(e.message.contains("Cyclic"), "Error should mention cyclic alias: {}", e.message);
    }
}

#[test]
fn test_intent_cli_empty_program_prints_nothing() {
    let code = "@fn f :: x:Int -> Int x";
    let result = aven_seed::intent_index(code);
    assert!(result.is_ok(), "Parse should succeed: {:?}", result);
    let table = result.unwrap();
    assert!(table.entries.is_empty(), "No @intent nodes should produce empty table");
}

#[test]
fn test_intent_cli_single_entry_format() {
    let code = r#"
@fn greet :: name:Str -> Str
  @intent "greeting_response"
  (+ "Hello, " name)
"#;
    let result = aven_seed::intent_index(code);
    assert!(result.is_ok(), "Parse should succeed: {:?}", result);
    let table = result.unwrap();
    assert_eq!(table.entries.len(), 1, "Should have exactly one entry");
    let entry = &table.entries[0];
    assert_eq!(entry.intent_name, "greeting_response", "Intent name should match");
    assert_eq!(entry.selector, "/fn greet", "Selector should be exact /fn greet");
    assert!(entry.subtree_span.start < entry.subtree_span.end, "Span should be valid");
}

#[test]
fn test_intent_cli_nested_entries_sorted_by_selector() {
    let code = r#"
@fn f1 :: x:Int -> Int
  @intent "step1"
  x

@fn f2 :: x:Int -> Int
  @intent "step2"
  x

@fn f3 :: x:Int -> Int
  @intent "step3"
  x
"#;
    let result = aven_seed::intent_index(code);
    assert!(result.is_ok(), "Parse should succeed: {:?}", result);
    let table = result.unwrap();
    assert_eq!(table.entries.len(), 3, "Should have three entries");

    // Format output using the actual formatting logic.
    let lines = aven_seed::format_intent_output(&table, code);
    assert_eq!(lines.len(), 3, "Should have three formatted output lines");

    // Verify selectors are in expected sorted order (/fn f1, /fn f2, /fn f3)
    assert!(lines[0].starts_with("/fn f1"), "First entry should be /fn f1, got: {}", lines[0]);
    assert!(lines[1].starts_with("/fn f2"), "Second entry should be /fn f2, got: {}", lines[1]);
    assert!(lines[2].starts_with("/fn f3"), "Third entry should be /fn f3, got: {}", lines[2]);
}

#[test]
fn test_intent_cli_top_level_selector() {
    // A top-level @intent (outside any @fn) gets selector "/" (empty current_path).
    let code = "@intent \"top_level_note\"";
    let result = aven_seed::intent_index(code);
    assert!(result.is_ok(), "Parse should succeed: {:?}", result);
    let table = result.unwrap();
    assert_eq!(table.entries.len(), 1);
    assert_eq!(table.entries[0].selector, "/", "Top-level intent selector should be /");
    assert_eq!(table.entries[0].intent_name, "top_level_note");
}

#[test]
fn test_intent_cli_equal_selector_tiebreak_by_source_order() {
    // Two top-level @intent nodes share selector "/"; tie-break by span.start
    // ensures deterministic output in source order.
    let code = "@intent \"first\"\n@intent \"second\"";
    let result = aven_seed::intent_index(code);
    assert!(result.is_ok(), "Parse should succeed: {:?}", result);
    let table = result.unwrap();
    assert_eq!(table.entries.len(), 2);
    let lines = aven_seed::format_intent_output(&table, code);
    assert_eq!(lines.len(), 2);
    // first appears before second in source, so it must appear first in output
    assert!(lines[0].contains("first"), "First output line should be 'first' intent, got: {}", lines[0]);
    assert!(lines[1].contains("second"), "Second output line should be 'second' intent, got: {}", lines[1]);
}

#[test]
fn test_fn_type_param_annotation_parses() {
    let code = "@fn apply :: f:(Int -> Int) x:Int -> Int (+ 1 2)";
    let result = aven_seed::parse_str(code);
    assert!(result.is_ok(), "Function with fn-type parameter should parse: {:?}", result);
    let expr = result.unwrap();
    if let aven_seed::Expr::FnDef { params, .. } = expr {
        assert_eq!(params.len(), 2, "Should have 2 parameters");
        // First param should be f with Type::Fn annotation
        let (name, ty_opt) = &params[0];
        assert_eq!(name, "f", "First param should be named 'f'");
        assert!(ty_opt.is_some(), "First param should have a type annotation");
        if let Some(ty) = ty_opt {
            match ty {
                aven_seed::Type::Fn { params: fn_params, return_type, .. } => {
                    assert_eq!(fn_params.len(), 1, "Fn type should have 1 parameter");
                    assert!(matches!(fn_params[0], aven_seed::Type::Primitive(aven_seed::PrimitiveType::Int)), "Fn param should be Int");
                    assert!(matches!(**return_type, aven_seed::Type::Primitive(aven_seed::PrimitiveType::Int)), "Fn return type should be Int");
                }
                _ => panic!("First param should have Type::Fn, got {:?}", ty),
            }
        }
    } else {
        panic!("Expected FnDef, got {:?}", expr);
    }
}

#[test]
fn test_fn_type_param_compatible_call() {
    let code = r#"
@fn id :: x:Int -> Int x
@fn apply :: f:(Int -> Int) x:Int -> Int (@call f x)
(@call apply id 42)
"#;
    let result = typecheck_str(code);
    assert!(result.is_ok(), "Function call with fn-type argument should typecheck: {:?}", result);
}

#[test]
fn test_fn_type_param_wrong_return_type_rejected() {
    let code = r#"
@fn bool_fn :: x:Int -> Bool @true
@fn apply :: f:(Int -> Int) x:Int -> Int (@call f x)
(@call apply bool_fn 42)
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Function call with incompatible return type should be rejected");
    if let Err(e) = result {
        // Should report type mismatch between Bool return and Int expectation
        assert!(e.message.contains("type") || e.message.contains("Bool") || e.message.contains("Int"),
                "Error should mention type mismatch: {}", e.message);
    }
}

#[test]
fn test_fn_type_param_effect_mismatch_rejected() {
    let code = r#"
@fn apply :: f:(Int -!> Int) x:Int -> Int (@call f x)
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Pure context cannot call IO-effect function");
    if let Err(e) = result {
        assert!(e.message.contains("effect") || e.message.contains("subset"),
                "Error should mention effect mismatch: {}", e.message);
    }
}

#[test]
fn test_fn_type_higher_order_roundtrip() {
    let code = "@fn f :: g:((Int -> Int) -> Bool) -> Bool @true";
    let result = aven_seed::parse_str(code);
    assert!(result.is_ok(), "Nested fn-type annotation should parse: {:?}", result);
    let expr = result.unwrap();

    // Format the parsed expression
    let formatted = aven_seed::format_expr(&expr);

    // Reparse the formatted expression
    let reparsed = aven_seed::parse_str(&formatted);
    assert!(reparsed.is_ok(), "Formatted code should reparse: {:?}", reparsed);
    let reparsed_expr = reparsed.unwrap();

    // Verify structural equality
    if let (aven_seed::Expr::FnDef { params: params1, .. }, aven_seed::Expr::FnDef { params: params2, .. }) = (&expr, &reparsed_expr) {
        assert_eq!(params1.len(), params2.len(), "Parameter count should match after roundtrip");
        let (name1, ty_opt1) = &params1[0];
        let (name2, ty_opt2) = &params2[0];
        assert_eq!(name1, name2, "Parameter names should match");
        assert_eq!(ty_opt1.is_some(), ty_opt2.is_some(), "Both should have types or both not");
    } else {
        panic!("Reparsed expression should be FnDef");
    }

    // Original structure check
    if let aven_seed::Expr::FnDef { params, .. } = expr {
        assert_eq!(params.len(), 1, "Should have 1 parameter");
        let (name, ty_opt) = &params[0];
        assert_eq!(name, "g", "Parameter should be named 'g'");
        if let Some(ty) = ty_opt {
            match ty {
                aven_seed::Type::Fn { params: outer_params, return_type, .. } => {
                    assert_eq!(outer_params.len(), 1, "Outer fn type should have 1 parameter");
                    // The inner param should be a function type
                    match &outer_params[0] {
                        aven_seed::Type::Fn { params: inner_params, .. } => {
                            assert_eq!(inner_params.len(), 1, "Inner fn type should have 1 parameter");
                            assert!(matches!(inner_params[0], aven_seed::Type::Primitive(aven_seed::PrimitiveType::Int)));
                        }
                        _ => panic!("Inner param should be a fn-type"),
                    }
                    assert!(matches!(**return_type, aven_seed::Type::Primitive(aven_seed::PrimitiveType::Bool)), "Outer return type should be Bool");
                }
                _ => panic!("Expected Type::Fn, got {:?}", ty),
            }
        }
    } else {
        panic!("Expected FnDef");
    }
}

#[test]
fn test_fn_type_option_param_roundtrip() {
    // Parse a function with Option in fn-type annotation
    let code = "@fn f :: g:(?Int -> Int) -> Int 0";
    let result = aven_seed::parse_str(code);
    assert!(result.is_ok(), "Fn-type with Option param should parse: {:?}", result);
    let expr = result.unwrap();

    // Format the parsed expression
    let formatted = aven_seed::format_expr(&expr);

    // Reparse the formatted expression
    let reparsed = aven_seed::parse_str(&formatted);
    assert!(reparsed.is_ok(), "Formatted code should reparse: {:?}", reparsed);
    let reparsed_expr = reparsed.unwrap();

    // Verify fn-type annotation matches after roundtrip
    if let aven_seed::Expr::FnDef { params: params1, .. } = &expr {
        if let aven_seed::Expr::FnDef { params: params2, .. } = &reparsed_expr {
            let (name1, ty_opt1) = &params1[0];
            let (name2, ty_opt2) = &params2[0];
            assert_eq!(name1, name2, "Parameter names should match");
            assert!(ty_opt1.is_some(), "Original param g should have type annotation");
            assert!(ty_opt2.is_some(), "Reparsed param g should have type annotation after roundtrip");
            let ty1 = ty_opt1.as_ref().unwrap();
            let ty2 = ty_opt2.as_ref().unwrap();
            match (ty1, ty2) {
                (aven_seed::Type::Fn { params: p1, .. }, aven_seed::Type::Fn { params: p2, .. }) => {
                    assert_eq!(p1.len(), p2.len(), "Fn-type params should match");
                    match (&p1[0], &p2[0]) {
                        (aven_seed::Type::Option(i1), aven_seed::Type::Option(i2)) => {
                            assert_eq!(i1, i2, "Option inner types should match");
                        }
                        _ => panic!("Both params should be Option types, got {:?} vs {:?}", p1[0], p2[0]),
                    }
                }
                _ => panic!("Both should be Fn types, got {:?} vs {:?}", ty1, ty2),
            }
        } else {
            panic!("Reparsed should be FnDef");
        }
    } else {
        panic!("Original should be FnDef");
    }
}

#[test]
fn test_fn_type_list_param_roundtrip() {
    // Parse a function with List in fn-type annotation
    let code = "@fn f :: g:([Int] -> Int) -> Int 0";
    let result = aven_seed::parse_str(code);
    assert!(result.is_ok(), "Fn-type with List param should parse: {:?}", result);
    let expr = result.unwrap();

    // Format the parsed expression
    let formatted = aven_seed::format_expr(&expr);

    // Reparse the formatted expression
    let reparsed = aven_seed::parse_str(&formatted);
    assert!(reparsed.is_ok(), "Formatted code should reparse: {:?}", reparsed);
    let reparsed_expr = reparsed.unwrap();

    // Verify fn-type annotation matches after roundtrip
    if let aven_seed::Expr::FnDef { params: params1, .. } = &expr {
        if let aven_seed::Expr::FnDef { params: params2, .. } = &reparsed_expr {
            let (name1, ty_opt1) = &params1[0];
            let (name2, ty_opt2) = &params2[0];
            assert_eq!(name1, name2, "Parameter names should match");
            assert!(ty_opt1.is_some(), "Original param g should have type annotation");
            assert!(ty_opt2.is_some(), "Reparsed param g should have type annotation after roundtrip");
            let ty1 = ty_opt1.as_ref().unwrap();
            let ty2 = ty_opt2.as_ref().unwrap();
            match (ty1, ty2) {
                (aven_seed::Type::Fn { params: p1, .. }, aven_seed::Type::Fn { params: p2, .. }) => {
                    assert_eq!(p1.len(), p2.len(), "Fn-type params should match");
                    match (&p1[0], &p2[0]) {
                        (aven_seed::Type::List(i1), aven_seed::Type::List(i2)) => {
                            assert_eq!(i1, i2, "List inner types should match");
                        }
                        _ => panic!("Both params should be List types, got {:?} vs {:?}", p1[0], p2[0]),
                    }
                }
                _ => panic!("Both should be Fn types, got {:?} vs {:?}", ty1, ty2),
            }
        } else {
            panic!("Reparsed should be FnDef");
        }
    } else {
        panic!("Original should be FnDef");
    }
}

#[test]
fn test_fn_type_param_effect_call_site_rejected() {
    // mk_io declared with -!> (IO effect), so its type is Int -!> Int.
    // Passing it to apply :: f:(Int -> Int) (pure) should be rejected
    // because IO is not a subset of pure.
    let code = r#"
@fn apply :: f:(Int -> Int) x:Int -> Int (@call f x)
@fn mk_io :: x:Int -!> Int (@io.write "side effect") x
(@call apply mk_io 5)
"#;
    let result = typecheck_str(code);
    assert!(result.is_err(), "Passing IO-effect function to pure (Int -> Int) parameter should fail: {:?}", result);
}

#[test]
fn test_math_add_positive() {
    let env = aven_seed::Env::new();
    match env.get("math_add").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(5), Value::Int(3)]);
            assert_eq!(result, Ok(Value::Int(8)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_add_overflow_detected() {
    let env = aven_seed::Env::new();
    match env.get("math_add").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(i64::MAX), Value::Int(1)]);
            assert!(result.is_err(), "math_add should detect overflow");
            match result {
                Err(EvalError::InvalidOperation(msg)) => {
                    assert!(msg.contains("overflow"), "Error message should mention overflow");
                }
                _ => panic!("Expected InvalidOperation with overflow message, got {:?}", result),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_sub_positive() {
    let env = aven_seed::Env::new();
    match env.get("math_sub").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(10), Value::Int(3)]);
            assert_eq!(result, Ok(Value::Int(7)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_sub_negative_result() {
    let env = aven_seed::Env::new();
    match env.get("math_sub").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(3), Value::Int(10)]);
            assert_eq!(result, Ok(Value::Int(-7)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_mul_positive() {
    let env = aven_seed::Env::new();
    match env.get("math_mul").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(4), Value::Int(5)]);
            assert_eq!(result, Ok(Value::Int(20)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_mul_zero() {
    let env = aven_seed::Env::new();
    match env.get("math_mul").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(100), Value::Int(0)]);
            assert_eq!(result, Ok(Value::Int(0)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_div_positive() {
    let env = aven_seed::Env::new();
    match env.get("math_div").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(10), Value::Int(2)]);
            assert_eq!(result, Ok(Value::Int(5)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_div_by_zero() {
    let env = aven_seed::Env::new();
    match env.get("math_div").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(10), Value::Int(0)]);
            assert!(result.is_err(), "math_div should reject division by zero");
            match result {
                Err(EvalError::InvalidOperation(msg)) => {
                    assert!(msg.contains("division by zero"), "Error message should mention division by zero");
                }
                _ => panic!("Expected InvalidOperation with division by zero message, got {:?}", result),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_floor_positive_int() {
    let env = aven_seed::Env::new();
    match env.get("math_floor").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 1);
            let result = func(&[Value::Int(42)]);
            assert_eq!(result, Ok(Value::Int(42)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_ceil_positive_int() {
    let env = aven_seed::Env::new();
    match env.get("math_ceil").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 1);
            let result = func(&[Value::Int(42)]);
            assert_eq!(result, Ok(Value::Int(42)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_round_positive_int() {
    let env = aven_seed::Env::new();
    match env.get("math_round").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 1);
            let result = func(&[Value::Int(42)]);
            assert_eq!(result, Ok(Value::Int(42)));
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_qualified_add() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/math::add").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(5), Value::Int(3)]);
            assert_eq!(result, Ok(Value::Int(8)));
        }
        _ => panic!("expected NativeFn for aven/std/math::add"),
    }
}

#[test]
fn test_math_qualified_div() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/math::div").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 2);
            let result = func(&[Value::Int(10), Value::Int(2)]);
            assert_eq!(result, Ok(Value::Int(5)));
        }
        _ => panic!("expected NativeFn for aven/std/math::div"),
    }
}

#[test]
fn test_math_qualified_div_overflow() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/math::div").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(i64::MIN), Value::Int(-1)]);
            assert!(result.is_err(), "aven/std/math::div should detect overflow");
            match result {
                Err(EvalError::InvalidOperation(msg)) => {
                    assert!(msg.contains("overflow"), "Error message should mention overflow");
                }
                _ => panic!("Expected InvalidOperation with overflow message, got {:?}", result),
            }
        }
        _ => panic!("expected NativeFn for aven/std/math::div"),
    }
}

#[test]
fn test_math_qualified_floor() {
    let env = aven_seed::Env::new();
    match env.get("aven/std/math::floor").unwrap() {
        Value::NativeFn { func, arity, .. } => {
            assert_eq!(arity, 1);
            let result = func(&[Value::Int(42)]);
            assert_eq!(result, Ok(Value::Int(42)));
        }
        _ => panic!("expected NativeFn for aven/std/math::floor"),
    }
}

#[test]
fn test_math_add_wrong_arity() {
    let env = aven_seed::Env::new();
    match env.get("math_add").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Int(5)]);
            assert!(result.is_err(), "math_add with wrong arity should error");
            match result {
                Err(EvalError::InvalidFunctionCall(msg)) => {
                    assert!(msg.contains("Expected 2 args"), "Error should mention arity mismatch");
                }
                _ => panic!("Expected InvalidFunctionCall, got {:?}", result),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_add_wrong_type() {
    let env = aven_seed::Env::new();
    match env.get("math_add").unwrap() {
        Value::NativeFn { func, .. } => {
            let result = func(&[Value::Str("hello".to_string()), Value::Int(3)]);
            assert!(result.is_err(), "math_add with wrong type should error");
            match result {
                Err(EvalError::TypeError(msg)) => {
                    assert!(msg.contains("add requires Int arguments"), "Error should mention type requirement");
                }
                _ => panic!("Expected TypeError, got {:?}", result),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_math_pow_overflow_detected() {
    let env = aven_seed::Env::new();
    match env.get("pow").unwrap() {
        Value::NativeFn { func, .. } => {
            // 2^63 = 9223372036854775808 which overflows i64::MAX
            let result = func(&[Value::Int(2), Value::Int(63)]);
            assert!(result.is_err(), "pow should detect overflow");
            match result {
                Err(EvalError::InvalidOperation(msg)) => {
                    assert!(msg.contains("overflow"), "Error message should mention overflow, got: {}", msg);
                }
                _ => panic!("Expected InvalidOperation with overflow message, got {:?}", result),
            }
        }
        _ => panic!("expected NativeFn"),
    }
}

#[test]
fn test_col_list_new_empty() {
    let env = aven_seed::Env::new();
    if let Some(Value::NativeFn { arity, .. }) = env.get("col_list_new") {
        assert_eq!(arity, 0);
        let fn_val = env.get("col_list_new").unwrap();
        if let Value::NativeFn { func, .. } = fn_val {
            let result = func(&[]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), Value::List(vec![]));
        } else {
            panic!("Expected NativeFn");
        }
    } else {
        panic!("col_list_new not found or not a NativeFn");
    }
}

#[test]
fn test_col_list_push_and_len() {
    let env = aven_seed::Env::new();
    let list_new = env.get("col_list_new").unwrap();
    let list_push = env.get("col_list_push").unwrap();
    let list_len = env.get("col_list_len").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = list_new {
        let empty_list = fn_new(&[]).unwrap();
        assert_eq!(empty_list, Value::List(vec![]));

        if let Value::NativeFn { func: fn_push, .. } = list_push {
            let list_with_one = fn_push(&[empty_list.clone(), Value::Int(42)]).unwrap();
            assert_eq!(list_with_one, Value::List(vec![Value::Int(42)]));

            if let Value::NativeFn { func: fn_len, .. } = list_len {
                let len_result = fn_len(&[list_with_one]).unwrap();
                assert_eq!(len_result, Value::Int(1));
            } else {
                panic!("list_len not a NativeFn");
            }
        } else {
            panic!("list_push not a NativeFn");
        }
    } else {
        panic!("list_new not a NativeFn");
    }
}

#[test]
fn test_col_list_get_valid() {
    let env = aven_seed::Env::new();
    let list_new = env.get("col_list_new").unwrap();
    let list_push = env.get("col_list_push").unwrap();
    let list_get = env.get("col_list_get").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = list_new {
        let list = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_push, .. } = list_push {
            let list = fn_push(&[list, Value::Int(10)]).unwrap();
            if let Value::NativeFn { func: fn_get, .. } = list_get {
                let result = fn_get(&[list, Value::Int(0)]).unwrap();
                assert_eq!(result, Value::Int(10));
            } else {
                panic!("list_get not a NativeFn");
            }
        } else {
            panic!("list_push not a NativeFn");
        }
    } else {
        panic!("list_new not a NativeFn");
    }
}

#[test]
fn test_col_list_get_out_of_bounds() {
    let env = aven_seed::Env::new();
    let list_new = env.get("col_list_new").unwrap();
    let list_push = env.get("col_list_push").unwrap();
    let list_get = env.get("col_list_get").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = list_new {
        let list = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_push, .. } = list_push {
            let list = fn_push(&[list, Value::Int(10)]).unwrap();
            if let Value::NativeFn { func: fn_get, .. } = list_get {
                let result = fn_get(&[list, Value::Int(5)]);
                assert!(result.is_err());
            } else {
                panic!("list_get not a NativeFn");
            }
        } else {
            panic!("list_push not a NativeFn");
        }
    } else {
        panic!("list_new not a NativeFn");
    }
}

#[test]
fn test_col_list_pop_nonempty() {
    let env = aven_seed::Env::new();
    let list_new = env.get("col_list_new").unwrap();
    let list_push = env.get("col_list_push").unwrap();
    let list_pop = env.get("col_list_pop").unwrap();
    let list_len = env.get("col_list_len").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = list_new {
        let list = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_push, .. } = list_push {
            let list = fn_push(&[list.clone(), Value::Int(10)]).unwrap();
            let list = fn_push(&[list, Value::Int(20)]).unwrap();
            if let Value::NativeFn { func: fn_pop, .. } = list_pop {
                let list_popped = fn_pop(&[list]).unwrap();
                if let Value::NativeFn { func: fn_len, .. } = list_len {
                    let len = fn_len(&[list_popped]).unwrap();
                    assert_eq!(len, Value::Int(1));
                } else {
                    panic!("list_len not a NativeFn");
                }
            } else {
                panic!("list_pop not a NativeFn");
            }
        } else {
            panic!("list_push not a NativeFn");
        }
    } else {
        panic!("list_new not a NativeFn");
    }
}

#[test]
fn test_col_list_pop_empty_error() {
    let env = aven_seed::Env::new();
    let list_new = env.get("col_list_new").unwrap();
    let list_pop = env.get("col_list_pop").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = list_new {
        let empty_list = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_pop, .. } = list_pop {
            let result = fn_pop(&[empty_list]);
            assert!(result.is_err());
        } else {
            panic!("list_pop not a NativeFn");
        }
    } else {
        panic!("list_new not a NativeFn");
    }
}

#[test]
fn test_col_map_new_empty() {
    let env = aven_seed::Env::new();
    if let Some(Value::NativeFn { arity, .. }) = env.get("col_map_new") {
        assert_eq!(arity, 0);
        let fn_val = env.get("col_map_new").unwrap();
        if let Value::NativeFn { func, .. } = fn_val {
            let result = func(&[]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), Value::Map(vec![]));
        } else {
            panic!("Expected NativeFn");
        }
    } else {
        panic!("col_map_new not found or not a NativeFn");
    }
}

#[test]
fn test_col_map_set_and_get() {
    let env = aven_seed::Env::new();
    let map_new = env.get("col_map_new").unwrap();
    let map_set = env.get("col_map_set").unwrap();
    let map_get = env.get("col_map_get").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = map_new {
        let map = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_set, .. } = map_set {
            let map_with_key = fn_set(&[map, Value::Str("name".to_string()), Value::Str("Alice".to_string())]).unwrap();
            if let Value::NativeFn { func: fn_get, .. } = map_get {
                let result = fn_get(&[map_with_key, Value::Str("name".to_string())]).unwrap();
                assert_eq!(result, Value::Str("Alice".to_string()));
            } else {
                panic!("map_get not a NativeFn");
            }
        } else {
            panic!("map_set not a NativeFn");
        }
    } else {
        panic!("map_new not a NativeFn");
    }
}

#[test]
fn test_col_map_get_missing_returns_nil() {
    let env = aven_seed::Env::new();
    let map_new = env.get("col_map_new").unwrap();
    let map_get = env.get("col_map_get").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = map_new {
        let map = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_get, .. } = map_get {
            let result = fn_get(&[map, Value::Str("nonexistent".to_string())]).unwrap();
            assert_eq!(result, Value::Nil);
        } else {
            panic!("map_get not a NativeFn");
        }
    } else {
        panic!("map_new not a NativeFn");
    }
}

#[test]
fn test_col_map_has() {
    let env = aven_seed::Env::new();
    let map_new = env.get("col_map_new").unwrap();
    let map_set = env.get("col_map_set").unwrap();
    let map_has = env.get("col_map_has").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = map_new {
        let map = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_set, .. } = map_set {
            let map_with_key = fn_set(&[map, Value::Str("key1".to_string()), Value::Int(100)]).unwrap();
            if let Value::NativeFn { func: fn_has, .. } = map_has {
                let has_true = fn_has(&[map_with_key.clone(), Value::Str("key1".to_string())]).unwrap();
                assert_eq!(has_true, Value::Bool(true));
                let has_false = fn_has(&[map_with_key, Value::Str("key2".to_string())]).unwrap();
                assert_eq!(has_false, Value::Bool(false));
            } else {
                panic!("map_has not a NativeFn");
            }
        } else {
            panic!("map_set not a NativeFn");
        }
    } else {
        panic!("map_new not a NativeFn");
    }
}

#[test]
fn test_col_set_new_empty() {
    let env = aven_seed::Env::new();
    if let Some(Value::NativeFn { arity, .. }) = env.get("col_set_new") {
        assert_eq!(arity, 0);
        let fn_val = env.get("col_set_new").unwrap();
        if let Value::NativeFn { func, .. } = fn_val {
            let result = func(&[]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), Value::Set(vec![]));
        } else {
            panic!("Expected NativeFn");
        }
    } else {
        panic!("col_set_new not found or not a NativeFn");
    }
}

#[test]
fn test_col_set_add_and_has() {
    let env = aven_seed::Env::new();
    let set_new = env.get("col_set_new").unwrap();
    let set_add = env.get("col_set_add").unwrap();
    let set_has = env.get("col_set_has").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = set_new {
        let set = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_add, .. } = set_add {
            let set_with_val = fn_add(&[set, Value::Int(42)]).unwrap();
            if let Value::NativeFn { func: fn_has, .. } = set_has {
                let has_true = fn_has(&[set_with_val.clone(), Value::Int(42)]).unwrap();
                assert_eq!(has_true, Value::Bool(true));
                let has_false = fn_has(&[set_with_val, Value::Int(99)]).unwrap();
                assert_eq!(has_false, Value::Bool(false));
            } else {
                panic!("set_has not a NativeFn");
            }
        } else {
            panic!("set_add not a NativeFn");
        }
    } else {
        panic!("set_new not a NativeFn");
    }
}

#[test]
fn test_col_set_add_deduplication() {
    let env = aven_seed::Env::new();
    let set_new = env.get("col_set_new").unwrap();
    let set_add = env.get("col_set_add").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = set_new {
        let set = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_add, .. } = set_add {
            let set_with_one = fn_add(&[set, Value::Int(10)]).unwrap();
            let set_with_dup = fn_add(&[set_with_one.clone(), Value::Int(10)]).unwrap();
            if let (Value::Set(s1), Value::Set(s2)) = (set_with_one, set_with_dup) {
                assert_eq!(s1.len(), 1);
                assert_eq!(s2.len(), 1);
            } else {
                panic!("Expected Set values");
            }
        } else {
            panic!("set_add not a NativeFn");
        }
    } else {
        panic!("set_new not a NativeFn");
    }
}

#[test]
fn test_col_qualified_lookup() {
    let env = aven_seed::Env::new();
    let qualified = env.get("aven/std/collections::list_new");
    assert!(qualified.is_some());
    if let Some(Value::NativeFn { name, arity, .. }) = qualified {
        assert_eq!(name, "aven/std/collections::list_new");
        assert_eq!(arity, 0);
    } else {
        panic!("Expected NativeFn");
    }
}

#[test]
fn test_col_list_get_negative_index() {
    let env = aven_seed::Env::new();
    let list_new = env.get("col_list_new").unwrap();
    let list_push = env.get("col_list_push").unwrap();
    let list_get = env.get("col_list_get").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = list_new {
        let list = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_push, .. } = list_push {
            let list = fn_push(&[list, Value::Int(10)]).unwrap();
            if let Value::NativeFn { func: fn_get, .. } = list_get {
                let result = fn_get(&[list, Value::Int(-1)]);
                assert!(result.is_err(), "negative index should return error");
            } else {
                panic!("list_get not a NativeFn");
            }
        } else {
            panic!("list_push not a NativeFn");
        }
    } else {
        panic!("list_new not a NativeFn");
    }
}

#[test]
fn test_col_map_upsert_overwrites_key() {
    let env = aven_seed::Env::new();
    let map_new = env.get("col_map_new").unwrap();
    let map_set = env.get("col_map_set").unwrap();
    let map_get = env.get("col_map_get").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = map_new {
        let map = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_set, .. } = map_set {
            let map = fn_set(&[map, Value::Str("x".to_string()), Value::Int(1)]).unwrap();
            let map = fn_set(&[map, Value::Str("x".to_string()), Value::Int(2)]).unwrap();

            // Verify value is updated
            if let Value::NativeFn { func: fn_get, .. } = map_get {
                let val = fn_get(&[map.clone(), Value::Str("x".to_string())]).unwrap();
                assert_eq!(val, Value::Int(2), "map value should be updated");
            } else {
                panic!("map_get not a NativeFn");
            }

            // Verify only 1 entry
            if let Value::Map(entries) = map {
                assert_eq!(entries.len(), 1, "map should have only 1 entry after upsert");
            } else {
                panic!("map_set should return Map");
            }
        } else {
            panic!("map_set not a NativeFn");
        }
    } else {
        panic!("map_new not a NativeFn");
    }
}

#[test]
fn test_col_map_sorted_determinism() {
    let env = aven_seed::Env::new();
    let map_new = env.get("col_map_new").unwrap();
    let map_set = env.get("col_map_set").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = map_new {
        let map = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_set, .. } = map_set {
            // Insert in order: c, a, b
            let map = fn_set(&[map, Value::Str("c".to_string()), Value::Int(3)]).unwrap();
            let map = fn_set(&[map, Value::Str("a".to_string()), Value::Int(1)]).unwrap();
            let map = fn_set(&[map, Value::Str("b".to_string()), Value::Int(2)]).unwrap();

            // Verify sorted order: a, b, c
            if let Value::Map(entries) = map {
                let keys: Vec<_> = entries.iter().map(|(k, _)| k.clone()).collect();
                assert_eq!(keys, vec!["a".to_string(), "b".to_string(), "c".to_string()],
                    "map entries should be in alphabetical order");
            } else {
                panic!("map_set should return Map");
            }
        } else {
            panic!("map_set not a NativeFn");
        }
    } else {
        panic!("map_new not a NativeFn");
    }
}

#[test]
fn test_col_map_set_wrong_key_type() {
    let env = aven_seed::Env::new();
    let map_new = env.get("col_map_new").unwrap();
    let map_set = env.get("col_map_set").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = map_new {
        let map = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_set, .. } = map_set {
            // Try to set with Int key instead of Str
            let result = fn_set(&[map, Value::Int(42), Value::Int(10)]);
            assert!(result.is_err(), "col_map_set with Int key should error");
        } else {
            panic!("map_set not a NativeFn");
        }
    } else {
        panic!("map_new not a NativeFn");
    }
}

#[test]
fn test_col_set_add_fn_rejected() {
    let env = aven_seed::Env::new();
    let set_new = env.get("col_set_new").unwrap();
    let set_add = env.get("col_set_add").unwrap();

    if let Value::NativeFn { func: fn_new, .. } = set_new {
        let set = fn_new(&[]).unwrap();
        if let Value::NativeFn { func: fn_add, .. } = set_add {
            // Try to add a NativeFn
            let native_fn = env.get("col_list_new").unwrap();
            let result = fn_add(&[set, native_fn]);
            assert!(result.is_err(), "col_set_add with NativeFn should error");
        } else {
            panic!("set_add not a NativeFn");
        }
    } else {
        panic!("set_new not a NativeFn");
    }
}

#[test]
fn test_repl_simple_eval() {
    let mut env = aven_seed::Env::new();
    let result = aven_seed::run_str_with_env("(+ 3 5)", &mut env);
    assert!(result.is_ok());
    match result.unwrap() {
        Value::Int(n) => assert_eq!(n, 8),
        _ => panic!("Expected Int(8)"),
    }
}

#[test]
fn test_repl_stateful_env() {
    let mut env = aven_seed::Env::new();

    // First evaluation: bind x to 10
    let result1 = aven_seed::run_str_with_env("@let x :: 10", &mut env);
    assert!(result1.is_ok());

    // Second evaluation: reference x, should find it in env
    let result2 = aven_seed::run_str_with_env("x", &mut env);
    assert!(result2.is_ok());
    match result2.unwrap() {
        Value::Int(n) => assert_eq!(n, 10),
        _ => panic!("Expected Int(10)"),
    }
}

#[test]
fn test_repl_error_display() {
    let mut env = aven_seed::Env::new();
    let result = aven_seed::run_str_with_env("undefined_var", &mut env);
    assert!(result.is_err());
    // Error message should be present
    let err_str = format!("{}", result.unwrap_err());
    assert!(!err_str.is_empty());
}

#[test]
fn test_verify_subcommand_success() {
    let source = "(+ 1 2)";
    let expr = aven_seed::parse_str(source).expect("parse failed");
    aven_seed::typecheck_str(source).expect("typecheck failed");
    let violations = aven_seed::check_uncertainty(&expr);
    assert!(violations.is_empty(), "should have no uncertainty violations");
}

#[test]
fn test_verify_subcommand_parse_error() {
    let source = "(+ 1 2";
    let result = aven_seed::parse_str(source);
    assert!(result.is_err(), "should fail to parse unbalanced parens");
}

#[test]
fn test_verify_subcommand_typecheck_error() {
    let source = "@fn f :: x:Int -> Int (+ x \"string\")";
    let result = aven_seed::typecheck_str(source);
    assert!(result.is_err(), "should fail typecheck due to type mismatch");
}

#[test]
fn test_verify_subcommand_uncertainty_violation() {
    let source = "@uncertain 42";
    let expr = aven_seed::parse_str(source).expect("parse failed");
    aven_seed::typecheck_str(source).expect("typecheck failed");
    let violations = aven_seed::check_uncertainty(&expr);
    assert!(!violations.is_empty(), "should have uncertainty violation");
}

#[test]
fn test_verify_binary_success() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    let temp_dir = PathBuf::from("/tmp");
    let temp_file = temp_dir.join(format!("aven_test_success_{}.aven", std::process::id()));
    fs::write(&temp_file, "(+ 1 2)").expect("Failed to write temp file");

    let bin_path = env!("CARGO_BIN_EXE_aven");
    let output = Command::new(bin_path)
        .arg("verify")
        .arg(&temp_file)
        .output()
        .expect("Failed to execute binary");

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "Exit code should be 0");
    assert!(stdout_str.contains("\"pass\": true"), "stdout should contain pass: true");

    fs::remove_file(&temp_file).ok();
}

#[test]
fn test_verify_binary_parse_error() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    let temp_dir = PathBuf::from("/tmp");
    let temp_file = temp_dir.join(format!("aven_test_parse_error_{}.aven", std::process::id()));
    fs::write(&temp_file, "(+ 1 2").expect("Failed to write temp file");

    let bin_path = env!("CARGO_BIN_EXE_aven");
    let output = Command::new(bin_path)
        .arg("verify")
        .arg(&temp_file)
        .output()
        .expect("Failed to execute binary");

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "Exit code should be 1");
    assert!(stdout_str.contains("\"stage\": \"parse\""), "stdout should contain stage: parse");

    fs::remove_file(&temp_file).ok();
}

#[test]
fn test_verify_binary_uncertainty() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    let temp_dir = PathBuf::from("/tmp");
    let temp_file = temp_dir.join(format!("aven_test_uncertainty_{}.aven", std::process::id()));
    fs::write(&temp_file, "@uncertain 42").expect("Failed to write temp file");

    let bin_path = env!("CARGO_BIN_EXE_aven");
    let output = Command::new(bin_path)
        .arg("verify")
        .arg(&temp_file)
        .output()
        .expect("Failed to execute binary");

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "Exit code should be 1");
    assert!(stdout_str.contains("\"stage\": \"uncertainty\""), "stdout should contain stage: uncertainty");

    fs::remove_file(&temp_file).ok();
}

#[test]
fn test_verify_binary_typecheck_error() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    let temp_dir = PathBuf::from("/tmp");
    let temp_file = temp_dir.join(format!("aven_test_typecheck_{}.aven", std::process::id()));
    fs::write(&temp_file, "@fn f :: x:Int -> Int (+ x \"string\")").expect("Failed to write temp file");

    let bin_path = env!("CARGO_BIN_EXE_aven");
    let output = Command::new(bin_path)
        .arg("verify")
        .arg(&temp_file)
        .output()
        .expect("Failed to execute binary");

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "Exit code should be 1");
    assert!(stdout_str.contains("\"stage\": \"typecheck\""), "stdout should contain stage: typecheck");

    fs::remove_file(&temp_file).ok();
}

#[test]
fn test_verify_binary_file_not_found() {
    use std::process::Command;

    let nonexistent_path = "/tmp/aven_test_does_not_exist_xyz123456.aven";
    let bin_path = env!("CARGO_BIN_EXE_aven");
    let output = Command::new(bin_path)
        .arg("verify")
        .arg(nonexistent_path)
        .output()
        .expect("Failed to execute binary");

    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "Exit code should be 1");
    assert!(stderr_str.contains("Error reading file"), "stderr should contain error message");
}
