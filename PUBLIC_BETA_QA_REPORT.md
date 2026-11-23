# Preparación del catálogo de beta pública

Rama local `stabilize/public-beta-catalog` · Base `9de6eb386a9061381e747b0de13e690b58302747`

## Vehículos

- **BMW M3 E30.** La toma incorrecta es `e_intakeroof`, registro 443, nodo 2996, 237 triángulos. Su padre es el nodo 2803, por lo que la exclusión de descendientes de la carrocería alterna no la alcanzaba. El INI original la marca `Switch=true`, `Predicate=NotUsed` y `external::carpaintx`, que apunta al DDS alternativo con la decoración del Swift. Ahora el cargador respeta ese estado si la toma comparte el atlas de la carrocería alterna ya identificada. `e_roof`, nodo 2809, es la toma opcional BMW que cuelga de `e_body` y usa su propio atlas. Su selección permanece bajo el INI original.
- **Citroën DS3 WRC.** Los nodos 2934 y 2939 de `e_bonnetvents` son copias exactas bajo el mismo padre. Coinciden transformaciones, vértices, normales, índices, material y UV. El cargador dibuja una de ellas y conserva las mallas de nombre repetido que difieren. La segunda cara de `e_antena`, 12 triángulos, tiene una declaración exacta de `external_parts::carpaint` en el SGC y su mapa de shaders. Se restablece `ds3_wrc_external_parts.dds` cuando el único bloque INI coincide con la cara principal y la cara adicional conserva una referencia declarada. Se conservaron alerón, espejos, toma activa, puertas, parachoques y sus texturas originales. El SGC coincide por SHA-256 en PC y respaldo. El pack `Citroen_DS3_WRC_ngp6` arrancó en Harwood Forest con NGP 7.4 y completó 3.240 pasos a 720 Hz, con contacto de ruedas y un pico de 9,82 m/s.
- **Suzuki Swift S1600.** La caché RSF identifica el modelo 31 en `Cars/SWIFT_S1600/swift_s1600.sgc`. El SGC no está en las fuentes locales inspeccionadas. La regla del BMW exige una carrocería alterna completa y desactivada con el mismo atlas que la toma. La toma activa del DS3 permanece visible, y queda pendiente contrastar el Swift con su SGC original.

Commit de modelos: `168c6c4`.

## Catálogos

| Compilación | Configuración | Comportamiento |
| --- | --- | --- |
| Developer QA | `VEHICLE_CATALOG=developer`, `CAREER_DEV_MODE=true` | Catálogo completo y herramientas de diagnóstico existentes. |
| Beta pública | `VEHICLE_CATALOG=beta`, `CAREER_DEV_MODE=false` | Cargo activa `public-beta`. La vista utiliza `DriveCapability::Drivable`, el modelo propio y la auditoría de archivos. Los RSF requieren pack listo y datos NGP 7.4 admitidos. Los coches originales comprueban el slot y las rutas de `Cars.ini` con la misma función que usa el cargador. |

La vista Beta se construye después de clasificar e identificar alias. Reutiliza los grupos existentes y conserva los IDs de las variantes elegibles para recuperar una selección o una propiedad anterior. La vista se usa en selector, garaje, concesionarios, portada, menú principal y opciones de Carrera. Los comandos para abrir una ficha, cambiar la selección, iniciar y reiniciar comprueban el catálogo activo. El arranque de un RSF mantiene la comprobación del modelo real y el diagnóstico del backend. En Beta, un error de esa comprobación bloquea la entrada al tramo y conserva el menú. La elección del catálogo usa exclusivamente la configuración de compilación. Se mantiene la preferencia NGP guardada para Developer.

## Pruebas locales

- En la misma instalación de prueba con NGP 7.4, Developer mostró 111 entradas y Beta mostró 52. Se comprobaron los dos concesionarios, el garaje general y de Carrera, la portada y el menú principal. Una selección guardada de un vehículo excluido no reapareció en Beta y un intento de iniciar rally con esa selección quedó bloqueado.
- Pasaron las tres pruebas dirigidas en cada modo de compilación, incluidas las regresiones con los SGC de BMW, DS3 y Lancia.
- Pasaron las 200 pruebas existentes de biblioteca en cada modo, con 108 de 108 SGC cargados por el barrido general. La prueba nueva de alias se repitió por separado en ambos modos tras cambiar su descriptor sintético por un alias RSF admisible.
- Clippy pasó en ambos modos para `aarch64-apple-ios`. La comprobación de Bash pasó y la configuración Beta con economía Developer se rechazó.

## Generar las IPA en Codemagic

1. Sube solo la rama de preparación al remoto conectado a Codemagic con `git push -u origin stabilize/public-beta-catalog`. Esta entrega deja la rama local.
2. En Codemagic abre la app del repositorio, pulsa **Start new build** y selecciona esa rama. Ejecuta los dos workflows por separado. [Documentación de Codemagic](https://docs.codemagic.io/yaml/yaml/)

| Workflow | ID | Archivo en Artifacts |
| --- | --- | --- |
| **RBR Mobile — DEVELOPER QA (US$9,999,999,999)** | `rerbr-probe-dev` | `build/RBR-Mobile-Developer.ipa` |
| **RBR Mobile — BETA PÚBLICA (iPadOS unsigned)** | `rerbr-probe-unsigned` | `build/RBR-Mobile-Beta.ipa` |

Comprueba **Vehicle catalog** y **Git commit** en `build/environment.txt` y coteja la IPA con `build/SHA256SUMS`. Los workflows generan IPA sin firma y terminan con artefactos de compilación. La instalación y validación beta siguen pendientes en el iPad.
