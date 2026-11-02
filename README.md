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
cargo run -p evo_core --bin evo_headless -- --eco --planet earth --days 4000 --grid 1000
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

`evo_core/src/eco.rs`. No score. The world is a set of uniform patches on a
Fibonacci sphere (`evo_core/src/terrain.rs`): no poles, no wrap seams, six
neighbours each. Elevation comes from layered value noise with ridged
mountain belts; sea level is the elevation quantile matching the
water-coverage dial; each patch's temperature falls with latitude and by
6.5 °C per kilometre of altitude, so peaks and poles are cold and barren.
The viewer renders a displaced icosphere with vertex colours (sand, rock,
vegetation, snow below 0 °C) under a translucent sea sphere. Each tick is a
day. Life starts in the oceans as floating algae sheets (a one-membrane plant
genome) and animals that can only graze them: the carnivory and detritivory
genes start at zero, and stems, roots, branching, land tolerance, predation
and scavenging all have to arise by mutation. There is no seeding from
archive mode. A `motility` gene decides kingdom: offspring at or above 0.5 are
born as animals, below it as sessile plants, so a lineage can cross over in
either direction. Motility scales muscle use, speed and activity cost, and
animals with membranes photosynthesise at half a leaf's efficiency in
proportion to how sessile they are. Herbivores assimilate land plants at 75%
against 60% for algae, a deliberate incentive to go ashore (real lignin cuts
the other way; algae being mostly water and salt is the excuse). Plants reach
land through a land-tolerance gene and the wet shore band. Animals reach land by normal selection: shore-dwelling
herbivores can graze adjacent land plants from the water, better the more
they can crawl, and every new body is measured on land as well as in water,
so a mutation that improves crawling pays off before the lineage ever hauls
out. Land entry is continuous (any body with measurable land speed can try),
but foraging on land scales with speed, so poor crawlers go hungry there.

In planet ecology the dials act on the living world as you drag them: the
sea level follows water coverage (flooded land drowns its plants, drained
seabed strands its animals), temperature and humidity shift every patch's
climate, gravity rescales every plant's structural cap and, with pressure
and oxygen, queues every living body for a fresh physics measurement.
"Apply & restart" starts a new planet instead.

The lineage table can be filtered by kind (animals, plants, both) and
habitat (all, land, sea, fliers) and sorted by count, mass, speed, height,
parts or land share; tapping a lineage replays a member from the filtered
habitat. Flight is measured for any winged body light enough for it to
matter: launched at 3 m/s from 10 m, metres beyond a thrown stone are the
glide, still aloft at the end is flight. Gliders use the canopy and
hillsides as perches to cross patches, cover ground and escape; fliers
range over many patches a day, cross water, survey carrion from above and
are out of reach of ground predators. Land and water speeds are sustained
progress after settling, minus the same body's drift with its motors off
(a sham control), with rolling resistance on the ground and bodies
starting where they would rest, so toppling, sliding and floating do not
count as locomotion. Nimbleness is measured too: turn rate from
asymmetric drive and climb speed from a posture shift, used in chases.
The shore is an intertidal band, under water half the day, where gills
still work, algae mats grow and slow grazers can live; dry land beyond it
is a crawl away, and a lineage survives there only if its breathing gene
has moved from gills toward lungs and it finds food. As of this writing no walkers or fish-like
swimmers have evolved in the ecology; `docs/MODEL.md` records what the
replays actually show and what higher fidelity would cost.

- **Plants** use the same graph genome as animals, read by a different
  decoder (`evo_core/src/plant.rs`): capsule nodes are stems and branches,
  membrane nodes are leaves (the membrane's spare radius gene is leaf
  thickness, so specific leaf area evolves), pad nodes are roots. The root
  node stands vertically; parts below ground count as root mass. The decoder
  runs a static cantilever check over every stem (weight above × lever over
  section × strength, scaled by gravity) and returns a growth cap: the
  fraction of full size the stems can carry. Plant genes added to the genome:
  land tolerance, seed mass, dispersal, lifespan, reproduction threshold
  (fraction of full size before seeding) and tissue dry-mass density.
  Grazing removes at most 15% of a plant's leaf per visit.
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
- **Animals** use the same Sims-style genomes as archive mode. The animal
  model is mechanistic (see `docs/MODEL.md` and the research report in
  `reports/`): field metabolic rate ∝ M^0.75 with a Q10 temperature factor
  (ectotherms); fat reserve capacity ∝ M^1.19 set by a gene; offspring born
  at a gene-set fraction of adult mass and growing by the West–Brown–Enquist
  curve, maturing at a gene-set fraction of asymptotic mass; lifespan ∝
  M^0.25 × a gene. A stomach (gut fraction gene) is filled by foraging and
  emptied by digestion ∝ M^0.75, with assimilation of plant versus animal
  tissue set by a continuous gut-type gene (fermenter 50%/45% to carnivore
  15%/90%). Foraging is a Holling type II response with Beddington–DeAngelis
  crowding, where the encounter rate is the area the body sweeps in a day
  (detection strip two reaches wide along eight hours at the measured speed);
  reach, mouth, claws, plating and uprightness are read from the body's
  geometry and its land physics episode, so form has consequences. Plants are
  scored on leaf area projected from nine sky directions with overlap
  removed, checked for bending and buckling under gravity, charged for
  transporting to every leaf, and shed leaves on a lifespan set by leaf mass
  per area. Predation
  is attack (M^0.75 × speed × claws × uprightness) against defence (prey
  M^0.75 × speed × plating) through a logistic, times a Gaussian hump in
  log10(predator/prey mass) centred at 10×; the energy returned from a kill
  falls with relative prey size (zero for prey ten times larger, Fritsch et
  al.); attacks happen only when expected gain per handling time beats the
  current alternatives. Carcasses persist as carrion that anyone with the gut
  for it can scavenge. Each new body is measured once in rapier for its land
  and water speed. Diet classes (herbivore, carnivore, scavenger, omnivore)
  are what an individual actually ate, not genes. Founders are a few hundred
  grams, seeded in warm shallow seas, with herbivore guts.
- **Carrion and detritus.** Dead animals and dung feed a carrion pool and a
  plant-litter pool that decompose to nutrient at temperature- and
  moisture-dependent rates. Scavenging needs no speed and is the natural
  bridge to predation.
- **Plants** carry a leaf-toughness gene on the leaf-economics axis: tough
  leaves lose less to grazers and digest worse but photosynthesise slower.
  Half of tender leaf tissue is takeable in a day; algae density in the sea
  is limited by light rather than a counter.
- **Diagnostics** logged every tick: daily energy flows (primary production,
  grazing, predation, scavenging, metabolic cost), death causes, the realised
  predator–prey mass ratio of kills (field mean ≈ 10^1.6, 80% of kills by the
  larger party), and the satiation fraction (≈ 88% in the field).
- **Species** are lineages. A child with a changed body plan starts a new
  lineage with probability 0.2 (plants 0.03 per seed). Random sea immigrants
  arrive only when the world empties. Caps: 8 plants per patch, 260 animals.

Observed on Earth over 6000 days (1000 patches, algae and grazers only at
the start): algae evolve stems, roots, branching and land tolerance and green
half the land from the shores inward; land plants grow past the 1.5 m
aquatic cap. Animals shrink to under a kilogram while food is sparse, then,
once a kill feeds the killer, predation erupts: by day 3000 a quarter of the
fauna are omnivores, dozens are outright carnivores, mean body mass peaks
near 100 kg in the arms race, and tens of thousands of kills follow. The boom
collapses into a scavenger-hunter fauna (detritivory gene around 0.7) with a
minority of herbivores, and body size falls back. Crawling speed rises under
shore grazing and a few dozen animals live on land at a time. Diet classes
in the readouts go by the dominant gene, so scavengers that hunt through the
scavenging term count as detritivores; the kill count and the omnivore count
show the predation.

Species are not evolved one at a time. Everything lives in the same world
at once; speciation is tracked as lineages, and fresh random immigrants
arrive in the sea only when the world empties.

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
