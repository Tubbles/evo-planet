# Evo Planet: system specification

A reading guide to how the simulation is built: the data it holds, the
systems that act on it, the order they run in, and how the pieces talk to
each other. For *why* each rule exists and what it rests on, see
`MODEL.md` (design notes and audit log) and `RULES_AUDIT.md` (every number
and its backing). This document is the *what*.

Diagrams are Mermaid; GitHub renders them inline.

## 1. The two modes and the layers

```mermaid
flowchart LR
  subgraph core["evo_core (Rust)"]
    G[genome] --> D[decode]
    D --> S[sim: rapier physics episodes]
    D --> P[plant: plant decoder]
    S --> E[eco: planet ecology]
    P --> E
    T[terrain] --> E
    PL[planet] --> S
    PL --> E
  end
  subgraph wasm["evo_wasm (wasm-bindgen)"]
    W2[EcoSim] --- E
    W3[ReplaySim] --- S
  end
  subgraph web["web/ (browser)"]
    WK[worker.js] --> W2
    IX[index.html: three.js globe, creature view, tables, dials] --> WK
    IX --> W3
  end
  H[evo_headless CLI] --> E
  SV[evo_serve: embeds web/] --> IX
```

There is one mode, the **planet ecology** (`eco.rs`): no score. Organisms
earn energy, pay for tissue and work, grow, breed and die on a sphere of
cells under a planet's gravity, atmosphere, light and water. The physics
(`sim.rs`) only measures what a body can do; nothing is ranked. An earlier
archive-evolution mode (a scored task with a MAP-Elites archive) was
removed on 2026-11-03.

## 2. Data structures

### 2.1 Genome (`genome.rs`)

A directed graph of nodes with recursion limits (Sims-style), plus global
genes. The same genome is read as an animal body (`decode.rs`) or as a
plant body (`plant.rs`); which reading the ecology uses follows from the
`motility` gene.

```mermaid
classDiagram
  class Genome {
    nodes: Vec~Node~
    root: usize
    base_freq: Hz
    lung_frac, water_store, density
    land_tolerance, seed_mass, dispersal, lifespan, repro_frac
    motility  (0 sessile .. 1 motile; born motile at >= 0.5)
    gut_type, gut_frac, fat_frac, offspring_frac
    longevity, leaf_toughness
    breathing  (0 gills .. 1 lungs)
    heterotrophy  (0 .. 1: absorb and capture, at the cost of chloroplasts)
    mutate() (Genome, body_changed)
  }
  class Node {
    len, r0, r1   (metres; r1 doubles as sheet thickness)
    flat  (0 capsule .. 1 sheet)
    grip  (0 skin .. 1 clawed foot)
    stiffness  (0 free .. 1 welded)
    freedom  (0 one driven axis .. 1 two)
    limit  (rad)
    recursive_limit  (1..4)
    cpg: Cpg
    connections: Vec~Connection~
    kind, joint  (labels derived from the genes)
  }
  class Connection {
    target: node index
    along  (0 base .. 1 tip)
    around, pitch, roll  (rad)
    scale
    reflect, terminal_only, phase_flip
  }
  class Cpg {
    amp, phase, bias
    amp2, phase2  (second axis)
    freq_mult  (0.5, 1, 2)
  }
  Genome "1" --> "1..10" Node
  Node "1" --> "0..4" Connection
  Node --> Cpg
```

Mutation operators: nudge every scalar gene; nudge the four form genes
(±0.1, one in five ±0.4); add a random node; **duplicate a segment**
(copy a used node onto its host, serial or mirrored, keeping its
oscillator); add or remove a connection; mutate connection parameters and
pointers; change a recursion limit; motility drifts with an occasional
flagellum-sized jump. `garbage_collect` drops unreachable nodes. Old JSON
genomes without the form genes resolve them from the `kind`/`joint`
labels.

### 2.2 Phenotype (`decode.rs`)

```mermaid
classDiagram
  class Phenotype {
    parts: Vec~Part~
    lung_frac, water_store, base_freq
    total_mass, membrane_area, membrane_mass
    torso_volume, total_section_area
  }
  class Part {
    node, parent
    len, r0, r1, radius
    mass, density  (blend of capsule and sheet by flat)
    flat, grip, stiffness, freedom, limit
    anchor_parent, rot_from_parent
    cpg, max_torque, section_area, reflected
  }
  class BodyMetrics {
    reach  (farthest extent from the centre of mass)
    mouth_area  (trunk front x (1 - flat))
    claws  (grip-weighted section / trunk section)
    plating  (mass held stiff on round parts / total)
  }
  Phenotype "1" --> "1..24" Part
  Phenotype ..> BodyMetrics : body_metrics()
```

`decode` expands the graph depth-first with per-path recursion counts,
clamps sizes (1 cm length, 4 mm radius floors), blends each part's mass,
tissue volume and muscle section by `flat`, applies lungs and stored
water to the trunk, clamps joined-part mass ratios to 12:1, and sums the
whole-body quantities. `placements` lays the parts out with the root at
the origin; `body_metrics` reads reach, mouth, claws and plating off that
geometry.

### 2.3 Plant body (`plant.rs`)

```mermaid
classDiagram
  class PlantBody {
    parts: Vec~PlantPart~  (kind label, flat, pose, mass)
    height, target_mass
    leaf_mass, stem_mass, root_mass
    leaf_area  (effective, nine-direction projection with overlap removed)
    leaf_area_total, transport  (kg x m of stem path)
    sla, growth_cap, stress, gravity
  }
```

Each part is `flat` leaf (chord 2·r0, thickness from r1) and `1 − flat`
stem; whatever is below ground is root. Structure is checked two ways
under gravity: bending moment at every stem from the horizontal lever arm
of all it carries, and Euler buckling of columns; `growth_cap` is the
fraction of full size the stems can hold. `set_gravity` rescales stress
without re-decoding.

### 2.4 Physics episode (`sim.rs`)

```mermaid
classDiagram
  class SimConfig {
    dt (1/120 s), settle_time (1 s), episode_time (4 s)
    motors: bool  (false = sham control)
    steer  (-1..1 asymmetric drive)
    bias_shift  (posture shift)
  }
  class CreatureSim {
    pheno, planet-derived fluid densities, environment
    rapier bodies, colliders, joints
    ground (flat, or the shore's slope)
    water level, fatigue, hydration
    step(), result()
  }
  class EpisodeResult {
    distance, sustained_speed  (second half, after settling)
    yaw_rate, climb_speed
    airborne_frac, time_aloft, air_distance
    mean_height  (uprightness)
    shore_progress, exploded
  }
  CreatureSim --> SimConfig
  CreatureSim ..> EpisodeResult
```

Environments: Land (flat ground, bodies placed on it), Water (12 m deep;
a body starts where it would rest: floating, on the bed, or mid-water),
Air (launched at 3 m/s from 10 m; a stone's distance is subtracted),
Shore (a bed rising 12 cm/m through the waterline; probe only).

Forces per step: per-part fluid forces blended by `flat` (capsule drag
on three axial samples; panel lift and drag on four sub-panels), both
capped at the force that would stop the sample's share of the part in one
step; spin drag about the part's own axis; buoyancy from tissue volume;
rolling resistance on ground contact; joint motors driven by the CPG with
a Hill force-velocity limit, a passive spring per `stiffness`, and a
second axis per `freedom`.

### 2.5 Ecology (`eco.rs`)

```mermaid
classDiagram
  class EcoWorld {
    planet: PlanetParams, derived: Derived
    patches: Vec~Patch~, neighbours: 6 per patch
    plants: Vec~Plant~, animals: Vec~Animal~
    day, patch_area (m2), terrain_seed
    trait_cache: body key to Traits
    pending: measurement queue (animal, environment)
    stats: EcoStats, history
    tick(), measure_pending(budget), set_planet()
  }
  class Patch {
    pos, elevation, temp_c, water, shore, moisture
    submersion  (1 sea, tide-swung ~0.5 shore, 0 inland)
    nutrient, dead_matter, carrion, bacteria  (kg pools)
    canopy, light_left, motile_area
  }
  class Plant {
    genome, body: PlantBody, lineage, patch
    biomass, energy, age, height
    light_frac, water_frac
  }
  class Animal {
    genome, key, traits: Traits, lineage, patch
    mass, reserve, gut_plant, gut_meat, age
    kills, offspring, ate_plant, ate_meat, ate_carrion, intake_rate
  }
  class Traits {
    mass, membrane_area, membrane_mass, section_area, activity
    land_speed, water_speed, air_speed, glide_distance
    turn_land, turn_water, climb_water, turn_air, climb_air
    reach, mouth_area, claws, plating, upright
    measured_land, measured_water, measured_air
  }
  EcoWorld "1" --> "*" Patch
  EcoWorld "1" --> "*" Plant
  EcoWorld "1" --> "*" Animal
  Animal --> Traits
```

`Traits` are what the physics and geometry say a body can do. They are
cached by body key, measured lazily (at birth, and again at half adult
mass), and a changed body inherits its parent's measured speeds as a
neutral prior until its own episodes run.

### 2.6 Planet and terrain (`planet.rs`, `terrain.rs`)

`PlanetParams` (gravity, pressure, O2 and CO2 fractions, mean
temperature, water coverage, humidity, insolation) derives `Derived`
(air and water density, O2 availability, NPP factor, aridity, flight
index, and so on). Terrain is ridged noise on a Fibonacci sphere; sea
level is the elevation quantile that matches water coverage; local
temperature falls with latitude and 6.5 °C per km. All of it can change
under a running world (`set_planet`).

## 3. Systems and the daily tick

```mermaid
flowchart TD
  A[measure_pending: run up to N physics test batteries] --> B[tick]
  B --> C[tide and canopy reset per patch]
  C --> D[Plants per patch: light, water, nutrients, osmotrophy, phagocytosis, respiration, growth, seeding, leaf shedding, death]
  D --> E[Seed dispersal: hop over the graph; motile seeds become animals]
  E --> F[Animals: metabolism, photosynthesis, foraging, predation, movement, growth, reproduction, death]
  F --> G[Resolve deaths and kills; births placed by larval drift]
  G --> H[Decomposition: litter to bacteria and nutrient; carrion rots; weathering]
  H --> I[Statistics and history]
```

### 3.1 The test battery (`measure_pending`)

For a body not yet measured in an environment, run:

| Episode | What it yields |
|---|---|
| Land, motors on | sustained speed, uprightness |
| Land, motors off | passive drift to subtract (sham control) |
| Land, asymmetric drive | turn rate |
| Water, motors on/off | sustained speed net of drift |
| Water, steer and posture shift | turn rate, climb speed |
| Air (if wing loading < 50 kg/m²) | glide beyond a stone, sustained flight, turn, climb |

The queue holds one request per (body, environment) and alternates
between the most-copied unmeasured body and the newest, so a spreading
body is checked soon and a new plan is not measured last.

### 3.2 Plants per patch

```mermaid
flowchart LR
  L[light x area x surface shade] --> BL[Beer-Lambert by height order]
  BL --> DA[depth attenuation 0.3/m for bed-anchored plants]
  DA --> PH[photosynthesis x water x habitat x nutrient x toughness x 1 - 0.6 heterotrophy]
  DM[dead matter] --> OS[osmotrophy by surface]
  BA[bacteria] --> PG[phagocytosis by surface and water movement]
  PH --> EN[energy]
  OS --> EN
  PG --> EN
  EN --> RS[respiration by tissue and motility]
  RS --> GR[growth to structural cap]
  GR --> SD[seeds when above repro_frac of full size]
  EN --> LS[leaf shedding by leaf lifespan]
```

### 3.3 Animals per individual

```mermaid
flowchart LR
  MB[maintenance: tissue rates, Q10, oxygen] --> DL[delta]
  AC[activity: driven muscle section x swing x rate] --> DL
  PH[photosynthesis at the surface mat, shared by area] --> DL
  RG[range: reach circle + reach x path, rings over cells] --> FR[Holling type II intake of plants, carrion, bacteria, with interference]
  FR --> GT[stomach and digestion by gut type]
  GT --> DL
  OS[osmotrophy and phagocytosis by surface] --> DL
  PR[predation: encounter by range, attack vs defence logistic, Brose hump, Fritsch return] --> GT
  DL --> RV[reserve, capped by fat capacity]
  RV --> GW[growth: WBE]
  GW --> RP[reproduction at repro_frac of full size]
  RV --> DT[death: starved, old, suffocated, killed]
  HG[hungry] --> MV[relocate within range to the best cell it can enter]
```

### 3.4 Energy accounting

Every quantity is in kilograms of dry biomass equivalent per day; the
statistics report the flows (NPP, grazing, predation, scavenging, cost)
and death causes so the ledger can be audited. The `seed_ledger`,
`gait_audit`, `flier_audit` and `origin_probe` examples print the
per-individual ledgers behind the aggregate numbers.

## 4. Interfaces

### 4.1 wasm (`evo_wasm/src/lib.rs`)

| Object | Purpose |
|---|---|
| `EcoSim` | ecology: `tick(days, evals)`, `set_planet`, `snapshot_json`, `lineages_json(kind, habitat, sort, max)`, `representative_json`, `plant_representative_json`, `terrain_json`, `immigrate` |
| `ReplaySim` | one physics episode stepped in the browser: `parts_json` (with flat and grip), `transforms`, `com`, `distance`, `slope` |

### 4.2 Worker protocol (`web/worker.js`)

```mermaid
sequenceDiagram
  participant Page
  participant Worker
  participant Wasm
  Page->>Worker: eco_start {planet, patches}
  Worker->>Wasm: EcoSim.new
  Worker-->>Page: eco_terrain {mesh}
  loop every few days
    Worker->>Wasm: tick(3, evals)
    Worker-->>Page: eco_snap {snapshot, lineages, history}
  end
  Page->>Worker: eco_planet {planet}  (dials dragged)
  Worker->>Wasm: set_planet
  Page->>Worker: eco_filter {kind, habitat, sort}
  Page->>Worker: eco_rep {lineage, habitat}
  Worker-->>Page: eco_rep {genome, env}
  Page->>Page: ReplaySim in the creature view
```

### 4.3 Page (`web/index.html`)

Globe (three.js, vertex colours from the snapshot), creature view (replay
with camera follow, continuous form rendering), ecology panel (stats,
energy flows, deaths, lineage table with filters), planet dials (live in
ecology mode), gamepad and TV layout. `window.evoDebug` exposes
`replayGenome`, `setZoom`, `replayTime`, `replayDistance` for scripted
screenshot audits.

### 4.4 Command line and tools

`evo_headless --eco --planet earth --days N --grid cells --seed S
[--patch-area m2] [--evals-per-day k]` prints the daily ledger and
lineage tables. Examples under `evo_core/examples/` are the audit probes
(see `MODEL.md`, "Audit tooling").

## 5. Invariants the model is held to

1. **Every rule is per area or per metre.** The cell size is a run
   parameter and must change nothing but resolution (tested at 25, 100
   and 400 m²; the coarse cut still disagrees, see `MODEL.md`).
2. **No binary switches in biology.** Form, joints, breathing, diet,
   maintenance, lifespan and reproduction are continua on genes. The one
   remaining split is in code: an organism is ticked as a plant below
   motility 0.5 and as a mover above it; nothing it pays or earns jumps
   there.
3. **Every measured ability has a control.** Speeds net of the
   motors-off twin; glide beyond a thrown stone.
4. **No founders, no rescues, no population guard.** Life starts as
   algae; extinction is a result.
5. **No claim without a replay looked at.** The audit examples and the
   page hook exist so that any number can be turned into a picture.

## 6. Known performance guards (not biology)

| Guard | Where | Binds when |
|---|---|---|
| Plant density caps | 0.48/m² sea, 0.08/m² land | always, in a full sea |
| Measurement budget | 8 bodies/day (headless), a few per slice (browser) | whenever births outrun it |
| Max nodes/parts | 10 nodes, 24 parts | rarely |
| Prey candidates per day | 3 | at high prey density |

The population guard was removed on 2026-11-03; cohorts (one object for
many identical small organisms) are the planned replacement for its
performance role and are not built yet.
