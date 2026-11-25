#!/usr/bin/env python3
"""THE SYNTAX, checked on the lines a change adds.

AGENTS.md § 0 says every widget and component is one builder expression: parts found by name,
nothing written by hand, no hand wiring. Saying it was not enough — the same breaks came back in
review again and again, each one found by a person. This finds the ones a machine can see, on the
lines a branch adds, so the author hears about it before anyone has to read the diff.

    scripts/syntax-check.py                    # this repo, against main
    scripts/syntax-check.py --base develop     # against another branch
    scripts/syntax-check.py --repo ../heca-pro --base develop

Only added lines count: code that was already there does not block a change that did not touch it.
A deliberate exception says why, on the line itself:

    let first = row.children[0];  // syntax-ok: the test reads the first cell it just built

Exit 1 when something is found.
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

# Where the library builds the machinery the rules point to. Code here is how a builder, a layout
# pass or the reactive system is written, not a use of them.
MACHINERY = (
    "heca-grid-ui/src/builders/",
    "heca-grid-ui/src/component/",
    "heca-grid-ui/src/layout/",
    "heca-grid-ui/src/reactive/",
    "heca-grid-ui/src/scene/",
    "heca-grid-ui/src/style/",
    "heca-grid-ui-macros/",
)

# Where numbers and colours are supposed to live.
THEME = ("heca-theme/",)

SIZE_BUILDERS = (
    "width|height|min_width|min_height|max_width|max_height|gap|row_gap|column_gap|"
    "padding|padding_x|padding_y|padding_top|padding_bottom|padding_left|padding_right|"
    "margin|margin_x|margin_y|radius|font_size|border_width"
)

# A number that is not zero.
NUMBER = r"-?(0*[1-9]\d*(\.\d+)?|0*\.\d*[1-9]\d*)"

RULES = [
    (
        "child-by-position",
        re.compile(r"children\s*\[\s*\d|children\.(get|get_mut)\(\s*\d|children\.(first|last)(_mut)?\(\)"),
        "a child found by its position — give the part a name (`.area(\"name\")`) and find it by "
        "that name",
        MACHINERY,
    ),
    (
        "hand-style",
        # The receiver is another widget: not `self`, and not the widget's own `base` (its own
        # style, set in its constructor or its own methods, is its business).
        re.compile(
            r"(?<![\w.])(?!self\b|base\b)([a-z_][a-z0-9_]*)(\(\))?(\.as_mut\(\))?"
            r"\.(authored_style_mut\(\)|(base_mut\(\)\.)?style\.layout\.\w+\s*=[^=])"
        ),
        "another widget's style written by hand — that widget needs a builder that takes the "
        "value (fixed or changing); if it has none, build it",
        MACHINERY,
    ),
    (
        "hand-wiring",
        re.compile(r"\bcreate_effect\s*\("),
        "an effect that copies one value into another by hand — pass a value that changes to the "
        "widget's builder instead; if the builder does not take one, build that",
        MACHINERY,
    ),
    (
        "number-in-layout",
        re.compile(
            # Zero is not a size but "may shrink to nothing" (`min_width(0.0)`), so it passes.
            r"\.(" + SIZE_BUILDERS + r")\(\s*(" + NUMBER + r"|\"" + NUMBER + r"|Length::Px\(\s*" + NUMBER + r")"
            r"|Length::Px\(\s*" + NUMBER + r"|\"100%\""
        ),
        "a size written as a number — sizes come from the theme (`Spacing::Sm`, a size variant, a "
        "theme value) or fill what holds them (`fill_parent()`)",
        MACHINERY + THEME,
    ),
    (
        "colour-by-hand",
        re.compile(r"Color::(rgb|rgba|from_rgb|from_rgba|from_hex)\s*\(|\"#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?\""),
        "a colour written by hand — colours come from the theme",
        MACHINERY + THEME,
    ),
]

ALLOW = re.compile(r"//\s*syntax-ok:\s*\S")


def git(repo, *args):
    return subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True, check=False
    ).stdout


def added_lines(repo, base):
    """`{path: {line number: text}}` for every line added since `base`, staged or not, plus every
    line of a file git does not track yet."""
    merge_base = git(repo, "merge-base", "HEAD", base).strip()
    diffs = [git(repo, "diff", "-U0", "--no-color", merge_base or base, "--", "*.rs")]
    found = {}
    for diff in diffs:
        path, line = None, 0
        for raw in diff.splitlines():
            if raw.startswith("+++ "):
                path = raw[6:] if raw.startswith("+++ b/") else None
            elif raw.startswith("@@"):
                line = int(re.search(r"\+(\d+)", raw).group(1))
            elif path and raw.startswith("+"):
                found.setdefault(path, {})[line] = raw[1:]
                line += 1
            elif path and not raw.startswith("-"):
                line += 1
    for path in git(repo, "ls-files", "--others", "--exclude-standard", "--", "*.rs").splitlines():
        text = (Path(repo) / path).read_text(errors="replace").splitlines()
        found[path] = {n: t for n, t in enumerate(text, start=1)}
    return found


def is_test_file(path):
    return "/tests/" in path or path.endswith("tests.rs") or path.endswith("_tests.rs") or "/benches/" in path


def test_module_start(repo, path):
    """The line a file's `#[cfg(test)] mod …` starts at, if it has one — tests sit at the end."""
    try:
        lines = (Path(repo) / path).read_text(errors="replace").splitlines()
    except OSError:
        return None
    for n, text in enumerate(lines, start=1):
        if text.strip() == "#[cfg(test)]":
            following = next((l.strip() for l in lines[n:] if l.strip()), "")
            if following.startswith("mod ") or following.startswith("pub mod "):
                return n
    return None


def check(repo, base):
    findings = []
    for path, lines in sorted(added_lines(repo, base).items()):
        if is_test_file(path):
            continue
        tests_from = test_module_start(repo, path)
        for n, text in sorted(lines.items()):
            if tests_from and n >= tests_from:
                continue
            code = text.split("//", 1)[0] if "//" in text and not ALLOW.search(text) else text
            if ALLOW.search(text) or code.strip().startswith(("///", "//!")):
                continue
            for name, pattern, why, exempt in RULES:
                if path.startswith(exempt):
                    continue
                if pattern.search(code):
                    findings.append((path, n, name, why, text.strip()))
    return findings


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--base", default="main")
    args = parser.parse_args()
    findings = check(args.repo, args.base)
    for path, n, name, why, text in findings:
        print(f"{path}:{n}: [{name}] {why}\n    {text}")
    if findings:
        print(f"\nTHE SYNTAX: {len(findings)} line(s) to fix (AGENTS.md § 0). A deliberate "
              "exception says why on the line: `// syntax-ok: <reason>`.")
        return 1
    print("THE SYNTAX: nothing found in the added lines.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
