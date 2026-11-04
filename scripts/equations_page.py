#!/usr/bin/env python3
"""Render docs/EQUATIONS.md (the source of truth for the maths report) as a
self-contained HTML page with a section index and a row filter.

    python3 -I scripts/equations_page.py docs/EQUATIONS.md out/evo_math.html

The Markdown is a sequence of `## Section` headings, an optional one-paragraph
note under each, and one pipe table per section with three columns. Nothing
else is interpreted, so keep the document in that shape.
"""
import html
import re
import sys

STYLE = """
:root {
  --bg: #f7f6f2; --surface: #ffffff; --fg: #1f2320; --muted: #5f655f; --line: #d9d8d0;
  --accent: #1f6f5a; --accent-soft: #e4efe9; --code-bg: #eef0ea;
  --font-display: "Source Serif 4", Georgia, "Times New Roman", serif;
  --font-body: "IBM Plex Sans", system-ui, -apple-system, "Segoe UI", sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, Menlo, Consolas, monospace;
}
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #15181a; --surface: #1c2022; --fg: #e8e6df; --muted: #a2a79f; --line: #33393a;
  --accent: #7fc8ad; --accent-soft: #1f3a32; --code-bg: #242a2a; color-scheme: dark; } }
:root[data-theme="dark"] {
  --bg: #15181a; --surface: #1c2022; --fg: #e8e6df; --muted: #a2a79f; --line: #33393a;
  --accent: #7fc8ad; --accent-soft: #1f3a32; --code-bg: #242a2a; color-scheme: dark; }
* { box-sizing: border-box; }
body { background: var(--bg); color: var(--fg); font-family: var(--font-body); font-size: 15px; line-height: 1.5;
  padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); padding-block: 32px 64px; }
.page { max-width: 1180px; margin: 0 auto; }
header h1 { font-family: var(--font-display); font-weight: 600; font-size: clamp(28px, 4vw, 40px); margin: 0 0 6px; text-wrap: balance; letter-spacing: -0.01em; }
header p { max-width: 68ch; color: var(--muted); margin: 0 0 18px; }
nav.index { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 20px; }
nav.index a { font-size: 12.5px; text-decoration: none; color: var(--accent); background: var(--accent-soft); border-radius: 999px; padding: 4px 11px; letter-spacing: 0.02em; }
nav.index a:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.search { display: flex; gap: 10px; align-items: center; margin-bottom: 24px; }
.search input { flex: 1; min-width: 0; font: inherit; padding: 8px 12px; border: 1px solid var(--line); border-radius: 6px; background: var(--surface); color: var(--fg); }
.search input:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
.search span { font-size: 13px; color: var(--muted); white-space: nowrap; }
section { margin-bottom: 36px; }
section h2 { font-family: var(--font-display); font-weight: 600; font-size: 22px; margin: 0 0 4px; }
section .note { color: var(--muted); font-size: 13.5px; max-width: 72ch; margin: 0 0 12px; }
.tablewrap { overflow-x: auto; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); }
table { border-collapse: collapse; width: 100%; min-width: 760px; font-size: 13.5px; }
th { text-align: left; font-weight: 600; font-size: 11.5px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted); padding: 10px 12px; border-bottom: 1px solid var(--line); background: var(--surface); position: sticky; top: 0; }
td { padding: 9px 12px; border-bottom: 1px solid var(--line); vertical-align: top; }
tr:last-child td { border-bottom: 0; }
td.q { font-weight: 500; width: 20%; }
td.d { color: var(--muted); width: 24%; }
td.e { font-family: var(--font-mono); font-size: 12.5px; line-height: 1.55; width: 56%; font-variant-numeric: tabular-nums; }
td.e b { font-weight: 500; color: var(--accent); }
tr.hide { display: none; }
footer { color: var(--muted); font-size: 13px; margin-top: 24px; max-width: 72ch; }
@media (max-width: 640px) { td.q, td.d, td.e { width: auto; } }
"""

SCRIPT = """
(function () {
  var input = document.getElementById('filter');
  var count = document.getElementById('count');
  var rows = Array.prototype.slice.call(document.querySelectorAll('tbody tr'));
  var sections = Array.prototype.slice.call(document.querySelectorAll('section'));
  function apply() {
    var q = input.value.trim().toLowerCase();
    var shown = 0;
    rows.forEach(function (r) {
      var hit = !q || r.textContent.toLowerCase().indexOf(q) !== -1;
      r.classList.toggle('hide', !hit);
      if (hit) shown++;
    });
    sections.forEach(function (s) { s.hidden = !s.querySelector('tbody tr:not(.hide)'); });
    count.textContent = q ? shown + ' of ' + rows.length + ' rows' : rows.length + ' rows';
  }
  input.addEventListener('input', apply);
  apply();
})();
"""


def inline(text):
    """Escape a cell; `**bold**` becomes the accent bold, ` ; ` becomes a line break."""
    text = html.escape(text.replace("\\|", "|"))
    text = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", text)
    return text.replace(" ; ", "<br>")


def slug(title):
    return re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")


def render(md):
    lines = md.splitlines()
    title = next((l[2:].strip() for l in lines if l.startswith("# ")), "Equations")
    intro = ""
    sections = []
    cur = None
    for line in lines:
        if line.startswith("# "):
            continue
        if line.startswith("## "):
            cur = {"title": line[3:].strip(), "note": "", "head": [], "rows": []}
            sections.append(cur)
            continue
        if cur is None:
            if line.strip():
                intro = (intro + " " + line.strip()).strip()
            continue
        if line.startswith("|"):
            cells = [c.strip() for c in re.split(r"(?<!\\)\|", line.strip())[1:-1]]
            if all(re.fullmatch(r"-+", c) for c in cells):
                continue
            if not cur["head"]:
                cur["head"] = cells
            else:
                cur["rows"].append(cells)
        elif line.strip():
            cur["note"] = (cur["note"] + " " + line.strip()).strip()

    out = [f"<title>{html.escape(title)}</title>",
           '<meta name="claude:viewport-fit-sides" content="cover">',
           '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500&family=IBM+Plex+Sans:wght@400;500;600&display=swap">',
           f"<style>{STYLE}</style>", '<div class="page">', "<header>",
           f"<h1>{html.escape(title)}</h1>", f"<p>{inline(intro)}</p>", "</header>",
           '<nav class="index" aria-label="Sections">' + "".join(f'<a href="#{slug(s["title"])}">{html.escape(s["title"])}</a>' for s in sections) + "</nav>",
           '<div class="search"><input id="filter" type="search" placeholder="Filter rows (for example: water, torque, nutrient)" aria-label="Filter rows"><span id="count"></span></div>']
    classes = ["q", "d", "e"]
    for s in sections:
        out.append(f'<section id="{slug(s["title"])}"><h2>{html.escape(s["title"])}</h2>')
        if s["note"]:
            out.append(f'<p class="note">{inline(s["note"])}</p>')
        out.append('<div class="tablewrap"><table><thead><tr>' + "".join(f"<th>{html.escape(h)}</th>" for h in s["head"]) + "</tr></thead><tbody>")
        for r in s["rows"]:
            out.append("<tr>" + "".join(f'<td class="{classes[i] if i < 3 else "e"}">{inline(c)}</td>' for i, c in enumerate(r)) + "</tr>")
        out.append("</tbody></table></div></section>")
    out.append("<footer>Generated from docs/EQUATIONS.md by scripts/equations_page.py. The Markdown is the source; edit it, not this page.</footer>")
    out.append("</div>")
    out.append(f"<script>{SCRIPT}</script>")
    return "\n".join(out)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    src, dst = sys.argv[1], sys.argv[2]
    with open(src, encoding="utf-8") as f:
        md = f.read()
    with open(dst, "w", encoding="utf-8") as f:
        f.write(render(md))
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
