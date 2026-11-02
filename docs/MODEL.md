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
Any body whose membranes give a wing loading under 50 kg/m² is also dropped
from 10 m in the physics. The horizontal distance it covers before touching
the ground, over the episode, is its air speed: a brick scores nothing, a
parachuter a little, a glider metres per second, a flapper more. It is
continuous so wings can grow by degrees. Over land the day's foraging path
uses the larger of crawl and air speed (the flier lands to eat), prey
escape uses it, and a flier may enter land patches it cannot crawl in.
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

## How we will know it works
Not by reaching a target. By emergent statistics matching ecological
regularities: body-size distributions spanning orders of magnitude,
predator–prey mass ratios with a wide distribution, several trophic
levels in the realised energy-flow web, size-structured food webs
(Loeuille & Loreau 2005), colonisation of new habitats along gradients,
turnover of lineages without global collapse.
