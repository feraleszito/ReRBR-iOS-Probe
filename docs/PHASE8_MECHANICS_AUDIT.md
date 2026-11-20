# RBR Mobile — Fase 8: Auditoría Técnica de Neumáticos, Desgaste, Daños, Asistencia y Economía

## 1. Capacidades Reales del Motor NGP (`rerbr-sim` / `rbr-gameplay`)

### 1.1 Daños Mecánicos Reales (`CarriedDamage`)
- **Fuente real en simulación:** `rbr_gameplay::car_damage::CarriedDamage` (`vendor/rerbr-sim/upstream/rbr-gameplay/src/car_damage.rs`) lee directamente la memoria de `DAMAGE_HANDLER` del coche activo mediante `Session::carried_damage(&self)` (`vendor/rerbr-sim/src/drive.rs`).
- **Resumen expuesto a Carrera:** `Session::damage_summary(&self) -> DamageSummary` evalúa `car_damage::level(d, id, Rules::Ngp)` (`0` intacto .. `5` destruido) agrupado en:
  - **Motor / Refrigeración / Turbo** (`engine_pct`): IDs `10..=13, 20..=23`
  - **Transmisión / Caja / Embrague / Diferenciales** (`transmission_pct`): IDs `30..=33, 35, 36, 90..=92`
  - **Suspensión / Dirección / Frenos / Ruedas** (`suspension_pct`): ID `37` + IDs por rueda `101..=171` (partes `0, 1, 2, 3, 4, 7, 9, 10`)
  - **Carrocería / Eléctrico** (`body_electrics_pct`): IDs `28, 29, 34, 40..=47, 50..=52`
  - **Daño de rueda y neumático NGP por esquina:** `ngp_wheel_damage_levels: [u8; 4]` (parte `4`) y `ngp_tyre_damage_levels: [u8; 4]` (parte `5`).
  - **Arrastre entre etapas:** `DamageSummary::raw_bytes` se serializa como hexadecimal en `VehicleCondition::carried_damage_hex` y se reinyecta en `BootOverrides::carried_damage_bytes` -> `Race.damage` al iniciar la siguiente etapa.

### 1.2 Neumáticos Reales (`Race.tyre`)
- `rerbr_sim::boot::Race` soporta selección real de compuesto mediante `tyre: u32`:
  - `0` = Asfalto Seco/Duro (`TarmacHard`)
  - `1` = Asfalto Intermedio (`TarmacIntermediate`)
  - `2` = Asfalto Lluvia (`TarmacWet`)
  - `3` = Grava Seca/Dura (`GravelHard`)
  - `4` = Grava Intermedia (`GravelIntermediate`)
  - `5` = Grava Barro/Lluvia (`GravelWet`)
  - `6` = Nieve/Clavos (`Snow`)
- **Limitación física documentada:** RBR/NGP no expone un porcentaje continuo de desgaste de banda de rodadura en `CarriedDamage` más allá de los niveles discretos de daño de neumático NGP (`0..=4`). Por fidelidad a las físicas originales de 720 Hz, **no se inventan multiplicadores artificiales de agarre ni de potencia**. El desgaste porcentual (`tyre_wear_pct: 0..=100`) se calcula de forma determinista tras cada etapa según la longitud real del tramo (`length_km`), compatibilidad neumático/superficie y nivel real `ngp_tyre_damage_levels` reportado por NGP.

---

## 2. Modelo Persistente y Migración (`CareerProfile` v1 -> v2)

- `CAREER_SCHEMA_VERSION = 2` (`src/driving/career.rs`).
- `parse_profile_json` acepta `schema_version` `1` y `2`:
  - Los perfiles v1 existentes en `career_slot_1.json` .. `career_slot_3.json` cargan sin pérdida de progreso ni dinero.
  - Todo vehículo ya poseído en v1 que no tenga entrada en `vehicle_states` se inicializa automáticamente al **100% mecánico, 0% desgaste, 0 daños** (`OwnedVehicleState::factory_new`), sin penalizar retrospectivamente partidas previas.
- Separación limpia para futuras fases online/comunidad: `OwnedVehicleState` incluye `setup_profile_id: Option<String>` y `livery_id: Option<String>` desacoplados de la economía y el estado mecánico.

---

## 3. Concesionarios, Catálogo Completo y Estado Nuevo vs Usado

- **`SALÓN AUTOMOTOR (0 KM)`:** Entrega `OwnedVehicleState` con `VehicleOrigin::NewZeroKm` (100% motor, transmisión, suspensión, carrocería, 0% desgaste de neumáticos, 0 daños).
- **`CONCESIONARIO 2ª MANO (USADO)`:** Genera una unidad determinista y reproducible (`UsedCarListing`) con estado mecánico coherente (64%–93%), desgaste de neumáticos (24%–57%), kilometraje, coste estimado de reparación y advertencias visibles antes de confirmar la compra.
- **Visibilidad completa del catálogo:** `DealershipFilter::All` es ahora el filtro por defecto y muestra todos los vehículos del concesionario (incluyendo categorías superiores y coches bloqueados) diferenciando claramente:
  - `DISPONIBLE`
  - `EN GARAJE`
  - `DINERO INSUFICIENTE`
  - `BLOQUEADO: <requisito exacto>` (p. ej. `REQUIERE PODIO EN COPA REGIONAL`, `PENDIENTE DE REGLA COMERCIAL`, `SOLO VISUALIZACIÓN 3D`).

---

## 4. Parque de Servicio, Costes Escalados y Restauración Final

- **Tabla centralizada de costes por categoría (`CareerTier::maintenance_multiplier_pct`):**
  - `Regional`: `100%` (1.00x)
  - `Rally3`: `145%` (1.45x)
  - `KitCarGroupA`: `160%` (1.60x)
  - `Rally2`: `220%` (2.20x)
  - `Historic`: `250%` (2.50x)
  - `Wrc`: `320%` (3.20x)
- **Ventana legal de asistencia (`can_service_vehicle`):**
  - En campeonato activo, solo se permiten reparaciones y cambio de neumáticos cuando `service_window_open == true` (entre etapas) y únicamente sobre `locked_vehicle_id`.
  - Tras la etapa final, se calcula `end_restoration_cost_usd` **solo sobre los daños restantes** (sin doble cobro por lo ya reparado en servicio) y el jugador elige explícitamente `RESTAURAR COCHE Y FINALIZAR` o `FINALIZAR SIN RESTAURAR`.

---

## 5. Campeonato Histórico Desbloqueado

- Al lograr podio (`position <= 3`) en `Copa Regional de Iniciación`, `historics_unlocked` pasa a `true`.
- **Causa raíz corregida:** Anteriormente `menu.rs` solo mostraba el texto `"HISTÓRICOS DESBLOQUEADOS"` pero `Page::CareerChampionship` estaba fijado únicamente a `REGIONAL_STAGES` y `DealershipFilter::Initiation` ocultaba los coches históricos.
- **Solución:** `ChampionshipKind::{Regional, Historic}` añade el **Campeonato Histórico de Rally** (`Joux Verte`, `Harwood Forest`, `Kaihuavaara` — Asfalto, Grava y Nieve) elegible para `CareerTier::{Historic, KitCarGroupA}`, con rivales y premios propios (`HISTORIC_BASE_PRIZES_USD`), y los coches históricos son visibles en `2ª MANO` desde el inicio (con insignia `BLOQUEADO: REQUIERE PODIO EN COPA REGIONAL` hasta desbloquearlos).

---

## 6. Modo Desarrollador Aislado (`career-dev-mode`) y Selector en Codemagic

- **Selector nativo de workflows en `codemagic.yaml`:**
  1. `rerbr-probe-unsigned` (**`RBR Mobile — RELEASE (iPadOS unsigned)`**): `CAREER_DEV_MODE="false"`, excluye `career-dev-mode`, verifica mediante `cargo tree` que la feature no esté activa y genera `build/RBR-Mobile-Release.ipa`.
  2. `rerbr-probe-dev` (**`RBR Mobile — DEVELOPER QA (US$9,999,999,999)`**): `CAREER_DEV_MODE="true"`, compila con `--features physics-stage,career-dev-mode` tras limpiar el artefacto previo de `rerbr-probe` y genera `build/RBR-Mobile-Developer.ipa`.
- **Aislamiento total de partidas:**
  - Carga y guarda exclusivamente en `Documents/RBR/career_dev_slot.json`.
  - `CareerStore::save()` y `CareerStore::save_to_root()` interceptan cualquier perfil con `is_dev_profile == true` para que jamás pueda escribirse en `career_slot_1..3.json`.
  - En compilaciones `RELEASE`, `parse_profile_json` fuerza `is_dev_profile = false` y `dev_unlock_all = false`, impidiendo privilegios DEV incluso si se manipula un JSON manualmente.
- **Acceso directo e controles exclusivos en la app DEV (`#[cfg(feature = "career-dev-mode")]`):**
  - Distintivo visual discreto `RBR MOBILE — DEV BUILD` en el encabezado del menú principal y pantallas de Carrera (ausente en conducción y en `RELEASE`).
  - Acceso en 1 toque a `PERFIL DESARROLLADOR QA (US$9,999,999,999)` desde `CARRERA` (`CareerHub`, `CareerCreate` y `CareerSlots`).
  - Botón `RESTABLECER SALDO DEV (US$9,999,999,999)` que restaura el saldo sin borrar los coches comprados en QA ni afectar partidas normales.
  - Botón `DESBLOQUEOS QA: ACTIVADOS / DESACTIVADOS` para alternar el desbloqueo completo de campeonatos y catálogo en pruebas.
