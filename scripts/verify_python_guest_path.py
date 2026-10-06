"""Print the readable Hyperlight Python guest artifact path."""

from pathlib import Path

from python_guest.path import get_module_path


def main() -> None:
    module_path = Path(get_module_path())
    if not module_path.is_file():
        raise SystemExit(f"Python guest artifact is not a file: {module_path}")
    try:
        with module_path.open("rb"):
            pass
    except OSError as error:
        raise SystemExit(
            f"Python guest artifact is not readable: {module_path}: {error}"
        ) from error
    print(module_path)


if __name__ == "__main__":
    main()
