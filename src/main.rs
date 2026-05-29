use aven_seed::Env;
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check for "verify" subcommand
    if args.len() >= 2 && args[1] == "verify" {
        return run_verify(&args);
    }

    // Check for "check-uncertainty" subcommand
    if args.len() >= 2 && args[1] == "check-uncertainty" {
        return run_check_uncertainty(&args);
    }

    // Check for "intent" subcommand
    if args.len() >= 2 && args[1] == "intent" {
        return run_intent(&args);
    }

    // Run REPL
    run_repl();
}

fn run_repl() {
    let stdin = io::stdin();
    let mut env = Env::new();
    let mut accumulated = String::new();
    let mut paren_depth = 0i32;

    eprint!("aven> ");
    let _ = io::stderr().flush();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        // Count paren depth, skipping chars inside string literals and ; comments.
        let mut in_string = false;
        let mut prev_backslash = false;
        for ch in line.chars() {
            if in_string {
                if prev_backslash {
                    prev_backslash = false;
                } else if ch == '\\' {
                    prev_backslash = true;
                } else if ch == '"' {
                    in_string = false;
                }
            } else if ch == ';' {
                break; // rest of line is a comment
            } else if ch == '"' {
                in_string = true;
                prev_backslash = false;
            } else if ch == '(' {
                paren_depth += 1;
            } else if ch == ')' {
                paren_depth -= 1;
            }
        }

        if !accumulated.is_empty() {
            accumulated.push('\n');
        }
        accumulated.push_str(&line);

        if paren_depth < 0 {
            // Unbalanced close parens — report error and reset
            eprintln!("error: unbalanced closing parenthesis");
            accumulated.clear();
            paren_depth = 0;
            eprint!("aven> ");
        } else if paren_depth == 0 && !accumulated.trim().is_empty() {
            // evaluate
            match aven_seed::run_str_with_env(&accumulated, &mut env) {
                Ok(val) => println!("{}", val),
                Err(e) => eprintln!("error: {}", e),
            }
            accumulated.clear();
            paren_depth = 0;
            eprint!("aven> ");
        } else if !accumulated.trim().is_empty() {
            eprint!("...> ");
        } else {
            eprint!("aven> ");
        }
        let _ = io::stderr().flush();
    }
}

fn run_check_uncertainty(args: &[String]) {
    if args.len() < 3 {
        eprintln!("Usage: aven check-uncertainty <file>");
        std::process::exit(1);
    }

    let path = &args[2];
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    let expr = match aven_seed::parse_str(&source) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    let violations = aven_seed::check_uncertainty(&expr);
    if violations.is_empty() {
        println!("No @uncertain annotations found.");
        std::process::exit(0);
    }

    for v in &violations {
        let (line, col) = aven_seed::source_to_line_col(&source, v.span.start);
        println!("{}:{}: @uncertain at path '/{}'", line, col, v.path);
    }

    eprintln!(
        "Found {} @uncertain annotation(s).",
        violations.len()
    );
    std::process::exit(1);
}

fn run_intent(args: &[String]) {
    let source = if args.len() >= 3 {
        // Read from file
        let path = &args[2];
        match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading file '{}': {}", path, e);
                std::process::exit(1);
            }
        }
    } else {
        // Read from stdin
        use std::io::Read;
        let mut buffer = String::new();
        if io::stdin().read_to_string(&mut buffer).is_err() {
            eprintln!("Error reading from stdin");
            std::process::exit(1);
        }
        buffer
    };

    let intent_table = match aven_seed::intent_index(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    if intent_table.entries.is_empty() {
        std::process::exit(0);
    }

    // Format and print intent entries with stable sort by selector + source position.
    let formatted = aven_seed::format_intent_output(&intent_table, &source);
    for line in formatted {
        println!("{}", line);
    }

    std::process::exit(0);
}

fn run_verify(args: &[String]) {
    if args.len() < 3 {
        eprintln!("Usage: aven verify <file>");
        std::process::exit(1);
    }

    let path = &args[2];
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    // Parse
    let expr = match aven_seed::parse_str(&source) {
        Ok(e) => e,
        Err(e) => {
            let error_json = format!(
                "{{\"file\": \"{}\", \"pass\": false, \"errors\": [{{\"stage\": \"parse\", \"message\": \"{}\"}}]}}",
                escape_json_string(path),
                escape_json_string(&e.to_string())
            );
            println!("{}", error_json);
            std::process::exit(1);
        }
    };

    // Typecheck
    if let Err(e) = aven_seed::typecheck_str(&source) {
        let error_json = format!(
            "{{\"file\": \"{}\", \"pass\": false, \"errors\": [{{\"stage\": \"typecheck\", \"message\": \"{}\"}}]}}",
            escape_json_string(path),
            escape_json_string(&e.message)
        );
        println!("{}", error_json);
        std::process::exit(1);
    }

    // Check uncertainty
    let violations = aven_seed::check_uncertainty(&expr);
    if !violations.is_empty() {
        let mut paths = Vec::new();
        for v in violations.iter() {
            paths.push(format!("/{}", v.path));
        }
        let message = format!("uncertain annotations at: {}", paths.join(", "));
        let error_json = format!(
            "{{\"file\": \"{}\", \"pass\": false, \"errors\": [{{\"stage\": \"uncertainty\", \"message\": \"{}\"}}]}}",
            escape_json_string(path),
            escape_json_string(&message)
        );
        println!("{}", error_json);
        std::process::exit(1);
    }

    // Success
    let success_json = format!(
        "{{\"file\": \"{}\", \"pass\": true, \"errors\": []}}",
        escape_json_string(path)
    );
    eprintln!("OK: {}", path);
    println!("{}", success_json);
    std::process::exit(0);
}

fn escape_json_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}
