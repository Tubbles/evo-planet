# Evo Planet

A Bevy + Rapier demo of Spore-inspired creatures whose abilities come from
physics, not stat tags. A Sims-style graph genome grows articulated bodies;
a background thread evolves them against planet dials (gravity, atmosphere,
oxygen, water, temperature, insolation); the viewer replays the current
champion in the exact simulation that scored it.

```
evo_core/    genome graph, decoder, rapier simulation, fluid forces, archive evolution, planet ecology
evo_wasm/    wasm-bindgen bindings used by the browser demo
web/         browser demo: index.html, worker.js, pkg/ (built wasm)
evo_viewer/  Bevy 0.16 desktop viewer (archive mode only)
dist/        static Linux x86_64 binaries: evo_serve (browser demo, self-contained) and evo_headless
```

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

Headless archive evolution (prints a generation per line and the archive grid):

```bash
cargo run -p evo_core --bin evo_headless -- --list-planets
cargo run -p evo_core --bin evo_headless -- --planet "dense" --env air --gens 30 --out champion.json
cargo run -p evo_core --bin evo_headless -- --planet earth --env water --gens 20
```

Headless planet ecology (implicit fitness, prints populations every tenth of the run):

```bash
cargo run -p evo_core --bin evo_headless -- --eco --planet earth --days 2000 --grid 24
```

Desktop Bevy viewer (archive mode):

```bash
cargo run -p evo_viewer
```

Linux needs `libxkbcommon-dev` and X11 dev headers for the viewer. Add the
`wayland` feature in `evo_viewer/Cargo.toml` if you run Wayland and have
`libwayland-dev`. Audio and gamepad features are disabled.

## What a node is

A node is one rigid segment type with a joint to its parent. Nodes are
expanded into parts by walking the graph from the root (Sims 1994):

| Gene | Effect |
|---|---|
| `kind` Capsule / Membrane / Pad | limb or torso; thin light lift-producing panel; high-friction foot |
| `len`, `r0`, `r1` | segment size, so mass (volume) and strength (cross-section) are real |
| `joint` Hinge / Ball / Rigid | 1 driven axis, 2 driven axes (twist locked), welded |
| `limit` | soft joint range, radians |
| `recursive_limit` | how many segments a self-loop produces (tails, multi-jointed legs) |
| `cpg` | local oscillator: amplitude, phase, bias, frequency multiplier |
| connections | where on the parent surface, direction, subtree scale, mirror, terminal-only, phase flip |

Global genes: `base_freq`, `lung_frac` (fraction of torso volume given to
lungs or gills), `water_store` (extra torso mass held as water).

Decode clamps to 24 parts, 10:1 neighbouring mass ratios and stable joint
ranges, as the engine docs recommend.

## Planet dials and what they do in the physics

| Dial | Derived quantity | Where it bites |
|---|---|---|
| Gravity | weight ∝ volume × g, strength ∝ cross-section | thick short limbs on heavy worlds; drop and glide dynamics |
| Pressure, O₂, CO₂, temperature | air density from the ideal gas law | every membrane's lift and drag, every limb's drag |
| Pressure × O₂ | O₂ partial pressure | oxygen budget: `lung_frac × torso volume × pO₂` against `muscle section × frequency`; the ratio scales motor strength |
| Humidity, temperature | aridity → water storage needed | land fitness multiplier unless `water_store` covers it; stored water adds mass |
| Water coverage | aquatic niche share | ecology readouts; choose the Water environment to evolve swimmers |
| Insolation, CO₂, temperature, water | plant productivity, plant:animal biomass ratio | ecology readouts (illustrative model, Earth-normalised) |

Three environments:

- **Land**: flat ground, settle 1 s with motors off, then 8 s. Fitness = horizontal distance × hydration.
- **Water**: submerged, buoyancy from displaced volume with submersion damping. Fitness = horizontal distance.
- **Air**: dropped from 10 m. Fitness = horizontal distance at landing + 0.25 × seconds aloft. Parachutes appear first, then gliders.

## Fluid model

Each capsule gets anisotropic quadratic drag at three points along its axis
(C_D 0.5 axial, 1.1 lateral). Each membrane is split into a 2×2 grid of
sub-panels; each receives a flat-plate normal force
`F = -½ ρ A |v|² C_Pα (v̂·n) n` with `C_Pα = 2.61`, plus parasitic drag
`C_D0 = 0.09` (Lu et al. 2026 closure). The same code runs for water and
air; only `ρ` changes, so swimmers pre-adapt wings.

## Evolution

MAP-Elites archive, 6×6 cells: part count × membrane area per kilogram
(a wing-loading proxy). One elite per cell keeps legged, finned and winged
lineages alive side by side. A nursery holds children whose bodies changed
but who lost to the cell occupant, and uses them as parents for six
generations (innovation protection, after Cheney et al. 2016/2018). Each
generation evaluates a batch of 24 on all cores; episodes are fixed-timestep
(120 Hz) and deterministic on one machine. Runs that go non-finite or exceed
60 m/s score zero, because evolution exploits any energy leak.

Typical throughput on four cores: 20–60 evaluations per second for land
walkers, over 100 for small swimmers, under 10 for 20-part winged bodies.

## On a TV (Bazzite, Steam Deck, any Linux box with a gamepad)

```bash
git clone https://github.com/<you>/evo-planet.git && cd evo-planet
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
globe; X pauses; Y switches Archive/Ecology; LB/RB cycle planet presets;
Start applies the planet and restarts; Back toggles Globe/Creature; right
stick orbits or spins; triggers zoom. A connected gamepad switches the page
to a larger TV layout.

Overrides: `PORT`, `PROGRESS_PORT`, `BROWSER="flatpak run org.mozilla.firefox --kiosk"`, `NO_PULL=1`.

## Planet ecology mode (implicit fitness)

`evo_core/src/eco.rs`. No score. A grid of 10 m patches; water patches set
by the water-coverage dial. Each tick is a day. Life starts in the oceans:
plants and animals are seeded only in water, and the globe view shows the
planet start blue and brown and green up as plants colonise the shores and
then the interior.

- **Plants** use the same graph genome as animals, read by a different
  decoder (`evo_core/src/plant.rs`): capsule nodes are stems and branches,
  membrane nodes are leaves (the membrane's spare radius gene is leaf
  thickness, so specific leaf area evolves), pad nodes are roots. The root
  node stands vertically; parts below ground count as root mass. The decoder
  runs a static cantilever check over every stem (weight above × lever over
  section × strength, scaled by gravity) and returns a growth cap: the
  fraction of full size the stems can carry. Plant genes added to the genome:
  land tolerance, seed mass, dispersal, lifespan, and tissue dry-mass density.
  Water attenuates light to 30%, which is the pull toward land; on land,
  photosynthesis and root water uptake scale with land tolerance, which costs
  upkeep everywhere, so aquatic lineages stay aquatic until a seed lands on
  the wet shore band and the trade-off flips. Light per patch is intercepted
  tallest-first with Beer-Lambert extinction, so neighbours shade each other.
  Leaves respire fast, woody tissue slowly. Photosynthesis scales with light, CO₂,
  temperature, a water factor (root uptake against leaf transpiration under
  aridity) and nutrient share. Respiration scales with biomass. A static
  structural check caps realised height at stem strength over weight above
  times gravity, so heavy worlds get squat plants. Surplus energy grows the
  plant, then makes seeds that disperse by height and air density. Dead
  plants join the patch's dead-matter pool.
- **Animals** use the same Sims-style genomes as archive mode plus two diet
  genes (carnivory, detritivory; herbivory is the remainder). Each new body is
  measured once in a 3 s rapier episode on land or in water to get its speed;
  results are cached per genome, so physics cost tracks new body plans, not
  population. Per day an animal pays basal (mass^0.75, temperature, oxygen),
  muscle activity (section × frequency × fatigue factor) and travel costs,
  forages with a search rate that grows with speed (plants, algae, detritus,
  or a predation attempt whose success is a logistic in speed difference),
  moves to a neighbouring patch when hungry if its body can use that habitat,
  and reproduces when energy exceeds 0.6 × mass, with 75% of births mutated.
- **Decomposition** returns dead matter to nutrient at a rate set by
  temperature and moisture; algae grow logistically in water patches.
- **Species** are lineages. A child with a changed body plan starts a new
  lineage with probability 0.2 (plants 0.03 per seed). Random immigrants
  arrive when the world empties; archive elites can be injected as seed stock.
  Caps: 8 plants per patch, 260 animals.

Observed on Earth over 3000 days (24² grid, ocean start): land goes from 0
to 143 of 167 patches vegetated, shore first, with taller plants appearing
on land; swimmers evolve from 0 to about 1 m/s; carnivores boom then crash
as herbivores recover; detritivores wax when the dead-matter pool is large.
Land walkers rarely arise from random immigrants, which matches the
literature; the archive elites injected on mode switch supply them.

Species are not evolved one at a time. Everything lives in the same world
at once; speciation is tracked as lineages, fresh random immigrants arrive
when the world empties, and archive champions can be injected as seed stock.

## Results from the headless runs used to validate this

- Earth, Land, 15 generations: a 4-part walker covering 7.8 m in 8 s.
- "Dense low-g" preset, Air, 30 generations: a 21-part body with 14 membranes
  and 9.8 m² of panel stayed aloft for the full drop. Parachute, not yet glider.
- Earth, Water, 12 generations: a 2-part body with one membrane fin swam 3.6 m.

## License

GNU Affero General Public License v3.0 or later. See `LICENSE`.

## Pointers for turning this into a game

- Replace the capsule meshes with an SDF skin (smooth-min primitives, surface
  nets) and derive bone weights from the same primitives.
- Seed the archive offline per planet preset, then continue evolution in the
  background in-session.
- Add macro-nodes (saved subgraphs) for a Spore-style part palette while
  keeping the segment-level genome underneath.
