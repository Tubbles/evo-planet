// Simulation worker: one EcoSim (the planet ecology, no score) stepped as
// fast as it can. Messages carry a runId so stale results from a previous
// run are ignored by the page.
import init, { EcoSim } from './pkg/evo_wasm.js';

let eco = null;
let paused = false;
let running = false;
let runId = 0;
let lastSnap = 0;
let dayStart = 0;
let daysSince = 0;

await init();
postMessage({ type: 'ready' });

let linFilter = { kind: 'both', habitat: 'all', sort: 'count', max: 10 };

onmessage = (e) => {
  const m = e.data;
  if (m.type === 'eco_start') {
    stopAll();
    runId = m.runId;
    eco = new EcoSim(JSON.stringify(m.planet), m.patches, BigInt(m.seed));
    // One arena pair (ten two-body bouts) costs a few hundred milliseconds;
    // in the browser fight one every four days so the world keeps moving.
    eco.set_arena_rate(0.25);
    postMessage({ type: 'eco_terrain', runId, terrain: JSON.parse(eco.terrain_json(m.detail || 5)) });
    paused = false;
    lastSnap = 0; dayStart = performance.now(); daysSince = 0;
    kick();
  } else if (m.type === 'eco_planet') {
    if (eco) {
      eco.set_planet(JSON.stringify(m.planet));
      postMessage({ type: 'eco_terrain', runId, terrain: JSON.parse(eco.terrain_json(m.detail || 5)) });
    }
  } else if (m.type === 'eco_filter') {
    linFilter = Object.assign(linFilter, m.filter);
    if (eco) postMessage({ type: 'eco_lineages', runId, lineages: JSON.parse(eco.lineages_json(linFilter.kind, linFilter.habitat, linFilter.sort, linFilter.max)) });
  } else if (m.type === 'eco_immigrate') {
    if (eco) eco.immigrate(m.count || 10);
  } else if (m.type === 'eco_rep') {
    if (eco) {
      const r = eco.representative_json(m.lineage, m.habitat || 'all');
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
  if (eco) { eco.free(); eco = null; }
}

function kick() { if (!running) { running = true; setTimeout(loop, 0); } }

function loop() {
  if (!eco) { running = false; return; }
  if (paused) { setTimeout(loop, 100); return; }
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
    const snap = JSON.parse(eco.snapshot_json());
    snap.lineages = JSON.parse(eco.lineages_json(linFilter.kind, linFilter.habitat, linFilter.sort, linFilter.max));
    postMessage({ type: 'eco_snap', runId, snap, history: JSON.parse(eco.history_json()), dps });
  }
  setTimeout(loop, 0);
}
