# Arquitectura Técnica — Rankings Mundiales Comunitarios ReRBR (iOS + Android)

> **Estado:** Documento de diseño técnico (FASE 3). No despliega ni conecta servicios externos sin autorización expresa.

---

## 1. Principios de Diseño

1. **Clasificaciones independientes y justas:** cada combinación de `(car_id, stage_id, stage_variant_id, physics_rules_version_id)` posee su propio ranking mundial en milisegundos enteros (`BIGINT`, orden ascendente). Nunca se mezclan tiempos obtenidos con distintas físicas (RBR retail 1.02 vs RSF NGP 7.4.778 vs futuro NGP 7.5), distintas variantes climáticas/humedad/desgaste, ni distintas reglas de carrera (p. ej. `clutch_help=0` vs `1`).
2. **Compartido entre iOS y Android:** ambos clientes producen exactamente el mismo `RecordKey` canónico (`src/driving/records.rs`) y cronometran estrictamente por pasos fijos de simulación a 720 Hz (`steps * dt`), nunca por FPS de renderizado.
3. **Separación estricta entre `provisional` y `verified`:** mientras no exista un verificador autoritativo desplegado con cobertura legal de los activos, los rankings deben separar resultados provisionales (validados por plausibilidad/telemetría/checkpoints) de resultados verificados deterministamente.
4. **Un mejor tiempo válido por jugador y combinación:** se conserva el historial completo en `runs`, y se materializa un único mejor tiempo válido por `(player_id, car_id, stage_variant_id, physics_rules_version_id)` en `personal_bests` y `leaderboard_entries`.

---

## 2. Stack Recomendado: Supabase (PostgreSQL 16 + Auth + Edge Functions + Storage)

- **Base de datos:** PostgreSQL 16 con Row Level Security (RLS) estricto.
- **Autenticación:** Supabase Auth con Apple Sign-In (obligatorio si se ofrece login en iOS), Google Sign-In (Android) y enlace opcional de cuenta anónima → cuenta verificada.
- **Ingesta segura:** Edge Function (`POST /functions/v1/submit-run`) o RPC `SECURITY DEFINER` que valida JWT, checkpoints, plausibilidad física, integridad del sobre de telemetría y escribe atómicamente en `runs` + `personal_bests` (el cliente **nunca** hace `INSERT`/`UPDATE` directo sobre `personal_bests` ni `leaderboard_entries`).
- **Almacenamiento de pruebas / replays:** Supabase Storage bucket privado `replay-proofs` con URLs firmadas de corta duración (`PUT` por run válida).

---

## 3. Esquema SQL Propuesto (PostgreSQL / Supabase)

```sql
-- 1. Jugadores (vinculados 1:1 a auth.users)
create table public.players (
  id uuid primary key references auth.users(id) on delete cascade,
  handle text not null unique check (char_length(handle) between 3 and 24),
  country_code char(2),
  created_at timestamptz not null default now(),
  banned_at timestamptz
);

-- 2. Catálogo de coches
create table public.cars (
  id text primary key,                -- ej: 'rsf-toyota-gr-yaris-rally2'
  name text not null,
  category text,
  rsf_pack text,
  pack_sha256 text,                   -- hash de common.lsp + gravel.lsp + tarmac.lsp + snow.lsp
  active boolean not null default true
);

-- 3. Catálogo de tramos
create table public.stages (
  id text primary key,                -- ej: 'diamond-creek'
  original_map integer,
  name text not null,
  country text not null,
  surface text not null check (surface in ('Gravel', 'Tarmac', 'Snow')),
  length_m integer not null check (length_m > 0),
  expected_splits smallint not null default 2
);

-- 4. Variantes de tramo (stem + clima + humedad + desgaste)
create table public.stage_variants (
  id text primary key,                -- ej: 'diamond-creek|track-63_O:w3:wet0:wear1'
  stage_id text not null references public.stages(id),
  stem text not null,
  weather_letter smallint not null,
  wetness smallint not null,
  wear smallint not null,
  dls_sha256 text,
  trk_sha256 text,
  unique (stage_id, stem, weather_letter, wetness, wear)
);

-- 5. Versiones de física y reglas (barrera estricta de comparación)
create table public.physics_rules_versions (
  id text primary key,                -- ej: 'rsf-ngp-7.4.778.507|rules-v1:manual:clutch_help=1'
  physics_backend text not null,      -- 'rbr-retail-1.02' | 'rsf-ngp-7.4.778.507'
  ngp_dll_sha256 text,
  rules_version text not null,        -- 'rules-v1:manual:clutch_help=1'
  sim_hz integer not null default 720,
  active boolean not null default true
);

-- 6. Intentos enviados (auditoría completa, inmutable)
create type public.verification_tier as enum ('rejected', 'provisional', 'verified');

create table public.runs (
  id uuid primary key default gen_random_uuid(),
  player_id uuid not null references public.players(id),
  car_id text not null references public.cars(id),
  stage_id text not null references public.stages(id),
  stage_variant_id text not null references public.stage_variants(id),
  physics_rules_version_id text not null references public.physics_rules_versions(id),
  platform text not null check (platform in ('ios', 'android')),
  app_version text not null,
  driving_ms bigint not null check (driving_ms > 0),
  penalty_ms bigint not null default 0 check (penalty_ms >= 0),
  total_ms bigint generated always as (driving_ms + penalty_ms) stored,
  false_start boolean not null default false,
  splits_ms bigint[] not null,
  splits_passed smallint not null,
  splits_total smallint not null,
  sim_steps bigint not null check (sim_steps > 0),
  verification public.verification_tier not null default 'provisional',
  rejection_reason text,
  submitted_at timestamptz not null default now()
);

create index idx_runs_player_submitted on public.runs (player_id, submitted_at desc);

-- 7. Metadatos de replay / prueba determinista
create table public.replay_proofs (
  run_id uuid primary key references public.runs(id) on delete cascade,
  storage_path text not null,
  payload_sha256 text not null,
  input_stream_bytes integer not null,
  rng_seed bigint not null,
  car_pack_sha256 text,
  stage_dls_sha256 text,
  verified_at timestamptz,
  verifier_version text,
  verifier_total_ms bigint,
  verifier_error text
);

-- 8. Mejores tiempos personales (1 fila válida por jugador + combinación + tier)
create table public.personal_bests (
  player_id uuid not null references public.players(id),
  car_id text not null references public.cars(id),
  stage_id text not null references public.stages(id),
  stage_variant_id text not null references public.stage_variants(id),
  physics_rules_version_id text not null references public.physics_rules_versions(id),
  tier public.verification_tier not null check (tier in ('provisional', 'verified')),
  run_id uuid not null references public.runs(id),
  best_total_ms bigint not null check (best_total_ms > 0),
  driving_ms bigint not null,
  penalty_ms bigint not null,
  splits_ms bigint[] not null,
  platform text not null,
  achieved_at timestamptz not null,
  primary key (player_id, car_id, stage_variant_id, physics_rules_version_id, tier)
);

-- Índice principal para consultas de ranking mundial en O(log N)
create index idx_pb_leaderboard
  on public.personal_bests (
    car_id,
    stage_variant_id,
    physics_rules_version_id,
    tier,
    best_total_ms asc,
    achieved_at asc
  );

-- 9. Vista de leaderboard con posición y delta respecto al líder
create view public.leaderboard_entries as
select
  pb.car_id,
  pb.stage_id,
  pb.stage_variant_id,
  pb.physics_rules_version_id,
  pb.tier,
  rank() over (
    partition by pb.car_id, pb.stage_variant_id, pb.physics_rules_version_id, pb.tier
    order by pb.best_total_ms asc, pb.achieved_at asc
  ) as position,
  pb.player_id,
  p.handle,
  p.country_code,
  pb.best_total_ms,
  pb.best_total_ms - first_value(pb.best_total_ms) over (
    partition by pb.car_id, pb.stage_variant_id, pb.physics_rules_version_id, pb.tier
    order by pb.best_total_ms asc, pb.achieved_at asc
  ) as gap_to_leader_ms,
  pb.driving_ms,
  pb.penalty_ms,
  pb.splits_ms,
  pb.platform,
  pb.run_id,
  pb.achieved_at
from public.personal_bests pb
join public.players p on p.id = pb.player_id
where p.banned_at is null;
```

---

## 4. Contrato de API (Edge Functions / REST)

| Endpoint | Método | Autenticación | Propósito |
|---|---|---|---|
| `/v1/runs/submit` | `POST` | Bearer JWT (Jugador) | Envía intento completado con `splits_ms`, `sim_steps`, hashes de activos y digest de entrada. Devuelve `run_id`, estado `provisional`/`rejected`, posición y URL firmada opcional para subir `replay_proof`. |
| `/v1/leaderboards` | `GET` | Pública / JWT opcional | Consulta paginada por `car_id`, `stage_variant_id`, `physics_rules_version_id`, `tier=provisional\|verified`, `limit`, `offset`, más la fila del jugador autenticado (`me`). |
| `/v1/players/me/personal-bests` | `GET` | Bearer JWT | Sincroniza PBs locales con los del servidor (resolución de conflictos por menor `best_total_ms`). |
| `/v1/runs/:run_id/proof` | `POST` | Bearer JWT | Confirma subida del stream de entradas comprimido (`zstd`/deflate) y encola verificación determinista si el verificador está habilitado. |

---

## 5. Seguridad, RLS y Protección Antitrampas

### 5.1 Row Level Security (RLS)
- `players`: lectura pública de `handle`/`country_code`; `UPDATE` restringido a `auth.uid() = id` (sin poder tocar `banned_at`).
- `runs`, `personal_bests`, `replay_proofs`: **cero escritura directa desde el cliente** (políticas `INSERT/UPDATE/DELETE` denegadas para `anon` y `authenticated`). Solo la Edge Function (usando `service_role` exclusivamente en servidor) puede insertar tras validar.
- `leaderboard_entries`: solo lectura (`SELECT`).

### 5.2 Defensa contra envíos arbitrarios (Capa 1 — Ranking `provisional`)
1. **Validación estructural estricta en servidor:**
   - `splits_passed == stages.expected_splits` y `splits_ms` estrictamente crecientes y `< driving_ms`.
   - `abs(sim_steps / 720.0 - driving_ms / 1000.0) <= 0.05s` (coherencia entre pasos 720 Hz y cronómetro).
   - Velocidad media por sector acotada físicamente (p. ej. rechazar medias imposibles `> 260 km/h` en tierra o `> 300 km/h` en asfalto).
   - Hash de `common.lsp`/`.dls`/`.trk` debe coincidir con el catálogo oficial del tramo/coche (impide alterar agarre o potencia en los `.lsp` locales).
2. **Atestación de dispositivo (opcional en beta avanzada):** Apple App Attest (iOS) y Google Play Integrity API (Android) para dificultar clientes falsificados o scripts `curl`.
3. **Rate limiting:** máx. 1 envío por cada 20 s por jugador y límite de envíos simultáneos.

### 5.3 Verificación determinista de replays (Capa 2 — Ranking `verified`)
- **Viabilidad técnica con `rerbr-sim`:**
  - El motor físico en `vendor/rerbr-sim` ejecuta un bucle determinista a **720 Hz fijo** (`dt = 1/720 s`) con una imagen de memoria inicial determinista (`boot::build` + `seed` + controles por paso `CarControl`).
  - Un stream de entradas delta-codificado para un tramo de 4 minutos (~172 800 pasos a 720 Hz) comprimido con `flate2`/`zstd` ocupa ~150–400 KB, totalmente viable para subir tras batir un récord.
- **Limitación técnica y legal clave (honestidad):**
  1. **Legal:** un verificador autoritativo en servidor necesita acceso a los archivos binarios de RBR/NGP (`RichardBurnsRally_SSE.exe`, `Plugins/NGP.dll`, `physics.rbz`, tramos y paquetes RSF) para re-simular. Alojar archivos propietarios de terceros en un servidor público **requiere permisos/licencia** o limitarse a un servidor privado de verificación sin redistribución de activos.
  2. **Determinismo entre arquitecturas (ARM64 vs x86_64):** las instrucciones FP de la emulación pure-Rust deben verificarse bit a bit entre ARM64 (iOS/Android) y x86_64/ARM64 Linux en el servidor (recomendable ejecutar el worker verificador en **Linux ARM64** para idéntica semántica FMA/IEEE-754).
  3. **Decisión de beta:** lanzar primero con `tier = 'provisional'` (validando hashes de `.lsp`/`.dls`/`.trk`, checkpoints y cotas físicas + guardando trazas de replay) y activar `tier = 'verified'` solo cuando el worker ARM64 esté validado y con cobertura legal clara.

---

## 6. Costes Estimados (sin desplegar todavía)

| Escenario | Usuarios activos/mes | Supabase | Worker verificador (opcional) | Coste mensual aprox. |
|---|---:|---|---|---|
| **Beta cerrada** | 50–300 | Plan Free (500 MB DB, 1 GB Storage, 500k Edge inv.) | No activo (solo `provisional`) | **\$0 / mes** |
| **Beta comunitaria** | 1 000–5 000 | Supabase Pro (8 GB DB, 100 GB Storage, 2M Edge inv.) | 1× VPS Linux ARM64 4 vCPU (Hetzner CAX21 ~\$7) | **\$25 – \$35 / mes** |
| **Comunidad consolidada** | 10 000+ | Supabase Pro + compute Small (\$15–\$50 extra) | 2× workers ARM64 + cola | **\$60 – \$120 / mes** |
