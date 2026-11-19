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
    car_damage::CarriedDamage,
    mode::{ModeEnd, ModeRules, StepView},
    stage::{Stage, State, NGP_LOAD_SUBSTEPS},
    weather::{self, Draws, Preset, Season},
};
use rbr_hybrid::pure::{self, Playback};
use rbr_physics::rigid_body::layout as body;
use rbr_school::{lesson::Lesson, step::SchoolEvent, SchoolRun};

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
    /// Deterministic simulation time accumulated from fixed 720 Hz steps (`steps * dt`), independent of FPS.
    pub sim_elapsed_seconds: f64,
    /// Authentic `CTimeKeeper` finish time (`finish_time - origin`) in simulation seconds once finished.
    pub official_finish_seconds: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchoolOutcome {
    Finished,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchoolEventNotice {
    pub speech_ids: Vec<u32>,
    pub incident_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SchoolSessionSnapshot {
    pub lesson_index: u32,
    pub started: bool,
    pub untimed: bool,
    pub ride: bool,
    pub ended: Option<SchoolOutcome>,
    pub elapsed_seconds: f32,
    pub perfect_time: f32,
    pub pass_time: f32,
    pub time_limit: f32,
    pub target_distance: Option<f32>,
    pub verdict: u32,
    /// `(key, from_lesson_dls)` pairs in upstream `explain::failure_reasons` order.
    pub failure_reasons: Vec<(String, bool)>,
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
    school_run: Option<SchoolRun>,
    school_started: bool,
    school_time_seconds: f32,
    school_end: Option<SchoolOutcome>,
    school_events: Vec<SchoolEventNotice>,
    school_driveline: Vec<rbr_formats::trk::DriveLineRecord>,
    school_driveline_values: Vec<f32>,
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
            .and_then(|s| s.strip_suffix(".sgc").or_else(|| s.strip_suffix(".SGC")))
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
            return Err(format!(
                "selected vehicle descriptor disagrees with original gearbox gear count (descriptor={} + 2, physics={})",
                selection.vehicle.forward_gears,
                ratios.len()
            ));
        }
        if aids.automatic {
            let audit = diagnostics::gearbox_audit(&playback);
            if audit.0[2..ratios.len()].iter().all(|rpm| *rpm == 0.0) {
                return Err("automatic transmission unavailable: original VCU upshift thresholds are empty; manual transmission remains supported".into());
            }
        }
        let (school_run, school_driveline, school_driveline_values) = if let Some(lesson) = aids.lesson {
            let (run, records, values) = load_school_runtime(&data, &name, lesson);
            (run, records, values)
        } else {
            (None, Vec::new(), Vec::new())
        };
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
            school_run,
            school_started: false,
            school_time_seconds: 0.0,
            school_end: None,
            school_events: Vec::new(),
            school_driveline,
            school_driveline_values,
        })
    }
    /// Begins authentic `rbr_school::SchoolRun` evaluation once the player confirms the briefing.
    pub fn start_school_drive(&mut self) {
        if let Some(run) = &mut self.school_run {
            self.school_started = true;
            self.school_time_seconds = 0.0;
            self.school_end = None;
            self.school_events.clear();
            let _ = self.playback.pure.take_wall_emits();
            let _ = self.playback.pure.take_retire();
            run.start();
        }
    }
    pub fn take_school_events(&mut self) -> Vec<SchoolEventNotice> {
        std::mem::take(&mut self.school_events)
    }
    pub fn school_state(&self) -> Option<SchoolSessionSnapshot> {
        let run = self.school_run.as_ref()?;
        let lesson = run.lesson();
        let bank = run.bank();
        let damage = self.carried_damage();
        let eval_time = self.school_time_seconds;
        let verdict =
            if self.school_end.is_some() || self.school_started { run.verdict(eval_time, &damage) } else { 0 };
        let failure_reasons = if self.school_end.is_some() && verdict == 0 {
            run.failure_reasons(eval_time).into_iter().map(|r| (r.key, r.lesson)).collect()
        } else {
            Vec::new()
        };
        Some(SchoolSessionSnapshot {
            lesson_index: lesson.index(),
            started: self.school_started,
            untimed: lesson.untimed(),
            ride: lesson.ride(),
            ended: self.school_end,
            elapsed_seconds: self.school_time_seconds,
            perfect_time: bank.perfect_time,
            pass_time: bank.pass_time,
            time_limit: bank.time_limit,
            target_distance: run
                .drive_line_at()
                .or_else(|| bank.subs.iter().find(|s| s.tag == 4 || s.tag == 5).map(|s| s.center)),
            verdict,
            failure_reasons,
        })
    }
    fn carried_damage(&self) -> CarriedDamage {
        let mem = &self.playback.pure.mem;
        let car = mem.u32(pure::CAR_PTR);
        if car == 0 {
            return CarriedDamage::default();
        }
        let handler = mem.u32(car + rbr_physics::car_body::layout::DAMAGE_HANDLER);
        if handler == 0 {
            return CarriedDamage::default();
        }
        CarriedDamage::read(|at| mem.u32(handler + at))
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
        let school_running = self.school_started && self.school_end.is_none();
        if school_running {
            if let Some(run) = &mut self.school_run {
                run.before_step(&input);
            }
        }
        self.playback.step(Some(&payload), 1);
        self.steps = self.steps.checked_add(1).ok_or("physics step counter exhausted")?;
        if school_running {
            let (wall_inside, wall_crossed) = self.playback.pure.take_wall_emits();
            let retire = self.playback.pure.take_retire();
            let mem = &self.playback.pure.mem;
            let car = mem.u32(pure::CAR_PTR);
            let (param, position, velocity) = if car != 0 {
                (mem.f32(car + 0x17c), mem.f32s(car + body::POSITION), mem.f32s(car + body::LINEAR_VELOCITY))
            } else {
                (0.0, [0.0; 3], [0.0; 3])
            };
            let distance = rbr_gameplay::driveline::sample_x(&self.school_driveline_values, param);
            let drive_line = self
                .school_run
                .as_ref()
                .and_then(|run| run.drive_line_at())
                .and_then(|d| sample_drive_line_point(&self.school_driveline, d));
            if let Some(run) = &mut self.school_run {
                run.after_step(&StepView {
                    sample: rbr_gameplay::game::StepSample {
                        driveline_parameter: param,
                        retire,
                        position,
                        velocity,
                        wall_inside,
                        wall_crossed,
                        ..Default::default()
                    },
                    distance,
                    drive_line,
                    app_state: 1,
                });
                self.school_time_seconds += self.dt;
                let ended = if run.lesson().ride() {
                    let tk = pure::time_keeper(&self.playback.pure.mem);
                    let splits_total = tk.card.splits().len();
                    let finished = tk.card.finish_time.is_some()
                        && tk.card.finish > 0.0
                        && tk.card.passed >= splits_total
                        && self.finish_crossed_forward;
                    finished.then_some(ModeEnd::Finished)
                } else {
                    run.frame(self.dt, self.school_time_seconds)
                };
                for ev in run.take_events() {
                    let speech_ids = run.speech_ids(&ev);
                    let incident_code = match ev {
                        SchoolEvent::Incident { code } => Some(code),
                        _ => None,
                    };
                    if !speech_ids.is_empty() || incident_code.is_some() {
                        self.school_events.push(SchoolEventNotice { speech_ids, incident_code });
                    }
                }
                if let Some(mode_end) = ended {
                    self.school_end = Some(match mode_end {
                        ModeEnd::Finished => SchoolOutcome::Finished,
                        ModeEnd::Retired => SchoolOutcome::Retired,
                    });
                }
            }
        }
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
            sim_elapsed_seconds: f64::from(self.steps) * f64::from(self.dt),
            official_finish_seconds: finished
                .then_some(tk.card.finish_time.map(|ft| (ft - tk.card.origin).max(0.0)))
                .flatten()
                .filter(|t| t.is_finite() && *t > 0.0),
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

fn load_school_runtime(
    data: &rbr_formats::data::Data,
    stage_name: &str,
    lesson: Lesson,
) -> (Option<SchoolRun>, Vec<rbr_formats::trk::DriveLineRecord>, Vec<f32>) {
    let Ok(dls_bytes) = rbr_formats::stage_files::read(data, stage_name, "dls") else {
        return (None, Vec::new(), Vec::new());
    };
    let Ok(dls) = rbr_formats::dls::parse(&dls_bytes) else {
        return (None, Vec::new(), Vec::new());
    };
    let Ok(entry) = rbr_formats::markers::driveline_entry(&dls) else {
        return (None, Vec::new(), Vec::new());
    };
    let mut prior = rbr_school::bank::Bank::fresh();
    // Clear the init hold so Trigger emits instructor speech cues during live driving.
    prior.live = false;
    let bank = rbr_school::bank::Bank::from_stream(&prior, &entry.replay, 0);
    let run = SchoolRun::new(lesson, bank, dls.string_bytes.clone());

    let records = rbr_formats::stage_files::read(data, stage_name, "trk")
        .ok()
        .and_then(|trk_bytes| rbr_formats::trk::parse(&trk_bytes).ok())
        .and_then(|track| {
            track.chunks.into_iter().find_map(|c| match c.payload {
                rbr_formats::trk::ChunkPayload::DriveLine(d) => Some(d.records),
                _ => None,
            })
        })
        .unwrap_or_default();
    let values = records.iter().map(|r| r.value).collect();
    (Some(run), records, values)
}

fn sample_drive_line_point(records: &[rbr_formats::trk::DriveLineRecord], dist: f32) -> Option<[f32; 3]> {
    if records.is_empty() || !dist.is_finite() {
        return None;
    }
    if records.len() == 1 {
        let p = records[0].vectors[0];
        return Some([p.x, p.y, p.z]);
    }
    let idx = records.windows(2).position(|w| w[0].value <= dist && dist <= w[1].value).unwrap_or_else(|| {
        records
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (a.value - dist).abs().total_cmp(&(b.value - dist).abs()))
            .map(|(i, _)| i.min(records.len() - 2))
            .unwrap_or(0)
    });
    let (a, b) = (&records[idx], &records[idx + 1]);
    let span = b.value - a.value;
    let frac = if span.abs() > 1e-5 { ((dist - a.value) / span).clamp(0.0, 1.0) } else { 0.0 };
    let pa = a.vectors[0];
    let pb = b.vectors[0];
    Some([pa.x + (pb.x - pa.x) * frac, pa.y + (pb.y - pa.y) * frac, pa.z + (pb.z - pa.z) * frac])
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

    #[test]
    fn school_all_17_dls_banks_and_rules_parse_cleanly() {
        let rbr = std::path::Path::new(r"C:\Richard Burns Rally");
        if !rbr.join("Maps/track-81_M.dls").is_file() || !rbr.join("strings.rbz").is_file() {
            return;
        }
        let data = crate::inputs::Paths { install: rbr.to_path_buf(), overlay: None, maps: rbr.join("Maps") }.data();
        let mut zip = rbr_formats::rbz::open(&rbr.join("strings.rbz")).unwrap();
        let mut spa_bytes = Vec::new();
        std::io::Read::read_to_end(&mut zip.by_name("Strings/bstrings.spa").unwrap(), &mut spa_bytes).unwrap();
        let spa = rbr_formats::strings::parse(&spa_bytes).unwrap();

        for idx in 0..rbr_school::lesson::Lesson::COUNT {
            let lesson = rbr_school::lesson::Lesson::new(idx).unwrap();
            let map = lesson.map();
            let stage_name = rbr.join("Maps").join(format!("track-{map}_M")).to_string_lossy().into_owned();
            let (run_opt, records, values) = super::load_school_runtime(&data, &stage_name, lesson);
            let run = run_opt.unwrap_or_else(|| panic!("lesson {idx} map {map} missing SchoolRun"));
            assert!(!records.is_empty(), "lesson {idx} missing driveline records");
            assert_eq!(records.len(), values.len());
            let bank = run.bank();
            let title = spa.get(lesson.title_key()).unwrap_or("<MISSING>");
            let (si_key, sa_key) = lesson.brief_keys();
            let si = spa.get(&si_key).unwrap_or("<MISSING>");
            let sa = spa.get(&sa_key).unwrap_or("<MISSING>");
            eprintln!(
                "DLS {:02} UI={} map={} title={:?} untimed={} ride={} times=({:.1}/{:.1}/{:.1}) subs={} checks={} pairs={} si={:?} sa={:?}",
                idx,
                idx + 1,
                map,
                title,
                lesson.untimed(),
                lesson.ride(),
                bank.perfect_time,
                bank.pass_time,
                bank.time_limit,
                bank.subs.len(),
                bank.checks.len(),
                bank.pairs.len(),
                si,
                sa
            );
            assert_ne!(title, "<MISSING>", "missing Spanish title {}", lesson.title_key());
            assert_ne!(si, "<MISSING>", "missing Spanish instruction {}", si_key);
        }
    }

    #[test]
    fn school_all_17_lessons_boot_and_expose_rules() {
        use crate::drive::Session;
        use crate::inputs::Paths;
        let desktop_rbr = std::path::Path::new(r"C:\Users\adria\Desktop\RBR");
        let rbr = std::path::Path::new(r"C:\Richard Burns Rally");
        if !desktop_rbr.join("Plugins/NGP.dll").is_file() || !rbr.join("Maps/track-81_M.col").is_file() {
            return;
        }
        let paths =
            Paths { install: rbr.to_path_buf(), overlay: Some(desktop_rbr.to_path_buf()), maps: rbr.join("Maps") };
        let impreza = rerbr_content::VehicleDescriptor::original_rbr_presets()
            .into_iter()
            .find(|v| v.original_slot == rbr_school::lesson::CAR)
            .expect("Car05 impreza preset");
        for idx in 0..rbr_school::lesson::Lesson::COUNT {
            let lesson = rbr_school::lesson::Lesson::new(idx).unwrap();
            let map = lesson.map();
            let stage =
                rerbr_content::StageDescriptor::school_lesson(map, format!("track-{map}_M"), "RBR/Maps", 1).unwrap();
            let sel = rerbr_content::Selection { vehicle: impreza.clone(), stage };
            let mut session = Session::boot_lesson(&paths, &sel, idx, false, false)
                .unwrap_or_else(|e| panic!("lesson {idx} (map {map}) boot failed: {e}"));
            session.start_school_drive();
            let st = session.school_state().expect("school_state");
            eprintln!(
                "LESSON {:02} map={} key={} untimed={} ride={} times=({:.1}/{:.1}/{:.1}) target={:?}",
                idx,
                map,
                lesson.title_key(),
                st.untimed,
                st.ride,
                st.perfect_time,
                st.pass_time,
                st.time_limit,
                st.target_distance
            );
            assert!(session.sample().rpm > 800.0, "lesson {idx} engine stalled");
        }
    }

    #[test]
    fn toyota_yaris_rally2_and_wrc18_boot_components_audit() {
        use rbr_formats::{car_pack::Pack, lsp, sgc};
        use rbr_physics::load::car_grids;
        use std::path::Path;

        let rbr = Path::new(r"C:\Richard Burns Rally");
        if !rbr.is_dir() {
            return;
        }
        // 1. Check collision meshes + node animation tables for both Toyotas
        for (label, rel) in [
            ("Focus 2006", "Cars/FORD_FOCUS_2006/focus06.sgc"),
            ("Yaris Rally2", "Cars/yaris_rally2/yaris_rally2.sgc"),
            ("Yaris WRC 2018", "Cars/YARIS_WRC18/yaris_wrc.sgc"),
        ] {
            let path = rbr.join(rel);
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let elems = sgc::parse(&bytes).unwrap_or_else(|e| panic!("{label} sgc::parse failed: {e}"));
            let bodies: Vec<sgc::Collision> = elems
                .into_iter()
                .filter_map(|e| match e {
                    sgc::Element::Collision(c) => Some(c),
                    _ => None,
                })
                .collect();
            let col_names: Vec<&str> = bodies.iter().map(|b| b.node.name.as_str()).collect();
            let meshes = car_grids::car_meshes(&bodies);
            let nodes = crate::handles::CarNodes::new(&bytes);
            let mut mem = rbr_physics::mem::Mem::default();
            for (at, size) in rbr_formats::exe_data::SECTIONS {
                mem.map(at, &vec![0; size as usize]);
            }
            for &(at, words) in rbr_formats::exe_data::RUNS {
                mem.map(at, &words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<_>>());
            }
            let handle_results: Vec<String> = match &nodes {
                Ok(n) => crate::boot::CAR_PARTS
                    .iter()
                    .filter_map(|p| {
                        let node_name = p.node?;
                        match n.handle(&mem, node_name, 4, 1) {
                            Ok(h) => Some(format!("{node_name}={h}")),
                            Err(e) => Some(format!("{node_name}=ERR({e})")),
                        }
                    })
                    .collect(),
                Err(e) => vec![format!("NODES_ERR({e})")],
            };
            eprintln!(
                "BOOT MODEL AUDIT [{label}]: collision_bodies={:?} car_meshes_ok={} car_meshes_len={:?} handles={:?}",
                col_names,
                meshes.is_some(),
                meshes.as_ref().map(Vec::len),
                handle_results,
            );
        }

        // 2. Check Toyota_GR_Yaris_Rally2_ngp7 AND Yaris WRC 2018 (rsfdata/Physics/t_coroll)
        // through real rbr_physics::load (physics_file, setup_file, options_file, select_drive_options, etc.)!
        let physics_rbz = rbr.join("physics.rbz");
        let misc_rbz = rbr.join("misc.rbz");
        for (label, pack_dir) in [
            ("Focus 2006 NGP6", rbr.join("rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6")),
            ("Yaris Rally2 NGP7", rbr.join("rsfdata/cars/Toyota_GR_Yaris_Rally2_ngp7")),
            ("Yaris WRC 2018 NGP6", rbr.join("rsfdata/cars/Toyota_Yaris_WRC_2018_ngp6")),
        ] {
            if !pack_dir.is_dir() || !physics_rbz.is_file() || !misc_rbz.is_file() {
                continue;
            }
            let pack = Pack::open(&pack_dir).unwrap();
            for surface in ["gravel", "tarmac", "snow"] {
                let audit = crate::physics_audit::inspect_pack(&physics_rbz, &pack, surface);
                eprintln!("PACK AUDIT [{label}] ({surface}): {audit:?}");

                // Run real rbr_physics::load!
                let mut mem = rbr_physics::mem::Mem::default();
                for (at, size) in rbr_formats::exe_data::SECTIONS {
                    mem.map(at, &vec![0; size as usize]);
                }
                for &(at, words) in rbr_formats::exe_data::RUNS {
                    mem.map(at, &words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<_>>());
                }
                let next = std::rc::Rc::new(std::cell::Cell::new(0x2000_0000u32));
                struct SimpleFiles(std::rc::Rc<std::cell::Cell<u32>>);
                impl SimpleFiles {
                    fn take(&mut self, mem: &mut rbr_physics::mem::Mem, size: u32) -> u32 {
                        let at = self.0.get();
                        let aligned = (size.max(1) + 0xfff) & !0xfff;
                        mem.map(at, &vec![0; aligned as usize]);
                        self.0.set(at + aligned);
                        at
                    }
                }
                impl rbr_physics::ntweak::ApplyHost for SimpleFiles {
                    fn alloc_name(&mut self, mem: &mut rbr_physics::mem::Mem, len: u32) -> u32 {
                        self.take(mem, len)
                    }
                    fn after_parse(&mut self, _: &mut rbr_physics::mem::Mem, _: u32) {}
                    fn def_index(&mut self, _: &rbr_physics::mem::Mem, _: &str) -> Option<u32> {
                        None
                    }
                    fn define(
                        &mut self,
                        _: &mut rbr_physics::mem::Mem,
                        _: &str,
                        _: u32,
                        _: &str,
                        _: &[lsp::Node],
                    ) -> Result<(), rbr_physics::ntweak::Unsupported> {
                        unreachable!()
                    }
                }
                impl rbr_physics::physics_core::layout::construct::Heap for SimpleFiles {
                    fn alloc(&mut self, mem: &mut rbr_physics::mem::Mem, size: u32) -> u32 {
                        self.take(mem, size)
                    }
                }
                let mut files = SimpleFiles(next.clone());
                let mut registry = rbr_physics::load::Registry::default();
                let def = rbr_physics::load::new_physics_def(&mut mem, &mut files);
                let read_rbz = |archive: &Path, entry: &str| -> Vec<lsp::Node> {
                    use std::io::Read;
                    let mut z = rbr_formats::rbz::open(archive).unwrap();
                    let mut s = String::new();
                    rbr_formats::rbz::by_name(&mut z, entry).unwrap().read_to_string(&mut s).unwrap();
                    rbr_physics::load::root(&lsp::parse(&s).unwrap()).unwrap().to_vec()
                };
                let read_pack = |entry: &str| -> Vec<lsp::Node> {
                    let b = pack.read(entry).unwrap();
                    let s = String::from_utf8_lossy(&b);
                    rbr_physics::load::root(&lsp::parse(&s).unwrap()).unwrap().to_vec()
                };
                let prime = read_rbz(&misc_rbz, "Misc/prime.lsp");
                let phys_lsp = read_rbz(&physics_rbz, "Physics/physics.lsp");
                let common = read_pack("common.lsp");
                let surf = read_pack(&format!("{surface}.lsp"));
                let setup = read_pack(&format!("setups/d_{surface}.lsp"));
                let tyres = read_rbz(&physics_rbz, "Physics/pirelli/tyres.lsp");
                let opts = read_pack(&format!("r_{surface}.lsp"));

                let r_prime = rbr_physics::load::physics_file(&mut mem, &mut files, &mut registry, def, &prime);
                let r_phys = rbr_physics::load::physics_file(&mut mem, &mut files, &mut registry, def, &phys_lsp);
                let r_common = rbr_physics::load::physics_file(&mut mem, &mut files, &mut registry, def, &common);
                let r_surf = rbr_physics::load::physics_file(&mut mem, &mut files, &mut registry, def, &surf);

                let complete = files.take(&mut mem, rbr_physics::load::layout::COMPLETE_SETUP_SIZE);
                let options = files.take(&mut mem, rbr_physics::load::layout::SETUP_OPTIONS_SIZE);
                rbr_physics::load::construct_complete_setup(&mut mem, complete);
                rbr_physics::load::construct_setup_options(&mut mem, options);
                let r_setup = rbr_physics::load::setup_file(&mut mem, &mut files, complete, &setup);
                let r_tyres = rbr_physics::load::options_file(&mut mem, &mut files, options, &tyres);
                let r_opts = rbr_physics::load::options_file(&mut mem, &mut files, options, &opts);
                let setups = rbr_physics::load::setups(complete);
                let tyre_idx = crate::boot::default_tyre(surface);
                rbr_physics::load::select_tyre_type(&mut mem, &setups, rbr_physics::load::tyre_type(options, tyre_idx));
                rbr_physics::load::select_drive_options(
                    &mut mem,
                    setups.drive,
                    rbr_physics::load::drive_options(options),
                );
                rbr_physics::load::select_top_mounts(&mut mem, &setups, options);
                eprintln!(
                    "LOAD [{label}] ({surface}): prime={r_prime:?} phys={r_phys:?} common={r_common:?} surf={r_surf:?} setup={r_setup:?} tyres={r_tyres:?} opts={r_opts:?}"
                );
                assert!(
                    r_prime.is_ok()
                        && r_phys.is_ok()
                        && r_common.is_ok()
                        && r_surf.is_ok()
                        && r_setup.is_ok()
                        && r_tyres.is_ok()
                        && r_opts.is_ok()
                );
            }
        }
    }
}
