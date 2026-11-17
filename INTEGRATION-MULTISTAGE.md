# Integración multitramos — revisión independiente

Base: `bf7f68f43f8bb2254824f1a7b60ee8f53d457c73`, versión probada en iPad.
El diff de Antigravity frente a su respaldo contiene 10 archivos de código/pruebas modificados y 3 informes añadidos. Se integró mediante comparación de tres versiones, conservando las últimas correcciones de interfaz, cámaras, pedales y ejes de ruedas. Ningún workspace de Antigravity fue modificado.

## Correcciones

- Se resolvió el conflicto de persistencia conservando `invert_pedals` junto con `stage_id`; prueba de serialización y restauración.
- El prototipo pasaba una lista vacía de triángulos de carretera a `Route::new`, y Chirdonhead también supera la separación máxima admitida por ese benchmark. Se conserva exactamente el recorrido de referencia de Harwood; los otros tramos usan `ChasePath::for_driving` para consultas de terreno y cámaras, sin inventar una spline. La física sigue leyendo los archivos originales LBS/DLS/TRK/COL/MAT/FNC seleccionados.
- Los mapas 41/46 arrancan con el neumático original de asfalto (0); 20/22 conservan el de grava (4). Cambio exclusivo del adaptador de arranque, sin modificar solver, fuerzas, constantes ni NGP.
- Se detectaron y declararon los cuatro `Sgs/mo_spectator_00.sgs` a `03.sgs` que requiere el arranque original de Montblanc (mapas 41/46). El catálogo los exige bajo `Documents/RBR` y el adaptador informa su ausencia antes de construir la sesión.
- El catálogo comprueba ambos paquetes de texturas originales y prefiere un directorio completo frente a otro que solo contiene LBS. Las comprobaciones de archivos ignoran mayúsculas, como los cargadores.
- Se rechazan cambios de selección mientras sigue cargándose la escena. Al cambiar, se detiene el worker anterior y se vacían pivotes, cámara, montajes, nodo interior y estado de dirección. Antes de preparar la nueva geometría se liberan referencias al terreno GPU, coche GPU, terreno CPU, frame y sesión anteriores. Una carga correcta limpia el estado fatal; la ausencia de Documents produce error explícito.
- El modo parcial `physics-drive` conserva únicamente Harwood. Los helpers de texturas siguen compilando en los modos anteriores.
- La selección persistida vuelve a Harwood si el identificador es inválido o faltan archivos. El cargador comprueba los contenidos reales; “archivos presentes” no certifica su integridad.

## Recursos privados para cada tramo

Se mantienen `Documents/Harwood/Maps`, `Documents/RBR/Maps` y la alternativa ya existente `Documents/Maps`; no se escribe, mueve ni elimina contenido privado. En uno de esos directorios se necesitan `<stem>.{lbs,col,trk,mat,dls,fnc,ini}`, `TextureFilenameMap.dat` y los dos archivos indicados:

| Tramo | ID / stem | Paquetes de texturas | Superficie / neumático |
| --- | --- | --- | --- |
| Harwood Forest | 20 / `track-20_O` | `track-20_O_textures.rbz`, `track-20_N_textures.rbz` | grava / 4 |
| Chirdonhead | 22 / `track-22_O` | `track-22_O_textures.rbz`, `track-22_N_textures.rbz` | grava / 4 |
| Côte d'Arbroz | 41 / `track-41_O` | `track-41_O_textures.rbz`, `track-41_M_textures.rbz` | asfalto / 0 |
| Côte d'Arbroz II | 46 / `track-46_O` | `track-46_O_textures.rbz`, `track-46_E_textures.rbz` | asfalto / 0 |

Para ambas Côte se requieren además `Documents/RBR/Sgs/mo_spectator_00.sgs`, `mo_spectator_01.sgs`, `mo_spectator_02.sgs` y `mo_spectator_03.sgs`. Son archivos privados originales; no se incluyen en Git ni en una IPA.

El Corolla y los archivos comunes de física/audio continúan en `Documents/RBR`. No se incorporan recursos comerciales al repositorio.

## Validación

- `cargo fmt --all -- --check`: correcto; `git diff --check`: correcto.
- Debug, `cargo test --offline --locked --all-features -p rerbr-probe -p rerbr-sim -p rerbr-content`: **131 aprobadas, 0 fallidas, 25 omitidas** (incluyen pruebas privadas no ejecutables en CI).
- Release, mismos paquetes con `--release --lib` y `CARGO_PROFILE_RELEASE_PANIC=unwind`, igual que CI: **87 aprobadas, 0 fallidas, 3 omitidas**.
- Pruebas privadas de `tests/stage.rs`: **3 aprobadas**. Lectura de geometría/texturas, comparación de alturas LBS/COL y preparación completa del Corolla en los cuatro tramos.
- Prueba privada `selected_stages_boot_step_and_repeat_without_stale_session_state` en release: **1 aprobada**, ocho arranques (dos pasadas por cuatro mapas), 3600 pasos por sesión. Comprueba archivos `.lsp` de la superficie, contacto, aceleración, estados finitos, igualdad exacta de muestras tras recarga y, en la primera pasada, soporte de terreno y encuadre de cámara con poses físicas inicial/final.
- Datos de pruebas leídos de la instalación privada y del respaldo; una fixture temporal fuera del repositorio combina NGP 7.4 original y los cuatro SGS de Montblanc. No se modificaron originales ni respaldos. El respaldo usado para las pruebas no contiene la textura interior opcional; su aviso no es una regresión de esta integración.
- Clippy `-D warnings`: correcto tanto para todos los targets/features de los tres paquetes como para la combinación exacta `physics-stage` de CI.
- Clippy/Rust para `aarch64-apple-ios`, `--lib --features physics-stage`: correcto. Es comprobación de código Rust; no se realizó enlace nativo con Xcode en Windows.
- `cargo check --all-targets` en modo predeterminado y `physics-drive`: correcto.
- `scripts/verify.py`: correcto, **623/623** hashes de procedencia (incluidas las cuatro adaptaciones preexistentes declaradas), manifiestos, referencias Xcode y exclusión de recursos privados.
- Solver, NGP, audio del motor, bundle ID, rutas Documents y workflows sin modificaciones. El Corolla sigue siendo un perfil experimental con sus avisos existentes.

## Archivos integrados

15 archivos de código/pruebas: `src/asset_probe.rs`, `src/car/{mod,placement,runtime}.rs`, `src/driving/{content_manager,menu,mod,private_tests}.rs`, `src/harwood/{gpu,stage}.rs`, `tests/{content,stage}.rs`, `vendor/rerbr-content/src/lib.rs` y `vendor/rerbr-sim/src/{drive,inputs}.rs`.

4 informes: los tres documentos del prototipo y esta revisión. Total: **19 archivos**.

## Límites y pruebas de hardware pendientes

- Ninguna certificación de paridad NGP. Se mantiene el gate de NGP 7.4.778; NGP 7.5 no se habilita. El Corolla conserva el aviso de física no certificada.
- Ford Focus RS WRC 2006 permanece UNSUPPORTED: física NGP sin validar, banco FMOD no compatible y material `e_bodyi2` sin conversión válida.
- Las comparaciones LBS/COL verifican alturas en puntos de ruta; no garantizan todos los contactos ni todos los colliders. Cielo, agua y animaciones conservan las limitaciones del render existente.
- Las estimaciones de 28–34 MiB de las escenas excluyen física, índices CPU, buffers del sistema y asignaciones del driver. La liberación lógica de recursos se revisó; el pico real y su devolución por Metal requieren iPad.
- Probar Harwood → Chirdonhead → ambas Côte → Harwood varias veces; verificar memoria, contactos, cámaras, ruedas, controles, sonido, pausa/reinicio y persistencia tras relanzar. Las ruedas visuales conservan el modelo de grava del Corolla existente.
- La validación local ARM64 de Rust no sustituye el enlace con Xcode ni un render en Metal. No se ha generado una IPA ni ejecutado Actions manualmente.
