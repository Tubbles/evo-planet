#!/usr/bin/env python3
"""Audit docs/EQUATIONS.md against the code's numeric constants.

    python3 -I scripts/check_equations.py

Every `pub const NAME: f32 = value;` (and u32/usize/u64) in evo_core/src is
listed, and the script reports which values do not appear anywhere in
docs/EQUATIONS.md. A value that is missing means either the constant is
not yet described (add a row) or the document is stale (fix it). The exit
code is the number of missing values, so a CI step can fail on it.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "evo_core", "src")
DOC = os.path.join(ROOT, "docs", "EQUATIONS.md")

CONST = re.compile(r"pub const ([A-Z0-9_]+): (?:f32|f64|u32|u64|usize) = ([^;]+);")


def normalise(value):
    v = value.strip().replace("_", "")
    v = re.sub(r"\s*\*\s*", "*", v)
    try:
        if "*" in v or "/" in v:
            return float(eval(v, {"__builtins__": {}}))  # simple arithmetic like 8.0 * 3600.0
        if v.startswith("1.0 /"):
            return float(eval(v, {"__builtins__": {}}))
        return float(v.replace("e", "e"))
    except Exception:
        return None


def mentions(doc, number):
    """Does the number appear in the doc in any common spelling?"""
    forms = {f"{number:g}", f"{number:.0f}", f"{number:.1f}", f"{number:.2f}", f"{number:.3f}", f"{number:.4f}"}
    if number >= 1000:
        forms.add(f"{number / 1e6:g} MJ")
        forms.add(f"{number:,.0f}")
        forms.add(f"{number:,.0f}".replace(",", " "))
    if 0 < number < 1:
        forms.add(f"{number * 100:g}%")
    return any(f in doc for f in forms)


def main():
    with open(DOC, encoding="utf-8") as f:
        doc = f.read()
    missing = []
    total = 0
    for dirpath, _, files in os.walk(SRC):
        for name in files:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(dirpath, name)
            with open(path, encoding="utf-8") as f:
                code = f.read()
            for m in CONST.finditer(code):
                total += 1
                value = normalise(m.group(2))
                if value is None:
                    continue
                if not mentions(doc, value):
                    missing.append((os.path.relpath(path, ROOT), m.group(1), m.group(2).strip()))
    print(f"{total} constants in evo_core/src; {len(missing)} values not found in docs/EQUATIONS.md")
    for path, name, value in missing:
        print(f"  {name} = {value}  ({path})")
    sys.exit(len(missing))


if __name__ == "__main__":
    main()
