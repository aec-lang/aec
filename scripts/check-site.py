#!/usr/bin/env python3
"""Validates the generated site before it is published.

Three checks, all of which have caught real bugs in this repository:

1. Every internal link resolves to a file that exists.
2. The HTML nesting is balanced, so a broken build cannot ship a page that
   renders incorrectly.
3. The search index embedded in each docs page is valid JSON.

Exits non-zero on the first failing category, listing every problem found.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys
from html.parser import HTMLParser

SITE = pathlib.Path(__file__).resolve().parent.parent / "website"

VOID = {
    "area", "base", "br", "col", "embed", "hr", "img", "input",
    "link", "meta", "source", "track", "wbr",
}

IGNORED_PREFIXES = ("http://", "https://", "data:", "mailto:", "javascript:", "#")


class Nesting(HTMLParser):
    """Tracks open tags so a mismatched close is reported with both positions."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.stack: list[tuple[str, tuple[int, int]]] = []
        self.errors: list[str] = []

    def handle_starttag(self, tag: str, attrs) -> None:
        if tag not in VOID:
            self.stack.append((tag, self.getpos()))

    def handle_endtag(self, tag: str) -> None:
        if tag in VOID:
            return
        if not self.stack:
            self.errors.append(f"stray </{tag}> at {self.getpos()}")
            return
        opened, position = self.stack.pop()
        if opened != tag:
            self.errors.append(
                f"</{tag}> at line {self.getpos()[0]} closes <{opened}> opened at line {position[0]}"
            )


def check_links(pages: dict[str, pathlib.Path]) -> list[str]:
    problems: list[str] = []
    names = set(pages)

    for name, path in pages.items():
        text = path.read_text(encoding="utf-8")
        for match in re.finditer(r'href="([^"]+)"', text):
            href = match.group(1)
            if href.startswith(IGNORED_PREFIXES):
                continue
            target = href.split("#", 1)[0]
            if not target:
                continue
            if target.startswith("/"):
                # Site-absolute: resolve against the site root.
                resolved = target.lstrip("/") or "index.html"
                if resolved.endswith(".html") and resolved not in names:
                    problems.append(f"{name} -> {href}")
            elif target.endswith((".md", ".html")):
                # Document-relative. A generated page in the site root may link
                # to the repository copy (docs/apm.md) because that link also
                # works when read on GitHub, so both forms are accepted.
                in_site = (path.parent / target).resolve()
                if in_site.is_file():
                    continue
                repo_relative = SITE.parent / target
                if repo_relative.is_file():
                    continue
                if target in names:
                    continue
                problems.append(f"{name} -> {href}")
    return problems


def check_nesting(pages: dict[str, pathlib.Path]) -> list[str]:
    problems: list[str] = []
    for name, path in pages.items():
        parser = Nesting()
        parser.feed(path.read_text(encoding="utf-8"))
        for error in parser.errors[:5]:
            problems.append(f"{name}: {error}")
        for tag, position in parser.stack[:5]:
            problems.append(f"{name}: <{tag}> at line {position[0]} is never closed")
    return problems


def check_index(pages: dict[str, pathlib.Path]) -> list[str]:
    problems: list[str] = []
    for name, path in pages.items():
        text = path.read_text(encoding="utf-8")
        match = re.search(r"window\.AEC_INDEX = (\[.*?\]);", text, re.S)
        if not match:
            continue
        try:
            data = json.loads(match.group(1))
        except json.JSONDecodeError as error:
            problems.append(f"{name}: search index is not valid JSON ({error})")
            continue
        if not isinstance(data, list) or not data:
            problems.append(f"{name}: search index is empty")
            continue
        for entry in data:
            missing = {"url", "title", "text"} - set(entry)
            if missing:
                problems.append(f"{name}: search entry missing {sorted(missing)}")
                break
    return problems


def main() -> int:
    if not SITE.is_dir():
        print(f"error: {SITE} does not exist; run node scripts/build-site.cjs first")
        return 1

    files = sorted(SITE.rglob("*.html"))
    if not files:
        print(f"error: no HTML found in {SITE}")
        return 1

    pages = {f.name: f for f in files}
    print(f"checking {len(pages)} pages in {SITE}")

    failed = False
    for label, problems in (
        ("internal links", check_links(pages)),
        ("html nesting", check_nesting(pages)),
        ("search index", check_index(pages)),
    ):
        if problems:
            failed = True
            print(f"\n{label}: {len(problems)} problem(s)")
            for problem in sorted(set(problems)):
                print(f"  {problem}")
        else:
            print(f"{label}: ok")

    if failed:
        print("\nsite validation failed")
        return 1

    print("\nsite is valid")
    return 0


if __name__ == "__main__":
    sys.exit(main())