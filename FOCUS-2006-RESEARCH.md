# FOCUS-2006-RESEARCH.md — Ford Focus Mk II RS WRC 2006

Investigación de solo lectura en `C:\Richard Burns Rally`.

## 1. Paquete disponible

Según `rsfdata\cache\cars.json` y `carmodels.json`:
- **Nombre RSF**: `Ford Focus Mk II RS WRC 2006` (`id = 55`, `rev = 1 / 2019-11-15`)
- **Modelo 3D**: `carmodel_id = 17` -> `Cars\FORD_FOCUS_2006\focus06.sgc` (`3,456,099` bytes), autores `JRodrigues & eddy3spain`
- **Físicas**: `rsfdata\cars\Ford_Focus_Mk_II_RS_WRC_2006_ngp6\` (`ngp = 6`, ejecutado bajo `Plugins\NGP.dll` versión `7.5.779.508`)
- **Audio**: `AudioFMOD\Focus_Mk2_WRC.bank` (`11,956,704` bytes) y `AudioFMOD\Focus_Mk2_WRC.strings.bank` (`3,306` bytes)

## 2. Estado de cada subsistema y bloqueos confirmados

### A. Físicas (BLOQUEADO)
- El Ford Focus RS WRC 2006 **no existe** en `physics.rbz` stock (solo están `c_xsara`, `h_accent`, `mg_zr`, `m_lancer`, `p_206`, `s_i2000`, `s_i2003`, `t_coroll`).
- Sus físicas están en formato NGP (`common.lsp`, `gravel.lsp`, `tarmac.lsp`, `snow.lsp`, `r_*.lsp`, `setups/d_*.lsp`).
- Inspección con `rerbr_sim::ngp_candidate::inspect_car_pack`:
  - `gravel`: `car_id = 745426673`, `missing_ngp_keys = []`
  - `tarmac`: `car_id = 2929182514`, `missing_ngp_keys = []`
  - `snow`: `car_id = 2933989622`, `missing_ngp_keys = []`
  - `CarSpecs`: masa `1430.6151 kg`, potencia `231535.39 W` @ `5000 RPM`, tracción `All`, carga aerodinámica `1950.0` (`FrontDownforce_NGP = 900.0`, `RearDownforce_NGP = 1050.0`).
- **Bloqueo**: El motor actual de conducción (`rerbr-sim`) está fijado a NGP 7.4.778 con datos stock y **NGP 7.5 permanece bloqueado** hasta validación real. No se sustituyen las físicas del Focus por las del Corolla.

### B. Audio de motor (BLOQUEADO)
- El audio real del Focus (`Focus_Mk2_WRC.bank`) usa **FMOD Studio `.bank`** (con autores Keeb, Mikson, JJBruce, `readme.Focus_Mk2_WRC.txt`).
- El backend de audio actual en `rerbr-sim` / `src/driving/audio.rs` reproduce exclusivamente bancos originales `.eng` de RBR (`subaru.eng`, etc.), no bancos FMOD `.bank`.

### C. Modelo 3D y materiales (`focus06.sgc`) (BLOQUEADO)
- Al validar `Cars\FORD_FOCUS_2006\focus06.sgc` + `focus06.ini` + `focus06_shaders.ini` + `focus06_shader_settings_clear.ini` con `rerbr_probe::car::mesh::convert`, la conversión falla en:
  `CAR MATERIAL: SGC/INI mismatch on e_bodyi2`
- Esto se debe a que el modelo comunitario `focus06.sgc` utiliza convenciones de bloques/materiales distintas a las estrictas del cargador SGC/INI validado para el Corolla original.

## 3. Decisión para esta iteración

- El `VehicleDescriptor::ford_focus_2006()` se expone en el catálogo (`Garage` / `Content Manager`) como **`UNSUPPORTED`** con explicación explícita (`NGP 7.5 + FMOD BANK + SGC e_bodyi2`), manteniendo `SELECT VEHICLE` deshabilitado y `VehicleDescriptor::validate_current_backend()` bloqueando cualquier intento de activarlo.
- **Ruta segura de integración futura**:
  1. Certificar primero NGP 7.5 en el motor de físicas (`rerbr-sim`).
  2. Adaptar `car::mesh::convert` para materiales compuestos de modelos comunitarios (`e_bodyi2`, etc.).
  3. Añadir soporte FMOD o mapeo explícito de banco `.eng` si el usuario lo autoriza.
