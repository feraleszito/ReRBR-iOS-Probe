# ORIGINAL-RBR-CARS-RESEARCH.md — Todos los coches originales de Richard Burns Rally (PC base)

Investigación de solo lectura sobre la instalación original de `C:\Richard Burns Rally` (`Cars\cars.ini.rsfbak`, `physics.rbz`, `Audio\Cars\Cars.ini` y carpetas `Cars\*`), excluyendo por completo coches de RallySimFans, mods y paquetes NGP.

## 1. Listado exacto de los 8 vehículos base originales

| Slot (`Cars.ini`) | Vehículo original | Carpeta 3D (`Cars\`) | Carpeta física (`physics.rbz`) | Audio (`Audio\Cars\Cars.ini`) | Marchas (`NumberOfGears`) |
| --- | --- | --- | --- | --- | --- |
| `Car00` | **Citroën Xsara Kit Car** | `Cars\xsara\xsara.sgc` | `Physics\c_xsara\` | `Car0=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |
| `Car01` | **Hyundai Accent WRC** | `Cars\accent\accent.sgc` | `Physics\h_accent\` | `Car1=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |
| `Car02` | **MG ZR Super 1600** | `Cars\zr\zr.sgc` | `Physics\mg_zr\` | `Car2=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |
| `Car03` | **Mitsubishi Lancer Evo VII** | `Cars\Lancer_EVO_VII\lancer_evo_vII.sgc` | `Physics\m_lancer\` | `Car3=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |
| `Car04` | **Peugeot 206 WRC** | `Cars\Peugeot_206\Peugeot_206.sgc` | `Physics\p_206\` | `Car4=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |
| `Car05` | **Subaru Impreza 2003 (A8)** | `Cars\impreza03\impreza03.sgc` | `Physics\s_i2003\` | `Car5=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |
| `Car06` | **Toyota Corolla WRC** *(referencia)* | `Cars\Corolla\Corolla.sgc` | `Physics\t_coroll\` | `Car6=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |
| `Car07` | **Subaru Impreza 2000** | `Cars\impreza00\impreza00.sgc` | `Physics\s_i2000\` | `Car7=subaru` (`subaru.eng`) | 8 (R, N, 1..6) |

*(Nota: en `C:\Richard Burns Rally\Cars\Cars.ini`, RSF reemplazó únicamente `[Car06]` por `DS3_WRC`, pero `Cars\cars.ini.rsfbak` y `Cars\COROLLA\` conservan la configuración original completa de los 8 coches).*

## 2. Resultados de verificación técnica por vehículo (parsers reales de `rerbr-probe` y `rerbr-sim`)

Todos los modelos 3D, archivos INI, shaders, cámaras de capó/interior, volante interior (`i_steeringwheel`), físicas stock y mapeos de audio se verificaron directamente sobre `RBR_ROOT`:

| Slot | SGC + `car::mesh::convert` | Texturas faltantes | `Mounts::parse` (`Cam_bonnet2` / `Cam_internal`) | Nodo `i_steeringwheel` | Física en `physics.rbz` (`common`, `gravel`, `tarmac`, `snow`) | Audio `.eng` |
| --- | --- | --- | --- | --- | --- | --- |
| `Car00` (Xsara) | OK (`12248` verts, `96` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000714`) | `subaru.eng` |
| `Car01` (Accent) | OK (`11884` verts, `91` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000714`) | `subaru.eng` |
| `Car02` (MG ZR) | OK (`13042` verts, `104` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000869`, `DriveType 3` FWD) | `subaru.eng` |
| `Car03` (Lancer VII) | OK (`13440` verts, `115` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000714`) | `subaru.eng` |
| `Car04` (Peugeot 206) | OK (`13545` verts, `93` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000714`) | `subaru.eng` |
| `Car05` (Impreza 03) | OK (`13698` verts, `90` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000714`) | `subaru.eng` |
| `Car06` (Corolla) | OK (`11225` verts, `99` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000714`, `DriveType 0` AWD) | `subaru.eng` |
| `Car07` (Impreza 00) | OK (`13542` verts, `96` draws, `3` mats) | `0` | OK | `true` | Completo (`InverseMass 0.000714`) | `subaru.eng` |

## 3. Físicas auténticas y compatibilidad con el motor actual

1. **Sin dependencia de NGP 7.5 ni de físicas del Corolla**: Cada uno de los 8 vehículos tiene su propia carpeta independiente en `physics.rbz` (`c_xsara`, `h_accent`, `mg_zr`, `m_lancer`, `p_206`, `s_i2003`, `t_coroll`, `s_i2000`) con sus curvas de par, masa, inercia, suspensión, diferenciales, relación de dirección y tipo de tracción (`DriveType 3` FWD en MG ZR / Xsara vs `DriveType 0` AWD en los WRC).
2. **Carga en `rerbr-sim`**: `Session::boot_selected` ya recibe `selection.vehicle.original_slot` (`0..=7`), carga `Cars/Cars.ini`, arranca el slot original correspondiente (`boot::Race::quick_race`) y resuelve la carpeta física real del slot (`diagnostics::physics_folder(&image.mem, slot)`).
3. **Audio original**: `Audio\Cars\Cars.ini` asigna `subaru` + parámetros de `LowShelfAmp`/`LowShelfFrequency` a `Car0..Car7`, totalmente soportado por `vendor/rerbr-sim/src/engine_audio.rs` y `src/driving/audio.rs`.
4. **Ruedas y dirección**: Los 8 archivos `.sgc` siguen exactamente la misma convención de nodos `e_wheel_{tarmac,gravel,snow}_{lf,rf,lb,rb}`, `i_steeringwheel` y `e_steeringwheel`.

## 4. Problemas / requisitos detectados antes de habilitar en producción

1. **Dependencia de `Cars\Cars.ini` en `Documents/RBR`**: `car::load_vehicle` e `inputs::check_selection` exigen que `[CarNN]` en `Documents/RBR/Cars/Cars.ini` apunte al `.sgc` del vehículo seleccionado. Si el usuario copió un `Cars.ini` modificado por RSF (donde `[Car06]` apunta a `DS3_WRC`), `[Car06]` fallará, aunque `[Car00..Car05]` y `[Car07]` sigan intactos.
2. **Validación de sesión física en vivo (`Session::boot_selected`)**: En este entorno Windows solo está presente `Plugins\NGP.dll` `7.5.779.508` (bloqueada por política), por lo que no se ejecutó una sesión de conducción en vivo de 60 s con cada uno de los otros 7 slots bajo `NGP 7.4.778`. Por seguridad, solo `toyota-corolla-wrc` permanece habilitado por defecto en `validate_current_backend()`, mientras los otros 7 se exponen en el catálogo como candidatos documentados listos para validar por slot.

## 5. Orden recomendado de integración

1. **Subaru Impreza 2003 (`Car05`) y Subaru Impreza 2000 (`Car07`)**: AWD, misma masa/arquitectura base WRC que el Corolla, `MaxSteeringLock = 2.35`, sonido `subaru.eng` nativo.
2. **Peugeot 206 WRC (`Car04`) y Mitsubishi Lancer Evo VII (`Car03`)**: WRC AWD clásicos con `Mounts` e interior validados.
3. **Hyundai Accent WRC (`Car01`)**: WRC AWD completo.
4. **MG ZR Super 1600 (`Car02`) y Citroën Xsara Kit Car (`Car00`)**: tracción delantera (`DriveType 3`, 1150 kg en MG ZR); validar comportamiento de diferencial delantero y pérdida de tracción en `NGP 7.4.778` antes de activarlos al usuario final.
