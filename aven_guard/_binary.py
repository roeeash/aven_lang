import os
import shutil
from pathlib import Path


def locate_binary() -> str:
    env_path = os.environ.get("AVEN_BINARY")
    if env_path:
        if Path(env_path).exists():
            return env_path
        raise RuntimeError(f"AVEN_BINARY is set but not found at {env_path}")
    found = shutil.which("aven")
    if found:
        return found
    raise RuntimeError(
        "aven binary not found. Set AVEN_BINARY env var or install aven."
    )
