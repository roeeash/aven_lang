"""
Orchestrator: Python source verification via AVEN engine.

Takes Python source, converts to AVEN via PythonToAVENBridge,
runs verification, and returns structured violations.
"""

from aven_guard._python_bridge import PythonToAVENBridge
from aven_guard._engine import AVENExecutionEngine


def check_python_source(source: str, binary_path: str = None) -> dict:
    """Verify Python source code for unsafe patterns.

    Args:
        source: Python source code string
        binary_path: Optional path to aven binary

    Returns:
        dict with keys:
            - file: "<source>"
            - pass: bool
            - errors: list of dicts with line/col/check/stage/message
            - violations: same list, aliased for semantic clarity
    """
    bridge = PythonToAVENBridge()
    aven_src = bridge.source_to_aven_string(source)

    # Short-circuit: bridge found no violations, skip AVEN engine entirely
    if aven_src is None:
        return {"file": "<source>", "pass": True, "errors": [], "violations": []}

    engine = AVENExecutionEngine(binary_path=binary_path) if binary_path else AVENExecutionEngine()
    result = engine.verify_source(aven_src)

    violations = []
    if not result.get("pass", False):
        if bridge._violations:
            for lineno, col, kind, check_type in bridge._violations:
                if check_type == "parse":
                    msg = "Python syntax error — could not parse source"
                else:
                    msg = f"dangerous pattern detected: {kind} at line {lineno}"
                violations.append({
                    "line": lineno,
                    "col": col,
                    "check": check_type,
                    "stage": check_type,
                    "message": msg,
                })
        else:
            for err in result.get("errors", []):
                violations.append({
                    "line": None,
                    "col": None,
                    "check": err.get("stage", "unknown"),
                    "stage": err.get("stage", "unknown"),
                    "message": err.get("message", ""),
                })

    return {
        "file": "<source>",
        "pass": result.get("pass", False),
        "errors": violations,
        "violations": violations,
    }
