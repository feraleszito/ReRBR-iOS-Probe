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
    finish_direction: Option<[f32; 3]>,
    finish_crossed_forward: bool,
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
        Self::boot_core(paths, data, checked.physics, selection.vehicle.original_slot, None, selection, automatic)
    }

    /// Experimental NGP car pack boot (for non-retail pack vehicles such as Ford Focus RS WRC 2006).
    /// Requires verified NGP 7.4 runtime and plaintext `.lsp` pack; never silently falls back to retail slots.
    pub fn boot_experimental_pack(
        paths: &inputs::Paths,
        selection: &rerbr_content::Selection,
        pack_path: &std::path::Path,
        automatic: bool,
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
        Self::boot_core(paths, data, physics_rbz, rbr_formats::car_pack::SLOT, car_pack, selection, automatic)
    }

    fn boot_core(
        paths: &inputs::Paths,
        data: rbr_formats::data::Data,
        physics_archive: std::path::PathBuf,
        slot: u32,
        car_pack: Option<boot::CarPack>,
        selection: &rerbr_content::Selection,
        automatic: bool,
    ) -> Result<Self, String> {
        if cfg!(panic = "abort") {
            return Err("physics-drive requires panic=unwind".into());
        }
        let season = Season::load(&data)?;
        let row = season
            .rows
            .iter()
            .position(|r| {
                r.map == selection.stage.original_map
                    && r.letter == selection.stage.weather_letter
                    && r.wetness == selection.stage.wetness
            })
            .ok_or_else(|| {
                format!(
                    "missing weather row for stage {} (letter {}, wetness {})",
                    selection.stage.id, selection.stage.weather_letter, selection.stage.wetness
                )
            })?;
        let weather = weather::quick_race(
            &season,
            selection.stage.original_map,
            Preset::Random,
            Draws { row: Some(row), wear: selection.stage.wear },
        )?;
        let audit_archive = physics_archive.clone();
        let mut race = boot::Race::quick_race(
            selection.stage.original_map,
            slot,
            boot::default_tyre(boot::surface(selection.stage.original_map)),
            weather,
            physics_archive,
        );
        race.car_pack = car_pack;
        race.options.aids.auto_gears = automatic;
        let name = paths.maps.join(&selection.stage.stem).to_string_lossy().into_owned();
        let finish_direction = finish_direction_for(&data, &name);
        let image = boot::build(&data, &name, State::Load.value(), &race)?;
        let folder = diagnostics::physics_folder(&image.mem, slot)?;
        let physics_audit = crate::physics_audit::inspect(&audit_archive, &folder, race.surface)?;
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
        stage = stage.go().countdown_tick(dt);
        if stage.state() != State::Driving {
            return Err("stage did not reach Driving".into());
        }
        pure::set_app_state(&mut playback.pure.mem.clone(), stage.state().value());
        let ratios = diagnostics::gear_ratios(&playback)?;
        if ratios.len() != selection.vehicle.forward_gears as usize + 2 {
            return Err("selected vehicle descriptor disagrees with original gearbox gear count".into());
        }
        if automatic {
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
}
