# Evo Planet

A planet with no score. Life starts in the sea as floating algae sheets and
nothing else; everything that comes after has to earn its energy under the
physics of the world you set. A Sims-style graph genome grows bodies that
are read as plants or as animals; rapier3d measures what each body can do
in land, water and air; the ecology decides what that is worth. Wings,
legs, fins, mouths, claws, lungs and leaves all get their abilities from
lift, friction, oxygen and light, not from stat tags.

```
evo_core/    genome graph, decoders (animal and plant), rapier simulation, fluid forces, terrain, planet ecology
evo_wasm/    wasm-bindgen bindings used by the browser demo
web/         browser demo: index.html, worker.js, pkg/ (built wasm)
docs/        SPEC.md (how it is built), MODEL.md (why, and the audit log), RULES_AUDIT.md (every number and its backing)
dist/        static Linux x86_64 binaries: evo_serve (browser demo, self-contained) and evo_headless
```

There is one mode. An earlier archive-evolution mode (a scored task with a
MAP-Elites archive) was removed; nothing is ranked and nothing is seeded.

## Run

Browser demo, no install (Linux x86_64 binary embeds the page and wasm):

```bash
./dist/evo_serve
```

Then open http://127.0.0.1:8765/ in any browser. Or build the web demo yourself:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
cargo build --release --target wasm32-unknown-unknown -p evo_wasm
wasm-bindgen --target web --out-dir web/pkg --no-typescript target/wasm32-unknown-unknown/release/evo_wasm.wasm
cargo run --release -p evo_core --bin evo_serve
```

Headless planet (prints the ledger every tenth of the run and the top lineages at the end):

```bash
cargo run -p evo_core --bin evo_headless -- --list-planets
cargo run -p evo_core --bin evo_headless -- --planet earth --days 4000 --grid 1000 --seed 4
cargo run -p evo_core --bin evo_headless -- --planet earth --days 2000 --grid 2400 --patch-area 25
```

`--grid` is the number of cells on the sphere and `--patch-area` their
size in square metres. Together they set the planet's area; every rule is
per square metre or per metre, so the cell size should change nothing but
resolution and speed.

## The genome

A directed graph of nodes with recursion limits (Sims 1994), plus
whole-organism genes. The same genome is read as an animal body
(`decode.rs`) or as a plant body (`plant.rs`); the `motility` gene decides
which, and nothing an organism earns or pays jumps at that point.

| Gene | Effect |
|---|---|
| `len`, `r0`, `r1` | segment size, so mass (volume) and strength (cross-section) are real; on a sheet `r1` is its thickness (0.1 to 2 mm) |
| `flat` 0..1 | capsule to sheet: mass, drag, lift, mouth and leaf area all blend on it |
| `grip` 0..1 | skin to clawed foot: friction and claws |
| `stiffness` 0..1 | free joint to welded: a passive spring and shrinking limits |
| `freedom` 0..1 | one driven axis to two |
| `limit` | soft joint range, radians |
| `recursive_limit` | how many segments a self-loop produces (tails, multi-jointed legs) |
| `cpg` | local oscillator: amplitude, phase, bias, second axis, frequency multiplier |
| connections | where on the parent surface, direction, subtree scale, mirror, terminal-only, phase flip |

Whole-organism genes: `motility`, `heterotrophy` (absorption and capture
at the cost of chloroplasts), `breathing` (gills to lungs), `gut_type`,
`gut_frac`, `fat_frac`, `offspring_frac`, `repro_frac`, `longevity`,
`lifespan`, `seed_mass`, `dispersal`, `land_tolerance`, `leaf_toughness`,
`lung_frac`, `water_store`, `density`, `base_freq`.

Mutation nudges every gene, adds nodes, duplicates working segments
(serial homology), adds and removes connections, and lets motility jump by
a flagellum's worth now and then. There are no binary switches in the
biology; the old kind and joint labels are derived from the form genes for
display only.

## Planet dials and what they do in the physics

| Dial | Derived quantity | Where it bites |
|---|---|---|
| Gravity | weight ∝ volume × g, strength ∝ cross-section | thick short limbs on heavy worlds; drop and glide dynamics; plant bending and buckling |
| Pressure, O₂, CO₂, temperature | air density from the ideal gas law | every sheet's lift and drag, every limb's drag |
| Pressure × O₂ | O₂ partial pressure | oxygen budget: `lung_frac × torso volume × pO₂` against `muscle section × frequency`; the ratio scales motor strength |
| Humidity, temperature | aridity → water storage needed | motor strength on land unless `water_store` covers it; stored water adds mass; plant transpiration |
| Water coverage | sea level | the elevation quantile that is under water; the intertidal band around it |
| Insolation, CO₂, temperature, water | plant productivity | photosynthesis per lit square metre |

In the browser the dials act on the living world as you drag them: seas
rise or fall, climates shift, gravity rescales every plant's structural cap
and queues every living body for a fresh measurement. "Apply & restart"
starts a new planet instead.

## Measurement, not scoring

Every new body (and every body again at half adult mass) runs a short
battery of rapier episodes:

- **Land**: placed on the ground, motors on and off; sustained speed over
  the second half of the episode, net of the motors-off twin, so a topple
  or an initial roll does not count. Uprightness from mean height. Turn
  rate from asymmetric drive.
- **Water**: started where the body would rest (floating, on the bed or
  mid-water); speed net of drift; turn rate and climb speed from steering
  and a posture shift.
- **Air** (bodies with wing loading under 50 kg/m²): launched at 3 m/s
  from 10 m; metres beyond where a stone lands are the glide; still aloft
  at the end is flight.

Fluid forces blend by flatness: capsule drag on three axial samples, and
flat-plate lift and drag on four sub-panels (`C = 2.61 sin α cos α`,
parasitic drag 0.09), the same code at water and air density. Spin drag,
rolling resistance, a Hill force-velocity limit and an explicit force
limiter close the exploits that evolution found. Speeds are also reported
in body lengths per second.

## The planet ecology

`evo_core/src/eco.rs`. The world is a set of uniform cells on a Fibonacci
sphere (`terrain.rs`): no poles, no seams, six neighbours each. Elevation
is ridged noise; sea level is the elevation quantile matching the
water-coverage dial; temperature falls with latitude and 6.5 °C per km.
Shore cells carry a tide-swung submersion that blends breathing, light,
water and locomotion between sea and land. Each tick is a day.

- **Plants** are the graph read as leaf (by `flat`), stem and root. Light
  per cell is intercepted tallest-first with Beer-Lambert extinction and
  attenuated 0.3 per metre of water; floating sheets are lit at the
  surface and shade the beds. Each body is checked for bending and
  buckling under gravity, charged for transport to every leaf, and sheds
  leaves on a lifespan set by leaf mass per area. Surplus energy grows the
  plant, then seeds disperse by the dispersal gene.
- **Animals** pay tissue upkeep (sheets 0.4 %/day, other tissue 0.05 to
  0.3 %/day with motility) times a Q10 factor and oxygen, plus the work of
  their driven muscle. Their daily range is π·reach² plus reach times the
  day's path, spilling over as many cells as it covers. Intake is a
  Holling type II response with crowding, assimilated by a continuous gut
  type; a stomach fills and digests at M^0.75. Predation is attack against
  defence through a logistic with a Gaussian hump in log predator:prey mass
  (Brose 2006) and a kill return that falls with relative prey size
  (Fritsch). Growth follows West-Brown-Enquist; fat capacity, lifespan and
  maturity scale with mass and genes.
- **The origin.** There are no founders, no rescues and no population
  guard. Heterotrophy arises as absorption of dead matter through the
  surface, then capture of bacteria, then motility, then mouths; a motile
  sheet keeps photosynthesising. A microbial loop turns litter into
  bacteria that filterers and phagocytes can eat. The "add grazers" button
  on the page is an experiment, not the model.
- **Species** are lineages: a child with a changed body plan starts a new
  lineage with some probability. The table filters by kind and habitat
  (all, land, shore, sea, gliders and fliers) and sorts by count, mass,
  speed, height, parts, land share or body lengths per second; tapping a
  lineage replays a member from that habitat.

What has actually evolved so far, and what the replays show, is recorded
honestly in `docs/MODEL.md`. As of this writing no walkers or fish-like
swimmers have appeared in the ecology; the unguarded world fills with
short-lived motile sheets living on photosynthesis, absorption and
carrion, and the physics measurement budget is the binding limit.

## Invariants the model is held to

1. Every rule is per area or per metre; the cell is a run parameter.
2. No binary switches in biology; everything is a continuum on genes.
3. Every measured ability has a control (the motors-off twin, the thrown stone).
4. No founders, no rescues, no population guard; extinction is a result.
5. No claim about behaviour without a replay looked at (`window.evoDebug.replayGenome` and the examples under `evo_core/examples/`).

## On a TV (Bazzite, Steam Deck, any Linux box with a gamepad)

```bash
git clone https://github.com/Tubbles/evo-planet.git && cd evo-planet
./update-and-run.sh
```

The script opens a kiosk browser window within a second showing a live
build log, then pulls the latest commits, installs Rust and wasm-bindgen on
first run, builds the wasm and the server, and swaps the window to the game
when it is ready. If the build fails it runs the previous binary and says so
on screen. Add the script to Steam as a non-Steam game and choose the
"Gamepad" controller template so the browser sees a normal controller.

Gamepad mapping (standard layout): d-pad or left stick moves the focus ring
and adjusts the focused slider or selector; A activates; B returns to the
globe; X pauses; Y or Back toggles Globe/Creature; LB/RB cycle planet
presets; Start applies the planet and restarts; right stick orbits or
spins; triggers zoom. A connected gamepad switches the page to a larger TV
layout.

Overrides: `PORT`, `PROGRESS_PORT`, `BROWSER="flatpak run org.mozilla.firefox --kiosk"`, `NO_PULL=1`.

## License

GNU Affero General Public License v3.0 or later. See `LICENSE`.

## Pointers for turning this into a game

- Replace the capsule meshes with an SDF skin (smooth-min primitives, surface
  nets) and derive bone weights from the same primitives.
- Cohorts: one object standing for many identical small organisms, so the
  origin's millions can be simulated without a population guard.
- Add macro-nodes (saved subgraphs) for a Spore-style part palette while
  keeping the segment-level genome underneath.
