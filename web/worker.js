// Simulation worker. Two modes:
//  * archive: one EvoWorld (explicit fitness, MAP-Elites), one creature per pump
//  * ecology: one EcoSim (implicit fitness on a spherical world), as fast as it can
// Both run on the same wasm module. Messages carry a runId so stale results
// from a previous run are ignored by the page.
import init, { EvoWorld, EcoSim } from './pkg/evo_wasm.js';

let world = null;      // archive mode
let eco = null;        // ecology mode
let paused = false;
let running = false;
let genStart = 0;
let runId = 0;
let mode = 'archive';
let lastSnap = 0;
let dayStart = 0;
let daysSince = 0;

await init();
postMessage({ type: 'ready' });

onmessage = (e) => {
  const m = e.data;
  if (m.type === 'start') {
    stopAll();
    mode = 'archive';
    runId = m.runId;
    world = new EvoWorld(JSON.stringify(m.planet), m.env, m.episode, m.batch, BigInt(m.seed));
    genStart = performance.now();
    paused = false;
    kick();
  } else if (m.type === 'eco_start') {
    stopAll();
    mode = 'ecology';
    runId = m.runId;
    eco = new EcoSim(JSON.stringify(m.planet), m.patches, BigInt(m.seed));
    postMessage({ type: 'eco_terrain', runId, terrain: JSON.parse(eco.terrain_json(m.detail || 5)) });
    paused = false;
    lastSnap = 0; dayStart = performance.now(); daysSince = 0;
    kick();
  } else if (m.type === 'eco_planet') {
    if (eco) {
      eco.set_planet(JSON.stringify(m.planet));
      postMessage({ type: 'eco_terrain', runId, terrain: JSON.parse(eco.terrain_json(m.detail || 5)) });
    }
  } else if (m.type === 'eco_immigrate') {
    if (eco) eco.immigrate(m.count || 10);
  } else if (m.type === 'eco_rep') {
    if (eco) {
      const r = eco.representative_json(m.lineage);
      postMessage({ type: 'eco_rep', runId, lineage: m.lineage, rep: r ? JSON.parse(r) : null });
    }
  } else if (m.type === 'eco_plant_rep') {
    if (eco) {
      const r = eco.plant_representative_json(m.lineage);
      postMessage({ type: 'eco_plant_rep', runId, lineage: m.lineage, rep: r ? JSON.parse(r) : null });
    }
  } else if (m.type === 'pause') {
    paused = !!m.paused;
  }
};

function stopAll() {
  if (world) { world.free(); world = null; }
  if (eco) { eco.free(); eco = null; }
}

function kick() { if (!running) { running = true; setTimeout(loop, 0); } }

function loop() {
  if (!world && !eco) { running = false; return; }
  if (paused) { setTimeout(loop, 100); return; }
  if (mode === 'archive' && world) {
    const done = world.pump(1);
    if (done) {
      const elapsed = (performance.now() - genStart) / 1000;
      genStart = performance.now();
      const s = world.finish(elapsed);
      if (s) postMessage({ type: 'gen', runId, summary: JSON.parse(s), grid: JSON.parse(world.grid_json()) });
      const c = world.take_new_champion_json();
      if (c) postMessage({ type: 'champion', runId, champion: JSON.parse(c) });
    }
  } else if (mode === 'ecology' && eco) {
    // A few days per slice, with a physics measurement budget that grows
    // when the queue backs up (every new body is measured on land and in water).
    const pending = eco.pending_evals();
    const evals = pending === 0 ? 0 : Math.min(8, 1 + Math.floor(pending / 6));
    eco.tick(3, evals);
    daysSince += 3;
    const now = performance.now();
    if (now - lastSnap > 400) {
      const dps = daysSince / ((now - dayStart) / 1000);
      dayStart = now; daysSince = 0; lastSnap = now;
      postMessage({ type: 'eco_snap', runId, snap: JSON.parse(eco.snapshot_json()), history: JSON.parse(eco.history_json()), dps });
    }
  }
  setTimeout(loop, 0);
}
