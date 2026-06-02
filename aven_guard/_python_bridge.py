"""
Python→AVEN bridge: AST-based source translator.

Walks Python AST to detect dangerous patterns and unsafe imports,
generates minimal AVEN source with @uncertain annotations,
and orchestrates verification via AVENExecutionEngine.
"""

import ast
from aven_guard._capability_map import caps_for_module


# Known limitation: simple aliasing (e = eval; e("x")) is not detected.
# Full alias tracking requires data-flow analysis beyond AST scope.

# eval/exec with a literal string argument are recursively verified rather than
# unconditionally flagged. compile/__import__ have no useful literal fast-path.
EVAL_EXEC_BUILTINS = {"eval", "exec"}
OPAQUE_BUILTINS = {"__import__", "compile"}
DANGEROUS_BUILTINS = EVAL_EXEC_BUILTINS | OPAQUE_BUILTINS  # kept for external callers

SUBPROCESS_DANGEROUS_ATTRS = {"run", "call", "Popen", "check_output", "check_call"}
OS_DANGEROUS_ATTRS = {
    "system", "popen", "remove", "unlink", "rename", "chmod",
    "mkdir", "rmdir", "makedirs", "removedirs",
}

# Scope: only subprocess and os are actively inspected in Stage 1.
# To extend, add the module name to the appropriate *_DANGEROUS_ATTRS set
# and ensure caps_for_module covers it.
_MODULE_ATTR_SETS = {
    "subprocess": SUBPROCESS_DANGEROUS_ATTRS,
    "os": OS_DANGEROUS_ATTRS,
}


class PythonToAVENBridge:
    """Translates Python source to AVEN for verification."""

    def __init__(self):
        self._violations = []

    def _check_eval_arg(self, call_node: ast.Call) -> bool:
        """Return True (uncertain) if eval/exec argument cannot be statically verified clean.

        Attempts to extract a literal string argument, parse it, and recursively
        run it through a fresh bridge. Returns False only when the inner code is
        confirmed clean by the bridge (no violations).
        """
        if len(call_node.args) != 1 or call_node.keywords:
            return True  # multiple args or kwargs — can't inspect safely
        arg = call_node.args[0]
        if not (isinstance(arg, ast.Constant) and isinstance(arg.value, str)):
            return True  # dynamic argument — can't inspect
        inner_src = arg.value
        try:
            ast.parse(inner_src)
        except SyntaxError:
            return True  # inner string is not valid Python
        inner_bridge = PythonToAVENBridge()
        inner_aven = inner_bridge.source_to_aven_string(inner_src)
        if inner_aven is None:
            return False  # inner code is clean
        # Propagate inner violations, annotated as originating inside an eval
        for lineno, col, kind, check_type in inner_bridge._violations:
            self._violations.append((lineno, col, f"eval-inner:{kind}", check_type))
        return True

    def source_to_aven_string(self, python_code: str) -> str:
        """Convert Python source to AVEN representation.

        Returns an AVEN source string with @uncertain annotations for dangerous
        patterns, None when no violations are found (caller short-circuits to pass),
        or "@uncertain 0" on SyntaxError.
        """
        self._violations = []

        try:
            tree = ast.parse(python_code)
        except SyntaxError:
            self._violations = [(0, 0, "syntax_error", "parse")]
            return "@uncertain 0"

        lines = []
        dangerous_names = {}  # name → canonical_module (from-imports)
        module_aliases = {}   # alias → canonical_module (import ... as ...)

        # Two-pass: first pass collects aliases/from-imports; second detects calls.
        # Kept separate for clarity; O(2n) is acceptable for typical source sizes.

        # First pass: collect module aliases and dangerous from-import names
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    canonical = alias.name.split(".")[0]
                    asname = alias.asname or alias.name
                    if canonical in _MODULE_ATTR_SETS:
                        module_aliases[asname] = canonical
            elif isinstance(node, ast.ImportFrom):
                canonical_mod = (node.module or "").split(".")[0]
                dangerous_verb_set = _MODULE_ATTR_SETS.get(canonical_mod, set())
                for alias in node.names:
                    if alias.name in dangerous_verb_set:
                        name = alias.asname or alias.name
                        dangerous_names[name] = canonical_mod

        # Second pass: detect dangerous call patterns
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call):
                continue

            func_name = None
            matched = False

            if isinstance(node.func, ast.Attribute) and isinstance(node.func.value, ast.Name):
                base = node.func.value.id
                attr = node.func.attr
                canonical = module_aliases.get(base, base)
                verb_set = _MODULE_ATTR_SETS.get(canonical, set())
                if verb_set and attr in verb_set:
                    caps = caps_for_module(canonical)
                    violation_kind = f"{canonical}:{','.join(caps)}"
                    self._violations.append((node.lineno, node.col_offset, violation_kind, "capability"))
                    lines.append(f"@uncertain {node.lineno}")
                    matched = True

            if not matched and isinstance(node.func, ast.Name):
                func_name = node.func.id

            if func_name in OPAQUE_BUILTINS:
                self._violations.append((node.lineno, node.col_offset, "eval/exec", "uncertainty"))
                lines.append(f"@uncertain {node.lineno}")
            elif func_name in EVAL_EXEC_BUILTINS:
                if self._check_eval_arg(node):
                    self._violations.append((node.lineno, node.col_offset, "eval/exec", "uncertainty"))
                    lines.append(f"@uncertain {node.lineno}")
            elif func_name in dangerous_names:
                canonical = dangerous_names[func_name]
                caps = caps_for_module(canonical)
                violation_kind = f"{canonical}:{','.join(caps)}"
                self._violations.append((node.lineno, node.col_offset, violation_kind, "capability"))
                lines.append(f"@uncertain {node.lineno}")

        if not lines:
            return None  # sentinel: no violations; caller returns pass=True directly

        return "\n".join(lines)
