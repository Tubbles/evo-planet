# Evo Planet Equations

Every quantity in the model and the equation that produces it, from the genome through the physics episodes to the daily tick of the planet. Units are kilograms, metres, seconds and days unless stated. `×` multiply, `^` power, `clamp(x, low, high)` limits x to the range, `exp` is e to the power. Source of truth: `evo_core/src` on main. Regenerate the page with `scripts/equations_page.py`; audit the numbers with `scripts/check_equations.py`.

## Genome

A directed graph of nodes and connections (Sims style) plus whole-organism genes. One genome decodes to an animal body or a plant body depending on how it is used; the motility gene decides which it is born as.

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Node genes | — | length, base radius, tip radius, flatness, grip, joint stiffness, joint freedom, joint limit, recursion limit, oscillator {amplitude, amplitude 2, amplitude 3, phase, phase 2, phase 3, frequency multiplier} |
| Connection genes | — | target node, position along the host, angle around the host, pitch, roll, scale, reflect, phase flip, terminal only |
| Whole-organism genes | — | base frequency, lung fraction, water store, land tolerance (cuticle or waterproof skin), motility, breathing (gills 0 to lungs 1), heterotrophy, gut type (plant 0 to meat 1), gut fraction, fat fraction, offspring fraction, reproduction fraction, longevity, lifespan (sessile), leaf toughness, seed mass, dispersal, density (plants) |
| Per-gene mutation chance | genome size | element count = Σ over nodes (8 + 9 × connections) + 4 ; base chance p = clamp(1.5 / element count, 0.02, 0.35) ; each gene is hit with chance k × p, k between 0.4 and 3 by gene |
| Animal body-gene rate | base chance | body genes (sizes, forms, joints, structure, lungs, water store, motility, density) use 0.2 × p |
| Flatness step | — | ±0.03 (one time in five ±0.15); grip, stiffness, freedom ±0.1 (one in five ±0.4); sizes nudged by about 20% |
| When mutation happens | reproduction | plants: every seed; animals: 3 births in 4; never during life |
| Structural operators | — | duplicate a used segment (serial or mirrored, keeping its oscillator); add a random node; add or remove a connection; collect unreachable nodes |

## Part geometry

Each decoded part is a slab whose thickness runs geometrically from a capsule's diameter (flatness 0) to a sheet's thickness (flatness 1).

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Length, radii | node genes, inherited scale | length = clamp(gene × scale, 0.01, 3) m ; base radius, tip radius = clamp(gene × scale, 0.004, 0.8) m ; mean radius = (base radius + tip radius) / 2 |
| Sheet thickness | tip radius | sheet thickness = clamp(tip radius × 0.002, 0.0001, 0.002) m |
| Thickness | mean radius, sheet thickness, flatness | thickness = (2 × mean radius)^(1 − flatness) × sheet thickness^flatness |
| Width | mean radius, base radius, flatness | width = 2 × ((1 − flatness) × mean radius + flatness × base radius) |
| Cross-section | width, thickness, flatness | cross-section = width × thickness × (π/4)^(1 − flatness) |
| Collider | cross-section, flatness | flatness ≤ 0.5: capsule of radius √(cross-section / π) ; flatness > 0.5: box width × length × max(thickness, 1 mm) |
| Mass | cross-section, length, mean radius, flatness | mass = 1000 kg/m³ × (cross-section × length + (1 − flatness) × 4/3 × π × mean radius³) |
| Trunk density | lung fraction, water store, flatness | trunk density = 1000 × (1 + (1 − flatness) × (water store − 0.9 × lung fraction)); trunk mass = tissue volume × trunk density |
| Mass ratio clamp | parent mass | a part lighter than parent mass / 30 is raised to parent mass / 30 (solver stability) |
| Muscle section | parent's cross-section | muscle section = cross-section of the part it hangs from (the root uses its own) |
| Maximum torque | muscle section | maximum torque = 4000 N·m per m² × muscle section |

## Body measures (read from geometry, no physics)

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Membrane area | all parts | membrane area = Σ flatness × length × 2 × base radius (one face) |
| Membrane mass | all parts | membrane mass = Σ mass × flatness |
| Reach (rest pose) | placements, centre of mass | reach = max over parts of \|part centre − centre of mass\| + length/2 + collider radius × (1 − flatness) |
| Mouth area | trunk cross-section, radii | mouth area = trunk cross-section × base radius / mean radius (the trunk's front face) |
| Gape | trunk thickness, radii | gape = trunk thickness × base radius / mean radius |
| Claws | parts' grip | claws = min(3, Σ cross-section × grip / trunk cross-section) |
| Plating | parts' stiffness | plating = Σ over non-trunk parts of mass × stiffness × (1 − flatness) / total mass |
| Fatigue (oxygen) factor | breathing, lung fraction, trunk volume, planet oxygen, muscle sections, base frequency, medium | gills = (1 − 0.9 × breathing) × 0.08; lungs = (0.1 + 0.9 × breathing) × lung fraction; capacity = submersion × gills + (1 − submersion) × lungs ; fatigue = clamp(40 × capacity × trunk volume × oxygen relative / (Σ muscle section × base frequency), 0.1, 1), computed for water (submersion 1) and for land (0) |
| Body surface (ecology) | current mass, membrane area, growth | surface = max(6 × (mass / 1000)^(2/3), 2 × membrane area × growth fraction^(2/3)) |

## Physics step (rapier, per time step)

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Oscillator target angle | amplitude, phase, frequency, time | target = amplitude × sin(2π × base frequency × frequency multiplier × time + phase) × amplitude scale, plus the steering bias; one target per driven axis (hinge X, swing Z, twist Y; twist opened by the freedom gene) |
| Hill speed factor | relative angular speed of the joint | hill = clamp(1 − relative angular speed / maximum angular speed, 0.05, 1) |
| Joint strength | maximum torque, fatigue, hill, stiffness gene | strength = maximum torque × fatigue × hill ; rigid = maximum torque × fatigue × rigid gain × stiffness gene |
| Motor torque | strength, rigid, angle, angular speed | torque = clamp(K × (target − angle) − D × angular speed, −strength, +strength) ; K = 3 × strength + rigid; D = 0.3 × strength + 0.1 × rigid |
| Dynamic pressure | fluid density, panel area, speed | q = ½ × density × area × speed²; density blends air and water by submersion |
| Plate force, attached flow | angle of attack α, aspect ratio | lift coefficient = 2π × sin α × aspect ratio / (aspect ratio + 2); lift ⟂ wind, with induced drag |
| Plate force, stalled | angle of attack | normal force = q × 2.61 × sin α × cos α |
| Stall blend | angle of attack | stall = clamp((\|α\| − 0.26 rad) / 0.12 rad, 0, 1); plate force = attached × (1 − stall) + stalled × stall, plus parallel drag ½ × density × area × C_D0 × speed² |
| Capsule drag and spin | radius, length, velocity, angular speed | drag ∝ ½ × density × speed²/3 × projected area; spin torque = ½ × density × 1.0 × effective radius⁴ × effective length × ω × \|ω\|, capped at inertia × ω / time step |
| Buoyancy | part mass, density, submersion | buoyancy = water density × (mass / part density) × g × submersion, upward |
| Mechanical work | torque, angular speed | work = Σ over steps and axes of (torque × angular speed if positive, else a fifth of its size) × time step (settled phase only; muscle pays a fifth for work absorbed while lengthening). The energy audit uses the full absolute sum. |

## Episode results

An episode is one body in one environment (land, water, air, shore) for a settling time plus 6 s of driven motion.

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Sustained speed | centre of mass | sustained speed = horizontal distance moved by the centre of mass over the second half of the episode / (episode time / 2) |
| Yaw rate | trunk angular velocity | mean vertical angular velocity of the trunk over the second half (signed) |
| Travel direction | centre of mass steps, trunk rotation | unit vector of Σ (trunk rotation⁻¹ × horizontal step of the centre of mass), in the trunk's own frame |
| Power | work | power = work / 6 s |
| Cost of transport | work, mass, gravity, distance | cost of transport = work / (mass × g × max(distance, 0.05)) in J per kg per m |
| Energy leak (audit) | mechanical energy, work | leak = (mechanical energy at end − at start) − work; should be ≤ 0 |
| Mean height | centre of mass | mean centre-of-mass height above ground after settling |
| Upright | mean height, reach | upright = clamp(mean height / (0.5 × reach), 0, 1) |
| Trunk clearance | trunk lowest point | mean height of the trunk's lowest point above ground |
| Trunk spin | trunk angular velocity | mean \|angular velocity\| of the trunk |
| Footfalls, stepping limbs | ground contacts | footfall = a non-trunk part's onset of ground contact; stepping limbs = number of parts with ≥ 2 footfalls |
| Swept reach | part positions | farthest any part end got from the centre of mass during the episode |
| Shore episode | start 4 m out | the body starts 4 m (shore start x = -4.0 m) seaward of the waterline on a tilted ground and its progress up the dry beach is measured |
| Air drop | launch 3 m/s from 10 m | air distance = horizontal distance to first ground contact; airborne fraction = share of steps off the ground; stone distance = ballistic reference |

## Measurement (traits an animal carries into the ecology)

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Land speed, water speed | driven episode, motors-off control | speed = max(0, sustained speed driven − sustained speed with motors off); the control is skipped when the driven speed is below 0.005 m/s |
| Turn, turn sign | steered episode (steer 0.6) | turn = \|yaw rate steered − yaw rate driven\|; turn sign = its sign; only for bodies with speed > 0.01 |
| Heading | driven episode | heading = travel direction of the driven episode, per medium |
| Power, cost of transport | driven episode | taken from the driven episode, per medium |
| Glide distance | air drop with wings held still | glide distance = max(0, air distance held − stone distance) |
| Flap lift | air drop driven vs held | flap lift = mean height driven − mean height held (metres) |
| Air speed | air drop driven | air speed = (distance / 6 s) × airborne fraction |
| Reach (measured) | swept reach | reach = swept reach of the land episode when > 0, else the rest-pose reach |
| Unmeasured mutant | parent's traits | a changed body carries its parent's traits entirely (mass, speeds, costs) until its own episodes run; an unchanged body inherits them outright |
| When measured | mass | at birth and again at half adult mass; land and water episodes each, and an air drop for any body with a sheet |
| Budget and order | pending queue | N bodies per day on every core (default 96 headless); bodies on dry ground or the upper shore first, then most-copied and newest alternately |

## Arena (predation fought out)

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Start | reaches | distance between centres = predator reach + prey reach + 3 × predator reach + 0.05 m; each body turned so its travel direction points along the line; prey yaw perturbed ±0.5 rad, side offset ±0.5 × prey reach |
| Steering | heading, bearing, turn sign | steer = clamp(turn sign × bearing / (π/4), −1, 1); bearing positive when a positive yaw turns the heading toward the target; prey steers away |
| Bite | mouth, gape, prey thickness | bite = mouth point within gape/2 of a prey part whose thickness ≤ gape; claw = clawed part tip within its radius + 1 cm of a prey part |
| Outcome of a bout | hold time, distance | eaten = bite or claw contact held 1 s; fled = no bite ever and farther than the start after 12 s; otherwise survived |
| Pair result | 10 bouts | eaten, fled, survived shares over 10 perturbed bouts, cached per (predator lineage, prey lineage, medium) |
| When fought | attacks | a pair is queued at its first actual attack attempt; 1 pair per day headless (about 24 single-body episodes) |

## Planet and cells

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Planet dials | — | gravity, water coverage, humidity, temperature, pressure, oxygen fraction, CO₂ fraction, insolation, aridity |
| Oxygen relative | pressure, oxygen fraction | oxygen relative = partial pressure of oxygen / Earth's 21.2 kPa |
| Elevation | fractal noise, cell count | elevation = continents × 2600 + mountains × 5200 × clamp(4 × continents, 0, 1) + detail × 800 (metres); noise octaves narrower than 3 cells are dropped; sea level = the water-coverage quantile of elevation |
| Cell area | run parameter | 100 m² by default; every rule below is per m² or per m |
| Moisture | humidity, elevation | land: clamp(0.8 × humidity × (1 − 0.45 × elevation / 4000), 0.02, 1); shore: at least 0.85; water: 1 |
| Shore level | elevation rank among shore cells | 0 at the low-water mark … 1 at the high-water mark |
| Submersion | shore level, day | shore: clamp(0.9 − 0.8 × shore level + 0.17 × sin(2π × day / 14.77), 0.05, 0.95); sea 1; dry land 0 |
| Lake | elevation of neighbours | an inland cell lower than all its neighbours (none of them sea) is water with submersion 1 |
| Temperature | latitude, elevation | planet temperature − 6.5 °C per km of elevation − a latitude term |
| Temperature factor | temperature | temperature factor = exp(−((temperature − 22) / 16)²) |
| Q10 factor | temperature | Q10 = clamp(2^((temperature − 20) / 10), 0.15, 3) |
| CO₂ factor | CO₂ ppm | CO₂ factor = (ppm / (ppm + 250)) / (400 / 650) |
| Light budget | insolation, cell area, submersion, mat area | light available = insolation × area × (0.3 × submersion + 1 − submersion) × exp(−0.5 × mat area / area); each plant, tallest first, takes light available × (1 − exp(−0.5 × leaf area / area)) |
| Nutrient (conserved element) | decay, metabolism, uptake | start 0.003 kg/m²; fraction of tissue 0.02 ; + 0.02 × 0.6 × decayed dead matter + 0.02 × bacterial turnover + 0.02 × rotted carrion + 0.02 × (maintenance + activity) per animal ; − 0.02 × photosynthesis (plants and mats) ; + start × 0.001 − nutrient × 0.001 (weathering and leaching) |
| Decomposition | dead matter, temperature factor, moisture | rate = 0.03 × (0.2 + temperature factor) × (0.3 + 0.7 × moisture); decayed = dead matter × rate; bacteria += 0.4 × decayed |
| Bacteria turnover | bacteria, temperature factor | turnover = bacteria × 0.2 × (0.2 + temperature factor) |
| Carrion rot | carrion, rate | rotted = carrion × min(3 × rate, 0.5); a corpse adds 0.8 × mass + reserve |

## Plants (per individual, per day)

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Plant body | genome, gravity | parts → leaf mass (flat parts), stem mass, root mass; leaf area; specific leaf area = leaf area / leaf mass; height; target mass = leaf + stem + root |
| Structural stress | bending moment, stem section, stem strength 60000 Pa, stem modulus 3.0e9 Pa | stress = moment / (π × effective radius² × 60000 Pa), checked against buckling π² × 3.0e9 Pa × second moment / 4; growth cap = 1 if stress ≤ 1 else √(1 / stress) |
| Grown fraction | biomass, target mass | grown = (biomass / target mass)^(1/3); realised height = body height × max(grown, 0.05) (≤ 1.5 m in water); leaf area now = body leaf area × grown² |
| Water uptake and demand | root mass, moisture, leaf area, aridity, temperature factor, specific leaf area, land tolerance | uptake = root mass × moisture × 0.6 ; demand = leaf area × (0.002 + 0.015 × aridity) × (0.5 + temperature factor) × (0.5 + min(specific leaf area / 20, 2)) × (1 − 0.8 × land tolerance) ; water fraction = submersion + (1 − submersion) × min(1, uptake / demand) |
| Wilting | water fraction, submersion, land tolerance | wilt = max(0, 0.3 − water fraction) / 0.3 × (1 − submersion); energy −= biomass × 0.02 × wilt × (1 − 0.8 × land tolerance) |
| Photosynthesis | light taken, CO₂ factor, temperature factor, water fraction, toughness, heterotrophy, nutrient | photosynthesis = min(light taken × 0.02 × CO₂ factor × temperature factor × water fraction × (1 − 0.35 × toughness) × (1 − 0.6 × heterotrophy), nutrient left / 0.02) kg dry per day; anchored plants attenuated by exp(−0.3 × (depth − height) × (1 − motility)) |
| Absorption (osmotrophy) | heterotrophy, leaf area, dead matter | absorbed = heterotrophy × leaf area × (dead matter / area) × 0.3 × share × (0.3 + temperature factor), pool limited to half per day |
| Capture (phagocytosis) | heterotrophy, leaf area, bacteria, motility | captured = heterotrophy × leaf area × (0.05 × submersion + 0.02 × motility) × 86400 × particles per m³ × 0.05 × share |
| Respiration | leaf mass, biomass, motility, transport, temperature factor, land tolerance | respiration = (leaf mass × 0.004 + (biomass − leaf mass) × (0.0005 + 0.0025 × motility) + transport × 0.002) × (0.3 + temperature factor) × (1 + 0.3 × land tolerance) |
| Energy | above | energy += photosynthesis + 0.6 × (absorbed + captured) − respiration − wilting loss |
| Leaf shedding | specific leaf area | leaf lifespan = clamp(365 × (leaf mass per area / 0.1)^1.5, 20, 2000) days; shed = leaf mass / leaf lifespan → litter |
| Growth | energy, growth cap | cap = target mass × (submersion + (1 − submersion) × growth cap³); grow = min(energy, 0.05 × biomass + 0.01, cap − biomass) |
| Seeds | energy, seed mass, reproduction fraction | seed cost = 1.5 × seed mass; if biomass ≥ cap × reproduction fraction and energy > 1.5 × seed cost: up to 3 seeds, each mutated |
| Seed dispersal | dispersal gene, height, air density | hops = dispersal × 10 m / cell width × (1 + 0.3 × height × √(air density / 1.225)), a random walk kept within one medium class of the parent's (water 2, shore 1, dry 0) |
| Plant death | energy, age | energy < −0.3 × biomass, or age > lifespan = (1 − motility) × lifespan gene + motility × 150 × target mass^0.25 × longevity |
| Plant to animal | child motility | a seed with motility ≥ 0.5 is born an animal (mass = offspring fraction × adult mass, at least the seed mass); an animal's child with motility < 0.5 settles as a plant |
| Density cap | cell area | 0.48 plants per m² in water and on land (performance guard) |

## Animals (per individual, per day)

| Quantity | Depends on | Equation or definition |
|---|---|---|
| Growth fraction | mass, adult mass | growth fraction = clamp(mass / adult mass, 0.01, 1); linear scale = growth fraction^(1/3); mass^(3/4) is used as the metabolic scale |
| Breathing efficiency | submersion, breathing gene | breathing efficiency = submersion × (1 − 0.9 × breathing) + (1 − submersion) × (0.1 + 0.9 × breathing); oxygen debt below 0.2 costs mass × 0.05 × (0.2 − breathing efficiency) / 0.2 per day (a starvation below zero reserve is then counted as suffocation) |
| Speed here | measured speeds, submersion, motility, plating, linear scale | speed here = (water speed × submersion + land speed × (1 − submersion)) × motility × linear scale × (1 − 0.3 × plating) |
| Air speed over the ground | measured air speed, motility, plating, flap lift | air speed over the ground = measured air speed × motility × linear scale × (1 − 0.3 × plating) × clamp(flap lift, 0, 1): only the stroke's own lift carries a body anywhere; travel = max(speed here, air speed over the ground) on land |
| Maintenance | membrane mass, mass, motility, temperature, oxygen, waterproofing | maintenance = (membrane mass × 0.004 + (mass − membrane mass)^0.75 × (0.0005 + 0.0025 × motility)) × (0.3 + temperature factor) × Q10 / clamp(oxygen relative, 0.3, 1) × (1 + 0.3 × land tolerance) kg dry per day |
| Activity (fatigue) | fatigue factor, breathing efficiency | activity = fatigue in water × submersion + fatigue on land × (1 − submersion) |
| Path needed | bite rate, gut space, food found, handling time, reach, day's path | wanted = min(bite rate, gut space at the start of the day); if handling time × wanted < 1: search needed = wanted / (food found × (1 − handling time × wanted)), path needed = clamp((search needed × area − π × reach²) / (2 × reach), 0, day's path); otherwise, or with no food found, the whole day's path |
| Movement cost | cost of transport, mass, gravity, path needed | movement cost = cost of transport × mass × g × path needed / 0.25 / 18 MJ per kg; unmeasured bodies: 10.7 × mass^0.68 × path needed / 18 MJ per kg; standing still costs nothing beyond maintenance |
| Excretion | maintenance, activity cost | cell nutrient += 0.02 × (maintenance + oxygen debt + movement cost) |
| Mat photosynthesis | membrane area, cell mat area, nutrient | intercepted = light × area × (0.3 × submersion + 1 − submersion) × (1 − exp(−0.5 × mat area / area)); photosynthesis = min((1 − 0.6 × heterotrophy) × intercepted × (own leaf area / mat area) × 0.02 × CO₂ factor × temperature factor, nutrient / 0.02) |
| Digestion | gut contents, mass, Q10 | digested = 0.12 × mass^0.75 × Q10 per day; energy += digested plant × plant efficiency + digested meat × meat efficiency; the rest returns as dung |
| Gut capacity | gut fraction, mass | gut capacity = gut fraction × mass; space = gut capacity − contents |
| Assimilation efficiencies | gut type g | plant efficiency = 0.5 − 0.35 × g; meat efficiency = 0.45 + 0.45 × g |
| Bite rate | mouth area, linear scale, Q10, breathing efficiency | bite rate = 12 × mouth area × linear scale² × Q10 × breathing efficiency kg per day; handling time = 1 / bite rate |
| Gape (ecology) | mouth area, linear scale | gape = 2 × √(mouth area × linear scale² / π) |
| Day's path | travel, activity, glide | path = max(travel × 28 800 s × activity, 20 × glide ratio × launch height) |
| Range | reach, path | range = π × reach² + 2 × reach × path m²; search = min(1, range / area); range beyond the cell spills over neighbour rings |
| Browsing height | reach, upright | browsing reach = reach × linear scale × (0.3 + 0.7 × upright); browse = clamp(browsing reach / plant height, 0.1, 1) on land, 1 in water |
| Filtering | mass, submersion | clearance per kg = 0.5 × mass^(−0.5) m³; filtered = min(1, clearance per kg × mass × submersion / (2 × area)); search += 0.2 × filtered; bacteria rate = filtered × bacteria × 0.5 |
| Plant food on offer | plants in range | accessible = (biomass − 0.004) × leaf fraction × 0.5 × (1 − 0.8 × toughness) × browse, times ring coverage beyond the cell |
| Sheet food on offer | other animals in the cell | accessible = sheet leaf mass × 0.5 × (1 − 0.8 × toughness) × clamp(1 − sheet speed / own speed, 0, 1) + whole sheet × filtered when the sheet's reach ≤ own gape (in water) |
| Plant intake | search, food on offer, handling time | plant intake = search × food / (1 + search × handling time × food) (Holling type II, competition is depletion) |
| Carrion intake | reach, path, carrion | carrion search = min(1, (π × reach² + strip × reach × path) / area), strip 6 (40 for a flier); carrion intake = carrion search × carrion / (1 + carrion search × handling time × carrion) |
| Absorption and capture | heterotrophy, surface, dead matter, wetness, bacteria, speed | wetness = submersion + (1 − submersion) × moisture; absorbed = min(heterotrophy × surface × wetness × (dead matter / area) × 0.3 × Q10, 0.2 × dead matter); captured = heterotrophy × surface × (0.05 × submersion + speed) × 86400 × particles × 0.05; energy += 0.6 × (absorbed + captured) |
| Water loss | surface, submersion, moisture, temperature factor, land tolerance | water loss = surface × 1 kg/m² × (1 − submersion) × (1 − moisture) × (0.5 + temperature factor) × (1 − 0.95 × land tolerance) |
| Water gain | food, surface, moisture, submersion | food water = 4 × plant eaten + 2.5 × (carrion + meat eaten); dew = surface × 0.2 × moisture × (1 − submersion); then the deficit is reduced by the share of the day in water: deficit ×= (1 − submersion) |
| Drying out | water deficit, mass, water store | death when deficit > (0.15 + water store) × mass |
| Predation prior (before the arena) | masses, body lengths per second, claws, upright, plating, turn, take-off | attack = mass^0.75 × (0.2 + min(own body lengths/s, 10)) × (0.5 + 3 × claws) × (0.5 + 0.5 × upright) × (0.5 + 0.5 × min(turn / 1.5, 1)) ; defence = prey mass^0.75 × (0.2 + min(prey body lengths/s, 10)) × (0.5 + 2 × prey plating) × (1 + 3 × take-off) × (0.5 + 0.5 × min(prey turn / 1.5, 1)) ; z = log10(own mass / prey mass); hump = exp(−(z − 1)² / 2) ; prior capture = hump / (1 + exp(−(attack − defence) / (0.25 × (attack + defence)))) |
| Capture probability | arena | eaten share of the fought pair if it exists; otherwise (prior × 5 + Σ eaten over the lineage's fought pairs) / (5 + fights), the prior alone before any fight |
| Take-off (prey's escape upward) | glide distance, launch height, flap lift, air speed | take-off = clamp(glide distance / 10 m × launch height / 10 m + flap lift (m) + air speed (m/s), 0, 1); launch height = canopy + drop to the lowest neighbour (≤ 50 m) |
| Encounter | range, reaches | encounter = min(1, range / area) × min(1, prey reach / own reach) |
| Kill value and decision | prey mass, efficiencies, handling, maintenance | edible = 0.8 × prey mass × clamp(0.4 + 0.4 × z, 0, 1); gain = meal × meat efficiency + leftovers × 0.3 × meat efficiency; handle = min(0.2 + edible × handling time, 5) days ; profitability = (encounter × capture probability × gain − 0.1 × maintenance) / handle; attack if profitability > max(plant value, carrion value, half of yesterday's intake); the attempt costs 0.1 × maintenance |
| Fat capacity | mass, fat fraction | fat capacity = 0.05 × mass + fat fraction × mass^1.19; starvation when reserve < 0 |
| Growth | surplus, mass, Q10 | growth potential = 0.12 × mass^0.75 × Q10 × max(1 − (mass / adult mass)^0.25, 0.02); grow = min(surplus, growth potential, adult mass − mass); mass += 0.7 × grow |
| Reproduction | reproduction fraction, offspring fraction, surplus | mature when mass ≥ reproduction fraction × adult mass; child mass = offspring fraction × adult mass; cost = 1.3 × child mass; 80% chance per day when surplus ≥ cost; store capped at fat capacity + 2 × cost |
| Larval dispersal | birth cell | a walk of 15 m / cell width hops kept within one medium class of the birth cell's |
| Lifespan | motility, adult mass, longevity | lifespan = (1 − motility) × lifespan gene + motility × 150 × adult mass^0.25 × longevity days |
| Relocation | hunger, path, cost of transport | hungry = energy < 0 and reserve < 0.5 × fat capacity; then with 60% chance move to the best cell within max(path, air speed over the ground × 3600 s) / cell width hops (≤ 12) it can enter (swims, crawls, flies or glides); cost = cost of transport × mass × g × hops × cell width / 0.25 / 18 MJ per kg (unmeasured: 10.7 × mass^0.68 × distance / 18 MJ per kg) |
| Death | above | suffocated, starved (reserve < 0), dried out, old age (age > lifespan), killed |

## Order of a day

| Step | Acts on | What happens |
|---|---|---|
| 1 Measure | pending bodies | physics episodes for up to the day's budget, then the day's arena pair |
| 2 Tide | shore cells | submersion from shore level and the spring-neap cycle; canopy heights |
| 3 Plants | every cell | light by height order, water, photosynthesis (drawing nutrient), absorption, capture, respiration, shedding, growth, seeds, death |
| 4 Seeds | seed list | dispersal walks; plant children settle, motile children are born animals |
| 5 Animals | every individual | costs, mat photosynthesis, digestion, food on offer, predation decision, eating, water balance, absorption, relocation, growth, reproduction, death checks |
| 6 Deaths and births | lists | corpses to carrion; births placed by larval dispersal; measurement queue rebuilt |
| 7 Decomposition | every cell | dead matter to bacteria and nutrient, bacterial turnover, carrion rot, weathering and leaching |
| 8 Statistics | world | counts, flows, lineages, history |
