# Estabilización de recursos de vehículos

21 de noviembre de 2026 · `stabilize/vehicle-resources` · Base `4bb43b9`

La prueba en iPad mini A17 Pro detectó una textura Suzuki en el BMW, paneles grises y copias en el catálogo. Para reproducir los fallos se usaron los recursos existentes de `Desktop/Respaldo RBR iPad/Cars 2` y se compararon con la instalación PC. La prueba de catálogo combina ambos orígenes en un directorio de trabajo con copias de packs numeradas. El respaldo se consultó en solo lectura.

## Historial y causa común

| Revisión | Evidencia |
| --- | --- |
| `32dc754` y sus antecedentes de recuperación | `c196d4b` separó materiales y recursos por showroom. `14f44cd` añadió invalidación de solicitudes. `0f2771b` mantuvo caras sin UV cuando carecen de textura. `32dc754` corrigió el orden de canales BGRA. El gris visible es el color de diagnóstico de una cara sin asociación. |
| `560b5a5` | Aceptó el primer FaceBlock cuando sobraban bloques con el mismo material. La regla recuperó la textura de los neumáticos del DS3. También eligió un DDS Suzuki para seis paneles BMW cuya cara SGC declaraba el atlas BMW. La regresión anterior exigía que el DDS Suzuki estuviera presente y dejó pasar el fallo. |
| `95a7c0b` y `1f70500` | Permitieron la ruta declarada si la copia comparte SGC e INI de shaders, y recuperaron el DDS local si falta el archivo declarado. Seguía activa una salida por nombre de archivo que aceptaba una ruta extranjera sin confirmar identidad. La prueba sintética reprodujo que seleccionaba un DDS distinto con el mismo nombre. |
| `124fe98` y `b320f6e` | Agruparon copias visuales numeradas con el mismo contenido y mejoraron la clasificación de packs. Las carpetas de física numeradas tienen otra ruta de entrada, el descubrimiento de packs personalizados. El Fiesta también tenía una carpeta visual numerada con los DDS ausentes en el original. |
| `e108108`, `6d62043`, `7bf5ef2` y `4bb43b9` | Añadieron diagnósticos, inventarios y documentación. La conformidad de carga CPU no verificaba el contenido visual del atlas de un panel ni la presencia de copias de paquetes en la instalación que ejecuta Mobile. |

El patrón es la pérdida de identidad al interpretar referencias parciales. El nombre de un material de editor puede estar desfasado, el primer FaceBlock puede contener una asociación ajena y el nombre de una carpeta de pack puede ser el de una copia. En la corrección, las asociaciones de pintura se comprueban con el atlas declarado de la carrocería, las rutas compartidas con archivos de origen y las copias de física con los archivos que consume el juego.

## BMW M3 E30

PC y respaldo tienen los mismos bytes en `Cars/BMW_M3_E30/BMW_M3_E30.sgc` (SHA-256 `8049f96dbfef07a2…`) y en ambos DDS candidatos. Se decodificó el primer nivel de cada DDS. `BMW_M3_E30_external2.dds` (`569fe9f76569839b…`) contiene el Suzuki Swift amarillo, incluidas sus puertas y los logotipos Suzuki. `BMW_M3_E30_external.dds` (`8799cc651615033e…`) contiene la decoración verde del BMW.

Las seis piezas proceden del SGC BMW. Cada pieza tiene una cara SGC `external::carpaint` y dos FaceBlocks INI `external::carpaintx`. La primera referencia tiene un bitmap exacto en el mapa de shaders del BMW y comparte DDS con `external::c0`, la pintura de `e_body`. La segunda referencia seleccionaba el atlas Suzuki. Las trazas de las dos puertas prueban el mismo defecto. El lado observado en el iPad queda por identificar en captura.

| Pieza | Registro SGC | ID de nodo | Cara | Triángulos |
| --- | ---: | ---: | ---: | ---: |
| `e_bonnet` | 364 | 2957 | 0 | 1644 |
| `e_door_l` | 380 | 2964 | 0 | 388 |
| `e_door_r` | 394 | 2972 | 0 | 388 |
| `e_door_b` | 408 | 2980 | 0 | 1579 |
| `e_bumper_f` | 424 | 2987 | 0 | 1078 |
| `e_bumper_b` | 435 | 2992 | 0 | 734 |

En las seis filas la traza posterior selecciona `external::carpaint` y `BMW_M3_E30_external.dds`. La prueba compara el rango de triángulos de cada registro, la ruta finalmente usada, los bytes del DDS y la repetición de índices después de alternar BMW → DS3 → BMW. La solicitud de showroom del garaje y el concesionario lleva el descriptor seleccionado y una lista de sustituciones vacía. El rastro encuentra la textura equivocada dentro del BMW y explica el síntoma sin atribuirlo a la caché.

## Paneles grises y resolución de DDS

| Vehículo del respaldo | Piezas que usaban el gris de diagnóstico | Asociación recuperada |
| --- | --- | --- |
| Ford Fiesta Rally2 2023 | Capó, portón y ambos parachoques | `Default Material` en una sola cara completa. El primer `CarPaint` apunta al atlas exterior de `e_body`. |
| Ford Fiesta WRC 2019 | Capó y parachoques trasero | Mismo patrón y atlas exterior propio. |
| Hyundai i20 Coupe WRC 2020 | Capó, puertas izquierda y derecha, portón y parachoques delantero | Los bloques `external_skin::carpaint` y el CarPaint de `e_body` declaran `i20_wrc_20_external_car_paint.dds`. El DDS de la etiqueta SGC antigua es una textura oscura de componentes. Se inspeccionaron ambos atlas. |
| Opel ADAM R2 | Portón | `Default Material` en una sola cara. El CarPaint de la pieza y el de la carrocería declaran el mismo DDS. |
| Škoda Fabia WRC 2006 | Nueve triángulos de la primera cara del capó | Pendiente. `FaceBlock0` declara explícitamente `Default Material` y carece de bitmap. La segunda cara, con 2.086 triángulos, tiene su material y DDS. |

Los registros de la tabla siguiente están indexados desde cero. Las caras indicadas son `FaceBlock0`.

| SGC del respaldo | Pieza y registro | DDS finalmente seleccionado |
| --- | --- | --- |
| `fiesta_rally2_23 2/fiesta_rally2.sgc` | Parachoques trasero 264, delantero 349, portón 394 y capó 491 | `fiesta_rally2_23 2/textures/fiesta_rally2_external.dds` |
| `fiesta_wrc_19/fiesta_wrc_19.sgc` | Parachoques trasero 12 y capó 139 | `fiesta_wrc_19/textures/fiesta_wrc_19_external.dds` |
| `i20_WRC_20/i20_wrc_20.sgc` | Puerta derecha 19, portón 338, puerta izquierda 352, capó 374 y parachoques delantero 404 | `i20_WRC_20/textures/i20_wrc_20_external_car_paint.dds` |
| `Opel_Adam_R2/Opel_Adam_R2.sgc` | Portón 106 | `Opel_Adam_R2/textures/Opel_Adam_R2_external.dds` |
| `FABIA06/fabia06.sgc` | Capó 61 | Cara 0 sin DDS. La cara 1 usa `FABIA06/textures/fabia05_external.dds`. |

El conversor sólo usa la comprobación adicional para paneles con una sola cara que cubre todos los índices, bloques INI sobrantes y un primer `CarPaint`. Conserva los FaceBlocks con coincidencia exacta de nombre. Entre los candidatos restantes sólo usa una asociación que tenga declaración DDS exacta y que coincida con el atlas de pintura de la carrocería. BMW y Hyundai aportan casos opuestos que impiden establecer prioridad universal del SGC o del INI. La regla anterior de materiales uniformes conserva la asignación `CarTire` del DS3.

El showroom lee la ruta declarada. Para una carpeta ajena exige el mismo SGC y el mismo mapa de shaders antes de poder leerla o usar la copia local del DDS ausente. Un nombre de archivo igual queda registrado como origen sin verificar. El Lancer Evo X V1.21 se comprobó sólo con los recursos PC. El informe de uso del iPad declara que el vehículo está ausente allí.

## Duplicados reproducidos

| Caso | Antes | Corrección verificada en el catálogo usado por Mobile |
| --- | --- | --- |
| DS3 WRC | `rsf-citroen-ds3-wrc-ngp6-2` surgía como pack personalizado y tenía una ruta SGC calculada e inexistente. | Los once archivos de física coinciden con `Citroen_DS3_WRC_ngp6`. Una fila visible y resolución del ID anterior al mismo modelo y pack verificados. |
| Xsara WRC 2006 | `rsf-citroen-xsara-wrc-2006-ngp6-2` seguía el mismo camino. | Una fila visible y alias del ID anterior. El Xsara original de `Car00` conserva su entrada aparte. |
| Fiesta Rally2 | El pack `ngp7 2` generaba un descriptor adicional. También coexistían el modelo de la carpeta principal sin DDS activos y la copia numerada con sus DDS. | Se verifica la igualdad de física de la copia y la igualdad byte a byte del SGC y de los INI visuales. La entrada principal usa las rutas visuales de la copia que contiene todo el atlas declarado. Un DDS contradictorio en el original mantiene filas separadas. |

La comparación de packs exige `common.lsp`, `repair.ini`, los LSP de grava, asfalto y nieve, sus opciones y los tres setups. El test conserva visible una copia numerada con un setup de nieve distinto. La agrupación se reutiliza en catálogo visible, showroom, concesionario y garaje. Conserva los IDs usados por perfiles. La auditoría de archivos sigue notificando un DDS ausente en su ruta declarada aun cuando el showroom verifica y usa la copia visual completa. El fixture local reproduce el mecanismo reportado. La lista de archivos instalada en el iPad queda por cotejar.

## Pruebas y próxima IPA

- Las pruebas de atlas de panel, rechazo de DDS homónimo y ausencia de duplicados numerados fallaron antes del ajuste correspondiente. El inventario del fixture registró las tres copias visibles antes de corregir la agrupación.
- Las pruebas con BMW y DS3 reales comparan el origen y las texturas. La regresión del Lancer compara seis DDS declarados con los datos PC.
- La suite de biblioteca obtuvo 197 aprobadas, 16 ignoradas y cero fallos. Su barrido cargó los 108 SGC principales de PC en CPU. El barrido del respaldo cargó los 44 descriptores de showroom y recorrió 47 SGC. De las 13 caras de capó, puertas y parachoques sin asociación en el volcado previo, queda una. Los intentos sobre el Fabia numerado sin INI activo y la ruta incompleta antigua del Fiesta se registran como recursos sin entrada propia.
- Las tres regresiones de copias numeradas pasaron con `career-dev-mode`. También pasaron `cargo fmt --check`, Clippy con `physics-stage,rerbr-3d,career-dev-mode` y `cargo check --target aarch64-apple-ios --features physics-stage,career-dev-mode`.
- Las trazas `AUDIT panel` en `.validation/rsf-inventory/stability-backup.tsv` guardan ID, SGC, pieza, índice de registro, ID de nodo, cara, triángulos, material y DDS original, material elegido y ruta final. Los volcados de catálogo `ipad-repro-before.tsv` y `stability-catalog-after.tsv` permiten comparar los IDs y su visibilidad.

| Commit local | Alcance |
| --- | --- |
| `0f0a6d0` | Asociación de paneles, prueba de procedencia y traza por cara. |
| `96dd2f2` | Resolución de rutas DDS con verificación de origen. |
| `6229ec5` | Agrupación de exportaciones numeradas por contenido y compatibilidad de IDs. |

Para la siguiente IPA DEVELOPER, comprobar ambos lados, capó, portón y parachoques del BMW. Inspeccionar los paneles de los dos Fiesta, i20 y ADAM R2, los nueve triángulos del Fabia 2006 y la antena del DS3. Alternar coches rápidamente y comprobar los cuatro neumáticos del DS3. Abrir garaje, concesionario y un perfil con un ID previo para comprobar filas, modelo y textos. Probar entrada y salida de tramo, conducción NGP, daños, reparación y 60 FPS como verificación de dispositivo.

La ejecución Windows verifica geometría, rutas y contenido de DDS. Las capturas multiángulo del render Metal y la confirmación visual de la IPA quedan pendientes. El trabajo se limita a los lectores y al catálogo de vehículos, y la compilación cruzada de Rust para iOS no produce una IPA firmada.

Los casos de varias caras que aún usan la correspondencia histórica por índice y las búsquedas por sufijo de material fuera de esta regla siguen siendo puntos que la traza permite revisar. El agrupamiento de copias numeradas exige evidencia local de ambos orígenes. La prueba en el dispositivo debe comprobar las rutas instaladas y los alias que realmente conserva cada perfil.
