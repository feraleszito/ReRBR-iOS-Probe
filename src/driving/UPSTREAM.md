# Driving presentation and controls

GPL-3.0-or-later, retaining reRBR's license in vendor/rerbr-render-core/LICENSE.
Physics executes vendor/rerbr-sim's original imported crates and boot. No physics
equation or imported numerical source is changed for this mode.

models() adapts the model/body/stage composition in the local original reference
crates/rbr-render/src/playback/frames.rs::{model_to_body,physics_pose,car_to_stage}
and playback/mod.rs::{model_to_stage,apply}. The body offset uses initial physical
wheel centres and actual SGC pivots, as the upstream mean-centre alignment does.
Wheel centres reuse rbr_recording::pose::WheelPose::centre with the original hub
offset; strut travel/roll/axle fields retain their original layouts. The original
coords.rs remains unchanged; the core adapter merely exports its existing Quat type.

worker.rs, controls.rs, motion bridge, wall-clock scheduling, interpolation and HUD
layout are new app-owned GPL code. The ground/near-plane chase checks reuse the
existing geometric chase helper, without using its scripted vehicle path.
No commercial resource or captured drive is distributed. See PHYSICS-DRIVE.md for
partial-stage/first-gear scope, private inputs, Windows evidence and Apple limits.
