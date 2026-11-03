# Rules audit: what each number rests on

Every rule in the model is listed here with the kind of backing it has.
The categories are honest, not flattering:

- **measured**: a published exponent or coefficient (with the source);
- **anchored**: an order-of-magnitude anchor from real biology or physics,
  not a fitted value;
- **numerical**: a solver or discretisation choice with no biological
  content, which must not change results beyond noise;
- **performance**: a guard that exists only to keep the program fast and
  that can bind and distort results;
- **arbitrary**: a number with no backing. These are the ones to replace
  with a mechanism or to test for sensitivity.

Status: 2026-11-03, after removing the population guard.

## Physics (sim.rs, decode.rs)

| Rule | Value | Backing | Notes |
|---|---|---|---|
| Flat-plate normal force | C = 2.61 sin a cos a | measured | Lu et al. flat-plate form |
| Panel parasitic drag | CD0 = 0.09 | anchored | high for a thin plate; limits glide ratio to ~2.6 |
| Capsule drag | axial 0.5, lateral 1.1 | anchored | cylinder/sphere values |
| Muscle strength per section | 4000 N·m per m² | anchored | order of vertebrate muscle stress × lever |
| Hill force-velocity limit | zero torque at 25 rad/s | anchored | vertebrate limb joints peak 10-40 rad/s |
| Passive joint spring | 20 × strength per unit stiffness | arbitrary | makes "welded" welded; the 20 is untested |
| Rolling resistance | 0.05 | anchored | soft tissue on soil 0.03-0.1 |
| Spin drag | C = 1.0 on r⁴·L·ω² | anchored | rotating cylinder form; coefficient untested |
| Animal tissue density | 1000 kg/m³ | measured | |
| Sheet thickness from r1 | 0.1-2 mm | anchored | leaves and wing membranes |
| Size floor | 1 cm length, 4 mm radius | numerical | solver stability; the origin happened far smaller |
| Air drop | 10 m, launch 3 m/s | anchored | a leap from a tree; stone baseline subtracted |
| Water depth of the test tank | 12 m | numerical | |
| Beach slope | 0.12 | arbitrary | probe only, not a gate |
| Episode | 4 s at 120 Hz, settle 1 s | numerical | speed from the second half |
| Sham control | same body, motors off, subtracted | method | |
| O2 budget | K = 40 | arbitrary | fatigue factor scale; untested |
| Contact shape | capsule below flat 0.5, plate above | numerical | rounded box with no inner box has no mass in rapier |
| Mass-ratio clamp | 12:1 between joined parts | numerical | solver stability |
| Fluid force limiter | ≤ share of part momentum per step | numerical | prevents blow-up |

## Genome and mutation (genome.rs)

| Rule | Value | Backing | Notes |
|---|---|---|---|
| Mutations per genome per birth | ~1.5 (p = 1.5 / elements) | arbitrary | real phenotypic mutation rates are far lower; matters for cohorts |
| Structural operators | add node p·2, duplicate segment p·2, add/remove connection p·1.5 | arbitrary | relative rates untested |
| Form genes step | ±0.1, one in five ±0.4 | arbitrary | |
| Motility step | ±0.08, one in ten ±0.35 | anchored | the jump stands for a flagellum |
| Max nodes, connections, parts | 10, 4 per node, 24 parts | performance | |
| Lineage split on body change | 20% (sessile seed: 3%) | arbitrary | bookkeeping for the table only |

## Plants (plant.rs, eco.rs plant tick)

| Rule | Value | Backing | Notes |
|---|---|---|---|
| Photosynthesis per lit area | 0.02 kg dry/m²/day | anchored | leaf-level net ~13 g; gross ~20 g |
| Beer-Lambert extinction | 0.5 per unit LAI | measured | random leaf angles |
| Nine-direction self-shading | zenith + 8 at 45° | numerical | |
| Water surface light | 0.3 of full | arbitrary | stands in for surface loss and attenuation; now also 0.3/m with depth |
| Light extinction in water | 0.3 per m | anchored | coastal water |
| Leaf respiration | 0.4%/day of leaf mass | anchored | |
| Wood/stem respiration | 0.05%/day | anchored | |
| Transport cost | 0.002 per kg·m per day | arbitrary | |
| Leaf lifespan | 365 d at 0.1 kg/m², ∝ LMA^1.5 | measured | Wright et al. 2004 |
| Stem strength | 60 kPa equivalent; modulus 3 GPa | anchored | strength is low; modulus is green wood |
| Leaf density (dry) | 250 kg/m³ | anchored | |
| Water demand | leaf area × (0.002 + 0.015·aridity) | arbitrary | |
| Root uptake | root mass × moisture × 0.6 | arbitrary | |
| Land tolerance upkeep | × (1 + 0.3·tol) | arbitrary | |
| Nutrient demand | 0.2% of biomass/day, 30% taken up | arbitrary | |
| Growth rate | ≤ 5% of biomass + 10 g per day | arbitrary | |
| Seeds per event | ≤ 3 | arbitrary | |
| Death | energy < −30% of biomass | arbitrary | |
| Plant density caps | 0.48/m² sea, 0.08/m² land | performance | individuals per cell; light should limit instead |

## Ecology, animal side (eco.rs)

| Rule | Value | Backing | Notes |
|---|---|---|---|
| Tissue maintenance | sheets 0.4%/day; other 0.05% → 0.3%/day with motility | anchored | plant rates and a fish's resting rate |
| Q10 | 2 around 20 °C | measured | |
| Fat capacity | 0.05 M + fat·M^1.19 | measured | Lindstedt & Boyce |
| Growth | 0.12·M^0.75·(1 − (m/M)^0.25) | measured form, anchored coefficient | West-Brown-Enquist |
| Lifespan | 150·M^0.25 × gene, blended with the sessile gene by motility | measured exponent, anchored coefficient | halved earlier for turnover |
| Reproduction | at repro_frac of full size, 0.8 attempts/day | gene; the 0.8 is arbitrary | |
| Offspring reserve at birth | 30% of mass | arbitrary | |
| Stomach, digestion | gut_frac·M; 0.12·M^0.75 | anchored | |
| Assimilation | plant 0.5 − 0.35·g, meat 0.45 + 0.45·g | anchored | Demment & Van Soest |
| Foraging time | 8 h × activity | anchored | |
| Range | reach² + 2·reach·path, rings over cells | geometry | |
| Interference | 0.15 per competitor per 100 m² | arbitrary | Beddington-DeAngelis form; coefficient untested |
| Bite rate | 12 × mouth area × size² × Q10 | anchored | a tenth of body mass per day at 1 kg |
| Browsing reach | reach × (0.3 + 0.7·upright) on land | arbitrary | |
| Takeable leaf per day | half, × (1 − 0.8·toughness) | arbitrary | |
| Shore grazing from water | 0.8 × (0.1 + 4·land speed) | arbitrary | legacy; the intertidal band mostly replaces it |
| Carrion strip | 6 reaches wide, 40 for fliers | arbitrary | |
| Filter clearance | 1.2 m³/kg/day | anchored | mussels and sponges |
| Suspended fraction | 20% of plant mass | arbitrary | |
| Bacterial yield | 0.4; turnover 20%/day | anchored | |
| Phagocytosis capture | 5% of particles passing the surface | arbitrary | |
| Osmotrophy | 0.3 kg per m² per day per kg/m² | arbitrary | |
| Current | 5 cm/s, half on the shore | anchored | |
| Predation hump | Gaussian in log10 ratio, centre 1 | measured | Brose 2006 |
| Attack/defence | M^0.75 × (0.2 + BL/s) × claws, plating, upright, turn, take-off | forms anchored; floors and multipliers arbitrary | the 0.2 floor, "3·claws", "2·plating", "3·takeoff", "turn/1.5" |
| Capture logistic scale | 0.25 (attack + defence) | arbitrary | |
| Kill return | 0.4 + 0.4·log ratio, clamped | measured | Fritsch |
| Edible fraction | 80% of prey | anchored | |
| Attempt cost | 10% of FMR | arbitrary | |
| Prey candidates per day | 3 | numerical | sampling; should scale with cell |
| Relocation | ≤ 50 m of the day's path; flier an hour's flight | arbitrary | |
| Hungry move chance | 60%/day, half directed | arbitrary | |
| Cost of transport on a move | 1e-5 · M^0.7 · g · width / speed | anchored | Tucker form |
| Glider launches | 20/day × ratio × launch height | arbitrary | |
| Gliding definition | glide angle < 45° | measured | standard |
| Escape by take-off | × (1 + 3·takeoff) | arbitrary | |
| Suffocation | efficiency < 0.2 → p = 0.5/day | arbitrary | |
| Breathing | 1 − 0.9b in water, 0.1 + 0.9b in air | anchored | |
| Larval drift | 15 m, same medium | arbitrary | |
| Seed dispersal unit | 10 m per gene unit | arbitrary | |
| Entry to a habitat | measured speed > 5 mm/s | arbitrary | the threshold is a switch |
| Measurement | at birth and at half adult mass; 8 bodies/day | performance | queue alternates most-copied and newest |
| Neutral prior for a changed body | parent's speeds | method | |
| Immigration button | 10 random grazers | experiment | not the model |
| Population guard | none (was 5,000, then 15,000) | removed | |

## World (eco.rs new, terrain.rs, planet.rs)

| Rule | Value | Backing | Notes |
|---|---|---|---|
| Cell | 100 m², Fibonacci sphere, 6 neighbours | numerical | invariance test in MODEL.md |
| Sea level | quantile of elevation = water coverage | geometry | |
| Lapse rate | 6.5 °C/km | measured | |
| Latitude profile | 12 − 45·lat² | anchored | |
| Shore submersion | 0.5 ± 0.17 over 14.8 d | anchored | spring-neap |
| Decomposition | 3%/day × temperature × moisture | anchored | |
| Weathering, leaching | +0.002, −0.1%/day | arbitrary | |
| Initial pools | 0.02 kg/m² nutrient, 0.01 kg/m² litter | arbitrary | |
| Founders | none | — | life starts as algae |

## What the audit says to do next

1. The floors and multipliers in attack and defence are the largest block
   of arbitrary numbers in one place. They should become one mechanism:
   a pursuit model (Howland) with turning radius and speed in body
   lengths, and a damage model from claws against plating.
2. Phagocytosis capture, osmotrophy rate and the suspended fraction set
   the whole origin economy and are guesses. Measure a protist's
   clearance rate per surface and use it.
3. The entry threshold (5 mm/s) is a switch. Entry should follow from
   the range model: a body with no measured ability simply has no range
   in that medium.
4. The plant density caps are the last performance guard that can bind.
   Light competition should limit plants; the caps should be raised until
   they never bind, or replaced by cohorts.
5. Mutation rate (1.5 per birth) is high and arbitrary; it also decides
   how well cohorts would work. A per-gene rate with a biological anchor
   is needed.
