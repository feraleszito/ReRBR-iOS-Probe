# BMW M3 E30, Lancia Delta HF 4WD y arranque táctil

22 de noviembre de 2026 · `stabilize/car-models-engine-start`

La revisión probada en el iPad fue `stabilize/beta-vehicle-qa` en `ff302922f04f1cf959564dd308f0513132a1833b`, según la confirmación del usuario. La rama local parte de esa revisión. `main` permanece en `1df3b9b9251f99d9d5f278404f451ad4fe2ab815`.

## BMW M3 E30

El SGC de PC y el del respaldo tienen el SHA-256 `8049f96dbfef07a2f5feb9f8e57ed1d3ffcd10d4d12c94a321c0044e9de7c3bc`. La auditoría recorrió los registros de malla y los IDs y padres de los nodos. Para cada pieza examinó los rangos de índices, las matrices compuestas en el espacio del coche y los materiales declarados.

El registro 63, `e_body`, contiene **11.402 triángulos** y ya incluye la forma de las puertas, el capó, la tapa del maletero y los parachoques del BMW. La proyección de esa malla con el alerón Evo 1 conserva la silueta BMW. Al añadir el segundo conjunto aparecen las puertas superpuestas y otro marco de portón y parachoques en ubicaciones distintas. El parachoques trasero de ese conjunto llega a Z = 0,466 m, mientras que la carrocería activa llega a Z = 1,054 m.

Los registros `364, 380, 394, 408, 424, 435` corresponden al capó, puertas izquierda y derecha, portón y parachoques delantero y trasero del conjunto alternativo. Sus secciones originales los marcan `Switch=true` y `Predicate=NotUsed`. Todas declaran `external::carpaintx`, que resuelve a `BMW_M3_E30_external2.dds` y contiene la decoración del Swift. La excepción para paneles estructurales ignoraba esas marcas y dibujaba las mallas alternas. El cambio anterior reasignaba sus texturas y dejaba activa su geometría.

El conversor ahora reconoce el conjunto completo de paneles desactivados cuando comparte un atlas distinto al de la carrocería activa. Aplica el estado a los nodos descendientes del conjunto, incluyendo las rejillas y los revestimientos de puerta de esa jerarquía. La cabina conserva sus puertas propias, IDs `2878` y `2882`. El showroom conserva `e_body`, `e_wing_evo1` y `e_bumperstrip_black`. El parser SGC, los índices originales y la composición de matrices se mantienen.

## Lancia Delta HF 4WD

El SGC original `delta_4wd.sgc` tiene SHA-256 `e24651c8739334df669c05406420bd178bc308f250b119de67abf32e1ba07409`. Los cristales `e_lightglass_lb/rb` (registros `232/234`) y los reflectores de freno `e_light_brake_lb/rb` (registros `512/514`) tienen 12 triángulos por lado. Cada par coincide en topología, posiciones, UV y transformaciones.

El INI asigna a los cristales `CarGlass / external transparent::carpaint`, resuelto a `delta_4wd_external_transparent.dds`. Los reflectores usan `CarLight / external dynamic::carpaint`, resuelto a `delta_4wd_external_dynamic.dds`. Al repetir las UV verticales, tal como hace el sampler, el DDS del cristal tiene rojo y alfa opaco en las muestras del piloto, mientras el DDS del reflector es gris en esa zona. El reflector opaco escribe profundidad primero. La prueba estricta del cristal descartaba los fragmentos de igual profundidad.

El nuevo pase admite la profundidad igual para un cristal trasero sólo cuando su reflector activo coincide en geometría, transformación y UV y tiene una asignación distinta. El reflector sigue en su pase original. La prueba con los archivos reales verifica ambas rutas DDS, los índices y las UV de los dos lados.

## Botón táctil

NGP recibe la ignición como nivel en el bit 0 de `Control::aux`. El motor de arranque existente aplica su ventana de 0,5 a 2,5 segundos y las condiciones de las físicas. El botón de encendido del HUD mantiene ese bit durante la pulsación. Soltar, cancelar o limpiar los controles lo libera. El toque pertenece al botón desde que empieza y deja activos los demás toques. La posición se calcula en la zona segura superior y evita los pedales movidos con el editor.

Para la próxima prueba, con el coche seguro en punto muerto o con el embrague pisado, mantener pulsado el símbolo de encendido hasta que arranque. El botón envía la entrada al motor de físicas. Las reglas de daños, el cálculo de RPM, el audio y la grabación de repeticiones siguen su recorrido habitual.

## Commits y archivos

| Commit | Archivos |
| --- | --- |
| `dffa259` · Cargador y modelos | `src/car/mesh.rs`, `src/car/materials.rs`, `src/car/gpu.rs`, `src/car/part_audit.rs`, `src/car/tests.rs`, `src/car/visual_audit.rs` |
| `f6baab8` · Arranque táctil | `src/driving/controls.rs`, `src/driving/layout.rs`, `src/driving/mod.rs`, `src/harwood/benchmark/hud.rs` |

## Validación

- La suite de biblioteca Developer obtuvo 200 aprobadas, 19 ignoradas por defecto y cero fallos. Cargó 108 de 108 SGC de la instalación y comprobó que, en esa muestra, sólo el BMW activa la selección de carrocería y sólo el Lancia activa el nuevo pase de cristal.
- Las tres regresiones dirigidas con recursos RSF originales pasaron. Cubren BMW y Lancia, alternancia BMW y DS3, y el grupo anterior de variantes, neblineros y ruedas.
- La prueba de entrada confirma la palabra de ignición en el payload, el agarre simultáneo del freno, la dirección, la liberación del botón y la separación de un pedal personalizado.
- Prueba NGP 7.4 a 720 Hz en Harwood Forest con el Fiesta Rally2 y su pack. El motor se caló frenando con cuarta engranada. En punto muerto estaba a 0 rpm. La pulsación enviada por `Controls` lo llevó a 1.111,3 rpm y quedó a 1.203,1 rpm tras soltar. La salud del motor se mantuvo al 100 %.
- Pasaron las dos pruebas del arrancador NGP. Una prueba directa adicional a 0,6 s de pulsación obtuvo par de arranque cero con cada una de las dos condiciones de disponibilidad desactivada. El intento de preparar una sesión con daño transportado no reprodujo un motor destruido, porque al iniciar la sesión la telemetría seguía marcando 100 %. Queda pendiente una prueba completa con daño adquirido en pista.
- Pasaron `cargo fmt --check`, `cargo check` para `aarch64-apple-ios` con el perfil Developer y Clippy para ese destino con `-D warnings`.

En el iPad faltan la confirmación visual del BMW en exterior y cabina, los pilotos del Lancia en el render Metal, y una prueba con varios toques sostenidos durante el arranque. La suite local cubre carga, física, FMOD y repeticiones. El rendimiento, el audio 3D y la interacción táctil final requieren una pasada en dispositivo. El entorno Windows permite comprobar el destino Rust de iOS. Una IPA nueva requiere el flujo de compilación macOS con Xcode.
