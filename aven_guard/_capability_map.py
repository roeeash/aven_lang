"""
Python import module → AVEN capability mapping.

Maps standard library module names to the capabilities they require.
"""

PYTHON_IMPORT_CAPS = {
    "os": ["read", "write", "list"],
    "os.path": ["read"],
    "subprocess": ["exec"],
    "socket": ["net"],
    "requests": ["net"],
    "urllib": ["net"],
    "urllib.request": ["net"],
    "http": ["net"],
    "ftplib": ["net"],
    "smtplib": ["net"],
    "json": ["read"],
    "pickle": ["read", "write"],
    "shelve": ["read", "write"],
    "sqlite3": ["read", "write"],
    "csv": ["read", "write"],
    "pathlib": ["read", "write", "list"],
    "shutil": ["read", "write", "list"],
    "tempfile": ["write"],
    "io": ["read", "write"],
    "sys": ["exec"],
    "importlib": ["exec"],
    "ctypes": ["exec"],
    "cffi": ["exec"],
}

FALLBACK_CAPS = ["read"]


def caps_for_module(name: str) -> list:
    """Get capabilities required for a module.

    Returns the capability list for the given module name,
    or FALLBACK_CAPS if the module is not in the mapping.
    """
    return PYTHON_IMPORT_CAPS.get(name, FALLBACK_CAPS)
