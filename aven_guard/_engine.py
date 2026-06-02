"""
AVENExecutionEngine — Python bridge to the aven verify subcommand.

Binary location priority:
1. AVEN_BINARY env var
2. Explicit binary_path argument to __init__
3. 'aven' on PATH via locate_binary()

Raises RuntimeError if no binary found.
"""
import json
import os
import subprocess
import tempfile
import shutil
from pathlib import Path

class AVENExecutionEngine:
    def __init__(self, binary_path=None):
        # determine binary: env var > explicit path > PATH (shutil.which)
        # Each step silently skips to the next if the file doesn't exist.
        env_path = os.environ.get("AVEN_BINARY")
        if env_path and Path(env_path).exists():
            path = env_path
        elif binary_path:
            path = binary_path
            if not Path(path).exists():
                raise RuntimeError(f"aven binary not found at {path}")
        else:
            found = shutil.which("aven")
            if not found:
                raise RuntimeError("aven binary not found. Set AVEN_BINARY env var or install aven.")
            path = found
        self._binary = str(path)

    def verify_source(self, source_str):
        """Write source to a temp file and run aven verify on it."""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".aven", delete=False, encoding="utf-8"
        ) as tmp:
            tmp.write(source_str)
            tmp_path = tmp.name
        try:
            result = self._run_verify(tmp_path)
            result["file"] = "<source>"
            return result
        finally:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass

    def verify_file(self, path):
        """Run aven verify on an existing file."""
        if not Path(path).exists():
            raise FileNotFoundError(f"File not found: {path}")
        result = self._run_verify(str(path))
        result["file"] = str(path)
        return result

    def _run_verify(self, path):
        try:
            result = subprocess.run(
                [self._binary, "verify", path],
                capture_output=True,
                text=True,
                timeout=5,
            )
            stdout = result.stdout.strip()
            if stdout:
                try:
                    parsed = json.loads(stdout)
                    # Validate required keys exist
                    if not isinstance(parsed, dict) or "pass" not in parsed or "errors" not in parsed:
                        return {
                            "file": path,
                            "pass": False,
                            "errors": [{"stage": "process", "message": "JSON missing required keys (pass, errors)"}],
                        }
                    # Validate pass is bool
                    if not isinstance(parsed["pass"], bool):
                        return {
                            "file": path,
                            "pass": False,
                            "errors": [{"stage": "process", "message": f"JSON pass field must be bool, got {type(parsed['pass']).__name__}"}],
                        }
                    # Validate errors is list of dicts with stage/message
                    if not isinstance(parsed["errors"], list):
                        return {
                            "file": path,
                            "pass": False,
                            "errors": [{"stage": "process", "message": f"JSON errors field must be list, got {type(parsed['errors']).__name__}"}],
                        }
                    if any(not isinstance(e, dict) for e in parsed["errors"]):
                        return {
                            "file": path,
                            "pass": False,
                            "errors": [{"stage": "process", "message": "JSON errors list contains non-dict element"}],
                        }
                    # Check for mismatch between pass/returncode
                    pass_value = parsed["pass"]
                    if (pass_value and result.returncode != 0) or (not pass_value and result.returncode == 0):
                        return {
                            "file": path,
                            "pass": False,
                            "errors": [{"stage": "process", "message": f"JSON pass field ({pass_value}) contradicts exit code ({result.returncode})"}],
                        }
                    return parsed
                except json.JSONDecodeError:
                    pass
            # Fallback: construct error dict from exit code and stderr
            if result.returncode != 0:
                stderr_msg = result.stderr.strip() or f"process exited with code {result.returncode}"
                return {
                    "file": path,
                    "pass": False,
                    "errors": [{"stage": "process", "message": stderr_msg}],
                }
            # returncode == 0 but no valid JSON: contract violation
            return {
                "file": path,
                "pass": False,
                "errors": [{"stage": "process", "message": "aven verify exited 0 but produced no valid JSON output"}],
            }
        except (subprocess.TimeoutExpired, OSError) as e:
            msg = "verify timed out after 5s" if isinstance(e, subprocess.TimeoutExpired) else str(e)
            stage = "timeout" if isinstance(e, subprocess.TimeoutExpired) else "process"
            return {
                "file": path,
                "pass": False,
                "errors": [{"stage": stage, "message": msg}],
            }
