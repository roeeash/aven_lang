"""
Integration tests for Python→AVEN bridge.

Tests the full pipeline: Python AST → AVEN verification → violations.
"""

import unittest
import os
from aven_guard._python_bridge import PythonToAVENBridge
from aven_guard._check import check_python_source
from aven_guard._capability_map import caps_for_module


AVEN_BINARY = os.environ.get("AVEN_BINARY")


class TestPythonBridge(unittest.TestCase):
    def setUp(self):
        """Set environment for each test."""
        if not AVEN_BINARY:
            self.skipTest("AVEN_BINARY not set — build the aven binary and set AVEN_BINARY")
        os.environ["AVEN_BINARY"] = AVEN_BINARY
        self.bridge = PythonToAVENBridge()

    def test_eval_dynamic_arg_detected_as_uncertain(self):
        """eval() with a dynamic (non-literal) argument produces @uncertain."""
        src = "result = eval(user_input)"
        aven = self.bridge.source_to_aven_string(src)
        self.assertIn("@uncertain", aven)

    def test_subprocess_call_flagged(self):
        """Test that subprocess.run() is flagged as violation."""
        src = "import subprocess\nsubprocess.run(['ls'])"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)

    def test_subprocess_alias_bypass_caught(self):
        """Aliased subprocess import (import subprocess as sp; sp.run) is flagged."""
        src = "import subprocess as sp\nsp.run(['ls'])"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)

    def test_import_os_maps_to_caps(self):
        """Test that os module maps to correct capabilities."""
        caps = caps_for_module("os")
        self.assertIn("read", caps)
        self.assertIn("write", caps)
        self.assertIn("list", caps)

    def test_mixed_violations(self):
        """Test source with both a dynamic eval() and subprocess.run()."""
        src = "eval(user_input)\nimport subprocess\nsubprocess.run(['ls'])"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreaterEqual(len(result["violations"]), 2)
        checks = [v["check"] for v in result["violations"]]
        self.assertTrue(any("uncertainty" in c for c in checks))

    def test_clean_source_passes(self):
        """Test that safe Python code passes verification."""
        src = "x = 1 + 2\nprint(x)"
        result = check_python_source(src)
        self.assertTrue(result["pass"])

    def test_from_import_subprocess_classified_as_capability(self):
        """from subprocess import run; run(['ls']) must be capability, not eval/exec."""
        src = "from subprocess import run\nrun(['ls'])"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)
        checks = [v["check"] for v in result["violations"]]
        self.assertTrue(all(c == "capability" for c in checks), f"Expected all capability, got: {checks}")

    def test_os_remove_flagged(self):
        """import os; os.remove() must be flagged as capability violation."""
        src = "import os\nos.remove('/tmp/x')"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)
        checks = [v["check"] for v in result["violations"]]
        self.assertTrue(all(c == "capability" for c in checks), f"Expected capability, got: {checks}")

    def test_os_system_flagged(self):
        """import os; os.system() must be flagged as capability violation."""
        src = "import os\nos.system('ls')"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        checks = [v["check"] for v in result["violations"]]
        self.assertTrue(all(c == "capability" for c in checks), f"Expected capability, got: {checks}")

    def test_from_os_import_remove_flagged(self):
        """from os import remove; remove() must be flagged as capability violation."""
        src = "from os import remove\nremove('/tmp/x')"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)
        checks = [v["check"] for v in result["violations"]]
        self.assertTrue(all(c == "capability" for c in checks), f"Expected capability, got: {checks}")

    def test_eval_with_clean_literal_passes(self):
        """eval() with a clean literal string argument must pass verification."""
        src = "eval('1+1')"
        result = check_python_source(src)
        self.assertTrue(result["pass"], f"Expected pass, got violations: {result['violations']}")
        self.assertEqual(result["violations"], [])

    def test_eval_with_dynamic_arg_fails_as_uncertainty(self):
        """eval() with a dynamic argument must fail with check=uncertainty."""
        src = "eval(user_input)"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)
        checks = [v["check"] for v in result["violations"]]
        self.assertTrue(all(c == "uncertainty" for c in checks), f"Expected uncertainty, got: {checks}")

    def test_eval_with_dangerous_inner_fails(self):
        """eval() whose literal string contains a dangerous call must fail."""
        src = "eval(\"import os; os.remove('/x')\")"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)

    def test_eval_with_syntax_error_inner_fails(self):
        """eval() whose literal string is invalid Python must fail."""
        src = "eval('def broken(')"
        result = check_python_source(src)
        self.assertFalse(result["pass"])

    def test_eval_with_nested_dynamic_eval_fails(self):
        """eval() whose literal string contains an eval() with a dynamic arg must fail."""
        src = "eval(\"eval(user_input)\")"
        result = check_python_source(src)
        self.assertFalse(result["pass"])

    def test_exec_with_clean_literal_passes(self):
        """exec() with a clean literal string must pass verification."""
        src = "exec('x = 1 + 2')"
        result = check_python_source(src)
        self.assertTrue(result["pass"], f"Expected pass, got violations: {result['violations']}")

    def test_os_alias_flagged(self):
        """import os as o; o.remove() must be flagged as capability violation."""
        src = "import os as o\no.remove('/tmp/x')"
        result = check_python_source(src)
        self.assertFalse(result["pass"])
        self.assertGreater(len(result["violations"]), 0)
        checks = [v["check"] for v in result["violations"]]
        self.assertTrue(all(c == "capability" for c in checks), f"Expected capability, got: {checks}")

    def test_syntax_error_in_python_source(self):
        """Test graceful handling of invalid Python syntax."""
        src = "def broken("
        result = check_python_source(src)
        # Should return structured dict, not crash
        self.assertIsInstance(result, dict)
        self.assertIn("pass", result)
        # Syntax errors should result in @uncertain, which fails verification
        self.assertFalse(result["pass"])


if __name__ == "__main__":
    unittest.main()
