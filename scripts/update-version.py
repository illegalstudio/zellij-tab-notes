#!/usr/bin/env python3
"""Update the workspace version and pinned README example from a release tag."""
from pathlib import Path
import re
import sys


def main():
    if len(sys.argv) != 2 or not re.fullmatch(
        r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", sys.argv[1]
    ):
        sys.exit("usage: python3 scripts/update-version.py 1.2.3")

    version = sys.argv[1]
    root = Path(__file__).resolve().parent.parent
    manifest = root / "Cargo.toml"
    readme = root / "README.md"
    manifest_text, manifest_count = re.subn(
        r'(\[workspace\.package\]\s*\nversion = ")[^"]*(")',
        lambda match: f"{match[1]}{version}{match[2]}",
        manifest.read_text(),
    )
    readme_text, url_count = re.subn(
        r"(https://github\.com/illegalstudio/zellij-tab-notes/releases/download/)v[^/]+(/tab-notes\.wasm)",
        lambda match: f"{match[1]}v{version}{match[2]}",
        readme.read_text(),
    )
    readme_text, pin_count = re.subn(
        r"(The snippet above pins `)v[^`]+(`)",
        lambda match: f"{match[1]}v{version}{match[2]}",
        readme_text,
    )
    if manifest_count != 1 or url_count != 2 or pin_count != 1:
        sys.exit("error: expected one workspace version, two release URLs and one README pin")

    manifest.write_text(manifest_text)
    readme.write_text(readme_text)


if __name__ == "__main__":
    main()
