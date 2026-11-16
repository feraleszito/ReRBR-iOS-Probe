//! App-owned, original boot and controlled fixed steps. No presentation writes to physics.
use crate::{boot, diagnostics, inputs};
pub use rbr_gameplay::control::CarControl as Control;

/// Independent read-only COL geometry for contact diagnostics, not a replacement collision solver.
pub fn collision_triangles(paths: &inputs::Paths) -> Result<Vec<[[f32; 3]; 3]>, String> {
    let bytes = std::fs::read(paths.maps.join("track-20_O.col")).map_err(|e| e.to_string())?;
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

pub struct Session {
    playback: Playback,
    control_at: u32,
    dt: f32,
    steps: u32,
}
impl Session {
    /// Call on the dedicated STACK_BYTES worker; callers must catch pure-host panics there.
    pub fn boot(paths: &inputs::Paths) -> Result<Self, String> {
        if cfg!(panic = "abort") {
            return Err("physics-drive requires panic=unwind".into());
        }
        let checked = inputs::check(paths)?;
        let data = paths.data();
        let season = Season::load(&data)?;
        let row = season
            .rows
            .iter()
            .position(|r| r.map == 20 && r.letter == 3 && r.wetness == 0)
            .ok_or("missing dry O weather row")?;
        let weather = weather::quick_race(&season, 20, Preset::Random, Draws { row: Some(row), wear: 1 })?;
        let race = boot::Race::quick_race(20, 6, 4, weather, checked.physics);
        let name = paths.maps.join("track-20_O").to_string_lossy().into_owned();
        let image = boot::build(&data, &name, State::Load.value(), &race)?;
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
        let mut stage = Stage::quick_race(20, 6).load_ready_with(NGP_LOAD_SUBSTEPS);
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
        eprintln!(
            "physics-drive BOOT READY: original stage 20 / car 6; dt={dt:.10}; built={:?}; panic_unwind={}; host={}/{}",
            image.built,
            cfg!(panic = "unwind"),
            std::env::consts::ARCH,
            std::env::consts::OS
        );
        Ok(Self { playback, control_at: image.control_at, dt, steps: 0 })
    }
    pub fn dt(&self) -> f32 {
        self.dt
    }
    pub fn step(&mut self, input: Control) -> Result<(), String> {
        if [input.throttle, input.brake, input.clutch, input.handbrake, input.steering].iter().any(|x| !x.is_finite()) {
            return Err("nonfinite input".into());
        }
        let mut payload = [0; 32];
        for (k, word) in input.words().iter().enumerate() {
            payload[k * 4..k * 4 + 4].copy_from_slice(&word.unwrap_or(0).to_le_bytes());
        }
        self.playback.step(Some(&payload), 1);
        self.steps = self.steps.checked_add(1).ok_or("physics step counter exhausted")?;
        Ok(())
    }
    pub fn sample(&self) -> diagnostics::Sample {
        diagnostics::read(&self.playback, self.control_at, self.steps)
    }
    pub fn payload_bytes(&self) -> usize {
        self.playback.pure.mem.pages().expect("paged boot").allocated_bytes()
    }
}
