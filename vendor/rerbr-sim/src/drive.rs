//! App-owned, original boot and controlled fixed steps. No presentation writes to physics.
use crate::{boot, diagnostics, inputs};
pub use rbr_gameplay::control::CarControl as Control;

/// Independent read-only COL geometry for contact diagnostics, not a replacement collision solver.
pub fn collision_triangles(paths: &inputs::Paths) -> Result<Vec<[[f32; 3]; 3]>, String> {
    collision_triangles_for(paths, &rerbr_content::StageDescriptor::default())
}
pub fn collision_triangles_for(
    paths: &inputs::Paths,
    stage: &rerbr_content::StageDescriptor,
) -> Result<Vec<[[f32; 3]; 3]>, String> {
    stage.validate_current_backend()?;
    let bytes = std::fs::read(paths.maps.join(stage.file("col")?)).map_err(|e| e.to_string())?;
    let col = rbr_formats::col::parse(&bytes).map_err(|e| e.to_string())?;
    Ok(col
        .cells
        .iter()
        .flat_map(|c| c.tree.triangles.iter().map(|t| t.vertices.map(|i| c.vertices[i as usize])))
        .collect())
}
use rbr_gameplay::{
    stage::{Stage, State, NGP_LOAD_SUBSTEPS},
    weather::{self, Draws, Preset, Season},
};
use rbr_hybrid::pure::{self, Playback};
use rbr_physics::rigid_body::layout as body;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StageTiming {
    pub started: bool,
    pub driveline_distance: f32,
    pub start_distance: f32,
    pub finish_distance: f32,
    pub splits_passed: usize,
    pub splits_total: usize,
    pub split_times: Vec<Option<f64>>,
    pub finished: bool,
    pub finish_crossed_forward: bool,
}

pub struct Session {
    physics_audit: crate::physics_audit::PhysicsAudit,
    playback: Playback,
    control_at: u32,
    dt: f32,
    steps: u32,
    clutch_help: bool,
    finish_direction: Option<[f32; 3]>,
    finish_crossed_forward: bool,
}
#[derive(Clone, Copy)]
struct BootAids {
    automatic: bool,
    clutch_help: bool,
    lesson: Option<rbr_school::lesson::Lesson>,
}
impl Session {
    /// Call on the dedicated STACK_BYTES worker; callers must catch pure-host panics there.
    pub fn boot(paths: &inputs::Paths) -> Result<Self, String> {
        Self::boot_with_transmission(paths, false)
    }
    /// Original driver aid applied through the race setup before loading.
    pub fn boot_with_transmission(paths: &inputs::Paths, automatic: bool) -> Result<Self, String> {
        Self::boot_selected(paths, &rerbr_content::Selection::default(), automatic)
    }
    pub fn boot_selected(
        paths: &inputs::Paths,
        selection: &rerbr_content::Selection,
        automatic: bool,
    ) -> Result<Self, String> {
        selection.validate_current_backend()?;
        let checked = inputs::check_selection(paths, selection)?;
        let data = paths.data();
        Self::boot_core(
            paths,
            data,
            checked.physics,
            selection.vehicle.original_slot,
            None,
            selection,
            BootAids { automatic, clutch_help: true, lesson: None },
        )
    }

    /// Experimental NGP car pack boot (for non-retail pack vehicles such as Ford Focus RS WRC 2006).
    /// Requires verified NGP 7.4 runtime and plaintext `.lsp` pack; never silently falls back to retail slots.
    pub fn boot_experimental_pack(
        paths: &inputs::Paths,
        selection: &rerbr_content::Selection,
        pack_path: &std::path::Path,
        automatic: bool,
    ) -> Result<Self, String> {
        Self::boot_experimental_pack_with_aids(paths, selection, pack_path, automatic, true)
    }

    pub fn boot_experimental_pack_with_aids(
        paths: &inputs::Paths,
        selection: &rerbr_content::Selection,
        pack_path: &std::path::Path,
        automatic: bool,
        clutch_help: bool,
    ) -> Result<Self, String> {
        selection.validate_for_mode(true)?;
        if cfg!(panic = "abort") {
            return Err("physics-drive requires panic=unwind".into());
        }
        let base_data = paths.data();
        let dll =
            base_data.resolve("Plugins/NGP.dll").ok_or_else(|| "missing private RBR/Plugins/NGP.dll".to_string())?;
        crate::ngp_candidate::validate_runtime(&std::fs::read(&dll).map_err(|e| e.to_string())?)?;
        let physics_rbz =
            base_data.resolve("physics.rbz").ok_or_else(|| "missing private RBR/physics.rbz".to_string())?;
        for ext in rerbr_content::PHYSICS_STAGE_EXTENSIONS {
            let file = selection.stage.file(ext)?;
            if rbr_formats::cars::resolve(&paths.maps, &file).is_none() {
                return Err(format!("missing private {}/{file}", selection.stage.documents_maps));
            }
        }
        let pack = rbr_formats::car_pack::Pack::open(pack_path)?;
        let _ = pack.read("common.lsp")?;
        let folder = selection.vehicle.model.split(['\\', '/']).nth(1).unwrap_or("FORD_FOCUS_2006");
        let stem = selection
            .vehicle
            .model
            .rsplit(['\\', '/'])
            .next()
            .and_then(|s| s.strip_suffix(".sgc"))
            .unwrap_or("focus06");
        let entry = rbr_formats::car_pack::model_entry(folder, stem);
        let data = base_data.with_car(paths.install.clone(), entry);
        let car_pack = Some(boot::CarPack { pack, tyre_brand: "pirelli" });
        Self::boot_core(
            paths,
            data,
            physics_rbz,
            rbr_formats::car_pack::SLOT,
            car_pack,
            selection,
            BootAids { automatic, clutch_help, lesson: None },
        )
    }

    pub fn boot_with_aids(
        paths: &inputs::Paths,
        selection: &rerbr_content::Selection,
        automatic: bool,
        clutch_help: bool,
    ) -> Result<Self, String> {
        selection.validate_current_backend()?;
        let checked = inputs::check_selection(paths, selection)?;
        let data = paths.data();
        Self::boot_core(
            paths,
            data,
            checked.physics,
            selection.vehicle.original_slot,
            None,
            selection,
            BootAids { automatic, clutch_help, lesson: None },
        )
    }

    pub fn boot_lesson(
        paths: &inputs::Paths,
        selection: &rerbr_content::Selection,
        lesson_index: u32,
        automatic: bool,
        clutch_help: bool,
    ) -> Result<Self, String> {
        let lesson = rbr_school::lesson::Lesson::new(lesson_index)
            .ok_or_else(|| format!("unknown rally school lesson index {lesson_index}"))?;
        if selection.stage.original_map != lesson.map() {
            return Err(format!(
                "lesson {lesson_index} expects map {}, got {}",
                lesson.map(),
                selection.stage.original_map
            ));
        }
        selection.validate_current_backend()?;
        let checked = inputs::check_selection(paths, selection)?;
        let data = paths.data();
        let mut session = Self::boot_core(
            paths,
            data,
            checked.physics,
            rbr_school::lesson::CAR,
            None,
            selection,
            BootAids { automatic, clutch_help, lesson: Some(lesson) },
        )?;
        // School stages skip countdown; burn off spawn override_timer and crank starter if lesson starts off.
        let car = session.playback.pure.mem.u32(rbr_hybrid::pure::CAR_PTR);
        for _ in 0..2000 {
            if session.playback.pure.mem.f32(car + rbr_physics::car_body::layout::T_OVERRIDE) <= 0.0 {
                break;
            }
            session.step(Control::default())?;
        }
        if session.sample().rpm < 50.0 {
            for k in 0..1080 {
                session.step(Control { aux: 1, ..Default::default() })?;
                if k >= 900 && session.sample().rpm > 1200.0 {
                    break;
                }
            }
        }
        Ok(session)
    }

    fn boot_core(
        paths: &inputs::Paths,
        data: rbr_formats::data::Data,
        physics_archive: std::path::PathBuf,
        slot: u32,
        car_pack: Option<boot::CarPack>,
        selection: &rerbr_content::Selection,
        aids: BootAids,
    ) -> Result<Self, String> {
        if cfg!(panic = "abort") {
            return Err("physics-drive requires panic=unwind".into());
        }
        let season = Season::load(&data)?;
        let weather = if let Some(row) = season.rows.iter().position(|r| {
            r.map == selection.stage.original_map
                && r.letter == selection.stage.weather_letter
                && r.wetness == selection.stage.wetness
        }) {
            weather::quick_race(
                &season,
                selection.stage.original_map,
                Preset::Random,
                Draws { row: Some(row), wear: selection.stage.wear },
            )?
        } else if aids.lesson.is_some() {
            rbr_gameplay::weather::Weather {
                map: selection.stage.original_map,
                row: None,
                letter: selection.stage.weather_letter,
                cloud: 0,
                cloud_letter: 0,
                preset: 0,
                wetness: selection.stage.wetness,
                wear: selection.stage.wear,
            }
        } else {
            return Err(format!(
                "missing weather row for stage {} (letter {}, wetness {})",
                selection.stage.id, selection.stage.weather_letter, selection.stage.wetness
            ));
        };
        let audit_archive = physics_archive.clone();
        let mut race = boot::Race::quick_race(
            selection.stage.original_map,
            slot,
            boot::default_tyre(boot::surface(selection.stage.original_map)),
            weather,
            physics_archive,
        );
        if let Some(lesson) = aids.lesson {
            race = race.in_school(lesson);
        }
        let physics_audit = match &car_pack {
            Some(cp) => crate::physics_audit::inspect_pack(&audit_archive, &cp.pack, race.surface)?,
            None => {
                // slot folder name resolved below after build
                crate::physics_audit::PhysicsAudit::default()
            }
        };
        race.car_pack = car_pack;
        race.options.aids.auto_gears = aids.automatic;
        race.options.aids.clutch_help = aids.clutch_help;
        let name = paths.maps.join(&selection.stage.stem).to_string_lossy().into_owned();
        let finish_direction = finish_direction_for(&data, &name);
        let image = boot::build(&data, &name, State::Load.value(), &race)?;
        let physics_audit = if race.car_pack.is_some() {
            physics_audit
        } else {
            let folder = diagnostics::physics_folder(&image.mem, slot)?;
            crate::physics_audit::inspect(&audit_archive, &folder, race.surface)?
        };
        eprintln!("physics-drive AUDIT: {}", physics_audit.summary());
        let mut playback = Playback::from_image(image.mem.clone(), image.regs, image.slot, image.control_at, false);
        playback.pure.ngp_base = image.ngp.base;
        playback.pure.direct = true;
        playback.pure.untrack_allocator();
        boot::update_a720(&playback.pure, &image);
        boot::ngp_init(&mut playback.pure.mem.clone(), &image);
        let dt = rbr_hybrid::timekeeper::layout::max_time_step(&playback.pure.mem);
        if dt.to_bits() != 0x3ab6_0b61 {
            return Err("unexpected physics timestep".into());
        }
        let mut stage = Stage::quick_race(selection.stage.original_map, slot).load_ready_with(NGP_LOAD_SUBSTEPS);
        while stage.state() == State::Load {
            pure::set_app_state(&mut playback.pure.mem.clone(), stage.state().value());
            playback.step(None, 40);
            stage = stage.load_tick(40);
        }
        stage = if aids.lesson.is_some() { stage.school_start() } else { stage.go().countdown_tick(dt) };
        if stage.state() != State::Driving {
            return Err("stage did not reach Driving".into());
        }
        pure::set_app_state(&mut playback.pure.mem.clone(), stage.state().value());
        let ratios = diagnostics::gear_ratios(&playback)?;
        if ratios.len() != selection.vehicle.forward_gears as usize + 2 {
            return Err("selected vehicle descriptor disagrees with original gearbox gear count".into());
        }
        if aids.automatic {
            let audit = diagnostics::gearbox_audit(&playback);
            if audit.0[2..ratios.len()].iter().all(|rpm| *rpm == 0.0) {
                return Err("automatic transmission unavailable: original VCU upshift thresholds are empty; manual transmission remains supported".into());
            }
        }
        eprintln!(
            "physics-drive BOOT READY: selected {} / {}; dt={dt:.10}; built={:?}; panic_unwind={}; host={}/{}",
            selection.stage.id,
            selection.vehicle.id,
            image.built,
            cfg!(panic = "unwind"),
            std::env::consts::ARCH,
            std::env::consts::OS
        );
        Ok(Self {
            physics_audit,
            playback,
            control_at: image.control_at,
            dt,
            steps: 0,
            clutch_help: aids.clutch_help,
            finish_direction,
            finish_crossed_forward: false,
        })
    }
    pub fn physics_audit(&self) -> &crate::physics_audit::PhysicsAudit {
        &self.physics_audit
    }
    pub fn dt(&self) -> f32 {
        self.dt
    }
    pub fn gear_ratios(&self) -> Result<Vec<f32>, String> {
        diagnostics::gear_ratios(&self.playback)
    }
    pub fn step(&mut self, input: Control) -> Result<(), String> {
        if [input.throttle, input.brake, input.clutch, input.handbrake, input.steering].iter().any(|x| !x.is_finite()) {
            return Err("nonfinite input".into());
        }
        if self.clutch_help {
            let car = self.playback.pure.mem.u32(pure::CAR_PTR);
            if car != 0 {
                // Allow manual clutch pedal override (e.g. clutch kick / start-line rev) even when ClutchHelp is active.
                let scratch = rbr_physics::car_body::layout::scratch_at(car);
                self.playback.pure.mem.set_u32(scratch + 0x4c, u32::from(input.clutch <= 0.05));
            }
        }
        let mut payload = [0; 32];
        for (k, word) in input.words().iter().enumerate() {
            payload[k * 4..k * 4 + 4].copy_from_slice(&word.unwrap_or(0).to_le_bytes());
        }
        let before = pure::time_keeper(&self.playback.pure.mem);
        self.playback.step(Some(&payload), 1);
        self.steps = self.steps.checked_add(1).ok_or("physics step counter exhausted")?;
        if before.card.finish_time.is_none() {
            let after = pure::time_keeper(&self.playback.pure.mem);
            if after.card.finish_time.is_some()
                && after.card.finish > 0.0
                && after.card.passed >= after.card.splits().len()
            {
                let mem = &self.playback.pure.mem;
                let car = mem.u32(pure::CAR_PTR);
                let vel: [f32; 3] = mem.f32s(car + body::LINEAR_VELOCITY);
                let forward = match self.finish_direction {
                    Some(dir) => vel[0] * dir[0] + vel[1] * dir[1] + vel[2] * dir[2] > 0.0,
                    None => after.card.last >= before.card.last,
                };
                if forward {
                    self.finish_crossed_forward = true;
                }
            }
        }
        Ok(())
    }
    pub fn timing(&self) -> StageTiming {
        let tk = pure::time_keeper(&self.playback.pure.mem);
        let splits_total = tk.card.splits().len();
        let finished = tk.card.finish_time.is_some() && tk.card.finish > 0.0 && tk.card.passed >= splits_total;
        StageTiming {
            started: tk.card.started,
            driveline_distance: tk.card.last,
            start_distance: tk.card.start,
            finish_distance: tk.card.finish,
            splits_passed: tk.card.passed,
            splits_total,
            split_times: tk.card.splits().iter().map(|s| s.time.map(|t| t - tk.card.origin)).collect(),
            finished,
            finish_crossed_forward: finished && self.finish_crossed_forward,
        }
    }
    pub fn sample(&self) -> diagnostics::Sample {
        diagnostics::read(&self.playback, self.control_at, self.steps)
    }
    pub fn sound(&self) -> rbr_recording::sound::Sound {
        let mem = &self.playback.pure.mem;
        let car = mem.u32(rbr_hybrid::pure::CAR_PTR);
        rbr_recording::sound::Sound::read_from(car, |at| mem.u32(at))
    }
    pub fn engine_noise(&self, min_rpm: f32) -> crate::engine_audio::EngineNoise {
        let sound = self.sound();
        crate::engine_audio::EngineNoise::from_payload(&sound.engine, min_rpm)
    }
    pub fn payload_bytes(&self) -> usize {
        self.playback.pure.mem.pages().expect("paged boot").allocated_bytes()
    }
}

fn finish_direction_for(data: &rbr_formats::data::Data, stage_name: &str) -> Option<[f32; 3]> {
    let dls_bytes = rbr_formats::stage_files::read(data, stage_name, "dls").ok()?;
    let markers = rbr_formats::markers::parse(&dls_bytes).ok()?;
    let dist = markers.finish().filter(|&d| d > 0.0 && d.is_finite())?;
    let trk_bytes = rbr_formats::stage_files::read(data, stage_name, "trk").ok()?;
    let track = rbr_formats::trk::parse(&trk_bytes).ok()?;
    let dl = track.chunks.iter().find_map(|c| match &c.payload {
        rbr_formats::trk::ChunkPayload::DriveLine(d) => Some(d),
        _ => None,
    })?;
    if dl.records.len() < 2 {
        return None;
    }
    let idx = dl.records.windows(2).position(|w| w[0].value <= dist && dist <= w[1].value).unwrap_or_else(|| {
        dl.records
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (a.value - dist).abs().total_cmp(&(b.value - dist).abs()))
            .map(|(i, _)| i.min(dl.records.len() - 2))
            .unwrap_or(0)
    });
    let (a, b) = (&dl.records[idx], &dl.records[idx + 1]);
    let span = b.value - a.value;
    let frac = if span.abs() > 1e-5 { ((dist - a.value) / span).clamp(0.0, 1.0) } else { 0.0 };
    let mut dir = [
        a.vectors[1].x + (b.vectors[1].x - a.vectors[1].x) * frac,
        a.vectors[1].y + (b.vectors[1].y - a.vectors[1].y) * frac,
        a.vectors[1].z + (b.vectors[1].z - a.vectors[1].z) * frac,
    ];
    let mut len_sq = dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2];
    if len_sq <= 1e-8 {
        dir = [b.vectors[0].x - a.vectors[0].x, b.vectors[0].y - a.vectors[0].y, b.vectors[0].z - a.vectors[0].z];
        len_sq = dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2];
    }
    if len_sq <= 1e-8 || !len_sq.is_finite() {
        return None;
    }
    let inv = 1.0 / len_sq.sqrt();
    Some([dir[0] * inv, dir[1] * inv, dir[2] * inv])
}

#[cfg(test)]
mod surface_tests {
    #[test]
    fn selected_retail_surfaces_use_original_reference_tyres() {
        for (map, surface, tyre) in [(20, "gravel", 4), (22, "gravel", 4), (41, "tarmac", 0), (46, "tarmac", 0)] {
            assert_eq!(crate::boot::surface(map), surface);
            assert_eq!(crate::boot::default_tyre(crate::boot::surface(map)), tyre);
        }
    }

    #[test]
    fn real_physics_parity_and_per_car_differentiation_if_pc_assets_present() {
        use crate::drive::{Control, Session};
        use crate::inputs::Paths;
        use rerbr_content::{Selection, StageDescriptor, VehicleDescriptor};
        use std::path::Path;
        use std::time::Instant;

        let desktop_rbr = Path::new(r"C:\Users\adria\Desktop\RBR");
        let main_rbr = Path::new(r"C:\Richard Burns Rally");
        if !desktop_rbr.join("Plugins/NGP.dll").is_file() || !main_rbr.join("Maps/track-20_O.col").is_file() {
            return;
        }
        let paths_harwood = Paths { install: desktop_rbr.to_path_buf(), overlay: None, maps: main_rbr.join("Maps") };
        let paths_main = Paths {
            install: main_rbr.to_path_buf(),
            overlay: Some(desktop_rbr.to_path_buf()),
            maps: main_rbr.join("Maps"),
        };

        // Helper to drive 2.5s throttle, 0.5s steer, 0.5s brake cleanly and collect metrics
        let probe_drive = |session: &mut Session| {
            let ratios = session.gear_ratios().unwrap();
            let t0 = Instant::now();
            let step_with_gear = |s: &mut Session, mut c: Control| {
                if s.sample().commanded_gear == 2 {
                    c.flags &= !4;
                    c.gear = None;
                } else {
                    c.flags |= 4;
                    c.gear = Some(2);
                }
                s.step(c).unwrap();
            };
            // Shift into 1st gear + full throttle for 2520 steps (3.5s)
            for _ in 0..2520 {
                step_with_gear(session, Control { throttle: 1.0, ..Control::default() });
            }
            let accel = session.sample();
            assert!(accel.finite());
            // Steer left for 360 steps (0.5s)
            for _ in 0..360 {
                step_with_gear(session, Control { throttle: 0.7, steering: -0.6, ..Control::default() });
            }
            let steer = session.sample();
            assert!(steer.finite());
            // Brake hard for 360 steps (0.5s)
            for _ in 0..360 {
                step_with_gear(session, Control { brake: 1.0, ..Control::default() });
            }
            let braked = session.sample();
            assert!(braked.finite());
            let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
            (ratios, accel, steer, braked, elapsed_ms)
        };

        // 1. Original Toyota Corolla WRC
        let corolla_sel = Selection::default();
        let mut corolla = Session::boot_selected(&paths_harwood, &corolla_sel, false).unwrap();
        let (cor_ratios, cor_acc, cor_str, cor_brk, cor_ms) = probe_drive(&mut corolla);
        eprintln!(
            "PHY Corolla WRC: gears={} r={:.3?} speed={:.2}m/s braked={:.2}m/s yaw_z={:.3} spin={:.1?} travel={:.4?} ({:.1}ms)",
            cor_ratios.len(),
            &cor_ratios[..cor_ratios.len().min(4)],
            cor_acc.speed(),
            cor_brk.speed(),
            cor_str.angular_velocity[2],
            cor_acc.wheel_spin,
            cor_acc.wheel_travel,
            cor_ms
        );
        assert!(cor_acc.speed() > 5.0);
        assert!(cor_brk.speed() < cor_str.speed());

        // 2. Original Peugeot 206 WRC (5-speed) on paths_main
        let p206_desc = VehicleDescriptor::presets().into_iter().find(|v| v.id == "peugeot-206-wrc").unwrap();
        if let Ok(mut p206) =
            Session::boot_selected(&paths_main, &Selection { vehicle: p206_desc, ..corolla_sel.clone() }, false)
        {
            let (p206_ratios, p206_acc, p206_str, p206_brk, p206_ms) = probe_drive(&mut p206);
            eprintln!(
                "PHY Peugeot 206 WRC: gears={} r={:.3?} speed={:.2}m/s braked={:.2}m/s yaw_z={:.3} ({:.1}ms)",
                p206_ratios.len(),
                &p206_ratios[..p206_ratios.len().min(4)],
                p206_acc.speed(),
                p206_brk.speed(),
                p206_str.angular_velocity[2],
                p206_ms
            );
            assert_eq!(p206_ratios.len(), 7);
            assert_ne!(cor_ratios, p206_ratios);
        }

        // 3. Real NGP6 Pack: Ford Focus Mk II RS WRC 2006 (Gravel AND Tarmac!)
        let focus_pack = main_rbr.join("rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6");
        if focus_pack.is_dir() {
            let focus_sel =
                Selection { vehicle: VehicleDescriptor::ford_focus_2006(), stage: StageDescriptor::harwood_forest() };
            let mut focus = Session::boot_experimental_pack(&paths_main, &focus_sel, &focus_pack, false).unwrap();
            let (foc_ratios, foc_acc, foc_str, foc_brk, foc_ms) = probe_drive(&mut focus);
            eprintln!(
                "PHY Focus 2006 NGP6 (Gravel): gears={} r={:.3?} speed={:.2}m/s braked={:.2}m/s yaw_z={:.3} spin={:.1?} travel={:.4?} ({:.1}ms)",
                foc_ratios.len(),
                &foc_ratios[..foc_ratios.len().min(4)],
                foc_acc.speed(),
                foc_brk.speed(),
                foc_str.angular_velocity[2],
                foc_acc.wheel_spin,
                foc_acc.wheel_travel,
                foc_ms
            );
            assert!(foc_acc.speed() > 5.0);
            assert!(foc_brk.speed() < foc_str.speed());
            assert!(foc_str.angular_velocity[2].abs() > 0.05);
            assert_ne!(foc_ratios, cor_ratios);

            if main_rbr.join("Maps/track-41_O.col").is_file() {
                let focus_tarmac_sel =
                    Selection { vehicle: VehicleDescriptor::ford_focus_2006(), stage: StageDescriptor::cote_darbroz() };
                if let Ok(mut focus_tar) =
                    Session::boot_experimental_pack(&paths_main, &focus_tarmac_sel, &focus_pack, false)
                {
                    let (_, ft_acc, ft_str, ft_brk, _) = probe_drive(&mut focus_tar);
                    eprintln!(
                        "PHY Focus 2006 NGP6 (Tarmac): speed={:.2}m/s braked={:.2}m/s yaw_z={:.3} spin={:.1?}",
                        ft_acc.speed(),
                        ft_brk.speed(),
                        ft_str.angular_velocity[2],
                        ft_acc.wheel_spin
                    );
                }
            }
        }

        // 4. Real RSF installed cars discovered automatically via rbr_formats::rsf::Install
        if let Ok(rsf_install) = rbr_formats::rsf::Install::open(main_rbr) {
            let retail = std::fs::read_to_string(main_rbr.join("Cars/Cars.ini"))
                .ok()
                .and_then(|t| rbr_formats::cars::parse(&t).ok())
                .unwrap_or_default();
            for car in &rsf_install.cars {
                let pack_dir = main_rbr.join("rsfdata/cars").join(&car.pack);
                if !pack_dir.is_dir() {
                    continue;
                }
                let Ok(model) = rsf_install.model_entry(car, &retail) else {
                    eprintln!("PHY RSF {}: model unavailable or encrypted", car.name);
                    continue;
                };
                // Try actual gearbox gear count 5 vs 6 (or inspect Pack setup) cleanly
                let mut booted = Err("no matching gear count".to_string());
                for gears in [6u32, 5u32, 4u32, 7u32] {
                    let mut vehicle = VehicleDescriptor::ford_focus_2006();
                    vehicle.name = car.name.clone();
                    vehicle.forward_gears = gears;
                    vehicle.model = model.entry.file_name.replace('\\', "/");
                    let sel = Selection { vehicle, stage: StageDescriptor::harwood_forest() };
                    match Session::boot_experimental_pack(&paths_main, &sel, &pack_dir, false) {
                        Ok(s) => {
                            booted = Ok(s);
                            break;
                        }
                        Err(e) if e.contains("gearbox gear count") => continue,
                        Err(e) => {
                            booted = Err(e);
                            break;
                        }
                    }
                }
                match booted {
                    Ok(mut s) => {
                        let (r, acc, str_s, brk, ms) = probe_drive(&mut s);
                        eprintln!(
                            "PHY RSF {} [group={:?} dt={:?} fmod={}]: audit={} gears={} r={:.3?} speed={:.2}m/s steer_spd={:.2}m/s braked={:.2}m/s yaw_z={:.3} spin={:.1?} ({:.1}ms)",
                            car.name,
                            car.group,
                            car.drive_train,
                            rsf_install.audio_bank(car).is_some(),
                            s.physics_audit().summary(),
                            r.len(),
                            &r[..r.len().min(4)],
                            acc.speed(),
                            str_s.speed(),
                            brk.speed(),
                            str_s.angular_velocity[2],
                            acc.wheel_spin,
                            ms
                        );
                        assert!(acc.speed() > 4.0, "{} failed acceleration", car.name);
                        assert!(brk.speed() < str_s.speed(), "{} failed braking", car.name);
                    }
                    Err(e) => eprintln!("PHY RSF {}: BOOT ERR: {e}", car.name),
                }
            }
        }
    }

    #[test]
    fn clutch_disengagement_telemetry_repro() {
        use super::*;
        use std::path::Path;
        let desktop_rbr = Path::new(r"C:\Users\adria\Desktop\RBR");
        let main_rbr = Path::new(r"C:\Richard Burns Rally");
        if !desktop_rbr.join("Plugins/NGP.dll").is_file() || !main_rbr.join("Maps/track-20_O.col").is_file() {
            return;
        }
        let paths = inputs::Paths { install: desktop_rbr.to_path_buf(), overlay: None, maps: main_rbr.join("Maps") };
        for clutch_help in [true, false] {
            let mut session =
                Session::boot_with_aids(&paths, &rerbr_content::Selection::default(), false, clutch_help).unwrap();
            let step_g1 = |s: &mut Session, mut c: Control| {
                if s.sample().commanded_gear == 2 {
                    c.flags &= !4;
                    c.gear = None;
                } else {
                    c.flags |= 4;
                    c.gear = Some(2);
                }
                s.step(c).unwrap();
            };
            for _ in 0..1500 {
                session.step(Control::default()).unwrap();
            }
            // Phase A: in gear 1 (word 2), clutch held (1.0) + full throttle (1.0) for 1.5s
            for _ in 0..1080 {
                step_g1(&mut session, Control { throttle: 1.0, clutch: 1.0, ..Default::default() });
            }
            let held = session.sample();
            eprintln!(
                "clutch_help={clutch_help} | HELD 1.0 + THROTTLE 1.0 (G1): gear={} speed={:.2}m/s rpm={:.0} payload_clutch={:.2} slip={:.3}",
                held.gear, held.speed(), held.rpm, held.payload_clutch, held.drive_clutch_slip
            );
            assert!(
                held.speed() < 0.05,
                "clutch_help={clutch_help} should stay stationary with clutch=1.0: speed={}",
                held.speed()
            );
            assert!(
                held.rpm > 6500.0,
                "clutch_help={clutch_help} should rev engine freely with clutch=1.0: rpm={}",
                held.rpm
            );
            // Phase B: half-clutch (0.5) for 1.0s
            for _ in 0..720 {
                step_g1(&mut session, Control { throttle: 1.0, clutch: 0.5, ..Default::default() });
            }
            let half = session.sample();
            eprintln!(
                "clutch_help={clutch_help} | HALF 0.5 + THROTTLE 1.0 (G1): gear={} speed={:.2}m/s rpm={:.0} payload_clutch={:.2} slip={:.3}",
                half.gear, half.speed(), half.rpm, half.payload_clutch, half.drive_clutch_slip
            );
            assert!(half.speed() > 1.0, "half-clutch should transfer partial drive torque");
            // Phase C: release clutch (0.0) for 1.5s
            for _ in 0..1080 {
                step_g1(&mut session, Control { throttle: 1.0, clutch: 0.0, ..Default::default() });
            }
            let released = session.sample();
            eprintln!(
                "clutch_help={clutch_help} | RELEASED 0.0 (1.5s): gear={} speed={:.2}m/s rpm={:.0} payload_clutch={:.2} slip={:.3}",
                released.gear, released.speed(), released.rpm, released.payload_clutch, released.drive_clutch_slip
            );
            assert!(released.speed() > 10.0, "released clutch should launch vehicle cleanly");
            // Phase D: re-press clutch (1.0) while moving for 1.0s
            for _ in 0..720 {
                step_g1(&mut session, Control { throttle: 1.0, clutch: 1.0, ..Default::default() });
            }
            let re_held = session.sample();
            eprintln!(
                "clutch_help={clutch_help} | RE-PRESSED 1.0 MOVING (1.0s): gear={} speed={:.2}m/s rpm={:.0} payload_clutch={:.2} slip={:.3}",
                re_held.gear, re_held.speed(), re_held.rpm, re_held.payload_clutch, re_held.drive_clutch_slip
            );
            assert!(re_held.speed() <= released.speed() + 0.05, "declutching while moving must not accelerate");
            assert!(re_held.rpm > 7000.0, "declutching under throttle must allow engine RPM to rise");
        }
    }

    #[test]
    fn school_lesson_boot_and_step_smoke() {
        use crate::drive::{Control, Session};
        use crate::inputs::Paths;
        let desktop_rbr = std::path::Path::new(r"C:\Users\adria\Desktop\RBR");
        let rbr = std::path::Path::new(r"C:\Richard Burns Rally");
        if !desktop_rbr.join("Plugins/NGP.dll").is_file()
            || !rbr.join("Maps/track-81_M.col").is_file()
            || !rbr.join("Maps/track-73_M.col").is_file()
        {
            return;
        }
        let paths =
            Paths { install: rbr.to_path_buf(), overlay: Some(desktop_rbr.to_path_buf()), maps: rbr.join("Maps") };
        let impreza = rerbr_content::VehicleDescriptor::original_rbr_presets()
            .into_iter()
            .find(|v| v.original_slot == rbr_school::lesson::CAR)
            .expect("Car05 impreza preset");
        for idx in [0u32, 1u32, 12u32] {
            let lesson = rbr_school::lesson::Lesson::new(idx).unwrap();
            let map = lesson.map();
            let stage =
                rerbr_content::StageDescriptor::school_lesson(map, format!("track-{map}_M"), "RBR/Maps", 1).unwrap();
            let sel = rerbr_content::Selection { vehicle: impreza.clone(), stage };
            let mut session = Session::boot_lesson(&paths, &sel, idx, false, false).unwrap();
            assert!(session.sample().rpm > 800.0, "school lesson {idx} should boot ready with running engine");
            for step_i in 0..1440 {
                let clutch = if step_i < 240 {
                    1.0
                } else if step_i < 480 {
                    0.5
                } else {
                    0.0
                };
                let c = Control {
                    throttle: 1.0,
                    clutch,
                    steering: 0.25,
                    flags: if session.sample().commanded_gear != 2 { 4 } else { 0 },
                    gear: (session.sample().commanded_gear != 2).then_some(2),
                    ..Default::default()
                };
                session.step(c).unwrap();
            }
            let s = session.sample();
            eprintln!(
                "SCHOOL lesson={idx} map={map}: gear={} cmd={} thr={:.2} speed={:.2}m/s rpm={:.0}",
                s.gear,
                s.commanded_gear,
                s.throttle_received,
                s.speed(),
                s.rpm
            );
            assert!(s.speed() > 6.0, "school lesson {idx} failed to accelerate: {}", s.speed());
        }
    }
}
