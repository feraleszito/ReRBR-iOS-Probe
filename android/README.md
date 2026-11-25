# RBR Mobile para Android — Primer Tramo Jugable (`OneCar-OneStage`)

Esta edición compila el juego compartido (`public-beta`, `aarch64-linux-android`, Vulkan 1.1) directamente sobre `port/android-full-game`, incorporando:

- **Selector nativo Android SAF dentro del juego** (`AJUSTES -> CONTENIDO -> IMPORTAR RECURSOS RBR`, banner directo en Menú Principal y botón en Garaje cuando faltan recursos):
  - **Seleccionar carpeta `RBR` (`ACTION_OPEN_DOCUMENT_TREE`)**: conserva permiso de lectura persistente (`takePersistableUriPermission`), navega exclusivamente por `DocumentsContract` / `ContentResolver` (nunca asume que un `content://` URI sea una ruta POSIX) y copia de forma selectiva **únicamente los archivos necesarios** (~96 MiB para 1 coche + 1 tramo) al almacenamiento privado de la aplicación (`files/RBR`), evitando duplicar los ~10,54 GB de una instalación completa.
  - **Importar paquete mínimo `.zip` (`ACTION_OPEN_DOCUMENT`)**: ideal para **BrowserStack App Live** o dispositivos remotos donde subir un único `.zip` de ~47 MiB es inmediato.
  - **Reimportar carpeta autorizada**: reutiliza el permiso persistente guardado sin volver a navegar el árbol si se actualiza el contenido.
- **Diagnóstico exacto de recursos faltantes**: distingue ausencia de `Plugins/NGP.dll`, bloqueo explícito de `NGP 7.5.779` frente a `NGP 7.4.778.507`, tramos incompletos (como la ausencia del archivo compartido `track-20_N_textures.rbz`) y bancos FMOD.
- **Dirección táctil opcional (`< IZQ` / `DER >`) activa por defecto en Android** además del giroscopio/acelerómetro, permitiendo girar el vehículo también en dispositivos remotos planos (BrowserStack) sin inyección de inclinación.

---

## 1. Crear el paquete mínimo (~95,7 MiB / 46,7 MiB en `.zip`) desde tus archivos legítimos

El script `scripts/extract-minimal-pack.py` opera en **modo estrictamente solo lectura** sobre tu instalación original de RBR (verifica tamaño y `mtime` antes y después) y rechaza escribir dentro del repositorio git.

> **Importante sobre NGP:** RSF actual incluye `NGP 7.5.779.508`, mientras que el motor verifica exactamente **`NGP 7.4.778.507`** (`SHA-256: 95b6dbf5e8957357d857a8e919cff177a6bfb832f5324da90467d43732edaaa4`). Pasa `--ngp74-root` apuntando a tu carpeta `RBR_NGP-7.4.778.507` para combinarla limpiamente sin tocar tu instalación de RBR.

En Windows PowerShell:

```powershell
python scripts\extract-minimal-pack.py `
  --rbr-root "C:\Richard Burns Rally" `
  --ngp74-root "C:\Users\adria\Downloads\RBR_NGP-7.4.778.507" `
  --output "$HOME\Downloads\rbr-android-onecar-onestage" `
  --vehicle ford-focus-rs-wrc-2006 `
  --stage harwood-forest `
  --zip
```

Esto genera fuera del repositorio:

- `$HOME\Downloads\rbr-android-onecar-onestage\RBR\` (63 archivos, **95,70 MiB**)
- `$HOME\Downloads\rbr-android-onecar-onestage.zip` (**46,67 MiB**, listo para subir en un solo paso)
- `$HOME\Downloads\rbr-android-onecar-onestage\MANIFEST.json` (inventario completo con SHA-256)

---

## 2. Instalar el APK y cargar los recursos en Android

### Opción A — BrowserStack App Live (sin ADB)

1. Sube e instala **`RBR-Mobile-Android-OneCar-OneStage.apk`** en un dispositivo Android 11+ ARM64 con soporte Vulkan 1.1.
2. En la barra de herramientas de BrowserStack (**Files / Upload Media**), sube el archivo **`rbr-android-onecar-onestage.zip`** (46,67 MiB) al dispositivo (se guarda en `Download/`).
3. Abre **RBR Mobile**, toca la pantalla de título para entrar al **Menú Principal** y pulsa el aviso superior **`FALTAN RECURSOS (...) · PULSA PARA IMPORTAR`** (o entra en **`AJUSTES -> CONTENIDO -> IMPORTAR RECURSOS RBR`**).
4. Pulsa **`IMPORTAR PAQUETE MÍNIMO .ZIP (ANDROID SAF)`** y selecciona `rbr-android-onecar-onestage.zip` en `Downloads` *(o si descomprimiste la carpeta `RBR`, pulsa **`SELECCIONAR CARPETA RBR (SELECTOR ANDROID SAF)`** y autoriza la subcarpeta `RBR`)*.
5. Al terminar la barra de progreso, el catálogo y el **Showroom 3D** cargan en caliente el **Ford Focus RS WRC 2006** y **Harwood Forest**.
6. Pulsa **`INICIAR TRAMO`** (arriba a la derecha en el Menú Principal) o entra en **`CONTRARRELOJ -> INICIAR RALLY`**.

### Opción B — Teléfono Android físico (Selector SAF en el dispositivo)

1. Copia la carpeta `RBR` (o `rbr-android-onecar-onestage.zip`) a una subcarpeta del teléfono, por ejemplo `Documentos/RBR` o `Download/RBR_Pack/RBR`.
2. Instala `RBR-Mobile-Android-OneCar-OneStage.apk`, abre el juego y pulsa **`SELECCIONAR CARPETA RBR (SELECTOR ANDROID SAF)`**.
3. Elige la carpeta `RBR` y confirma **Usar esta carpeta / Permitir**. El permiso queda guardado de forma persistente y solo se copian los archivos requeridos a `Android/data/org.rerbr.mobile/files/RBR`.

### Opción C — Transferencia directa por USB (`adb push`)

```powershell
adb install -r RBR-Mobile-Android-OneCar-OneStage.apk
adb shell am start -n org.rerbr.mobile/.GameActivity
adb shell am force-stop org.rerbr.mobile
adb push --sync "$HOME\Downloads\rbr-android-onecar-onestage\RBR" /sdcard/Android/data/org.rerbr.mobile/files/
adb shell am start -n org.rerbr.mobile/.GameActivity
```

---

## 3. Conducción y controles en el tramo

- **Acelerar / Frenar**: pedales táctiles progresivos **`THROTTLE`** (derecha) y **`BRAKE`** (izquierda).
- **Cambiar de marcha**: **`GEAR +`** / **`GEAR -`** (para salir en el semáforo sube de `N` a `1`).
- **Girar**:
  - **Inclinación física** del teléfono en horizontal (gravedad / acelerómetro).
  - **Botones táctiles `< IZQ` y `DER >`** junto al cuadro central (activos por defecto en Android para poder girar también en BrowserStack; conmutables en `AJUSTES -> CONTROLES`).
- **Encender el motor** (si se cala): botón de **encendido (icono power)** en la barra superior derecha (`Control::aux = 1`).

---

## 4. Lista exacta de recursos requeridos (1 vehículo + 1 tramo = 63 archivos)

Todos relativos a `files/RBR/`:

### Núcleo compartido y físicas NGP 7.4 (13 archivos)
- `Plugins/NGP.dll` *(obligatorio NGP 7.4.778.507, SHA-256 `95b6dbf5...`)*
- `physics.rbz` *(revisión 778 compatible con NGP 7.4)*
- `misc.rbz` *(`Misc/prime.lsp`, `Misc/nice_w.ini`, `Misc/bad_w.ini`)*
- `Scripts.rbz` *(`Scripts/pirelli/load_car_flip1..5.lua`)*
- `Cars/Cars.ini`
- `Sgs/br_spectator_00.sgs`, `Sgs/br_spectator_01.sgs`, `Sgs/br_spectator_02.sgs`, `Sgs/br_spectator_03.sgs`
- `Generic/PC_Generic_1280.bnk`, `Generic/PC_1280_01.dds`, `Generic/PC_1280_05.dds` *(iconos visuales de notas; opcional)*
- `season.dat` *(opcional)*

### Vehículo: Ford Focus RS WRC 2006 (40 archivos)
- **Geometría, partes y shaders (7 archivos)**:
  - `Cars/FORD_FOCUS_2006/focus06.sgc`
  - `Cars/FORD_FOCUS_2006/focus06.ini`
  - `Cars/FORD_FOCUS_2006/focus06_shaders.ini`
  - `Cars/FORD_FOCUS_2006/focus06_shader_settings_clear.ini`
  - `Cars/FORD_FOCUS_2006/focus06_shader_settings_overcast.ini`
  - `Cars/FORD_FOCUS_2006/focus06_settings.ini`
  - `Cars/FORD_FOCUS_2006/focus06_personal.ini`
- **Texturas DDS exterior + cockpit interior (15 archivos)**:
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_external.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_dynamic.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_dynamic_mud.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_lightcone.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_mud.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_scrape.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_transparent.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_extra_lights.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_kwindscreen.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_scrape.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_transparent.dds`
  - `Cars/FORD_FOCUS_2006/textures/focus_wrc06_parts.dds`
  - `Cars/FORD_FOCUS_2006/textures/Light.dds`
  - `Cars/FORD_FOCUS_2006/textures/Light2.dds`
- **Pack de físicas NGP6 y metadatos RSF (16 archivos)**:
  - `rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6/{common.lsp, gravel.lsp, tarmac.lsp, snow.lsp, r_gravel.lsp, r_tarmac.lsp, r_snow.lsp, repair.ini, Ford Focus Mk II RS WRC 2006}`
  - `rsfdata/cars/Ford_Focus_Mk_II_RS_WRC_2006_ngp6/setups/{d_gravel.lsp, d_tarmac.lsp, d_snow.lsp}`
  - `rsfdata/cache/{cars.json, carmodels.json, cargroups.json, audiobank.ini}`
- **Audio FMOD Studio (2 archivos)**:
  - `AudioFMOD/Focus_Mk2_WRC.bank`
  - `AudioFMOD/Focus_Mk2_WRC.strings.bank`

### Tramo: Harwood Forest (`track-20_O`, 10 archivos)
- `Maps/TextureFilenameMap.dat`
- `Maps/track-20_O.lbs`, `Maps/track-20_O.col`, `Maps/track-20_O.trk`, `Maps/track-20_O.mat`, `Maps/track-20_O.dls`, `Maps/track-20_O.fnc`, `Maps/track-20_O.ini`
- `Maps/track-20_O_textures.rbz`
- `Maps/track-20_N_textures.rbz` *(imprescindible: `TextureFilenameMap.dat` redirige las texturas compartidas del tramo a este archivo)*

---

## 5. Compilación y verificación CI

El workflow `.github/workflows/android-full-game.yml` compila `librerbr_probe.so` (`aarch64-linux-android`, `--locked --features public-beta`), empaqueta `RBR-Mobile-Android-OneCar-OneStage.apk` con SDK 35 + NDK, verifica firma (`apksigner`), alineación de 16 KiB (`zipalign -P 16`), actividad de arranque (`aapt`), ausencia total de activos comerciales y exportación de los 6 puntos de entrada JNI (`android/inspect-apk.py`).
