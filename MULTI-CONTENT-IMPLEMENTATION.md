# MULTI-CONTENT-IMPLEMENTATION.md — Arquitectura, cambios y pruebas

> Informe del prototipo aislado. La integración corrige bloqueos; véase [INTEGRATION-MULTISTAGE.md](INTEGRATION-MULTISTAGE.md) para el estado final y las pruebas independientes.

Implementación aislada en `C:\Users\adria\.gemini\antigravity\scratch\ios-multi-content-work` (respaldo limpio en `ios-multi-content-backup`).

## 1. Arquitectura de catálogo y selección

- **Descriptores (`vendor/rerbr-content/src/lib.rs`)**:
  - `StageDescriptor::presets()` define 4 tramos verificados: `harwood_forest` (`track-20_O`, 282 bloques), `chirdonhead` (`track-22_O`, 229 bloques), `cote_darbroz` (`track-41_O`, 147 bloques) y `cote_darbroz_ii` (`track-46_O`, 144 bloques).
  - `StageDescriptor::validate_current_backend()` acepta cualquiera de los 4 presets en `Harwood/Maps`, `RBR/Maps` o `Maps`.
  - `VehicleDescriptor::presets()` expone `toyota_corolla_wrc` (habilitado) y `ford_focus_2006` (bloqueado por `validate_current_backend()`).
- **Descubrimiento automático (`src/driving/content_manager.rs`)**:
  - `Catalog::scan_root` y `resolve_stage_maps_dir` buscan automáticamente los archivos del tramo en `Documents/Harwood/Maps`, `Documents/RBR/Maps` y `Documents/Maps` (sin rutas absolutas de Windows), verifican los 6 archivos físicos (`PHYSICS_STAGE_EXTENSIONS`) + `<stem>.ini` + `TextureFilenameMap.dat` y reportan `Selected`, `Installed`, `MissingFiles` o `Unsupported`.
- **Cargador de tramos dinámico (`src/asset_probe.rs`, `src/harwood/stage.rs`, `src/car/mod.rs`)**:
  - `asset_probe` añade `read_stage_stem`, `diffuse_names_stem` y `load_diffuse_shared_stem` parametrizados por `&stage.stem` en lugar de depender únicamente de `track-20_O`.
  - `car::prepare_selected` bajo `physics-stage` construye `Route` directamente desde `full.route_points` y `full.render.mesh` de `stage::load_selected`, evitando el benchmark exclusivo de 20 bloques de Harwood.
- **Menús, persistencia y recarga de escena (`src/driving/menu.rs`, `src/driving/mod.rs`, `src/harwood/gpu.rs`)**:
  - `Page::Stages` lista todos los tramos del catálogo con su estado; `Page::StageDetail` muestra metadatos, rutas y archivos presentes/faltantes, y permite seleccionar cualquier tramo instalado y validado.
  - `Page::Garage` lista Corolla (`SELECTED`) y Ford Focus 2006 (`UNSUPPORTED`), bloqueado en `Page::GarageDetail`.
  - `Settings` persiste `stage_id` en `rerbr-driving-settings.txt` (con respaldo automático a `harwood-forest` si falta algún archivo o no es válido).
  - Al cambiar de tramo en el menú, `Runtime::configure_selection` reinicia el worker de física y señala `pending_scene_reload` para que `harwood::gpu::Scene` recargue geometría, texturas y coche en segundo plano sin recompilar la IPA.

## 2. Archivos modificados

- `vendor/rerbr-content/src/lib.rs`
- `src/driving/content_manager.rs`
- `src/driving/menu.rs`
- `src/driving/mod.rs`
- `src/asset_probe.rs`
- `src/harwood/stage.rs`
- `src/harwood/gpu.rs`
- `src/car/mod.rs`
- `tests/content.rs`
- `tests/stage.rs`

## 3. Limitaciones conocidas

- **Ford Focus RS WRC 2006**: permanece bloqueado (`UNSUPPORTED`) por depender de NGP 7.5, audio FMOD `.bank` y material `e_bodyi2` no compatible en `car::mesh::convert`.
- **Clima**: los presets multi-tramo utilizan la variante seca `_O` (`weather_letter = 3`, `wetness = 0`, `wear = 1`), igual que Harwood Forest.
- **Física en vivo (`Session::boot_selected`)**: en Windows no se ejecutó una sesión de conducción completa sobre Chirdonhead/Côte d'Arbroz porque la instalación local usa `Plugins\NGP.dll` `7.5.779.508` (bloqueada por diseño; la consola iOS/Documents usa NGP 7.4.778). Se debe probar en dispositivo iPadOS con los archivos de `RBR/Maps` copiados a `Documents`.
