# Investigación Técnica y Hoja de Ruta — Port Android (ARM64 / Vulkan)

> **Estado:** Investigación y auditoría técnica sobre el código real del repositorio (FASE 5). **No** se ha modificado ni tocado el proyecto iOS (`xcode/`, `Cargo.toml`, `Cargo.lock`). No se afirma compatibilidad Android ejecutada hasta probar un APK en dispositivo físico.

---

## 1. Resumen Ejecutivo de Viabilidad

El port a **Android ARM64 (`aarch64-linux-android`)** es **técnicamente muy viable y sorprendentemente limpio** porque más del **95 % del motor es Rust puro multiplataforma**:
- `rerbr-sim` (física a 720 Hz + host puro NGP 7.4): **0 dependencias de Apple**.
- `rerbr-asset-formats`, `rerbr-content`, `rerbr-render-core`, `harwood/*`, `car/*`: **0 dependencias de Apple**.
- `fmod_synth`, `vorbis`, `fsb5_vorbis_setup`, `codriver`, `records`, `replay`, `championship`, `school`, `rsf_catalog`: **0 dependencias de Apple**.

---

## 2. Auditoría Exacta de Dependencias Específicas de Apple en el Código

Búsqueda exhaustiva de `target_os = "ios"` y `extern "C"` en `src/`:

| Componente | Archivo actual | Acoplamiento iOS actual | Equivalente Android (ARM64) |
|---|---|---|---|
| **Backend GPU (`wgpu 30.0.1`)** | `Cargo.toml:41`, `src/lib.rs` | `wgpu` compilado con feature `metal` únicamente y `Backends::METAL` | Activar feature `vulkan` en target Android (`[target.'cfg(target_os = "android")'.dependencies]`) y seleccionar `Backends::VULKAN` en `src/lib.rs` |
| **Entrypoint & EventLoop (`winit 0.30.13`)** | `src/lib.rs:55` (`rerbr_probe_main`) | `xcode/ReRBRProbe/main.m` invoca `rerbr_probe_main()` en hilo principal UIKit | `android-activity` (`game-activity` recomendado para baja latencia táctil/gamepad) + `#[no_mangle] fn android_main(app: AndroidApp)` usando `EventLoop::builder().with_android_app(app).build()` |
| **Audio tiempo real** | `src/driving/audio.rs:917-953` (`rerbr_audio_render`, `prepare`, `set_paused`, `set_interrupted`, `set_volume`) | CoreAudio / RemoteIO en `xcode/` llama al callback C `rerbr_audio_render(*mut f32, frames, rate)` | **Oboe / AAudio** (C++ o crate `oboe`/`ndk` AAudio) llamando **directamente** a la misma función `rerbr_audio_render` sin cambiar el mezclador Rust ni FMOD synth |
| **Giroscopio / Inclinación** | `src/driving/mod.rs:3030` (`rerbr_drive_tilt`) | CoreMotion en Obj-C devuelve `f64` | `ASensorManager` / `ASENSOR_TYPE_ACCELEROMETER` o `GAME_ROTATION_VECTOR` vía NDK (`ndk::sensor`) o bridge Kotlin/JNI |
| **Mando / Gamepad** | `src/driving/gamepad.rs:83` (`rerbr_gamepad_poll`) | `GCController` en Obj-C rellena struct C | `GameActivity` (`MotionEvent` / `KeyEvent` de `InputDevice.SOURCE_GAMEPAD \| SOURCE_JOYSTICK`) o `gilrs` rellenando el mismo `GamepadSnapshot` |
| **Safe Area Insets** | `src/driving/layout.rs:293` (`rerbr_safe_area_insets`) | `UIWindow.safeAreaInsets` en Obj-C | `WindowInsets.Type.displayCutout() \| systemBars()` vía JNI |
| **Directorio de datos** | `RERBR_PROBE_DOCUMENTS` env var | Obj-C fija `Documents/` antes de entrar en Rust | En `android_main`, fijar `RERBR_PROBE_DOCUMENTS` a `app.external_data_path()` (o `internal_data_path()`, accesible por el usuario en `Android/data/<pkg>/files/RBR/` o vía importador ZIP SAF) |
| **Tipo de librería** | `Cargo.toml:33` (`crate-type = ["staticlib", "rlib"]`) | `staticlib` enlazado por Xcode | Añadir `"cdylib"` en rama Android (o crate envoltorio `android-shell/` separado con `crate-type = ["cdylib"]` para **no tocar `Cargo.toml` de iOS**) |

---

## 3. Puntos Críticos a Verificar en Hardware Android Real

1. **Texturas BC1/BC2/BC3 (DDS DXT1/DXT3/DXT5) en Vulkan móvil:**
   - **¡MUY IMPORTANTE!** Apple Silicon (iPad mini A17 Pro) soporta compresión desktop BC (`TEXTURE_COMPRESSION_BC`) en Metal.
   - **La mayoría de GPUs Android móviles (Qualcomm Adreno, ARM Mali, PowerVR) NO soportan `TEXTURE_COMPRESSION_BC` en Vulkan** (soportan ETC2 y ASTC).
   - **Solución técnica necesaria:** al detectar `!adapter.features().contains(Features::TEXTURE_COMPRESSION_BC)`, decodificar en CPU los bloques DXT1/DXT3/DXT5 del DDS a `RGBA8` (o transcodificar/comprimir a ASTC en carga de tramo/coche con caché en disco) antes de subir a Vulkan, respetando un presupuesto de VRAM móvil (p. ej. bajando 1 mip level en dispositivos de 4–6 GB RAM si fuera necesario).
2. **Simulación 720 Hz + NGP 7.4 en ARM64 Android:**
   - La arquitectura de CPU es idéntica (`aarch64`) a la del iPad; el código pure-Rust de `rerbr-sim` compila sin x86 nativo.
   - **Stack size:** igual que en iOS, el hilo de física debe crearse con `STACK_BYTES` explícito (`std::thread::Builder::stack_size`) porque el stack por defecto de hilos en Bionic (Android) es solo 1 MB.
   - **`panic = "unwind"`:** `rerbr-sim` requiere `panic = "unwind"` en el perfil Android para capturar paradas del host puro sin matar el proceso (misma configuración que el build iOS).
3. **Ciclo de vida Android (`SurfaceDestroyed` / `Resumed` / `Suspended`):**
   - En Android, la `wgpu::Surface` **debe destruirse** en `suspended()` y **recrearse** en `resumed()` (a diferencia de iOS donde el `CAMetalLayer` persiste). `src/lib.rs` ya separa `Renderer` y pausa física/audio en `suspended()`.

---

## 4. Estrategia Recomendada Sin Romper iOS (Crate Envoltorio en Rama Independiente)

En lugar de modificar el `Cargo.toml` raíz congelado de iOS, la ruta más limpia en una rama independiente `android-prototype` es:

```text
android/
  app/
    build.gradle.kts        # Gradle + NDK + GameActivity + APK ARM64
    src/main/
      AndroidManifest.xml
      java/com/rerbr/probe/MainActivity.kt   # GameActivity + SAF ZIP picker + sensors
      jniLibs/arm64-v8a/librerbr_android.so
  rust-android/             # Crate cdylib separado que depende de `rerbr-probe`
    Cargo.toml              # wgpu(vulkan), winit(android-game-activity), android-activity, oboe
    src/lib.rs              # android_main + AAudio callback -> rerbr_audio_render
```

### Entorno local actual y CI para APK
- En este PC Windows actual: `ANDROID_HOME` y `ANDROID_NDK_HOME` no están instalados y el target `aarch64-linux-android` no está añadido en `rustup` (`rustup target list --installed` muestra solo `x86_64-pc-windows-msvc` y `aarch64-apple-ios`).
- Por tanto, no creamos ni afirmamos un APK funcional sin SDK/NDK ni dispositivo Android real.
- **Integración CI recomendada (GitHub Actions `ubuntu-latest`):**
  1. `rustup target add aarch64-linux-android`
  2. `cargo-ndk -t arm64-v8a -o android/app/src/main/jniLibs build --release` (con `profile.release.panic = "unwind"`)
  3. `./gradlew :app:assembleDebug` (o `assembleRelease` con keystore en GitHub Secrets) → artefacto `ReRBR-arm64-debug.apk`.
  4. Paso obligatorio antes de declarar Android funcional: probar el APK en hardware Android ARM64 real (validando superficie Vulkan, decodificación BC→RGBA8, audio AAudio, controles táctiles/mando y 720 Hz NGP 7.4).
