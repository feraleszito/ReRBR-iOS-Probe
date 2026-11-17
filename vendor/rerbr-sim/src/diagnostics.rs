//! Read-only adapters to original layouts. No writes or simulated values.
use rbr_hybrid::pure::{Playback, CAR_PTR};
use rbr_physics::{
    engine_driveline::layout::{Drive, DriveDef, DriveObjects, DriveState, EngineState, GearBand},
    rigid_body::layout as body,
    wheel::layout as wheel,
};

/// Original immutable per-slot folder table, also used by the unchanged boot resolver.
pub(crate) fn physics_folder(mem: &rbr_physics::mem::Mem, slot: u32) -> Result<String, String> {
    if slot > 7 {
        return Err("unsupported original physics slot".into());
    }
    let address = mem.u32(0x007c_18d4 + 4 * slot);
    let bytes: Vec<_> = (0..256).map(|i| mem.u8(address + i)).take_while(|b| *b != 0).collect();
    if bytes.is_empty() || bytes.len() == 256 {
        return Err("invalid original physics folder".into());
    }
    String::from_utf8(bytes).map_err(|_| "original physics folder is not UTF-8".into())
}

/// Read the configured bands after the original setup has been applied; no ratio table in the app.
pub fn gear_ratios(playback: &Playback) -> Result<Vec<f32>, String> {
    let mem = &playback.pure.mem;
    let objects = DriveObjects::at(mem, mem.u32(CAR_PTR) + 0x4b0);
    let count = DriveDef::load(mem, objects.def).gear_count;
    if !(2..=10).contains(&count) {
        return Err(format!("invalid configured gear count: {count}"));
    }
    let ratios: Vec<_> = (0..count).map(|k| GearBand::load(mem, objects.def, k).ratio).collect();
    if !ratios.iter().all(|r| r.is_finite()) {
        return Err("nonfinite configured gear ratio".into());
    }
    Ok(ratios)
}

/// Original VCU configuration for investigating automatic-shift integration.
pub fn gearbox_audit(playback: &Playback) -> ([f32; 10], [f32; 10], u32, f32) {
    use rbr_physics::vcu::store::VcuStore;
    let mem = &playback.pure.mem;
    let scratch = mem.scratch(mem.u32(CAR_PTR) + 0x850);
    let def = VcuStore::def(mem, scratch.def);
    (def.gear_upshift_rpm, def.gear_downshift_rpm, scratch.launch_enabled, def.rpm_limit)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sample {
    pub step: u32,
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub rpm: f32,
    pub gear: i32,
    pub commanded_gear: i32,
    pub throttle_received: f32,
    /// Steering channel delivered at the original CSigCarControl sink, not a visual estimate.
    pub steering_received: f32,
    /// Physical rack travel divided by its original ratio (steering-wheel radians).
    pub steering_angle: f32,
    pub brake_received: f32,
    pub payload_clutch: f32,
    pub drive_clutch_slip: f32,
    pub drive_engine_speed: f32,
    pub brake_torque: f32,
    /// Original engine state field; NGP's output torque is not claimed to be this legacy slot.
    pub legacy_engine_torque: f32,
    pub wheel_spin: [f32; 4],
    pub wheel_ground: [u32; 4],
    /// Original WheelPose strut travel (+0x5bc, rbr-recording/pose.rs).
    pub wheel_travel: [f32; 4],
    pub wheel_anchor: [[f32; 3]; 4],
    pub wheel_roll: [f32; 4],
    pub wheel_centre: [[f32; 3]; 4],
    /// Physical hub axle in the car's z-up frame (+0x9e0, original WheelPose layout).
    /// Read-only presentation input; independent of the steering-wheel angle.
    pub wheel_axle: [[f32; 3]; 4],
}
impl Sample {
    pub fn speed(&self) -> f64 {
        self.velocity.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt()
    }
    pub fn finite(&self) -> bool {
        self.position
            .iter()
            .chain(&self.orientation)
            .chain(&self.velocity)
            .chain(&self.angular_velocity)
            .chain(&self.wheel_spin)
            .chain(&self.wheel_travel)
            .chain(self.wheel_anchor.iter().flatten())
            .chain(&self.wheel_roll)
            .chain(self.wheel_centre.iter().flatten())
            .chain(self.wheel_axle.iter().flatten())
            .all(|x| x.is_finite())
            && [
                self.rpm,
                self.throttle_received,
                self.steering_received,
                self.steering_angle,
                self.brake_received,
                self.payload_clutch,
                self.drive_clutch_slip,
                self.drive_engine_speed,
                self.brake_torque,
                self.legacy_engine_torque,
            ]
            .iter()
            .all(|x| x.is_finite())
    }
}
pub fn read(playback: &Playback, control_at: u32, step: u32) -> Sample {
    use rbr_physics::car_body::store::CarStore;
    let mem = &playback.pure.mem;
    let car = mem.u32(CAR_PTR);
    // Standalone live/picture.rs: CCar drive +0x4b0, engine +0x690, state pointer +8.
    let drive = Drive::load(mem, car + 0x4b0);
    let state = DriveState::load(mem, DriveObjects::at(mem, car + 0x4b0).state);
    let engine = EngineState::load(mem, mem.u32(car + 0x690 + 8));
    let wheels: [u32; 4] = std::array::from_fn(|k| mem.u32(car + rbr_recording::sound::CAR_WHEELS + 4 * k as u32));
    Sample {
        step,
        position: mem.f32s(car + body::POSITION),
        orientation: mem.f32s(car + body::ORIENTATION),
        velocity: mem.f32s(car + body::LINEAR_VELOCITY),
        angular_velocity: mem.f32s(car + body::ANGULAR_VELOCITY),
        rpm: engine.speed * (60.0 / std::f32::consts::TAU),
        gear: state.gear,
        commanded_gear: state.commanded_gear.value,
        // ngp/engine.rs::input: live engine pedal, not the legacy drive throttle lane.
        throttle_received: mem.f32(car + 0x690 + 0x20),
        steering_received: mem.f32(control_at + 0x10),
        steering_angle: mem.steer_target(car) / mem.steering_rack_ratio(car),
        brake_received: drive.brake_in.value,
        payload_clutch: mem.f32(control_at + 0x0c),
        drive_clutch_slip: drive.clutch_slip.value,
        drive_engine_speed: state.engine_speed,
        brake_torque: drive.brake_torque.value,
        legacy_engine_torque: engine.torque,
        wheel_spin: wheels.map(|w| mem.f32(w + wheel::SPIN_RATE)),
        wheel_ground: wheels.map(|w| mem.u32(w + wheel::ON_GROUND)),
        wheel_travel: wheels.map(|w| mem.f32(w + 0x5bc)),
        wheel_anchor: wheels.map(|w| mem.f32s(w + 0x140)),
        wheel_roll: wheels.map(|w| mem.f32(w + 0x5ac)),
        wheel_axle: wheels.map(|w| mem.f32s(w + 0x9e0)),
        wheel_centre: wheels.map(|w| {
            let anchor: [f32; 3] = mem.f32s(w + 0x140);
            let axle: [f32; 3] = mem.f32s(w + 0x9e0);
            let hub_offset = mem.f32(mem.u32(w + wheel::TYRE_TAIL) + wheel::TAIL_HUB_OFFSET);
            rbr_recording::pose::WheelPose {
                anchor,
                axle,
                hub_offset,
                orientation: [0.0, 0.0, 0.0, 1.0],
                roll: 0.0,
                strut_travel: 0.0,
            }
            .centre()
        }),
    }
}
