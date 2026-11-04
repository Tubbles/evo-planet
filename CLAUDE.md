# Working rules for this repository

## The maths report is part of every model change

`docs/EQUATIONS.md` is the source of truth for the model's quantities and
the equations that produce them, one row per quantity, grouped by section.
It is reviewed by the project owner, so it must always match the code.

- Any commit that changes a formula, a constant, a rule or a unit in
  `evo_core/src` updates the matching row(s) of `docs/EQUATIONS.md` in the
  same commit. New quantities get a new row; removed ones lose theirs.
- Spell names out: no abbreviations in the document (write "temperature
  factor", not "tf"; "cost of transport", not "COT").
- Run `python3 -I scripts/check_equations.py` before committing: it lists
  every numeric constant in the code whose value does not appear in the
  document. Zero missing is the target; a listed constant means a row to
  add or a stale number to fix.
- Regenerate the page with `python3 -I scripts/equations_page.py
  docs/EQUATIONS.md <out>.html` and republish it as the "Evo Planet
  Equations" artifact (same file path keeps its URL). The page is
  generated; never edit it by hand.
- When reporting a batch of model changes to the owner, give an "updated
  maths report": the rows that changed, old equation to new, in a short
  table, and the link to the republished page.

## Other standing rules

- Abilities are measured in physics episodes with controls, never
  assumed; no tuned thresholds or binary switches; every rule is per area
  or per metre; report negative results plainly.
- `docs/MODEL.md` records each run's finding and why the model changed.
- Commit messages end with the attribution lines given by the session.
