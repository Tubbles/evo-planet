//! Bevy viewer: planet dials on the left, live evolution stats, and the
//! current champion replayed in the exact rapier simulation that scored it.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use evo_core::decode::decode;
use evo_core::evolve::{EvolveConfig, Evolver, GenSummary, GRID};
use evo_core::genome::{Genome, NodeKind};
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, EpisodeResult, SimConfig};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

// ----------------------------------------------------------------------------
// Background evolution thread
// ----------------------------------------------------------------------------

enum EvoCommand {
    Restart(EvolveConfig),
    Pause(bool),
}

enum EvoMessage {
    Summary(GenSummary, Vec<Option<f32>>),
    Champion { genome: Genome, result: EpisodeResult, planet: PlanetParams, env: Environment },
}

#[derive(Resource)]
struct EvoLink {
    tx: Sender<EvoCommand>,
    rx: Mutex<Receiver<EvoMessage>>,
}

fn spawn_evolution_thread(initial: EvolveConfig) -> EvoLink {
    let (cmd_tx, cmd_rx) = channel::<EvoCommand>();
    let (msg_tx, msg_rx) = channel::<EvoMessage>();
    std::thread::Builder::new()
        .name("evolution".into())
        .spawn(move || {
            let mut ev = Evolver::new(initial);
            let mut paused = false;
            let mut last_best = -1.0f32;
            loop {
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        EvoCommand::Restart(cfg) => {
                            ev.reset(cfg);
                            last_best = -1.0;
                        }
                        EvoCommand::Pause(p) => paused = p,
                    }
                }
                if paused {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                let summary = ev.step_generation();
                if msg_tx.send(EvoMessage::Summary(summary.clone(), ev.grid_fitness())).is_err() {
                    return;
                }
                if let Some(ch) = ev.champion() {
                    if ch.result.fitness > last_best {
                        last_best = ch.result.fitness;
                        let _ = msg_tx.send(EvoMessage::Champion {
                            genome: ch.genome.clone(),
                            result: ch.result.clone(),
                            planet: ev.cfg.planet.clone(),
                            env: ev.cfg.env,
                        });
                    }
                }
            }
        })
        .expect("spawn evolution thread");
    EvoLink { tx: cmd_tx, rx: Mutex::new(msg_rx) }
}

// ----------------------------------------------------------------------------
// Resources and components
// ----------------------------------------------------------------------------

#[derive(Resource)]
struct PlanetUi {
    params: PlanetParams,
    preset_idx: usize,
    env: Environment,
    paused: bool,
    episode_time: f32,
}

#[derive(Resource, Default)]
struct Stats {
    last: GenSummary,
    grid: Vec<Option<f32>>,
    history: Vec<f32>,
}

#[derive(Resource, Default)]
struct Replay {
    sim: Option<CreatureSim>,
    genome: Option<Genome>,
    result: Option<EpisodeResult>,
    planet: Option<PlanetParams>,
    env: Option<Environment>,
    pending: Option<(Genome, EpisodeResult, PlanetParams, Environment)>,
    entities: Vec<Entity>,
    follow_champion: bool,
    speed: f32,
    needs_spawn: bool,
}

#[derive(Component)]
struct CreaturePart(usize);

#[derive(Component)]
struct Ground;

#[derive(Component)]
struct WaterSurface;

#[derive(Component)]
struct FollowCamera;

#[derive(Resource)]
struct Palette {
    torso: Handle<StandardMaterial>,
    limb: Handle<StandardMaterial>,
    membrane: Handle<StandardMaterial>,
    pad: Handle<StandardMaterial>,
}

// ----------------------------------------------------------------------------

fn main() {
    let planet = PlanetParams::earth();
    let cfg = EvolveConfig { planet: planet.clone(), env: Environment::Land, ..Default::default() };
    let link = spawn_evolution_thread(cfg.clone());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Evo Planet — Sims-style creatures under planet dials".into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin { enable_multipass_for_primary_context: false })
        .insert_resource(ClearColor(Color::srgb(0.55, 0.7, 0.9)))
        .insert_resource(Time::<Fixed>::from_hz(120.0))
        .insert_resource(link)
        .insert_resource(PlanetUi {
            params: planet,
            preset_idx: 0,
            env: Environment::Land,
            paused: false,
            episode_time: cfg.sim.episode_time,
        })
        .insert_resource(Stats::default())
        .insert_resource(Replay { follow_champion: true, speed: 1.0, ..Default::default() })
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (poll_evolution, ui_panel, spawn_creature_entities, sync_transforms, follow_camera, update_environment_look))
        .add_systems(FixedUpdate, step_replay)
        .run();
}

fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(6.0, 4.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        FollowCamera,
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(20.0, 40.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight { color: Color::WHITE, brightness: 300.0, ..default() });

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.5, 0.3), perceptual_roughness: 1.0, ..default() })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Ground,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 0.35, 0.7, 0.45),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.2,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden,
        WaterSurface,
    ));

    commands.insert_resource(Palette {
        torso: materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.55, 0.6), perceptual_roughness: 0.7, ..default() }),
        limb: materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.6, 0.3), perceptual_roughness: 0.8, ..default() }),
        membrane: materials.add(StandardMaterial {
            base_color: Color::srgba(0.95, 0.65, 0.2, 0.75),
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            perceptual_roughness: 0.5,
            ..default()
        }),
        pad: materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.2, 0.2), perceptual_roughness: 0.9, ..default() }),
    });
}

fn poll_evolution(link: Res<EvoLink>, mut stats: ResMut<Stats>, mut replay: ResMut<Replay>) {
    let Ok(rx) = link.rx.lock() else { return };
    while let Ok(msg) = rx.try_recv() {
        match msg {
            EvoMessage::Summary(s, grid) => {
                stats.history.push(s.best_fitness);
                if stats.history.len() > 400 {
                    stats.history.remove(0);
                }
                stats.last = s;
                stats.grid = grid;
            }
            EvoMessage::Champion { genome, result, planet, env } => {
                if replay.follow_champion {
                    replay.pending = Some((genome, result, planet, env));
                }
            }
        }
    }
}

fn start_replay(replay: &mut Replay, genome: Genome, result: EpisodeResult, planet: PlanetParams, env: Environment, episode_time: f32) {
    let cfg = SimConfig { episode_time, ..Default::default() };
    let sim = CreatureSim::new(decode(&genome), &planet, env, cfg);
    replay.sim = Some(sim);
    replay.genome = Some(genome);
    replay.result = Some(result);
    replay.planet = Some(planet);
    replay.env = Some(env);
    replay.needs_spawn = true;
}

fn step_replay(mut replay: ResMut<Replay>, planet_ui: Res<PlanetUi>) {
    let episode_time = planet_ui.episode_time;
    let finished = replay.sim.as_ref().map(|s| s.finished()).unwrap_or(true);
    if finished {
        if let Some((g, r, p, e)) = replay.pending.take() {
            start_replay(&mut replay, g, r, p, e, episode_time);
        } else if let (Some(g), Some(r), Some(p), Some(e)) =
            (replay.genome.clone(), replay.result.clone(), replay.planet.clone(), replay.env)
        {
            // Loop the same champion.
            start_replay(&mut replay, g, r, p, e, episode_time);
        }
        return;
    }
    let speed = replay.speed.max(0.0);
    let steps = if speed > 0.0 { (speed.round() as usize).max(1) } else { 0 };
    if let Some(sim) = replay.sim.as_mut() {
        for _ in 0..steps {
            sim.step();
        }
    }
}

fn spawn_creature_entities(
    mut commands: Commands,
    mut replay: ResMut<Replay>,
    mut meshes: ResMut<Assets<Mesh>>,
    palette: Res<Palette>,
) {
    if !replay.needs_spawn {
        return;
    }
    replay.needs_spawn = false;
    for e in replay.entities.drain(..) {
        commands.entity(e).despawn();
    }
    let Some(sim) = replay.sim.as_ref() else { return };
    let mut ents = Vec::new();
    for (i, part) in sim.pheno.parts.iter().enumerate() {
        let (mesh, mat) = match part.kind {
            NodeKind::Membrane => (
                meshes.add(Cuboid::new(part.r0 * 2.0, part.len, evo_core::decode::MEMBRANE_THICKNESS)),
                palette.membrane.clone(),
            ),
            NodeKind::Pad => (meshes.add(Capsule3d::new(part.radius, part.len)), palette.pad.clone()),
            NodeKind::Capsule => (
                meshes.add(Capsule3d::new(part.radius, part.len)),
                if i == 0 { palette.torso.clone() } else { palette.limb.clone() },
            ),
        };
        let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), CreaturePart(i))).id();
        ents.push(e);
    }
    replay.entities = ents;
}

fn sync_transforms(replay: Res<Replay>, mut q: Query<(&CreaturePart, &mut Transform)>) {
    let Some(sim) = replay.sim.as_ref() else { return };
    for (part, mut tf) in q.iter_mut() {
        if part.0 >= sim.handles.len() {
            continue;
        }
        let iso = sim.part_isometry(part.0);
        let t = iso.translation.vector;
        let q = iso.rotation;
        tf.translation = Vec3::new(t.x, t.y, t.z);
        tf.rotation = Quat::from_xyzw(q.i, q.j, q.k, q.w);
    }
}

fn follow_camera(time: Res<Time>, replay: Res<Replay>, mut q: Query<&mut Transform, With<FollowCamera>>) {
    let Some(sim) = replay.sim.as_ref() else { return };
    let c = sim.com();
    let com = Vec3::new(c.x, c.y, c.z);
    let size = sim.pheno.parts.iter().map(|p| p.len).sum::<f32>().clamp(1.5, 8.0);
    let offset = Vec3::new(1.2, 0.7, 1.6) * (3.0 + size);
    let target = com + offset;
    let k = 1.0 - (-3.0 * time.delta_secs()).exp();
    for mut tf in q.iter_mut() {
        tf.translation = tf.translation.lerp(target, k);
        tf.look_at(com, Vec3::Y);
    }
}

fn update_environment_look(
    planet_ui: Res<PlanetUi>,
    replay: Res<Replay>,
    mut clear: ResMut<ClearColor>,
    mut ground: Query<&mut Transform, (With<Ground>, Without<WaterSurface>)>,
    mut water: Query<(&mut Transform, &mut Visibility), With<WaterSurface>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ground_mat: Query<&MeshMaterial3d<StandardMaterial>, With<Ground>>,
) {
    let d = planet_ui.params.derive();
    // Sky: thicker air is paler, CO2 tints orange, thin air goes dark.
    let thickness = (d.air_density / 1.225).clamp(0.0, 3.0);
    let haze = (thickness / 3.0).sqrt();
    let co2 = (planet_ui.params.co2_frac * 20.0).clamp(0.0, 1.0);
    let base = Vec3::new(0.55, 0.7, 0.9);
    let dark = Vec3::new(0.02, 0.02, 0.06);
    let orange = Vec3::new(0.85, 0.6, 0.35);
    let sky = dark.lerp(base, haze.clamp(0.0, 1.0)).lerp(orange, co2 * 0.6);
    let env = replay.env.unwrap_or(planet_ui.env);
    let sky = if env == Environment::Water { Vec3::new(0.05, 0.2, 0.4) } else { sky };
    clear.0 = Color::srgb(sky.x, sky.y, sky.z);

    // Ground greener with productivity, sandier with aridity.
    if let Ok(h) = ground_mat.single() {
        if let Some(m) = materials.get_mut(&h.0) {
            let green = Vec3::new(0.3, 0.5, 0.28);
            let sand = Vec3::new(0.7, 0.6, 0.4);
            let g = (d.npp_rel.clamp(0.0, 1.5) / 1.5) * (1.0 - d.aridity.clamp(0.0, 1.0) * 0.7);
            let c = sand.lerp(green, g.clamp(0.0, 1.0));
            m.base_color = Color::srgb(c.x, c.y, c.z);
        }
    }

    let ground_y = replay.sim.as_ref().map(|s| s.ground_y).unwrap_or(0.0);
    for mut tf in ground.iter_mut() {
        tf.translation.y = ground_y;
    }
    for (mut tf, mut vis) in water.iter_mut() {
        if env == Environment::Water {
            tf.translation.y = 0.0;
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

// ----------------------------------------------------------------------------
// UI
// ----------------------------------------------------------------------------

fn fitness_color(f: f32, max: f32) -> egui::Color32 {
    let t = if max > 0.0 { (f / max).clamp(0.0, 1.0) } else { 0.0 };
    let r = (40.0 + 200.0 * t) as u8;
    let g = (60.0 + 120.0 * t) as u8;
    let b = (120.0 - 100.0 * t) as u8;
    egui::Color32::from_rgb(r, g, b)
}

fn ui_panel(
    mut contexts: EguiContexts,
    mut planet_ui: ResMut<PlanetUi>,
    stats: Res<Stats>,
    mut replay: ResMut<Replay>,
    link: Res<EvoLink>,
) {
    let ctx = contexts.ctx_mut();
    let presets = PlanetParams::presets();
    let mut restart = false;

    egui::SidePanel::left("planet_panel").default_width(380.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Planet");
            let mut idx = planet_ui.preset_idx;
            egui::ComboBox::from_label("Preset")
                .selected_text(presets.get(idx).map(|p| p.name.as_str()).unwrap_or("Custom"))
                .show_ui(ui, |ui| {
                    for (i, p) in presets.iter().enumerate() {
                        ui.selectable_value(&mut idx, i, &p.name);
                    }
                });
            if idx != planet_ui.preset_idx {
                planet_ui.preset_idx = idx;
                planet_ui.params = presets[idx].clone();
                restart = true;
            }

            let p = &mut planet_ui.params;
            let mut changed = false;
            changed |= ui.add(egui::Slider::new(&mut p.gravity, 1.0..=30.0).text("Gravity (m/s²)")).changed();
            changed |= ui.add(egui::Slider::new(&mut p.pressure_atm, 0.01..=6.0).logarithmic(true).text("Pressure (atm)")).changed();
            changed |= ui.add(egui::Slider::new(&mut p.o2_frac, 0.0..=0.6).text("O₂ fraction")).changed();
            changed |= ui.add(egui::Slider::new(&mut p.co2_frac, 0.00001..=0.2).logarithmic(true).text("CO₂ fraction")).changed();
            changed |= ui.add(egui::Slider::new(&mut p.temp_c, -80.0..=90.0).text("Mean temperature (°C)")).changed();
            changed |= ui.add(egui::Slider::new(&mut p.water_coverage, 0.0..=1.0).text("Water coverage")).changed();
            changed |= ui.add(egui::Slider::new(&mut p.humidity, 0.0..=1.0).text("Humidity")).changed();
            changed |= ui.add(egui::Slider::new(&mut p.insolation, 0.1..=3.0).text("Insolation (× Earth)")).changed();
            if changed {
                p.name = "Custom".into();
                planet_ui.preset_idx = usize::MAX;
            }

            let d = planet_ui.params.derive();
            ui.separator();
            ui.heading("Derived physics");
            egui::Grid::new("derived_phys").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Air density");
                ui.label(format!("{:.3} kg/m³  (Earth 1.225)", d.air_density));
                ui.end_row();
                ui.label("O₂ partial pressure");
                ui.label(format!("{:.1} kPa  (× {:.2} Earth)", d.po2_kpa, d.o2_rel));
                ui.end_row();
                ui.label("Flight index");
                ui.label(format!("{:.2}  (air density / gravity, Earth = 1)", d.flight_index));
                ui.end_row();
                ui.label("Stall speed @ 50 N/m²");
                ui.label(format!("{:.1} m/s", d.stall_speed_ref));
                ui.end_row();
                ui.label("Wing loading at 8 m/s");
                ui.label(format!("{:.0} N/m²", d.wing_loading_at_8ms));
                ui.end_row();
            });

            ui.separator();
            ui.heading("Derived ecology");
            egui::Grid::new("derived_eco").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Plant productivity");
                ui.label(format!("× {:.2} Earth", d.npp_rel));
                ui.end_row();
                ui.label("Plant : animal biomass");
                ui.label(format!("{:.0} : 1", d.plant_animal_ratio));
                ui.end_row();
                ui.label("Animal biomass");
                ui.label(format!("× {:.2} Earth", d.animal_biomass_rel));
                ui.end_row();
                ui.label("Aquatic niche share");
                ui.label(format!("{:.0}%", d.aquatic_share * 100.0));
                ui.end_row();
                ui.label("Expected lung fraction");
                ui.label(format!("{:.0}% of torso (gills {:.0}%)", d.lung_frac * 100.0, d.gill_frac * 100.0));
                ui.end_row();
                ui.label("Water storage needed");
                ui.label(format!("{:.0}% of mass (aridity {:.2})", d.water_storage_frac * 100.0, d.aridity));
                ui.end_row();
            });

            ui.separator();
            ui.heading("Evolution");
            ui.horizontal(|ui| {
                ui.label("Environment:");
                for env in Environment::ALL {
                    if ui.radio_value(&mut planet_ui.env, env, env.name()).changed() {
                        restart = true;
                    }
                }
            });
            ui.add(egui::Slider::new(&mut planet_ui.episode_time, 3.0..=15.0).text("Episode length (s)"));
            ui.horizontal(|ui| {
                if ui.button("Apply planet & restart evolution").clicked() {
                    restart = true;
                }
                let mut paused = planet_ui.paused;
                if ui.checkbox(&mut paused, "Pause").changed() {
                    planet_ui.paused = paused;
                    let _ = link.tx.send(EvoCommand::Pause(paused));
                }
            });
            let s = &stats.last;
            egui::Grid::new("evo_stats").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Generation");
                ui.label(format!("{}  ({} evaluations)", s.generation, s.evaluations));
                ui.end_row();
                ui.label("Best fitness");
                ui.label(format!("{:.3}   (batch best {:.3})", s.best_fitness, s.batch_best));
                ui.end_row();
                ui.label("Archive cells");
                ui.label(format!("{} / {}   nursery {}", s.occupied_cells, GRID * GRID, s.nursery_size));
                ui.end_row();
                ui.label("Throughput");
                ui.label(format!("{:.1} evaluations/s   exploded {}", s.evals_per_second, s.exploded_in_batch));
                ui.end_row();
            });

            ui.label("Archive: rows = part count (1–4 … 21–24), columns = membrane area per kg (none → high)");
            let max = stats.grid.iter().flatten().cloned().fold(0.0f32, f32::max);
            let cell = 22.0;
            let (resp, painter) = ui.allocate_painter(egui::vec2(cell * GRID as f32 + 2.0, cell * GRID as f32 + 2.0), egui::Sense::hover());
            let origin = resp.rect.min;
            for r in 0..GRID {
                for c in 0..GRID {
                    let rect = egui::Rect::from_min_size(
                        origin + egui::vec2(c as f32 * cell + 1.0, r as f32 * cell + 1.0),
                        egui::vec2(cell - 2.0, cell - 2.0),
                    );
                    let col = match stats.grid.get(r * GRID + c).cloned().flatten() {
                        Some(f) => fitness_color(f, max),
                        None => egui::Color32::from_gray(35),
                    };
                    painter.rect_filled(rect, 2.0, col);
                }
            }

            ui.separator();
            ui.heading("Champion replay");
            ui.checkbox(&mut replay.follow_champion, "Switch to new champion when it appears");
            ui.add(egui::Slider::new(&mut replay.speed, 0.0..=4.0).text("Replay speed (× real time)"));
            if let (Some(sim), Some(res)) = (replay.sim.as_ref(), replay.result.as_ref()) {
                let ph = &sim.pheno;
                egui::Grid::new("champ").num_columns(2).striped(true).show(ui, |ui| {
                    ui.label("Parts / membranes");
                    ui.label(format!("{} / {}", ph.part_count(), ph.membrane_count()));
                    ui.end_row();
                    ui.label("Mass");
                    ui.label(format!("{:.1} kg", ph.total_mass));
                    ui.end_row();
                    ui.label("Membrane area");
                    ui.label(format!("{:.2} m²  ({:.4} m²/kg)", ph.membrane_area, ph.area_per_mass()));
                    ui.end_row();
                    ui.label("Lung fraction (gene)");
                    ui.label(format!("{:.1}%  → O₂ fatigue factor {:.2}", ph.lung_frac * 100.0, sim.fatigue));
                    ui.end_row();
                    ui.label("Water store (gene)");
                    ui.label(format!("{:.1}%  → hydration {:.2}", ph.water_store * 100.0, sim.hydration));
                    ui.end_row();
                    ui.label("Base frequency");
                    ui.label(format!("{:.2} Hz", ph.base_freq));
                    ui.end_row();
                    ui.label("Scored");
                    ui.label(format!("fitness {:.3}, {:.2} m, airborne {:.0}%", res.fitness, res.distance, res.airborne_frac * 100.0));
                    ui.end_row();
                    ui.label("Replay");
                    ui.label(format!("t = {:.1} s, travelled {:.2} m{}", sim.time, sim.horizontal_distance(), if sim.exploded { "  EXPLODED" } else { "" }));
                    ui.end_row();
                });
                if ui.button("Copy champion genome JSON to clipboard").clicked() {
                    if let Some(g) = replay.genome.as_ref() {
                        ui.ctx().copy_text(g.to_json());
                    }
                }
            } else {
                ui.label("Waiting for the first generation…");
            }

            ui.separator();
            ui.small("Land: distance walked. Water: distance swum. Air: dropped from 10 m, distance at landing + 0.25 × seconds aloft.");
            ui.small("Strength scales with limb cross-section; weight with volume × gravity. Lungs lighten the torso but feed the O₂ budget; stored water adds mass but keeps land fitness on arid worlds. Membranes get flat-plate lift and drag from the local fluid density.");
        });
    });

    if restart {
        let cfg = EvolveConfig {
            planet: planet_ui.params.clone(),
            env: planet_ui.env,
            sim: SimConfig { episode_time: planet_ui.episode_time, ..Default::default() },
            ..Default::default()
        };
        let _ = link.tx.send(EvoCommand::Restart(cfg));
        replay.pending = None;
    }
}
