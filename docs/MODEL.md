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
the current width). With that fix the same test gave plants 80, 74 and 39 kg/ha
and animals 59, 77 and 117 per ha at 25, 100 and 400 m²: the fine and
medium cuts now agree within about 25%, the coarse cut (150 cells for
the planet) does not and oscillates hard (1393 to 77 to 781 animals
between prints). Candidates for what remains at coarse cells: the
sequential per-cell light allocation among many plants, predation
sampling three candidates a day from a much larger pool, and the graph
itself being too coarse for ranges of a few hundred square metres.

One rescue remains and is not biology: when the sea's animals fall below
about one per 1,200 m² (the founder density), a few immigrants arrive. A
planet has no off-map. It keeps the demo from ending in extinction and
should go once the ecology holds itself up. The
earlier "hectare patch" experiment and the "home range spill" were symptoms
of this leak; the spill survives only as the honest geometry of a range
larger than one cell.

### Origin: algae only, no founders
The world used to be seeded with a few dozen random grazers and topped up
whenever the sea emptied. Both are gone: life starts as floating algae
sheets and nothing else, and extinction is a result, not an error. For
heterotrophy to arise it needs the cheap first step real biology took:
a heterotrophy gene (0..1) invests in absorbing dissolved and dead organic
matter through the surface, at 0.3 kg per m² of surface per day per kg/m²
of dead matter, and displaces photosynthetic machinery in proportion (a
mixotroph's trade-off; photosynthesis × (1 − 0.6·h)). A sheet can earn in
the dark or under a canopy before it has any mouth. Motility still drifts
in small steps, but one mutation in ten is a jump of up to 0.35, a
flagellum rather than a slow slide. A seed with motility over 0.5 is born
motile: it keeps photosynthesising through its membranes (halved, for a
surface not held flat to the sun, and displaced by heterotrophy), keeps
absorbing through its surface, filter-feeds at the clearance rate, and can
begin to graze with whatever mouth its geometry gives it. The first
algae-only run (4000 days) produced eleven motile seeds and every one
died: at that point a motile sheet lost photosynthesis the moment it
moved, which is a sudden animal, not a mixotroph. Fixed as above. The
second run (5000 days) kept one lineage alive from day 3000: a 153 kg
raft of 22 m² of membranes, zero speed, "grazing" two kilograms a day.
Two leaks: a membrane root was given a mouth of its own chord (two
square metres of bite), and a swimming photosynthesiser got full sun
while the plants beside it competed under Beer-Lambert. Now only a
capsule trunk has a mouth (a sheet absorbs or filters), and a motile
photosynthesiser gets the light left under the patch's canopy, like any
understorey. The third run (5000 days): about thirty motile seeds
arose and every one starved; no heterotroph persisted. With the exploits
closed, the honest answer so far is that the model's sea does not yet
offer a living to a first heterotroph. The dead-matter pool it could
absorb stands at 600-1000 kg across the planet, so the limit is not the
food but the step from sheet to feeder; the candidates below are the
missing steps.

### A decode bug found while checking the enrichments
A body whose root node is a membrane took its mass from a capsule of the
membrane's radius: a 5 cm sheet weighed 630 g instead of 8 g, carried a
lung cavity, and rose through the water as a buoyant slab at 1.8 m/s.
Every "fast swimmer" the audits had flagged (0.5-1.3 m/s, two parts) was
one of these. A membrane root now keeps its membrane mass and has no lung;
the same bodies measure 20-30 g and move under 1 cm/s. The sham control
had been subtracting the drift, which is why it never showed as a fake
speed, but the mass and the buoyancy were wrong all along.

### Origin enrichments (built)
- **Microbial loop.** Decomposition turns dead matter into bacterial
  biomass (growth yield 0.4) and mineral nutrient; bacteria respire and die
  back to nutrient at a fifth a day unless eaten. Filter feeders strain
  them (meat-like quality); phagotrophs capture them.
- **Phagocytosis as the first mouth.** An organism with heterotrophy
  captures a fraction (0.05 per unit heterotrophy) of the particles that
  water movement (5 cm/s of waves and current, plus its own speed) carries
  past its surface each day. A sheet needs no trunk to eat bacteria.
- **Size floor at a centimetre.** Parts can be 1 cm long and 4 mm thick,
  so a body can weigh under a gram instead of the old 80 g floor. The
  physics probes (single capsule, hand-built gliders, audited bodies)
  were rerun unchanged.
- **Light through water.** Light reaching a plant's top falls off with the
  water above it (extinction 0.3 per metre), so deep beds are dark and the
  osmotroph's niche is real; a motile photosynthesiser near the surface
  gets the canopy's leftover light.
- **Tide.** The intertidal band's submersion swings between a third and two
  thirds over a 14.8-day spring-neap cycle. Seasons (axial tilt) remain
  unbuilt.

The check: in the algae-only world before these enrichments,
heterotrophy among sessile plants rose slowly (mean 0.07, 38% of plants
over 0.1 at day 3000) and 28 motile seeds arose over 3000 days, none
persisting. After them: mean 0.28 at day 5000 with 99.8% of plants over
0.1 and a third over 0.3, so the osmotroph/phagotroph niche is real and
strongly selected; motility stayed near zero (mean 0.014) and 15 motile
seeds all died. That run also showed 20,000 of 27,000 sea plants in near
darkness, because the founding algae were being treated as bed-anchored
under twelve metres of water; a sheet with no stem and no root now floats
and is lit at the surface. With that fix (5000 days): heterotrophy
again rose to a mean of 0.28 with a third of plants over 0.3, 15 motile
seeds arose, none persisted. The likeliest reason is now in the
accounting rather than the ecology: the moment a seed is motile it pays
a vertebrate's field metabolic rate (0.006·M^0.75 per day) plus muscle
work, while the sessile sheet beside it pays 0.4% of its leaf mass a day.
A Euglena does not pay a fish's bills. Maintenance should be a continuum
tied to activity and tissue, not a kingdom switch; that is the next step.

### No switches in the body: form, joints and kingdom as continua
Node kinds (capsule, membrane, pad) and joint kinds (hinge, ball, rigid)
were categories a mutation flipped. They are now four continuous genes
per node, and every quantity the physics and the ecology read is a blend:
- **flat** (0 round capsule .. 1 thin sheet of chord 2·r0): mass, tissue
  volume (buoyancy), muscle section, fluid forces (capsule drag weighted
  1 − flat, panel lift and drag weighted flat), spin drag, mouth (the
  trunk's front times 1 − flat), plant leaf share (flat) against stem share
  (1 − flat) in area, mass, load-bearing section and transport path.
- **grip** (0 skin .. 1 clawed foot): contact friction, and the claw metric
  in attack.
- **stiffness** (0 free .. 1 welded): a passive spring at the joint of
  twenty times muscle strength per unit gene holding the rest pose, with
  the driven amplitude and the limits shrinking by 1 − stiffness; muscle
  cost counts only the driven share; plating is mass held stiff on round
  parts.
- **freedom** (0 one driven axis .. 1 two): the second axis's amplitude
  and limit.
The contact shape handed to the solver is the nearer of capsule or plate,
because a rounded box with no inner box has no mass in rapier; its mass is
set from the blend. Old genomes still carry the labels and resolve to the
ends of each continuum. Mutation nudges each gene by ±0.1, one time in
five by ±0.4.

Maintenance is the same continuum across the kingdom line: tissue upkeep
at a plant's rates (membranes 0.4%/day, the rest 0.05%/day) plus the
vertebrate field rate scaled by motility, paid identically whether the
organism sits in the plant tick or the animal tick. A sheet at motility
0.5 pays half an animal's bills; at 0.05 it pays a sheet's. The first
algae-only run on this continuum (5000 days) selected heterotrophy even
harder (half of all plants over 0.3) and motility rose only to a mean of
0.05: paying for motility before it is an animal bought nothing, which is
a switch of its own. Motility now pays continuously on the sessile side
too: a swimming cell meets more particles (2 cm/s per unit motility added
to the water's 5 cm/s) and can hold nearer the light (its effective depth
shrinks by 1 − motility). The run with that payoff (5000 days): heterotrophy
mean 0.24, motility mean 0.03, eleven motile seeds, none persisting. The
payoff is too small against the cost at these particle densities (15 kg of
bacteria across the planet); the honest state at that point was that no heterotroph had
yet arisen from algae.

### The first-day economy, read off a ledger
Following the first motile seeds day by day (`seed_ledger` example) showed
why: a 184 g motile sheet took in 1.0 g a day and burned 1.2 g. Of the
1.2 g, 1.05 g was "a vertebrate's field rate times motility", charged for
being 62% motile before it had moved a millimetre, while the sessile
sheet beside it paid 0.74 g of tissue upkeep; and the motile sheet's
photosynthesis had been halved for "not being held flat to the sun",
which the plant beside it was not charged. Both were accounting.
Maintenance is now tissue upkeep only: sheets at a leaf's 0.4%/day, other
tissue from 0.05%/day (a plant's wood) to 0.3%/day for a fully motile body
(muscle tone, nerves, osmoregulation), with Q10 and the oxygen limit;
locomotion is charged as the work actually done (muscle section times
swing times rate, and the cost of transport on moves). A swimming sheet
photosynthesises like the plant beside it. The microbial loop itself was
not the blocker: 25 g of bacteria per 200 m³ patch is in the real range,
and the seed with a little heterotrophy was already taking 1.5 g a day
from it. With the ledger fixed, the followed seed grew from 37 g to 78 g
in its first 80 days with its reserve rising. The 5000-day run after
that: 28 motile seeds, three to five alive at a time, none established.
They grew but never bred: the animal side gated reproduction on a
maturity gene the plant side never selects (0.7 of full size) and gave a
770 g sheet a 140-day life, so it died of age a decade short of maturity.
Two more switches removed: reproduction uses the one gene both kingdoms
have (the plant's repro_frac, the fraction of full size at which to
breed), and lifespan blends the sessile lifespan gene into the animal's
150·M^0.25·longevity by motility. That run: six motile seeds, none
established, and the ledger showed why: a 1.85 m² sheet weighed 57 g in
the plant decoder (0.12 mm thick, dry) and 2.4 kg in the animal decoder
(a fixed 1.5 kg/m² "skin and spars" for wings), so the same genome had
forty times the body to grow and maintain the moment it moved. One
thickness gene now sets a sheet's mass in both decoders (0.1-2 mm of
tissue; a bird's wing with bone is about 1 kg/m², an alga a tenth of
that). With that, the followed seed grew from 35 g to its 305 g adult
size in a hundred days and had ten offspring, themselves born motile:
the first motile lineage to establish from algae. In the full world it
arose at day 1000 and filled the sea and the shore to the 5,000-animal
guard within 250 days: motile photosynthetic sheets, a thousand lineages
by mutation, grazing nothing, with the guard as their only limit. A guard
is not a limit the world has, and a population held by a lottery of
discarded births stops evolving. Their real limit is light: floating
sheets shade each other and the beds beneath them. The surface mat now
intercepts 1 − exp(−0.5·A/a) of the light, each sheet taking its share by
area, and the plants below get what passes. The guard is raised to 15,000
and is a performance stop only. The 3000-day run after that: the motile
line arose at day 1000, reached the new guard by day 1500 and held it,
3,800 lineages, 0.1 kg sheets of 1-2 m² with no swimming speed at all
(they drift), living on their own photosynthesis, absorbed dead matter
and bacteria strained from the water (the diet labels call them
detritivores), seven thousand of them on the shore, a hundred gliding in
the drop test as paper would. Grazing of plants is half a kilogram a day
across the planet, predation nil. So: a motile kingdom has come back by
itself, as floating mixotrophic mats. Grazers and hunters have not, and
the guard is binding again, which means the mats' real limit (light and
particles) has not yet been reached at this density. Next: a per-area
guard so density regulation is the world's, and the ledger again for why
nothing swims when motility has arisen. The remaining
switch is in the code, not the biology: an organism is ticked as a plant
below motility 0.5 and as a mover above it, because the two have different
data structures. Nothing it pays or earns jumps at that line.

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

## Flight, proved and disproved (2026-11-03)

The question was whether flapping sheets make a body fly in this physics,
with the claim held to a test rather than a replay impression.

Three probes (`examples/flap_probe*.rs`) built a bat-like body by hand:
trunk, two mirrored arm bones, a sheet on each. The first two probes found
no gain from flapping, but for the wrong reason: the mountings they chose
by best glide had both wings stacked above the body, so the "flap" was a
sideways wag. `flap_probe3` chooses the mounting by construction (lateral
arm, flap axis along the flight direction, horizontal sheet, feather
about the span). With that geometry:

- Under the flat-plate model (force normal to the plate) flapping lowered
  the mean height by 0.3 to 1.8 m at every frequency and feather phase.
  With the force normal to the plate there is no way to tilt it forward
  without pitching the wing, and the wing's own hinge, driven by the thin
  arm's muscle, was too weak to pitch it against the air.
- Two model changes followed, both real-life systems: attached-flow lift
  below the stall (thin-aerofoil slope, aspect-ratio corrected, induced
  drag; the flat-plate normal force past the stall), and a third driven
  joint axis, the twist about the segment's length, which is the shoulder
  pronation birds and bats use to pitch the wing. The twist is driven by
  the parent's muscle like the other axes.
- With both, flapping with the twist a quarter cycle off the flap keeps
  the body 0.7 to 1.25 m higher than holding the same wing, and aloft for
  the whole 4 s against 69 to 73% held. The same body with the sheets
  replaced by rods of the same mass falls like a stone (mean height
  2.6 m from a 10 m launch). This is the test
  `flapping_and_feathering_sheets_add_lift`.

Two other changes came out of the same work. A part's torque now scales
with its parent's cross-section (the muscle that swings a thigh is on the
hip, the one that swings a wing is on the chest). And the air measurement
in the ecology gained its control: the same body holding its wings still.
Glide is what the held wing does beyond a thrown stone; powered lift is
the metres of mean height the stroke adds. Reach is measured as the
farthest any part end sweeps during the land, water or air episode, not
read off the rest pose.

What this does not show: that flight will evolve. The test says the
physics permits it; the ecology decides whether a lineage ever pays for
it.

### A single run is not evidence

While turning the flapping proof into a test, the same body built in two
files gave different numbers: the mounting angle differed by one float
rounding (1.5707963 against 1.5707964 radians) and the held wing's mean
height moved from 4.45 to 4.71 m, the stroke's gain from +0.91 to +0.06 m.
Over a grid of seven mounting perturbations (down to a millionth of a
radian), three frequencies and four feather phases, the gain was positive
in 77 of 84 runs with a mean near +0.45 m and the rod twin always at
2.57 m. The flight dynamics are chaotic at the level of a float; the claim
holds as an ensemble and is tested as one (mean, share positive, best).

This matters beyond the test. The ecology measures each body with single
episodes. For bodies whose motion is this sensitive, a single episode is a
sample, not a measurement, and a body's inherited speeds may be a lucky
draw. Averaging a few perturbed episodes would cost a few times the
physics budget, which is already the binding limit.

## The arena: predation fought out (2026-11-03)

Whether one body type can eat another is no longer a formula. At a
predator's first real attack on a prey of another lineage, the pair goes
into a two-body physics world in their medium (`arena.rs`): the predator
steers at the prey and the prey steers away through a bearing controller
on the turn their own tests measured (its sign from the turn test). A
bout is eaten when the mouth (the trunk's front, with a gape of the
trunk's diameter, on a prey part that fits in it) or a clawed foot holds
the prey for one second; fled when no bite landed and the prey is farther
away at the end of six seconds; survived otherwise. Ten bouts from
perturbed starts per pair of lineages; the shares are cached and reused
for every later meeting. Until a pair has been fought, the old
attack-against-defence form is the prior.

Two design lessons on the way:

- Keying the fights by full genome made every mutation a new type, and
  queueing a pair for every candidate the predator considered grew the
  work without bound; a 1,500-day probe did not finish in half an hour.
  The type is now the lineage and a pair is queued only when an attack
  is actually attempted.
- Behind the body measurement budget the arena starved: in a populated
  world bodies are always waiting, and 1,300 days fought no pair. The
  arena has its own allowance (one pair a day headless, one every four
  days in the browser; a pair costs 0.2 to 0.5 s for small bodies).

What the arena found, Earth, 400 cells, seed 4, day 1,100: 102 pairs
fought, 48,840 animals. Eaten 0% in every pair; fled 8%, survived 92% on
average; not one pair with any kill. The ecology was recording 22 kills a
day at that moment, all from pairs still on the prior formula. The
formula was wrong about these bodies and the physics is right: they are
sheets with no mouth (a membrane root has a gape of zero), moving at
millimetres per second, and cannot catch or bite anything. Every kill
count reported from earlier runs came from the formula, and the arena
now says those kills could not have happened.

So predation in this world starts at zero and has to be earned: a trunk
with a mouth, a gait that closes distance, a turn that follows. That is
what the user asked for, and it is also why the biggest remaining lever
is the physics budget, which sets how fast a new body plan gets its
chance to be measured and fought.

## Gaits: what the physics can do, found by search (2026-11-03)

The planet had never produced a body with a gait: pills and sheets only,
in every run. The first question was whether this physics and genome
can represent an upright multi-segment gait at all. `examples/gait_search`
searches genomes directly on the land episode (an audit tool, not the
planet) and the answer came in stages, each one a measure the episode
did not have:

1. Scored on speed, the search found a heavy pill thrashing two arms
   (3 m in a lunge, then nothing).
2. With belly contact and segment count in the score, it found giants
   (300 to 2,600 kg) tumbling at 6 to 10 m/s with the trunk held off the
   ground by enormous limbs.
3. With footfalls and trunk spin added, still giants: many limbs make
   many footfalls, and a Gaussian penalty on spin was needed before
   rolling stopped winning.
4. The missing physics was energy. Nothing charged for the work the
   joints did, so flailing was free. Episodes now measure joint work from
   the motor model (torque against angular rate, each axis, each step)
   and report cost of transport in J/(kg m); an energy audit (mechanical
   energy gained minus work) confirms the physics does not leak (it
   dissipates 7 to 36 kJ per 5 s episode in these bodies).
5. Scored on efficiency as well, with distinct stepping limbs and trunk
   clearance, the search (population 160, 100 generations, two seeds)
   found `docs/gaits/stepper_seed12.json`: 7 parts, 270 kg, 2.0 m/s,
   four stepping limbs, trunk carried 0.36 m clear (more than its own
   diameter), spin 1.6 rad/s, cost of transport 1.4 J/(kg m). The replay
   (`stepper_seed12_a.png`, `_b.png`) shows a trunk on a two-segment
   leg with a knee plus three thinner limbs, stepping forward steadily:
   11.8 m in 8.8 s. `stilt_seed11.json` is a 10-part body on a jointed
   stilt at 2.1 m/s; `shuffler_seed9.json` a trunk on two stubby feet.

So the physics can walk, once walking is what pays. Two measurement
bugs were found on the way: the sustained-speed window started early and
divided by the wrong span (speeds overstated by 1.4x), and the solver's
joint impulses are not populated for these joints, so work had to come
from the motor model.

What this does not show: that the planet finds such bodies. In the
ecology nothing charged for the gait either (activity cost was a guess
from muscle section), measurement starved behind 100,000 births a day,
and a sheet absorbing dead matter at the surface never needed to move.
The ecology now charges the measured power and cost of transport, and
measures a day's batch on every core. Whether that is enough is the next
run's question.

## Why the planet grew flakes, and the water it was missing

The next run answered: no. Over 1,200 days (600 cells, seed 4, 32 bodies
measured a day) the land filled with 146,000 animals of 20 g and 2.3
parts; the best land speed was 0.21 m/s; only 16 of 174 land cells were
still green. The ledger shows the attractor and the reason selection
could not leave it.

1. Motile sheets lived on dry land as mats: photosynthesis, osmotrophy of
   the dead-matter pool through their surface, and a carrion loop of
   their own corpses. Nothing charged a wet surface for being in air.
   Animals had no water balance at all; plants did.
2. Maintenance was linear in mass, so a gram of a tiny body cost what a
   gram of a large one did, while intake through a mouth or a surface
   scales with area. Small won on every term.
3. Measurement starved: 15,834 bodies measured against 246,679 waiting.
   A mutated body inherits its parent's speed as a neutral prior but pays
   the mass of its new part at once, so under a backlog every added
   segment is a pure cost and selection removes it. The backlog itself
   selected for pills.

The changes are real systems, not thresholds. A land animal now loses
water through its skin per square metre (1 kg/m²/day in dry air, scaled
by the cell's dryness, warmth and the land tolerance gene, which is now
the one integument gene for a plant's cuticle and an animal's skin),
drinks from the cell's wetness, eats the water in fresh food, and dies
past a 15% deficit plus its water store. A 20 g unwaterproofed body dries
out on ordinary land in a day or two; a 1 kg body does not. Bulk tissue
maintenance now scales as mass to the three quarters (Kleiber). Osmotrophy
needs a wet surface. A hungry body relocates as far as its day's path,
paying the measured cost of transport per cell crossed (the fifty-metre
cap is gone).

The first run with the water balance killed nothing: 99,000 land animals
at day 1,600 and not one dried out. The drinking rule had let a body
refill the share of its deficit equal to the cell's moisture every day,
which on ordinary land (moisture about 0.5) is an unlimited well. Humid
air is not a drink. Inland water is now dew on the body's own surface and
the water in fresh food; open water is drunk only in or beside the sea.
The same run showed the mats photosynthesising without the nutrient limit
plants have; they now share it.

## The land that was not there

Run 3 (scale-free arena starts, body genes mutating at a fifth) ended at
day 2,000 with 136,873 animals "on land" and the new ledger column
answering where: 0 inland. Every land animal was on the intertidal
shore. The probe `land_probe` then showed why: of the 600-cell world's
174 land cells, 141 were shore and 33 interior, and the interior held 0.4
kg of plants, stunted seedlings at the land cap of eight per cell. There
was no land to walk on and nothing on it to eat.

Three causes, three changes. The terrain's fine noise octaves shattered
the coast at the cell scale; terrain is now drawn at the grid's own
resolution (`elevation_at_res`), which gives the same seed 72 interior
cells at 600 and 505 at 3,000. The land plant cap was six times the
water cap's reciprocal for no stated reason; it is now the same. And the
measurement queue's prior was neutral on one side only: a mutated body
got its parent's speed but paid its own mass, so a backlog selected
against every added segment; an unmeasured body is now its parent's body
on both sides until measured.

Land tolerance in plants rises slowly (0.01 to 0.05 in 1,200 days on the
shore, where seed rain from the sea swamps it); the inland flora is the
next thing to watch.
