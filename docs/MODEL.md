# Evo Planet ecology model: design notes

Status: exploration. Everything here is provisional and expected to change.
The rule for adding anything: it must have a real-world analogue and a
reason, not a symptom it papers over. Where a number appears it should be
a measured exponent or an order-of-magnitude anchor, not a tuned threshold.

## Principles

1. **Energy is the only currency.** Every organism has an energy budget in
   kilograms of dry biomass equivalent (about 17 MJ). Nothing is scored.
2. **Body size is the master trait.** Metabolism, lifespan, growth, speed,
   stomach size, starvation tolerance, what you can kill and what can kill
   you all scale with mass with known exponents. Size then evolves because
   of those consequences, not because a rule says big is good.
3. **No hard cut-offs between organisms.** Outcomes are probabilities from
   continuous comparisons (attack vs defence, profitability vs alternatives).
4. **Trade-offs, not weights.** A trait that helps somewhere must cost
   something somewhere else, in the same energy currency or in time.
5. **Classes are observations, not genes.** "Herbivore" and "carnivore" are
   computed from what an individual actually ate. Genes shape anatomy
   (gut, weapons, armour, size, timing); behaviour follows from payoff.
6. **Carrying capacity emerges.** No global population caps if we can avoid
   them; limits come from food, space and predation.

## Systems (animal side)

### Allometric metabolism
Field metabolic rate `FMR = b0 · M^0.75 · f(T)`. The exponent is Kleiber's
(Brown et al. 2004); `b0` anchors ectotherms around one tenth of mammals.
Temperature dependence through a Boltzmann/Q10 factor on body temperature,
which for an ectotherm is the patch temperature. Replaces the fixed
"basal × (0.4 + tf)" constant soup.

### Energy storage and starvation
Reserve capacity is a fraction of body mass (fat fraction gene, costs mass to
carry). Starvation time therefore scales as `M^0.25`: big animals ride out
lean periods, small ones must eat daily. Replaces "die when energy < 0" with
a reserve that is part of the body plan.

### Growth and life history
Offspring are born at a fraction of adult mass (gene) and grow by converting
energy into body mass (West–Brown–Enquist style: anabolism ∝ M^0.75 minus
maintenance). Reproduction needs maturity (mass fraction) and enough reserve
to make an offspring. Lifespan ∝ `M^0.25` × a gene. Juveniles are slower,
weaker and worth less as prey. This yields r- versus K-strategies as
outcomes, not settings. Replaces "threshold = 0.6 × mass + 0.02".

### Stomach and digestion
Gut capacity ∝ mass × gut fraction gene; digestion rate ∝ `M^0.75`.
Food enters the stomach and energy is extracted over time; a full stomach
blocks further intake. Assimilation efficiency depends on a continuous gut
type gene: a fermenting herbivore gut extracts 40–55% from plant tissue and
little from meat per unit time; a short carnivore gut extracts 80–90% from
meat and 10–20% from plant tissue (Demment & Van Soest 1985; Jarman–Bell).
A 0.1 carnivore can fill its stomach with meat and still get little out of
it, which is exactly the gradient we lacked. Replaces the diet-weight genes.

### Foraging as a functional response
Encounter rate = swept area / patch area (Holling type II): a detection
strip two reaches wide along eight hours of travel at the measured speed,
plus the circle a still body can reach. Reach is the farthest the body
extends from its centre of mass; on land only an upright body (mean centre
of mass height in the physics episode) gets its full reach for browsing.
A body that cannot move eats only what is within arm's length. Bite rate
comes from the mouth: the trunk's front cross-section. Intake = aN / (1 + ahN), limited by stomach space. Big prey
means fewer, longer meals; small items mean many quick ones. Movement cost
uses cost of transport ∝ `M^-0.3` per kg per metre (Tucker 1970).

### Predation as attack versus defence
Attack = `M^0.75 · speed · claws · uprightness`; defence = `prey M^0.75 ·
prey speed · plating`. Success probability is a logistic of
`(attack − defence) / scale`. Claws and plating are not genes: claws is the
cross-section of Pad parts relative to the trunk, plating the fraction of
body mass in rigidly attached capsules (which also slows the body). Both
are paid for as real parts in the mass budget. Nothing forbids a lion taking a buffalo; it is just
unlikely per attempt, and handling a large carcass takes days during which
it is also carrion for everyone else. Replaces "prey must be under 2.5×"
and "under 0.2× is engulfed".

### Flight as measured function
Any body whose membranes give a wing loading under 50 kg/m² is also launched
at 3 m/s from 10 m in the physics, the way a gliding animal leaps. Metres
beyond where a thrown stone lands are the wings' doing (the glide, with
glide ratio = that over 10 m); a body still aloft at the end of the episode
is a flier at its measured air speed. Hand-built gliders at 3-4 kg/m² reach
about 6 m beyond the stone, so the gradient exists but is modest: the flat
plate model has no camber and a fixed parasitic drag.

What flight buys, each with a real analogue:
- **Launch height.** Gliders use the patch's canopy (tallest plant) and
  the drop to the lowest neighbour as a perch. About twenty launches a day
  times glide ratio times launch height is ground covered for free, and a
  launch that carries ten metres reaches the next patch. Tall plants make
  gliding useful, as trees did.
- **Range.** A flier ranges over every patch within an hour's flight
  (up to eight hops) and crosses water, so islands and far coasts are
  reachable before anything can crawl there.
- **Surveying.** A flier's carrion search strip is forty reaches wide
  instead of six, as vultures find carcasses from height.
- **Escape.** Taking off is the escape a ground predator cannot follow: a
  flier's defence is quadrupled, a glider's in proportion to glide ratio
  times launch height over ten metres.
Lift scales with the planet's air density, so dense atmospheres favour it.

### Decision by profitability
An animal attacks or eats an item when the expected energy per unit
handling time exceeds its current intake rate from alternatives (Charnov's
diet breadth model). Prey size preference, omnivory and specialisation
emerge from this and from gut type.

### Carrion and detritus
Dead animals persist as carrion that anyone with the gut for it can eat;
plant litter decomposes to nutrient. Scavenging needs no speed and is the
natural bridge to predation.

## Systems (plant side, already mechanistic)
Light interception with Beer–Lambert extinction between plants, and within
a plant by projecting its leaves from nine sky directions and removing
overlap (a lone vertical blade captures about 0.6 of its area, a pile of
leaves much less than their sum); water balance from roots versus
transpiration; nutrient uptake from the patch pool; structure checked two
ways under gravity, bending moment at every stem from the horizontal lever
arm of everything it carries, and Euler buckling of columns; a transport
cost for every leaf proportional to its mass times the stem path to the
base; leaf economics, where lifespan scales with leaf mass per area
(Wright et al. 2004) so thin leaves are cheap to build but shed in weeks;
respiration by tissue type; seeds and dispersal; land tolerance as an
evolvable cost. Next:
defence compounds (digestibility trade-off, leaf economics spectrum) and
leaf nitrogen as the thing herbivores actually value.

## Systems (world)
Spherical patches with elevation, temperature by latitude and altitude,
moisture, sea level from water coverage. Next: seasons (axial tilt),
rainfall shadow from mountains, rivers, and soil depth.

## What is deliberately not modelled yet
Sex and mating (asexual with mutation), senses and brains beyond the
physics CPG, disease, parasites, symbiosis, social hunting, migration by
season. Each is a candidate system, each with a real analogue.

## Changes made from the research pass (see reports/Mechanistic ecology for evolution sims.md)

- Kill return scales with relative size (Fritsch et al.): zero for prey ten
  times larger, 40% at equal size, full for prey much smaller. This is the
  rule that stops predation running away to ever-smaller predators.
- Capture probability is attack-versus-defence times a Gaussian hump in
  log10(predator/prey mass) centred at 10× (Brose 2006 ratios).
- Fat capacity ∝ M^1.19 (Lindstedt & Boyce), growth follows West–Brown–Enquist.
- Crowding enters feeding as Beddington–DeAngelis interference; the animal
  count is only a performance guard now.
- Plants carry a leaf-toughness gene on the leaf-economics axis: tough leaves
  lose less to grazing, digest worse, and photosynthesise slower.
- Founders are a few hundred grams, seeded in warm shallow seas; the sea's
  algae density is light-limited rather than counter-limited.
- Logged for falsification: realised predator–prey mass ratio of kills
  (field mean ≈ 10^1.6, 80% by the larger), satiation fraction (≈ 88%),
  daily energy flows, death causes.

## Audit log (what the energy flows showed, in order)

1. Founders starved while 95% of production went uneaten: Holling search and
   bite parameters were an order of magnitude low. Fixed from first
   principles (a patch is covered in a day; intake tenth of M^0.75).
2. Still starving: algae covered 16% of patch light at a 16-per-patch cap.
   Cap raised so light limits production; founders scaled to a few hundred
   grams and seeded in warm shallow seas.
3. Population peaked then died of old age: WBE growth is asymptotic, so
   reproduction gated on full adult mass never triggered. Maturity became a
   gene (fraction of asymptotic mass).
4. Births ≈ deaths with few starved or old: adults were eating their own
   young in the same patch (the cannibalism trap). Larval dispersal and
   detection scaling with prey length fixed it; the fauna then filled the
   performance guard.
5. Runaway miniaturisation to grams: evolution had found the plant tissue
   density gene in animals (20 kg/m³ bodies). Animal density is now fixed at
   water density. Unmeasured young could also enter any habitat; entry now
   needs measured ability.
6. Current state (Earth, 600 patches, 4000 days): 1,000–2,500 animals
   limited by food (two thirds of production grazed, starvation the main
   death cause), realised predator–prey mass ratio 10^0.64 with 96% of kills
   by the larger party, carnivores and scavengers appearing by realised diet
   (21 and 385 of 2,174), gut type and weapon genes drifting up, about 300
   animals on land at a time. Mean body mass still drifts to the size floor
   (0.05 kg, one or two parts): the Brännström/Loeuille–Loreau runaway. Next
   systems, not thresholds: predation pressure that falls on small bodies
   (a detection model that does not shelter the tiny, visual predators),
   seasonality so fat and size buy survival through lean periods, and plant
   structure (height, toughness) that only large or specialised grazers can
   exploit.

7. Body plan had no consequences: only speed was measured, the search
   baseline fed anything, and limbs only cost mass. Reach, mouth, claws,
   plating and uprightness are now read off the geometry and the land
   episode (see above); the search baseline is gone, replaced by swept
   area. Plants lacked self-shading, horizontal lever arms and transport,
   so a jumble of leaves was as good as a tree; all three added. Lifespan
   halved (150·M^0.25), growth and reproduction sped up.
8. With leaves rescored, the sea's algae went extinct: leaf thickness had
   been mapped to 1–4 mm (5–10× real), so respiration per area outweighed
   the halved capture. Thickness now spans 0.1–2 mm, and leaf lifespan
   follows leaf mass per area so the thinnest leaf is not free.
9. One-part bodies were "swimming" at inherited speeds: the physics queue
   measured 3 bodies a day against ~1,000 births, so most animals lived on
   their parent's numbers. The queue is now one request per body, ordered
   by living copies, and the default budget is 8 a day. After that: lineages
   swim at 0.07–0.33 m/s, a 0.09 kg lineage with claws 0.96 and plating 0.19
   takes 18% of its diet as meat, 254 carnivores by realised diet at day
   4000. Mean body still 2–3 parts and drifting small; the next question is
   what function a fourth part buys in the sea.

10. Land speeds were toppling, not walking: speed was total displacement over
    the episode, so a pill that fell over and rolled scored 0.2 m/s and was
    credited with kilometres of foraging path a day. Two fixes, both
    physical: speed is now sustained progress over the second half of a 4 s
    episode after settling, and bodies on the ground feel rolling resistance
    (coefficient 0.05), without which a toppled capsule rolls forever in a
    frictionless solver. A single capsule now scores under 1 mm/s on land.
    Land bodies are also placed on the ground instead of dropped 15 cm,
    which had handed every body kinetic energy and a topple for free.
11. Flight was never measured and had no function. See the flight section:
    leap launch, glide beyond a stone, launch height from canopy and
    hillsides, range, surveying and escape. First runs: no gliders yet by day
    3000; the plants are 1-2 m tall, so the perch is low. Taller plants are
    the lever, and they need the bending and transport model to pay off.

12. Looked, not inferred. Replaying the fastest bodies of a 2500-day run
    in the 3D view (zoomed) showed no walkers and no fish-like swimmers:
    the best land body is a capsule twitching a flap (0.3 m in 6 s), the
    next a capsule sliding on a plate; the best "swimmers" were floating
    capsules rising through the water with a tilted fin, 90% as fast with
    their motors off. Earlier claims of "honest walkers" in these notes
    were wrong and are withdrawn. Fixes, all physical: a sham control (the
    same body with motors off, its drift subtracted), water bodies start
    where they would rest (a buoyant body at the surface, a dense one on
    the bottom), and land bodies on the ground. What remains is what the
    physics and open-loop oscillators can actually produce; the next
    question is why coordinated gaits do not appear in the ecology, which
    only a replay-and-look audit can answer.
13. Nimbleness is measured, not only speed. A body that locomotes is run
    again with asymmetric drive (mirrored parts weaker, the others
    stronger: how animals turn) and with a shifted posture (climb or dive).
    The yaw rate and vertical speed gained are its turn rate and climb
    speed per environment. Chases use turn rate on both sides (Howland
    1974: manoeuvrability decides as much as speed).

### The test battery (what a body is measured on)
Every measurement is a physics episode with a control, never a number read
off the genome:
- **Water, Land:** sustained speed over the second half of a 4 s episode
  minus the same body with motors off; turn rate from asymmetric drive;
  climb speed (water) from a posture shift.
- **Beach (probe only):** a bed rising 12 cm per metre through the
  waterline; the body starts submerged and must climb out. It was briefly
  the gate from sea to land, which made it a crutch standing in for the old
  water-or-land switch. With the intertidal band the move inland is a
  crawl, which the land test measures, and survival inland is breathing
  and food. The scene stays for audits and replays.
- **Air:** launched at 3 m/s from 10 m; glide beyond a stone, sustained
  flight if still aloft; turn and climb for fliers.
- **Posture:** mean centre-of-mass height on land (uprightness) and the
  geometric reach, mouth, claws and plating from the decoded body.

### The intertidal shore
Land patches touching the sea are the intertidal band: under water half
the day (submersion 0.5, against 1 at sea and 0 inland). Nothing switches
on water-or-land any more; everything blends on submersion. A plant there
gets 0.3·0.5 + 0.5 of full light (algae do better than at sea), is never
short of water for the submerged half, and needs land tolerance only for
the dry half, so algae mats cover the shoreline without any new gene. An
animal there breathes with gills half the day and lungs the other half,
swims half the day and crawls the other half, and is reached by swimming
in with the tide or crawling. Dry land beyond the band is a crawl away,
and living there is a matter of breathing and food, not a permit. Larvae drift through water and shore alike, never onto dry
land. Speed is also reported in body lengths per second (length taken as
twice the reach), so a centimetre-long grazer at 2 cm/s reads as the
mover it is; the ecology keeps absolute speed for area swept, which is
physical.

### Breathing
A breathing gene runs from gills (0) to lungs (1). Gas exchange efficiency
is 1 − 0.9·b under water and 0.1 + 0.9·b in air, blended on the patch's
submersion, so a gill-breather on the intertidal shore gets about 0.55 and
lives, and lungs pay off gradually as a lineage moves inland. Efficiency scales the day's activity and
the bite rate; below 0.2 the animal suffocates with probability one half
per day. A sea lineage that climbs the beach still dies on land unless its
breathing has moved toward lungs, and a lung-breather drowns. Deaths by
suffocation are counted.

### Larvae stay in their medium
Newborns drift one or two patches from the parent, but only through
patches of the birth medium. Before this, sea larvae landed on beaches and
died: 6,000 suffocations in a 3,000-day run, all of them newborns, and
every "land animal" in the count was one of them. With it, the land count
was zero until the intertidal band replaced the shore gate.

### Numerical honesty
Explicit fluid drag is capped at the force that would stop a sample's
share of the part in one step, which removed a blow-up that threw light
parts at hundreds of metres per second (found by tracing a floating body
in the shore scene). A membrane displaces its tissue mass in water, not
its 2 cm collider volume, which had made a 53 g wing float like 700 g of
cork.

### Fidelity: where ten times more would go, and what it costs
The physics runs at 120 Hz for four seconds per test, five tests per body,
one body measured per living copy. The ecology runs one tick per day with
a 100 m² patch as the unit of space. "Ten times" could mean any of:
- longer and more tests per body (the battery above is the start; closed-
  loop control, so a body can steer toward food rather than run its
  oscillators open-loop, is the big missing piece and the likeliest reason
  no gait has evolved);
- sub-day ticks with within-patch position, so finding food is a path and
  not a sweep fraction;
- ten times the patches;
- finer physics (smaller step, deformable ground, water currents).
Each is roughly a tenfold cost on its own axis; the browser now runs about
ten days a second at 800 patches. Closed-loop control is the one that
changes what evolves rather than how precisely it is measured.

### Replay audit of the first "fliers" and the biggest lineage
Forty "fliers" at day 3000 were 10 cm plates with a stick that tumble as
they fall and drift two metres further than a stone, the same with motors
off. That is parachuting, not gliding. The label now follows the standard
definition: gliding is a glide angle under 45°, so a body must get ten
metres beyond the stone from ten metres up (ratio ≥ 1) or stay aloft. The
continuous glide ratio still feeds the ecology (a parachuter at 0.25 gets
a quarter of the launch-height benefits), only the count and filter changed.
The most numerous lineage (804 members) is three flat plates jammed
together, tumbling through the water at 0.35 m/s net of a 0.33 m/s passive
drift. The sham control handles the drift; it is still not a fish.

### Why bodies stayed at two or three parts, and what changed
Three biases against new body plans, none of them biology: a changed body
started at 0.7× its parent's speed until measured; the measurement queue
put the most-copied bodies first, so a brand-new plan was measured last
and in practice never; and every limb paid full muscle cost whether it
moved or not. Now a new body inherits its parent's speeds as a neutral
prior, the queue alternates between the most-copied and the newest body,
and muscle cost counts only driven joints weighted by how far and how often
they swing (a strut or a limp limb costs its mass only).

Then the payoff: a body's daily range (reach times path) is an area, and
the food it meets is density times that area, drawn from its own cell and
then the neighbours (up to seven cells); speed pays up to the point where
the range exceeds the cells the graph lets it reach. See "The cell is not
a thing" below for why the cell size itself must not matter. Chases
are judged in body lengths per second (Howland), with the floor lowered
from 0.5 to 0.2 so a sessile party is as easy as it should be. Filter feeding:
a fifth of the sea's plant mass is suspended, and a body clears about a
cubic metre of water per kilogram per day, as mussels and sponges do. A
first version used frontal area times a current, which at 100 m² handed
every still body a fifth of the patch's plants a day and bred 120 kg
sessile giants with idle parts within 900 days; clearance scales with mass
and gives no such advantage.

### Segment duplication
Limbs multiply in animals by duplicating a working module (serial homology,
Hox-patterned segments), not by inventing a random one. Mutation now has
that operator: a used node is copied with its attachment onto the same
host, either further along it (serial) or mirrored (bilateral), keeping its
oscillator with a random phase flip. Two legs become four; a fin becomes a
pair. The random-new-node operator stays. In a 900-day probe, three-part
bodies went from 15 of 226 to 71 of 281, and the fastest bodies were
six-part; two-part bodies still dominate, and the open-loop oscillators
remain the likeliest ceiling on coordination.

### Two more physics leaks, found by the audit
The fastest "swimmers" after the home-range change spun at 65-95 rad/s.
Tracing both parts showed them co-rotating, so it was not the motor: the
drag model samples points on a capsule's axis, so a short fat part had no
resistance to spinning about its own axis, and the counter-torque from
damping its limb spun it up without limit. Every part now has spin drag
about its own axis (a rotating cylinder, torque ~ ½ρ·C·r⁴·L·ω|ω|). Joint
motors also now obey a Hill force-velocity limit: torque falls linearly to
zero at 25 rad/s of joint speed, so muscle power is bounded. The spinning
bodies went from 0.97 to 0.07 m/s; a fin-sculler from 0.57 to 0.28; the
"best land body" from 0.09 to 0.004 m/s, so it too had been spin-powered.
The hand-built gliders are unchanged.

### The cell is not a thing
There is no patch size in nature. A patch is a discretisation cell, and
the model must not know how big it is beyond noise. Every rule is now a
density or a distance: light, nutrients and litter per square metre; plant
caps as individuals per square metre (a performance guard); crowding per
competitor per 100 m²; a body's daily range in square metres, swept from
its own cell and spilling into neighbours; predation encounter as the share
of the cell that range covers; movement cost per metre of cell width;
larval drift 15 m; seed dispersal in tens of metres. A body's range spills
ring by ring over as many cells as it covers (up to four rings), and a
predator meets prey across those cells, so the reachable range is a
quantity in square metres and not "the six neighbours". Founders are per
area of sea. The cell size is a run parameter (`evo_headless
--patch-area`), and the test is to run the same planet at 25, 100 and
400 m² and compare time-averaged densities per area. The first comparison was flawed on
my side (600 cells of each size, so three planets of 1.5, 6 and 24 ha);
the fair test holds the planet at 6 ha and cuts it into 2400, 600 or 150
cells. Time-averaged over days 1200-3000 that gave plants 136, 77 and
32 kg/ha and animals 37, 167 and 78 per ha at 25, 100 and 400 m² cells:
not invariant. One leak named and fixed from that: a hungry animal
relocated one cell a day, 5 m on fine cells and 20 m on coarse ones;
relocation is now a distance (up to 50 m of the day's path, in cells of
the current width). The next run of the same test is the measure of what
remains; candidates are the four-ring cap on a body's range, the
per-cell sorting of plants for light, and demographic noise.

One rescue remains and is not biology: when the sea's animals fall below
about one per 1,200 m² (the founder density), a few immigrants arrive. A
planet has no off-map. It keeps the demo from ending in extinction and
should go once the ecology holds itself up. The
earlier "hectare patch" experiment and the "home range spill" were symptoms
of this leak; the spill survives only as the honest geometry of a range
larger than one cell.

### Audit tooling
`cargo run --example shore_probe -- <genome.json>...` runs bodies on the beach
with motors on and off. `cargo run --example gait_audit -- <days> <out_dir>` re-runs the fastest
land and water bodies from an ecology with motors on and off, logs spin,
height and progress, and writes their genomes. The page exposes
`window.evoDebug.replayGenome(json, env)` and `setZoom(z)` so a script
can screenshot any genome in the real replay. The rule from here: no
claim about what bodies do without a replay looked at.

## How we will know it works
Not by reaching a target. By emergent statistics matching ecological
regularities: body-size distributions spanning orders of magnitude,
predator–prey mass ratios with a wide distribution, several trophic
levels in the realised energy-flow web, size-structured food webs
(Loeuille & Loreau 2005), colonisation of new habitats along gradients,
turnover of lineages without global collapse.
