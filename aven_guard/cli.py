import argparse
import json
import sys
from aven_guard._engine import AVENExecutionEngine


def main():
    parser = argparse.ArgumentParser(prog="aven-guard")
    sub = parser.add_subparsers(dest="command")

    check = sub.add_parser("check", help="Verify AVEN source")
    group = check.add_mutually_exclusive_group(required=True)
    group.add_argument("file", nargs="?", help="Path to .aven file")
    group.add_argument("--source", metavar="TEXT", help="Inline AVEN source string")
    check.add_argument("--lang", choices=["aven", "python"], default=None, help="Language of input (auto-detected from extension)")
    check.add_argument("--json", action="store_true", help="Output raw JSON")

    args = parser.parse_args()

    if args.command != "check":
        parser.print_help()
        sys.exit(1)

    try:
        # Detect language
        lang = args.lang
        if lang is None and args.file and args.file.endswith(".py"):
            lang = "python"

        if lang == "python":
            from aven_guard._check import check_python_source
            if args.source is not None:
                result = check_python_source(args.source)
            else:
                with open(args.file, "r", encoding="utf-8") as f:
                    content = f.read()
                result = check_python_source(content)
            # result already contains both "errors" and "violations" keys
        else:
            # existing AVEN path
            engine = AVENExecutionEngine()
            if args.source is not None:
                result = engine.verify_source(args.source)
            else:
                result = engine.verify_file(args.file)
    except (FileNotFoundError, RuntimeError) as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)

    if args.json:
        print(json.dumps(result))
    else:
        if result["pass"]:
            print("PASS")
        else:
            print("FAIL")
            for e in result.get("errors", []):
                stage = e.get("stage", "?")
                msg = e.get("message", "?")
                print(f"  stage: {stage} — message: {msg}")

    sys.exit(0 if result["pass"] else 1)


if __name__ == "__main__":
    main()
