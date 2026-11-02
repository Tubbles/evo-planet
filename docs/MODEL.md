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
Encounter rate = search speed × detection width × resource density
(Holling type II). Handling time per item ∝ item mass / bite rate, bite
rate ∝ `M^0.75`. Intake = aN / (1 + ahN), limited by stomach space. Big prey
means fewer, longer meals; small items mean many quick ones. Movement cost
uses cost of transport ∝ `M^-0.3` per kg per metre (Tucker 1970).

### Predation as attack versus defence
Attack = `M^0.75 · relative speed · weapon`; defence = `prey M^0.75 ·
prey speed · armour`. Success probability is a logistic of
`(attack − defence) / scale`. Weapons and armour are genes that cost mass
and maintenance. Nothing forbids a lion taking a buffalo; it is just
unlikely per attempt, and handling a large carcass takes days during which
it is also carrion for everyone else. Replaces "prey must be under 2.5×"
and "under 0.2× is engulfed".

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
Light interception with Beer–Lambert extinction, water balance from roots
versus transpiration, nutrient uptake from the patch pool, structural cap
from stem strength against weight under gravity, respiration by tissue
type, seeds and dispersal, land tolerance as an evolvable cost. Next:
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

## How we will know it works
Not by reaching a target. By emergent statistics matching ecological
regularities: body-size distributions spanning orders of magnitude,
predator–prey mass ratios with a wide distribution, several trophic
levels in the realised energy-flow web, size-structured food webs
(Loeuille & Loreau 2005), colonisation of new habitats along gradients,
turnover of lineages without global collapse.
