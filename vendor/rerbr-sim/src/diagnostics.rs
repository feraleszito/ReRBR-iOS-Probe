//! Read-only adapters to original layouts. No writes or simulated values.
use rbr_hybrid::pure::{Playback, CAR_PTR};
use rbr_physics::{
    engine_driveline::layout::{Drive, DriveObjects, DriveState, EngineState},
    rigid_body::layout as body,
    wheel::layout as wheel,
};

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
            .all(|x| x.is_finite())
            && [
                self.rpm,
                self.throttle_received,
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
