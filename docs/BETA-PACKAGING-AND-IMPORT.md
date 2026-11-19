# Arquitectura de Empaquetado e Importación — Beta Comunitaria ReRBR

> **Estado:** Documento de diseño técnico (FASE 4). Define el flujo completo para pasar de copia manual de cientos de archivos a un paquete ZIP validado por el usuario desde su propia instalación legal de RBR/RSF.

---

## 1. Separación Legal y de Propiedad de Recursos

Para distribuir una beta pública (TestFlight / AltStore / SideStore / APK) sin infringir derechos de autor:

| Categoría | Ejemplos concretos | ¿Se incluye en IPA/APK pública? | Origen |
|---|---|---|---|
| **Código propio y port (GPL-3.0-or-later)** | `rerbr-probe`, `rerbr-sim`, `rerbr-asset-formats`, `rerbr-content`, UI, audio FMOD synth/Vorbis, `records.rs`, empaquetador Windows | **Sí** | Este repositorio |
| **Metadatos y documentación propios** | Esquemas de manifest, reglas de validación, documentación | **Sí** | Este repositorio |
| **Recursos propietarios de RBR 1.02** | `RichardBurnsRally_SSE.exe`, `physics.rbz`, `misc.rbz`, `Scripts.rbz`, `Maps/track-*`, `Cars/*`, `Audio/*`, `Sgs/*` | **NO (nunca)** | Instalación legal en Windows del tester |
| **Recursos de terceros RSF / NGP** | `Plugins/NGP.dll` (7.4.778.507), `rsfdata/cars/*`, `rsfdata/cache/*`, `AudioFMOD/*.bank`, modelos 3D de la comunidad | **NO (nunca)** | Instalación local RSF del tester |

---

## 2. Flujo de Instalación Sencilla en 2 Pasos

### Paso A — Empaquetador de Windows (`rerbr-pack`)
Herramienta CLI (y futura interfaz simple en Windows) ejecutada por el tester sobre su carpeta `C:\Richard Burns Rally\`:

1. **Escaneo automático de la instalación local:**
   - Verifica base RBR (`physics.rbz`, `misc.rbz`, `Scripts.rbz`, `season.dat`, `Cars/Cars.ini`, `Sgs/*_spectator_*.sgs`).
   - Verifica `Plugins/NGP.dll`: comprueba SHA-256 exacto (`95b6dbf5...` para NGP 7.4.778.507). Si solo encuentra NGP 7.5, avisa claramente y permite señalar una copia local de `NGP.dll 7.4` sin empaquetar una DLL bloqueada en silencio.
   - Detecta coches instalados (`Cars/*`, `rsfdata/cars/*`, `rsfdata/cache/{cars,carmodels}.json`, `rsfdata/cache/audiobank.ini`, `AudioFMOD/*.bank` + `.strings.bank`) aplicando las mismas reglas que `src/driving/rsf_catalog.rs`.
   - Detecta tramos instalados (`Maps/track-*`), resolviendo automáticamente los **archivos hermanos `*_textures.rbz`** (p. ej. `track-63_N_textures.rbz` para Diamond Creek I, `track-66_E_textures.rbz` para Diamond Creek II, `track-54_{E,M}_textures.rbz` para Tanner) y los espectadores por país (`ar_`, `au_`, `br_`, `mo_`, `ho_`, `us_`).
   - Ejecuta la auditoría de `src/harwood/stage_audit.rs` antes de empaquetar para descartar o marcar claramente mapas incompletos.
2. **Generación de `rerbr-import-bundle.zip`:**
   - Estructura normalizada y canónica:
     ```text
     manifest.json
     RBR/
       physics.rbz
       misc.rbz
       Scripts.rbz
       season.dat
       Plugins/NGP.dll
       Cars/...
       Sgs/...
       Maps/...
       Audio/...
       AudioFMOD/...
       rsfdata/cache/{cars.json,carmodels.json,audiobank.ini}
       rsfdata/cars/<pack>/...
     ```
   - `manifest.json` (esquema v1): versión de manifest, resumen de coches/tramos incluidos, estado de compatibilidad (`verified`, `partial`, `missing_files`), tamaño total descomprimido y tabla de `sha256` por archivo.

### Paso B — Importador en la App Móvil (iOS + Android)
1. **Selector nativo del sistema:**
   - **iOS:** `UIDocumentPickerViewController` (`UTType.zip`) en `xcode/` que entrega la URL security-scoped a Rust (`rerbr_import_zip_begin(path)`).
   - **Android:** Storage Access Framework (`Intent.ACTION_OPEN_DOCUMENT`, `application/zip`) leyendo vía file descriptor (`ParcelFileDescriptor`).
2. **Validación previa antes de escribir (sin corrupción):**
   - Leer `manifest.json` primero.
   - Validar cada entrada del ZIP con `rerbr_content::validate_relative_path` (bloquea `../`, rutas absolutas, symlinks y zip bombs por límite de bytes).
   - Mostrar pantalla de previsualización: coches detectados, tramos compatibles, espacio requerido y avisos de archivos faltantes.
3. **Extracción streaming en hilo secundario con progreso:**
   - Extraer cada archivo a `.tmp` y renombrar atómicamente hacia `Documents/RBR/...`.
   - Publicar progreso (`archivo N/M`, porcentaje, errores no fatales) en la UI sin bloquear el hilo gráfico.
   - **Preservación de datos del usuario durante actualizaciones e importaciones:** el importador **nunca** sobrescribe ni borra:
     - `Documents/RBR/personal_bests.json`
     - `Documents/RBR/championship_save.txt`
     - `Documents/RBR/school_progress.txt`
     - Ajustes de controles, cámaras, mando y pacenotes HUD
     - Repeticiones guardadas

---

## 3. Política de Configuración y Secretos (`.env` vs Cliente vs Servidor)

| Variable / Secreto | Dónde debe vivir | ¿Puede ir en la IPA/APK? | Motivo |
|---|---|---|---|
| `SUPABASE_URL`, `SUPABASE_ANON_KEY` | Config pública de compilación (inyectada en CI) | **Sí** (solo con RLS estricto) | Son identificadores públicos de proyecto; la seguridad recae en JWT + RLS, nunca en ocultar la anon key |
| `RERBR_API_BASE_URL`, `APP_ENV`, `RULES_VERSION` | Config pública de compilación | **Sí** | Parámetros de entorno no sensibles |
| `.env` local (tokens de prueba, rutas locales de RBR) | Solo máquina del desarrollador (`.gitignore`) | **NO** | Nunca subir `.env` a Git |
| `SUPABASE_SERVICE_ROLE_KEY`, `JWT_SECRET` | **Solo servidor** (Supabase Secrets / Edge Functions) | **NUNCA** | Da acceso administrador total saltándose RLS |
| Claves HMAC / firma del verificador de replays | **Solo servidor** | **NUNCA** | Cualquier secreto en el binario móvil puede extraerse por ingeniería inversa |
| Certificados Apple `.p12`, Provisioning Profiles, Keystore Android, tokens Codemagic/GitHub | **Solo secretos de CI** (Codemagic / GitHub Actions Secrets) | **NUNCA** | Credenciales de firma y publicación |
