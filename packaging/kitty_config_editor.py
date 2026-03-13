#!/usr/bin/env python3

import os
import shutil
import sys
from pathlib import Path


def _candidate_binaries():
    env_bin = os.environ.get("KITTY_CONFIG_EDITOR_BIN")
    here = Path(__file__).resolve()
    home = Path.home()

    candidates = [
        env_bin,
        str(home / ".local" / "bin" / "kitty-config-editor"),
        str(home / ".cargo" / "bin" / "kitty-config-editor"),
        str((here.parent.parent / "target" / "release" / "kitty-config-editor").resolve()),
        str((here.parent.parent / "target" / "debug" / "kitty-config-editor").resolve()),
        shutil.which("kitty-config-editor"),
    ]

    seen = set()
    for candidate in candidates:
        if not candidate:
            continue
        candidate = os.path.expanduser(candidate)
        if candidate in seen:
            continue
        seen.add(candidate)
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            yield candidate


def _find_binary():
    binary = next(_candidate_binaries(), None)
    if binary is not None:
        return binary

    print(
        "kitty-config-editor binary not found. Set KITTY_CONFIG_EDITOR_BIN or install the binary to "
        "~/.local/bin/kitty-config-editor.",
        file=sys.stderr,
    )
    raise SystemExit(127)


def main(args):
    binary = _find_binary()
    env = os.environ.copy()
    env["KITTY_CONFIG_EDITOR_LAUNCHED_BY"] = "kitty-kitten"
    argv = [binary, "--runtime=kitten", *args]
    os.execvpe(binary, argv, env)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
