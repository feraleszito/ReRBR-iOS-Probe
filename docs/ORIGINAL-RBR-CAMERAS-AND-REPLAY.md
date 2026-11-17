# Original Richard Burns Rally Cameras & Replay Engine

## 1. Camera Architecture in Richard Burns Rally Formats

### 1.1 Where Stage Cameras Are Stored
Stage TV and replay cameras in Richard Burns Rally are **not** stored in `.trk`, `.col`, `.ini` or script files. They reside entirely inside the stage's **AnimData (`.dls`)** file:
- **Table 2 (`table_offsets[2]`) — Trigger Nodes & Camera Mounting Posts:**
  - Stride: 56 bytes (14 words).
  - `ids: [u32; 2]` — Unique object ID pair (e.g. `[46308, 46308]`).
  - `spline_group: u32` / `spline_id: u32`.
  - `position: [f32; 3]` — 3D World position `(x, y, z)` in `.trk` coordinate space (where `z` is elevation).
  - `rotation_deg: [f32; 3]` — Euler rotation angles in degrees `(pitch, yaw, roll)`.
  - `word_10: u32` / `word_11: f32` / `words_12_13: [u32; 2]`.

- **Table 3 (`table_offsets[3]`) — Camera Definitions:**
  - Stride: 24 bytes (6 words).
  - `id: u32` — Links directly to `DlsTrigger.ids[0]` in Table 2 (100% matched across retail stages).
  - `look_at: u32` — Target mode:
    - `0xffffffff` (-1): Dynamically track the vehicle.
    - `0x0`: Fixed orientation / tangent tracking.
  - `words_2_3[0]: f32` — Field of View (FOV) in degrees (e.g., 10.0° - 80.0°).
  - `words_2_3[1]: f32` — Tracking lag / damping factor (0.0 to 0.4).
  - `mode: u32` — 0 = Tracking car, 1 = Static/orbit, 2 = Drive-by.
  - `extra: f32` — Priority / zoom multiplier (e.g., 1.0, 0.9, 0.5).

### 1.2 Coordinate System & Spatial Transformation
- In `.dls` and `.trk`, coordinates are z-up: `(x_right, y_forward, z_up)`.
- In `.lbs` and the iOS Metal stage renderer, coordinates are y-up: `(x_right, y_up, z_forward)`.
- Conversion: `coords::physics_to_stage()`, which maps `(x, y, z) -> (x, z, y)`.
- Target: The car's world transform `car_model.transform_point3(Vec3::ZERO)`.
- Camera eye: `coords::physics_to_stage().transform_point3(Vec3::from(cam_pos))`.
- Camera target: `car_render_pos + Vec3::Y * 0.75` (car centre of mass).
- Camera up: `Vec3::Y`.

### 1.3 Stage Correlation & Activation
Every camera post correlates to an arc distance along the stage driveline:
- **Harwood Forest (Track 20):** 32 cameras (from 44.8 m to 6,283.9 m).
- **Chirdonhead (Track 22):** 51 cameras (from 80.9 m to 7,272.3 m).
- **Côte d'Arbroz (Track 41):** 54 cameras (from 119.3 m to 4,768.7 m).
- **Côte d'Arbroz II (Track 46):** 52 cameras (from 135.0 m to 4,642.8 m).

As the car drives along the driveline, the replay engine selects the camera post that has optimal line-of-sight and proximity to the vehicle, maintaining camera continuity and authentic TV broadcast angles.

---

## 2. Replay Engine Architecture

### 2.1 Decoupled Recording
- Separate from the physics solver and simulation step.
- Records lightweight visual frames during `Phase::Driving`:
  - `time: f32`
  - `car_transform: Mat4`
  - `driveline_distance: f32`
  - `speed_mps: f32`, `engine_rpm: f32`, `steer_angle: f32`, `gear: i8`
  - `wheel_transforms: [Mat4; 4]`
- Memory footprint: ~80 bytes/frame * 60 fps * 300 s = ~1.44 MB for a full 5-minute stage.

### 2.2 Replay Playback
- Triggered after finishing a stage (`Phase::Finished`) from `STAGE COMPLETE` screen.
- State machine: `Playing`, `Paused`, `Finished`.
- Sub-frame interpolation for smooth 60/120 Hz playback on ProMotion displays.
- Selectable camera modes:
  1. `StageTV`: Authentic stage TV broadcast cameras from `.dls`.
  2. `Chase`: Dynamic third-person chase camera.
  3. `Bonnet`: Hood-mounted camera.
- Interactive on-screen touch controls: Play/Pause, Restart, Switch Camera, Exit.

---

## 3. Stage Catalog Expansion

### 3.1 Original RBR Stages Catalog
Retail RBR contains 36 original stages across 6 countries + Rally School.
All 30 tested maps in `C:\Richard Burns Rally\Maps` parse with 100% success:
- **Great Britain:** Harwood Forest (282 blocks), Falstone (193 blocks), Chirdonhead (229 blocks), Shepherds Shield (250 blocks), Harwood Forest II (282 blocks), Falstone II (242 blocks).
- **France (Mont Blanc):** Côte d'Arbroz (147 blocks), Joux Plane (158 blocks), Joux Verte (112 blocks), Bisanne (321 blocks), Joux Plane II (158 blocks), Côte d'Arbroz II (144 blocks).
- **Arctic (Finland):** Kaihuavaara (154 blocks), Mustaselkä (182 blocks), Sikakämä (257 blocks), Kaihuavaara II (177 blocks), Mustaselkä II (154 blocks), Sikakämä II (182 blocks).
- **USA:** Fraizer Wells (150 blocks), Diamond Creek (232 blocks), Hualapai Forest (139 blocks), Fraizer Wells II (332 blocks), Diamond Creek II (232 blocks), Hualapai Forest II (139 blocks).
- **Japan:** Noiker (357 blocks), Pirka Menoko (344 blocks), Tanner (183 blocks), Noiker II (131 blocks), Pirka Menoko II (357 blocks), Tanner II (130 blocks).

### 3.2 Compatibility & Safety Gates
- Validated reference: Harwood (20), Chirdonhead (22), Côte d'Arbroz (41), Côte d'Arbroz II (46).
- Extended original presets: Supported with experimental status, authentic block counts, and file verification.
- Unvalidated / non-original BTB mods: Blocked by schema validation.
