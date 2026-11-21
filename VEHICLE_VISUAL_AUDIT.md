# RBR Mobile · Auditoría de vehículos originales y RSF

21 de noviembre de 2026 · Rama `audit/rsf-vehicle-visuals` · Base `32dc754`

## Fuentes inspeccionadas

| Fuente | Alcance observado |
| --- | --- |
| `C:\Users\adria\Desktop\Respaldo RBR Ipad` | Carpeta vacía en la inspección recursiva, incluida la búsqueda de elementos ocultos. |
| `C:\Richard Burns Rally` | Instalación PC usada como referencia. 20.519 archivos, entre ellos 8.999 en `Cars`, 110 SGC, 1.790 DDS en `Cars` y 570 DDS en otros directorios. Se inventariaron texturas, configuraciones, skins, presets de piezas y luces, y archivos NGP. |
| `RBR Mobile - Autos Listos para iPad` | `LISTOS PARA CONDUCIR/RBRRRRR` contiene 28 SGC y 436 DDS. `SOLO VISUALIZACION/RBR` contiene 2 SGC y 30 DDS. `PENDIENTES` contiene un listado de texto. Los 30 SGC coinciden por SHA-256 con archivos de la instalación PC. |
| `RBR-backup-20261120` | Seis archivos de código Rust. |

En PC hay 108 SGC principales y dos SGC de tapas de rueda de asfalto en `Cars/DS3_WRC/tarmac wheels caps options`. El SGC `without caps` tiene el mismo SHA-256 que el DS3 principal (`cad6e87f091b…`). El SGC `with caps` sí es una alternativa distinta (`8e43f454d9be…`). Ambos se conservaron. El archivo activo `Cars/Cars.ini` define los slots y la copia `cars.ini.rsfbak` conserva un índice previo. La caché PC tiene 102 registros de coches y 248 de modelos. La carpeta local `rsfdata/cars` contiene 34 directorios NGP, 30 con sufijo NGP6 y cuatro NGP7, y 34 archivos ZIP.

Los datos de PC y de transferencia se leyeron desde orígenes separados mediante enlaces locales de diagnóstico con la estructura `Documents/RBR`. La tabla cruza el catálogo efectivo, el SGC y sus tres INI activos, el resultado del showroom, la conversión del cockpit y el canal comercial. El campo de transferencia identifica el catálogo de las carpetas exportadas por separado. La colección de 30 SGC representa los archivos exportados disponibles, y el contenido instalado en iPad y la propiedad de un perfil de Carrera siguen pendientes de cotejo. El garaje usa el ID del catálogo y la propiedad del perfil para listar coches adquiridos.

## Defectos prioritarios

| Vehículo y gravedad | Causa y corrección | Regresión |
| --- | --- | --- |
| **BMW M3 E30 GrpA · alta** | Los seis paneles `e_bonnet`, `e_door_l`, `e_door_r`, `e_door_b`, `e_bumper_f` y `e_bumper_b` tienen una cara SGC `external::carpaint`. Los dos FaceBlocks de cada panel declaran `external::carpaintx`, que apunta a `BMW_M3_E30_external2.dds`. El cargador rechazaba el vínculo por la diferencia de nombre y de cardinalidad y enviaba los paneles al material neutro. Se recuperó la asociación declarada y se preservó el rango original de triángulos. Los SGC, INI de piezas, INI de shaders y ajustes de cielo claro son idénticos entre PC y transferencia. | `bmw_and_ds3_original_face_block_regressions` exige los seis paneles, la textura y su clase `CarPaint` en el showroom y comprueba BMW → DS3 → BMW. La prueba previa de carrocería y variantes valida alerones, espejos, tiras de paragolpes y luces. |
| **Citroën DS3 WRC · alta** | Cada rueda de grava tiene una cara SGC `external dynamics::tire` con 3.664 triángulos. Los bloques INI `CarTire` y `CarRim` nombran `external dynamic::tire`. El INI de shaders referencia `ds3_wrc_external_dynamic.dds`, DXT3 de 12 niveles. La cara del neumático usaba el material gris por falta de asociación. Ahora toma el primer FaceBlock, `CarTire`, y sube el DDS original en los cuatro slots. La antena tiene una segunda cara SGC sin FaceBlock correspondiente y sigue registrada. | La regresión con recursos PC y de transferencia comprueba material y DDS en cada slot 1–4. La prueba sintética verifica que el vínculo sólo se recupera cuando la cara cubre toda la submalla y los bloques sobrantes coinciden en material. |
| **Mitsubishi Lancer Evo X V1.21 · alta** | `Cars/LANCER_EVO_X` y `Cars/LANCER_EVO_X_v1_21` contienen el mismo SGC y los mismos INI, verificados byte a byte. El mapa de shaders apunta a `LANCER_EVO_X_v1_21/textures`. El showroom sustituía seis archivos por homónimos de `LANCER_EVO_X/textures`, cinco con SHA-256 distinto. El cargador usa la ruta explícita cuando las dos carpetas contienen el mismo SGC y el mismo mapa de shaders. Conserva los límites de lectura dentro de la instalación y la protección sobre carpetas de coches distintos. | La regresión real comprueba las seis rutas y sus bytes, además de los mismos índices e IDs distintos. La prueba sintética rechaza una carpeta con shader diferente aunque contenga el mismo modelo. |

La corrección se aplica al cargador común de showroom y conducción de vehículos modificados. Conserva la coincidencia nominal para caras con bloques reordenados y usa el FaceBlock posicional adicional cuando el SGC tiene una sola cara que cubre toda la submalla y los bloques declaran el mismo material. Cuando el INI ofrece materiales diferentes y no hay correspondencia suficiente, se conserva el diagnóstico de cara sin asociación.

## Resultado del inventario

| Medida | Resultado |
| --- | ---: |
| Entradas del catálogo móvil sobre los archivos PC | 111 |
| SGC principales seleccionados por esas entradas | 108 |
| SGC recorridos y analizados, incluidos los dos SGC anidados del DS3 | 110 |
| SGC únicos por SHA-256 | 108 |
| Entradas con exterior convertido y DDS preparados en CPU | 111 |
| Entradas con interior convertido y DDS preparados en el diagnóstico | 111 |
| Cargas de conducción completadas con los packs y el índice PC | 40 de 41 intentadas |
| CORRECTO VERIFICADO en captura | 0 |
| CORRECTO ESTRUCTURALMENTE en PC | 44 |
| ERROR DETECTADO en PC | 67 |
| RECURSOS INCOMPLETOS para el diagnóstico visual PC | 0 |
| PENDIENTE entre las 111 entradas del catálogo PC | 0 |
| Pendientes de inspección en pantalla | 111 entradas y la variante anidada distinta |

En el showroom, las caras sin asociación bajaron de **251 a 181** en **21 entradas**. El diagnóstico independiente del interior encontró **91** caras sin asociación después del cambio. Estas cifras cuentan caras por entrada, por lo que las variantes de catálogo que comparten SGC pueden aportar el mismo defecto. En PC, los 67 errores incluyen fallos de asociación reproducibles y el rechazo de la configuración de conducción del Corolla. Algunas superficies con material neutro pueden tener un propósito especial en RBR. Su correspondencia con la referencia original sigue pendiente donde el archivo presenta una asociación ambigua.

El catálogo de `LISTOS PARA CONDUCIR` por separado produjo 39 entradas, 27 cargas de showroom y 12 entradas sin SGC propio. El de `SOLO VISUALIZACION` produjo 11 entradas, dos cargas y nueve entradas sin SGC propio. Estas carencias se consideran **RECURSOS INCOMPLETOS** en la carpeta correspondiente aunque el recurso esté presente en PC. Los nombres de los directorios de exportación reflejan el destino del paquete. La habilitación de conducción en iPad depende de los archivos efectivamente combinados en `Documents/RBR`.

Tres pares comparten geometría con packs distintos y se conservaron como variantes legítimas. Mitsubishi Lancer Evo IX N4 y R4, Subaru Impreza N14 N4 y N15 R4, y Renault Clio Rally4 y Rally5. El Lancer Evo X V1.21 es un duplicado de SGC en dos rutas, con una entrada visual adicional y una entrada RSF con pack propio. Ambos IDs se conservaron. En el conjunto de transferencia coinciden los 30 SGC con PC, incluidos los dos SGC anidados del DS3. `with caps` añade las cuatro submallas `e_wheel_tarmac_cap_*`, de 464 triángulos cada una. Los vértices, índices, UV, FaceBlocks y matrices de las submallas preexistentes coinciden. La variante queda **PENDIENTE** de comparación visual con las tapas de asfalto. `without caps` es una copia exacta del SGC principal. El selector actual usa las cuatro ruedas de grava y el SGC principal.

| Hallazgo reproducible | Gravedad y seguimiento |
| --- | --- |
| `Toyota Corolla WRC`, conducción desde la instalación PC | Alta para ese directorio. `[Car06]` de `Cars/Cars.ini` apunta al Toyota Celica TCT. El cargador rechaza la discrepancia y el showroom carga `Cars/COROLLA/corolla.sgc` de forma independiente. Pendiente cotejar el índice activo de iPad. La prueba de aislamiento comprueba el rechazo ante recursos fuera del perfil; el volcado de auditoría reproduce este caso real. |
| `Citroen DS3 WRC`, antena | Baja. La cara SGC `external_parts::carpaint` tiene 12 triángulos y el INI del nodo `e_antena` declara sólo el bloque `CarPaint / external::carpaint`. El INI de shaders contiene un bitmap para el segundo material, pero falta la clase del FaceBlock. Queda pendiente cotejar el render original y los presets de piezas antes de asignarla. El diagnóstico registra `e_antena FaceBlock1` y la regresión de ruedas comprueba los cuatro slots por separado. |
| Resto de caras sin asociación, con piezas indicadas por ID en la tabla | Prioridad alta para paneles y ruedas, media para luces, y dependiente de visibilidad en interiores y detalles. Las filas `unresolved` indican cada `FaceBlock`, nombre SGC y motivo. Los casos con varias asociaciones candidatas requieren comprobar el INI activo y los rangos de la variante. Las regresiones del conversor preservan la geometría y evitan elegir un material distinto por aproximación. |
| `LANCER EVO X`, caras de rueda sin asociación | Alta. Las cuatro ruedas aún tienen caras sin vínculo seguro de FaceBlock. La corrección de rutas DDS confirma los seis mapas declarados, incluida la textura dinámica, y deja pendiente la asociación individual de estas caras. El diagnóstico y la regresión de rutas distinguen ambos resultados. |

## Pruebas y commits

- `cargo fmt --check` y `cargo clippy --offline --features physics-stage,rerbr-3d --all-targets -- -D warnings` completados en Windows.
- `cargo check --offline --target aarch64-apple-ios --features physics-stage` completado. La comprobación cruzada verifica compilación de Rust para iOS; el dispositivo y Metal requieren una sesión de visualización.
- Suite de biblioteca `physics-stage` con **193 pruebas aprobadas**, 15 ignoradas y cero fallos en la ejecución principal final. Incluye pruebas de geometría, ruedas, selección de luces, catálogo, conversión de texturas e aislamiento del showroom.
- `cargo test --offline -p rerbr-asset-formats -p rerbr-content` con **79 pruebas aprobadas** entre SGC, DDS, FaceBlocks y descriptores.
- `cargo test --offline --features rerbr-3d --test asset_loading` con seis pruebas aprobadas y una ignorada por requerir una ruta de mapas privada.
- Ejecuciones explícitas del diagnóstico PC, los dos orígenes de transferencia, los interiores PC y las regresiones BMW/DS3 con recursos PC y de transferencia y Lancer Evo X con recursos PC. Los volcados locales se guardaron en `.validation/vehicle-audit/`, incluidos `pc-before.tsv`, `pc-after.tsv`, `pc-interiors.tsv`, `transfer-after.tsv`, `visual-after.tsv` y los inventarios `*-models.csv`. La tabla usa el barrido posterior al ajuste de FaceBlocks. La resolución DDS posterior del Lancer se verificó aparte con los seis archivos declarados.
- `560b5a5` guarda la corrección del conversor y la regresión sintética. `e108108` guarda el diagnóstico y la regresión con recursos originales. `95a7c0b` guarda la corrección de rutas DDS y su regresión. Este informe se guarda en un commit independiente.

El entorno Windows permitió comprobar la preparación de geometría y texturas para el renderizador. El backend Metal del proyecto y el contenido del garaje del perfil real requieren la verificación en iPad. Las capturas comparativas con el RBR original siguen pendientes.

## Reproducción

Con `$env:VEHICLE_AUDIT_DOCUMENTS` apuntando al directorio padre de `RBR`, desde el repositorio:

```powershell
cargo test --features physics-stage --lib car::visual_audit::audit_vehicle_resources -- --ignored --nocapture
cargo test --features physics-stage --lib car::visual_audit::audit_vehicle_interiors -- --ignored --nocapture
cargo test --features physics-stage --lib bmw_and_ds3_original_face_block_regressions -- --ignored
cargo test --features physics-stage --lib lancer_evo_x_showroom_uses_the_declared_atlas -- --ignored
```

Las filas `file`, `material`, `interior-dds`, `drawn`, `skipped`, `unresolved` y `error` dan las rutas declaradas y resueltas, formatos que prepara el cargador, pasos de render, slots de rueda, submallas y fallos. `catalog` incluye ID, nombre, SGC, INI, pack, capacidad, canal y dependencias que el catálogo marca como ausentes. El volcado de FaceBlocks se reproduce con `dump_face_blocks` y `$env:VEHICLE_AUDIT_SGC` apuntando al SGC original.

## Validación en iPad mini A17 Pro

1. Registrar el commit de la aplicación y el catálogo real del dispositivo. Comparar los SHA-256 de SGC, INI activos y DDS contra la referencia para cada discrepancia del catálogo.
2. Capturar en portada, concesionario y garaje las vistas frontal, trasera, laterales y superior. Capturar el interior en conducción para coches con pack disponible. Comparar a igual variante, skin, superficie y estado de luces con RBR PC.
3. Priorizar los seis paneles del BMW, la selección de sus piezas de carrocería y luces, y las cuatro ruedas de grava del DS3. Comprobar también las dos opciones de tapas de asfalto en RBR PC y la antena del DS3 en el dispositivo.
4. Alternar BMW → DS3 → BMW y variantes con SGC compartido, incluyendo cambios rápidos mientras carga el showroom. Verificar ID mostrado, modelo activo, rueda por slot y DDS. Repetir al cambiar skin o importar recursos y al reiniciar la aplicación.
5. Registrar capturas y resultados por ID. El estado **CORRECTO VERIFICADO** requiere confirmar en imagen materiales, geometría, ruedas y los componentes visibles que correspondan.

## Tabla individual del catálogo PC

**L** es `LISTOS PARA CONDUCIR` y **V** es `SOLO VISUALIZACION`. «Sin SGC» indica una entrada anunciada o predefinida sin su modelo en esa carpeta. El estado de la última columna corresponde al origen PC. La columna Pack muestra el pack declarado y la capacidad calculada en esa raíz. Un pack declarado puede faltar en disco. «Exterior / DDS · interior · residuos» muestra el número de draws de exterior y DDS asociados, el número de draws de interior y las caras sin asociación exterior / interior. «Correcto estructuralmente» indica conversión de geometría, pivotes y UV de las caras dibujadas, resolución de las texturas que llegaron al plan de upload y ausencia de incidencias en esos diagnósticos. La comparación de posición, escala, orientación, transparencia e iluminación en pantalla sigue pendiente para todos.

| Nombre e ID | Modelo | Pack · capacidad / canal | Transferencia | Exterior / DDS · interior · residuos | Estado PC |
| --- | --- | --- | --- | --- | --- |
| Citroen Xsara (Car00) · `citroen-xsara` | `Cars/xsara/xsara.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 96 / 3 · 27 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Ford Focus Mk II RS WRC 2006 · `ford-focus-rs-wrc-2006` | `Cars/FORD_FOCUS_2006/focus06.sgc` | `Ford_Focus_Mk_II_RS_WRC_2006_ngp6` · Drivable / UsedOnly | L sin SGC, V sin SGC | 82 / 6 · 23 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Hyundai Accent WRC · `hyundai-accent-wrc` | `Cars/accent/accent.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 91 / 3 · 26 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| MG ZR Super 1600 · `mg-zr-s1600` | `Cars/zr/zr.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 104 / 3 · 26 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Mitsubishi Lancer Evo VII · `mitsubishi-lancer-evo-vii` | `Cars/Lancer_EVO_VII/lancer_evo_vII.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 115 / 3 · 28 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Peugeot 206 WRC · `peugeot-206-wrc` | `Cars/Peugeot_206/Peugeot_206.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 93 / 3 · 21 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Abarth Grande Punto S2000 · `rsf-abarth-grande-punto-s2000` | `Cars/g.punto_s2000/g.punto_s2000.sgc` | `Abarth_Grande_Punto_S2000_ngp6` · VisualOnly / UsedOnly | — | 77 / 3 · 16 · 4 / 1 | **ERROR DETECTADO** · e_bumper_f, e_door_b, e_bumper_b, i_door_b |
| Alpine A110 Rally RGT · `rsf-alpine-a110-rally-rgt` | `Cars/Alpine_A110_2020/Alpine_A110_2020.sgc` | `Alpine_A110_Rally_RGT_ngp6` · VisualOnly / UsedOnly | — | 95 / 4 · 22 · 1 / 0 | **ERROR DETECTADO** · e_bonnet |
| Aston Martin Vantage RGT · `rsf-aston-martin-vantage-rgt` | `Cars/AstonMartin_Vantage_RGT/AstonMartin_Vantage_RGT.sgc` | `Aston_Martin_Vantage_RGT_ngp6` · VisualOnly / UsedOnly | — | 90 / 4 · 19 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf |
| Audi 200 quattro GrpA · `rsf-audi-200-quattro-grpa` | `Cars/Audi_200_quattro/Audi_200_quattro.sgc` | `Audi_200_quattro_GrpA_ngp6` · Drivable / UsedOnly | L | 76 / 3 · 50 · 0 / 2 | **ERROR DETECTADO** · i_exlights, i_cover1 |
| Audi quattro A1 GrpB · `rsf-audi-quattro-a1-grpb` | `Cars/AUDI_QUATTRO_A1_GrpB/AUDI_QUATTRO_A1_GrpB.sgc` | `Audi_quattro_A1_GrpB_ngp6` · Drivable / UsedOnly | L | 77 / 3 · 21 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Audi quattro A2 GrpB · `rsf-audi-quattro-a2-grpb` | `Cars/AUDI_QUATTRO_A2_GrpB/AUDI_QUATTRO_A2_GrpB.sgc` | `Audi_quattro_A2_GrpB_ngp6` · Drivable / UsedOnly | L | 84 / 6 · 24 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Audi quattro Grp4 · `rsf-audi-quattro-grp4` | `Cars/AUDI_QUATTRO_Grp4/AUDI_QUATTRO_Grp4.sgc` | `Audi_quattro_Grp4_ngp6` · VisualOnly / UsedOnly | — | 79 / 3 · 21 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Audi Sport quattro GrpB · `rsf-audi-sport-quattro-grpb` | `Cars/Audi_Sport_Quattro/Audi_Sport_Quattro.sgc` | `Audi_Sport_quattro_GrpB_ngp6` · Drivable / UsedOnly | L | 73 / 4 · 13 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| BMW M1 GrpB · `rsf-bmw-m1-grpb` | `Cars/BMW_M1/BMW_M1.sgc` | `BMW_M1_GrpB_ngp6` · Drivable / UsedOnly | L | 65 / 5 · 15 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| BMW M3 E30 GrpA · `rsf-bmw-m3-e30-grpa` | `Cars/BMW_M3_E30/BMW_M3_E30.sgc` | `BMW_M3_E30_GrpA_ngp6` · Drivable / UsedOnly | L | 108 / 20 · 40 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| BMW M3 E36 GrpA · `rsf-bmw-m3-e36-grpa` | `Cars/bmw_m3_e36/bmw_m3_e36.sgc` | `BMW_M3_E36_GrpA_ngp6` · VisualOnly / UsedOnly | — | 81 / 6 · 26 · 1 / 2 | **ERROR DETECTADO** · e_steeringwheel, i_body |
| Citroen C2 GT S1600 · `rsf-citroen-c2-gt-s1600` | `Cars/CITROEN_C2_s1600/C2s1600.sgc` | `Citroen_C2_GT_S1600_ngp6` · VisualOnly / UsedOnly | — | 71 / 4 · 23 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Citroen C2 R2 Max · `rsf-citroen-c2-r2-max` | `Cars/C2_R2_Max/C2_R2_Max.sgc` | `Citroen_C2_R2_Max_ngp6` · Drivable / UsedOnly | L | 75 / 5 · 24 · 0 / 3 | **ERROR DETECTADO** · i_mirror1_l, i_mirror_l |
| Citroen C3 R5 · `rsf-citroen-c3-r5` | `Cars/C3_r5/C3_r5.sgc` | `Citroen_C3_R5_ngp6` · VisualOnly / UsedOnly | — | 85 / 6 · 18 · 2 / 3 | **ERROR DETECTADO** · e_wheelgrunk_lb, e_wheelgrunk_rb, i_body, i_door_b |
| Citroen C3 WRC 2017 · `rsf-citroen-c3-wrc-2017` | `Cars/Citroen_C3_WRC17/Citroen_C3_WRC17.sgc` | `Citroen_C3_WRC_2017_ngp6` · Drivable / PendingCommercialRule | L | 85 / 5 · 17 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Citroen C4 WRC 2008 · `rsf-citroen-c4-wrc-2008` | `Cars/C4_08/c4_08.sgc` | `Citroen_C4_WRC_2008_ngp6` · Drivable / UsedOnly | L | 106 / 5 · 38 · 1 / 1 | **ERROR DETECTADO** · e_sparewheelstrap_gravel, i_bonnet |
| Citroen DS3 R1 · `rsf-citroen-ds3-r1` | `Cars/Citroen_DS3_R1/Citroen_DS3_R1.sgc` | `Citroen_DS3_R1_ngp6` · VisualOnly / UsedOnly | — | 81 / 5 · 27 · 4 / 0 | **ERROR DETECTADO** · g_wheel |
| Citroen DS3 R3-MAX · `rsf-citroen-ds3-r3-max` | `Cars/Citroen_DS3_R3/Citroen_DS3_R3.sgc` | `Citroen_DS3_R3-MAX_ngp6` · VisualOnly / UsedOnly | — | 81 / 5 · 27 · 4 / 0 | **ERROR DETECTADO** · g_wheel |
| Citroen DS3 R5 · `rsf-citroen-ds3-r5` | `Cars/DS3_R5/ds3_r5.sgc` | `Citroen_DS3_R5_ngp6` · VisualOnly / UsedOnly | — | 92 / 5 · 31 · 10 / 1 | **ERROR DETECTADO** · e_wheelgrunk_lb, e_wheelgrunk_rb, e_body, i_door_r |
| Citroen DS3 WRC · `rsf-citroen-ds3-wrc` | `Cars/DS3_WRC/ds3_wrc.sgc` | `Citroen_DS3_WRC_ngp6` · Drivable / UsedOnly | L | 85 / 5 · 23 · 1 / 0 | **ERROR DETECTADO** · e_antena |
| Citroen Xsara Kit Car · `rsf-citroen-xsara-kit-car` | `Cars/Citroen_Xsara_KitCar/Citroen_Xsara_KitCar.sgc` | `Citroen_Xsara_Kit_Car_ngp6` · VisualOnly / UsedOnly | — | 93 / 4 · 26 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Citroen Xsara WRC 2006 · `rsf-citroen-xsara-wrc-2006` | `Cars/Xsara_2006/xsara.sgc` | `Citroen_Xsara_WRC_2006_ngp6` · Drivable / UsedOnly | L | 80 / 5 · 22 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Fiat 124 Abarth Rally RGT · `rsf-fiat-124-abarth-rally-rgt` | `Cars/124_Abarth_RGT/124_Abarth_RGT.sgc` | `Fiat_124_Abarth_Rally_RGT_b_ngp6` · VisualOnly / UsedOnly | — | 91 / 5 · 28 · 1 / 0 | **ERROR DETECTADO** · e_wing |
| Fiat 126 Grp2 · `rsf-fiat-126-grp2` | `Cars/126P/126p.sgc` | `Fiat_126_Grp2_ngp6` · VisualOnly / UsedOnly | — | 60 / 4 · 30 · 0 / 1 | **ERROR DETECTADO** · i_body |
| Fiat 131 Abarth Grp4 · `rsf-fiat-131-abarth-grp4` | `Cars/131_Abarth_Gr4/131_Abarth_Gr4.sgc` | `Fiat_131_Abarth_Grp4_ngp6` · VisualOnly / UsedOnly | — | 89 / 5 · 28 · 0 / 4 | **ERROR DETECTADO** · i_door_l_int |
| Fiat Abarth 500 R3T · `rsf-fiat-abarth-500-r3t` | `Cars/500_Abarth_R3T/500_Abarth_R3T.sgc` | `Fiat_Abarth_500_R3T_ngp6` · VisualOnly / UsedOnly | — | 97 / 4 · 20 · 1 / 0 | **ERROR DETECTADO** · e_sparewheel_gravel |
| Ford Escort Mk II RS Grp4 · `rsf-ford-escort-mk-ii-rs-grp4` | `Cars/Escort_mk_2/Escort_mk_2.sgc` | `Ford_Escort_Mk_II_RS_Grp4_ngp6` · VisualOnly / UsedOnly | — | 86 / 4 · 23 · 14 / 2 | **ERROR DETECTADO** · e_steeringwheel, e_wheel_gravel_lf, i_steeringwheel |
| Ford Escort Mk V RS Cosworth Grp · `rsf-ford-escort-mk-v-rs-cosworth-grp` | `Cars/ESCORT_COSWORTH_GRA/Escort_Cosworth_GrA.sgc` | `Ford_Escort_Mk_V_RS_Cosworth_GrpA_ngp6` · VisualOnly / UsedOnly | — | 85 / 6 · 21 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Ford Fiesta Mk VIII R2 · `rsf-ford-fiesta-mk-viii-r2` | `Cars/Fiesta_R2_2019/Fiesta_R2_2019.sgc` | `Ford_Fiesta_Mk_VIII_R2_ngp6` · Drivable / UsedOnly | L sin SGC | 79 / 6 · 23 · 0 / 5 | **ERROR DETECTADO** · i_bonnet |
| Ford Fiesta Mk VI S2000 · `rsf-ford-fiesta-mk-vi-s2000` | `Cars/Fiesta_S2000/fiesta_s2000.sgc` | `Ford_Fiesta_Mk_VI_S2000_ngp6` · VisualOnly / UsedOnly | — | 101 / 5 · 25 · 3 / 3 | **ERROR DETECTADO** · e_exhaust, e_sparewheelstrap_gravel, i_door_b, i_door_l |
| Ford Fiesta R2 · `rsf-ford-fiesta-r2` | `Cars/fiesta_r2/fiesta_r2.sgc` | `Ford_Fiesta_R2_ngp6` · VisualOnly / UsedOnly | — | 84 / 5 · 21 · 0 / 1 | **ERROR DETECTADO** · i_body |
| Ford Fiesta R5 · `rsf-ford-fiesta-r5` | `Cars/F_Fiesta_R5/F_Fiesta_R5.sgc` | `Ford_Fiesta_R5_ngp6` · VisualOnly / UsedOnly | — | 119 / 7 · 46 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Ford Fiesta Rally2 · `rsf-ford-fiesta-rally2` | `Cars/fiesta_rally2_23/fiesta_rally2.sgc` | `Ford_Fiesta_Rally2_ngp7` · Drivable / Both | L sin SGC | 72 / 7 · 20 · 4 / 8 | **ERROR DETECTADO** · e_bumper_b, e_bumper_f, e_door_b, i_door_lint, i_door_l |
| Ford Fiesta Rally3 · `rsf-ford-fiesta-rally3` | `Cars/Fiesta_rally3/Fiesta_rally3.sgc` | `Ford_Fiesta_Rally3_ngp6` · VisualOnly / UsedOnly | V (SGC) | 82 / 6 · 26 · 1 / 1 | **ERROR DETECTADO** · e_steeringwheel, i_parts |
| Ford Fiesta rally3 evo · `rsf-ford-fiesta-rally3-evo` | `Cars/fiesta_rally3_evo/fiesta_rally3_evo.sgc` | `Ford_Fiesta_Rally3_Evo_ngp7` · Drivable / NewSalonOnly | L | 97 / 8 · 25 · 0 / 2 | **ERROR DETECTADO** · i_body |
| Ford Fiesta Rally4 · `rsf-ford-fiesta-rally4` | `Cars/Fiesta_rally4/Fiesta_rally4.sgc` | `Ford_Fiesta_Rally4_ngp6` · Drivable / Both | L | 82 / 6 · 26 · 1 / 1 | **ERROR DETECTADO** · e_steeringwheel, i_parts |
| Ford Fiesta RS WRC 2014 · `rsf-ford-fiesta-rs-wrc-2014` | `Cars/fiesta_wrc_14/fiesta_wrc_14.sgc` | `Ford_Fiesta_RS_WRC_2014_ngp6` · Drivable / UsedOnly | L | 114 / 6 · 20 · 2 / 1 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf, i_parts |
| Ford Fiesta WRC 2019 · `rsf-ford-fiesta-wrc-2019` | `Cars/fiesta_wrc_19/fiesta_wrc_19.sgc` | `Ford_Fiesta_WRC_2019_ngp6` · Drivable / NewSalonOnly | L | 79 / 6 · 21 · 3 / 3 | **ERROR DETECTADO** · e_bumper_b, e_bonnet, e_tapas, i_door_rint, i_parts |
| Honda Civic Type R(FN2) R3 · `rsf-honda-civic-type-r-fn2-r3` | `Cars/HONDA_Civic_R3/Honda_Civic_R3.sgc` | `Honda_Civic_Type_R_FN2_R3_ngp6` · VisualOnly / UsedOnly | — | 93 / 5 · 35 · 0 / 4 | **ERROR DETECTADO** · i_door_l |
| Hyundai i20 Coupe WRC 2017 · `rsf-hyundai-i20-coupe-wrc-2017` | `Cars/i20_WRC_17/i20_wrc_17.sgc` | `Hyundai_i20_Coupe_WRC_2017_ngp6` · VisualOnly / PendingCommercialRule | — | 93 / 11 · 33 · 6 / 1 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf, e_bolt_lb, i_body |
| Hyundai i20 Coupe WRC 2020 · `rsf-hyundai-i20-coupe-wrc-2020` | `Cars/i20_WRC_20/i20_wrc_20.sgc` | `Hyundai_i20_Coupe_WRC_2020_ngp6` · Drivable / NewSalonOnly | L | 85 / 13 · 29 · 8 / 0 | **ERROR DETECTADO** · e_door_r, e_wheelgrunk_lf, e_wheelgrunk_rf |
| Hyundai i20 Coupe WRC 2021 · `rsf-hyundai-i20-coupe-wrc-2021` | `Cars/i20_WRC_21/i20_wrc_21.sgc` | `Hyundai_i20_Coupe_WRC_2021_ngp7` · VisualOnly / PendingCommercialRule | — | 92 / 16 · 37 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Hyundai i20 N Rally2 · `rsf-hyundai-i20-n-rally2` | `Cars/i20_rally2/i20_rally2.sgc` | `Hyundai_i20_N_Rally2_ngp7` · VisualOnly / Both | — | 94 / 14 · 38 · 6 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf, e_bolt_lb |
| Hyundai i20 R5 · `rsf-hyundai-i20-r5` | `Cars/i20_R5/i20_R5.sgc` | `Hyundai_i20_R5_ngp6` · VisualOnly / UsedOnly | — | 89 / 11 · 32 · 6 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf, e_bolt_rf |
| Lada Kalina RC2 GrpA · `rsf-lada-kalina-rc2-grpa` | `Cars/kalina_rc2/kalina_rc2.sgc` | `Lada_Kalina_RC2_GrpA_ngp6` · VisualOnly / UsedOnly | — | 84 / 5 · 20 · 0 / 1 | **ERROR DETECTADO** · i_body |
| Lada VFTS GrpB · `rsf-lada-vfts-grpb` | `Cars/Lada_VFTS_NGP/Lada_VFTS_NGP.sgc` | `Lada_VFTS_GrpB_ngp6` · VisualOnly / UsedOnly | — | 99 / 7 · 18 · 1 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf |
| LANCER EVO X · `rsf-lancer-evo-x` | `Cars/LANCER_EVO_X/lancer_evo_X.sgc` | — · VisualOnly / UsedOnly | — | 119 / 6 · 35 · 4 / 0 | **ERROR DETECTADO** · e_wheel_gravel_rf, e_wheel_gravel_lf, e_wheel_gravel_rb |
| Lancia Delta HF 4WD GrpA · `rsf-lancia-delta-hf-4wd-grpa` | `Cars/delta_4wd/delta_4wd.sgc` | `Lancia_Delta_HF_4WD_GrpA_ngp6` · VisualOnly / UsedOnly | — | 96 / 5 · 34 · 3 / 2 | **ERROR DETECTADO** · e_bumper_b_saf, e_bumper_f, i_door_l, i_door_r |
| Lancia Stratos HF Grp4 · `rsf-lancia-stratos-hf-grp4` | `Cars/Lancia_stratos/Lancia_Stratos.sgc` | `Lancia_Stratos_HF_Grp4_ngp6` · VisualOnly / UsedOnly | — | 89 / 4 · 24 · 6 / 0 | **ERROR DETECTADO** · e_door_l, e_door_r, e_wheel_gravel_lb |
| Lotus Exige S RGT · `rsf-lotus-exige-s-rgt` | `Cars/Lotus_Exige/Lotus_Exige.sgc` | `Lotus_Exige_S_RGT_ngp6` · VisualOnly / UsedOnly | — | 75 / 4 · 16 · 0 / 2 | **ERROR DETECTADO** · i_door_l, i_door_r |
| Mazda 323 BF 4WD Turbo GrpA · `rsf-mazda-323-bf-4wd-turbo-grpa` | `Cars/MAZDA_323_BFMR/MAZDA_323_BFMR.sgc` | `Mazda_323_BF_4WD_Turbo_GrpA_ngp6` · VisualOnly / UsedOnly | — | 62 / 4 · 14 · 1 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf |
| Mini JCW WRC · `rsf-mini-jcw-wrc` | `Cars/MINI_WRC01B/mini_wrc01b.sgc` | `Mini_JCW_WRC_ngp6` · VisualOnly / PendingCommercialRule | — | 101 / 7 · 25 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lb, e_wheelgrunk_rb |
| Mitsubishi Lancer Evo II GrpA · `rsf-mitsubishi-lancer-evo-ii-grpa` | `Cars/LANCER_EVO_II/LANCER_EVO_II.sgc` | `Mitsubishi_Lancer_Evo_II_GrpA_ngp6` · VisualOnly / UsedOnly | — | 81 / 4 · 40 · 0 / 4 | **ERROR DETECTADO** · i_exlight1, i_exlight1_lit |
| Mitsubishi Lancer Evo IX N4 · `rsf-mitsubishi-lancer-evo-ix-n4` | `Cars/evo_IX/evo_IX.sgc` | `Mitsubishi_Lancer_Evo_IX_N4_ngp6` · VisualOnly / UsedOnly | — | 110 / 5 · 43 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Mitsubishi Lancer Evo IX R4 · `rsf-mitsubishi-lancer-evo-ix-r4` | `Cars/evo_IX/evo_IX.sgc` | `Mitsubishi_Lancer_Evo_IX_R4_ngp6` · VisualOnly / UsedOnly | — | 110 / 5 · 43 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Mitsubishi Lancer Evo X R4 · `rsf-mitsubishi-lancer-evo-x-r4` | `Cars/LANCER_EVO_X_v1_21/lancer_evo_X.sgc` | `Mitsubishi_Lancer_Evo_X_R4_V1.2_ngp6` · VisualOnly / UsedOnly | — | 119 / 6 · 35 · 4 / 0 | **ERROR DETECTADO** · e_wheel_gravel_rf, e_wheel_gravel_lf, e_wheel_gravel_rb |
| Opel ADAM R2 · `rsf-opel-adam-r2` | `Cars/Opel_Adam_R2/Opel_Adam_R2.sgc` | `Opel_ADAM_R2_ngp6` · Drivable / UsedOnly | L | 72 / 4 · 13 · 1 / 0 | **ERROR DETECTADO** · e_door_b |
| Opel Ascona 400 Grp4 · `rsf-opel-ascona-400-grp4` | `Cars/Opel_Ascona_400/Opel_Ascona_400.sgc` | `Opel_Ascona_400_Grp4_ngp6` · VisualOnly / UsedOnly | — | 74 / 4 · 13 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Opel Manta 400 GrpB · `rsf-opel-manta-400-grpb` | `Cars/OPEL_MANTA_400/OPEL_MANTA_400.sgc` | `Opel_Manta_400_GrpB_ngp6` · VisualOnly / UsedOnly | — | 76 / 3 · 17 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lb, e_wheelgrunk_rb |
| Peugeot 106 Rallye S20 GrpA · `rsf-peugeot-106-rallye-s20-grpa` | `Cars/Peugeot106_rally/Peugeot106_rally.sgc` | `Peugeot_106_Rallye_S20_GrpA_ngp6` · VisualOnly / UsedOnly | — | 113 / 22 · 38 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Peugeot 205 T16 GrpB · `rsf-peugeot-205-t16-grpb` | `Cars/PEUGEOT_205_T16_e1/PEUGEOT_205_T16_e1.sgc` | `Peugeot_205_T16_GrpB_ngp6` · Drivable / UsedOnly | L | 73 / 4 · 15 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Peugeot 207 S2000 Evolution Plus · `rsf-peugeot-207-s2000-evolution-plus` | `Cars/207_s2000_2012/207_s2000_2012.sgc` | `Peugeot_207_S2000_Evolution_Plus_ngp6` · VisualOnly / UsedOnly | — | 100 / 4 · 41 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Peugeot 208 R2 · `rsf-peugeot-208-r2` | `Cars/PEUGEOT_208_R_2/PEUGEOT_208_R_2.sgc` | `Peugeot_208_R2_ngp6` · Drivable / UsedOnly | L | 93 / 5 · 23 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf |
| Peugeot 208 Rally4 · `rsf-peugeot-208-rally4` | `Cars/PEUGEOT_208_Rally4/PEUGEOT_208_Rally4.sgc` | `Peugeot_208_Rally4_ngp6` · Drivable / Both | L | 96 / 4 · 27 · 3 / 0 | **ERROR DETECTADO** · e_body_black, e_wheelgrunk_lf, e_wheelgrunk_rf |
| Peugeot 208 T16 R5 · `rsf-peugeot-208-t16-r5` | `Cars/208_T16_R5_evo/208_T16_R5_evo.sgc` | `Peugeot_208_T16_R5_ngp7` · VisualOnly / UsedOnly | — | 117 / 9 · 41 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Peugeot 306 Maxi Kit Car · `rsf-peugeot-306-maxi-kit-car` | `Cars/p306maxi/p306maxi.sgc` | `Peugeot_306_Maxi_Kit_Car_ngp6` · Drivable / UsedOnly | — | 68 / 5 · 21 · 0 / 1 | **ERROR DETECTADO** · i_steeringwheel |
| Porsche 911 GT3 RS (2007) RGT · `rsf-porsche-911-gt3-rs-2007-rgt` | `Cars/PORSCHE911GT3RS/PORSCHE911GT3RS.sgc` | `Porsche_911_GT3_RS_2007_RGT_ngp6` · VisualOnly / UsedOnly | — | 76 / 3 · 13 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Porsche 911 GT3 RS (2010) RGT · `rsf-porsche-911-gt3-rs-2010-rgt` | `Cars/PORSCHE_GT3_RGT/PORSCHE_GT3_RGT.sgc` | `Porsche_911_GT3_RS_2010_RGT_ngp7` · VisualOnly / UsedOnly | — | 80 / 4 · 21 · 2 / 2 | **ERROR DETECTADO** · e_door_r, e_door_b, i_body, i_door_b |
| Porsche 911 SC 3.0 Grp4 · `rsf-porsche-911-sc-3-0-grp4` | `Cars/Porsche_911SC/Porsche_911SC.sgc` | `Porsche_911_SC_3.0_Grp4_ngp6` · VisualOnly / UsedOnly | — | 70 / 4 · 13 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Porsche 911 SC RS GrpB · `rsf-porsche-911-sc-rs-grpb` | `Cars/PORSCHE911_SCRS_GrpB/PORSCHE911_SCRS_GrpB.sgc` | `Porsche_911_SC_RS_GrpB_ngp6` · Drivable / UsedOnly | L sin SGC | 84 / 3 · 22 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Renault 5 GT Turbo GrpA · `rsf-renault-5-gt-turbo-grpa` | `Cars/5GTT/5gtt.sgc` | `Renault_5_GT_Turbo_GrpA_ngp6` · VisualOnly / UsedOnly | — | 77 / 5 · 16 · 4 / 1 | **ERROR DETECTADO** · e_body2, e_sparewheel_gravel, i_body |
| Renault Clio 16S Williams GrpA · `rsf-renault-clio-16s-williams-grpa` | `Cars/RENAULT_CLIO/Renault_Clio.sgc` | `Renault_Clio_16S_Williams_GrpA_ngp6` · VisualOnly / UsedOnly | — | 96 / 7 · 31 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Renault Clio III R3 · `rsf-renault-clio-iii-r3` | `Cars/Clio_R3/Clio_R3.sgc` | `Renault_Clio_III_R3_ngp6` · VisualOnly / UsedOnly | — | 112 / 5 · 23 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Renault Clio IV R3T · `rsf-renault-clio-iv-r3t` | `Cars/Clio4_R3T/Clio4_R3T.sgc` | `Renault_Clio_IV_R3T_ngp6` · VisualOnly / UsedOnly | — | 69 / 4 · 14 · 1 / 0 | **ERROR DETECTADO** · e_bumper_f |
| Renault Clio Rally3 · `rsf-renault-clio-rally3` | `Cars/Renault_ClioV_Rally3/Renault_ClioV_Rally3.sgc` | `Renault_Clio_Rally3_ngp7` · VisualOnly / UsedOnly | V (SGC) | 100 / 4 · 32 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf |
| Renault Clio Rally4 · `rsf-renault-clio-rally4` | `Cars/Renault_ClioV_Rally4/Renault_ClioV_Rally4.sgc` | `Renault_Clio_Rally4_ngp6` · Drivable / Both | L | 100 / 4 · 31 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf |
| Renault Clio Rally5 · `rsf-renault-clio-rally5` | `Cars/Renault_ClioV_Rally4/Renault_ClioV_Rally4.sgc` | `Renault_Clio_Rally5_ngp6` · Drivable / NewSalonOnly | L | 100 / 4 · 31 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf |
| Renault Twingo R1 · `rsf-renault-twingo-r1` | `Cars/Twingo_R1/Twingo_R1.sgc` | `Renault_Twingo_R1_ngp6` · VisualOnly / UsedOnly | — | 73 / 5 · 21 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Renault Twingo R2 Evo · `rsf-renault-twingo-r2-evo` | `Cars/Twingo_R2/Twingo_R2.sgc` | `Renault_Twingo_R2_Evo_ngp6` · VisualOnly / UsedOnly | — | 73 / 5 · 21 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Seat Leon Cupra R GrpN · `rsf-seat-leon-cupra-r-grpn` | `Cars/leon_cupra_grn/leon_cupra_grn.sgc` | `Seat_Leon_Cupra_R_GrpN_ngp6` · VisualOnly / UsedOnly | — | 71 / 4 · 15 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Skoda 130 LR GrpB · `rsf-skoda-130-lr-grpb` | `Cars/skoda_130LR/skoda_130LR.sgc` | `Skoda_130_LR_GrpB_ngp6` · VisualOnly / UsedOnly | — | 83 / 4 · 13 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Skoda 130 RS Grp2 · `rsf-skoda-130-rs-grp2` | `Cars/Skoda_130RS/Skoda_130RS.sgc` | `Skoda_130_RS_Grp2_ngp6` · VisualOnly / UsedOnly | — | 73 / 4 · 13 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Skoda Fabia R5 · `rsf-skoda-fabia-r5` | `Cars/Fabia_r5_2018/Fabia_r5_2018.sgc` | `Skoda_Fabia_R5_b_ngp6` · VisualOnly / UsedOnly | — | 86 / 6 · 18 · 2 / 1 | **ERROR DETECTADO** · e_wheelgrunk_lb, e_wheelgrunk_rb, i_internal |
| Skoda Fabia R5 evo · `rsf-skoda-fabia-r5-evo` | `Cars/Fabia_R5_evo/Fabia_R5_evo.sgc` | `Skoda_Fabia_R5_evo_ngp6` · VisualOnly / UsedOnly | — | 89 / 4 · 26 · 2 / 5 | **ERROR DETECTADO** · e_steeringwheel, i_bonnet |
| Skoda Fabia RS Rally2 · `rsf-skoda-fabia-rs-rally2` | `Cars/fabia_rs_rally2/fabia_rs_rally2.sgc` | `Skoda_Fabia_RS_Rally2_ngp7` · Drivable / Both | L | 102 / 7 · 35 · 18 / 8 | **ERROR DETECTADO** · e_wheel_gravel_lf, i_body |
| Skoda Fabia S2000 Evo 2 · `rsf-skoda-fabia-s2000-evo-2` | `Cars/Fabia_S2000_2012/Fabia_S2000_2012.sgc` | `Skoda_Fabia_S2000_Evo_2_ngp6` · VisualOnly / UsedOnly | — | 115 / 8 · 30 · 2 / 0 | **ERROR DETECTADO** · e_door_l, e_door_r |
| Skoda Fabia WRC 2006 · `rsf-skoda-fabia-wrc-2006` | `Cars/FABIA06/fabia06.sgc` | `Skoda_Fabia_WRC_2006_ngp6` · VisualOnly / UsedOnly | L | 89 / 3 · 18 · 1 / 0 | **ERROR DETECTADO** · e_bonnet |
| Skoda Favorit 136 L GrpA · `rsf-skoda-favorit-136-l-grpa` | `Cars/Favorit/Favorit.sgc` | `Skoda_Favorit_136_L_GrpA_ngp6` · VisualOnly / UsedOnly | — | 91 / 5 · 24 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Subaru Impreza GC8 555 GrpA · `rsf-subaru-impreza-gc8-555-grpa` | `Cars/impreza95/impreza95.sgc` | `Subaru_Impreza_GC8_555_GrpA_ngp6` · VisualOnly / UsedOnly | — | 91 / 4 · 27 · 1 / 1 | **ERROR DETECTADO** · e_wingi, i_bonneti |
| Subaru Impreza GDA WRC2003 (S9) · `rsf-subaru-impreza-gda-wrc2003-s9` | `Cars/impreza_2003_kazhol/impreza03.sgc` | `Subaru_Impreza_GDA_WRC2003_S9_2006_ngp6` · Drivable / UsedOnly | L | 96 / 3 · 19 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Subaru Impreza N14 N4 · `rsf-subaru-impreza-n14-n4` | `Cars/Impreza_N14_Pro/impreza_N14.sgc` | `Subaru_Impreza_N14_N4_ngp6` · VisualOnly / UsedOnly | — | 84 / 3 · 19 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lb, e_wheelgrunk_rb |
| Subaru Impreza N15 R4 · `rsf-subaru-impreza-n15-r4` | `Cars/Impreza_N14_Pro/impreza_N14.sgc` | `Subaru_Impreza_N15_R4_ngp6` · VisualOnly / UsedOnly | — | 84 / 3 · 19 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lb, e_wheelgrunk_rb |
| Toyota Celica 2000GT(ST185) GrpA · `rsf-toyota-celica-2000gt-st185-grpa` | `Cars/CELICA_st185/CELICA_st185.sgc` | `Toyota_Celica_2000GT-FOUR_RC_ST185_GrpA_ngp6` · VisualOnly / UsedOnly | — | 99 / 6 · 58 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Toyota Celica TCT (TA64) GrpB · `rsf-toyota-celica-tct-ta64-grpb` | `Cars/TOYOTA_CELICA_TA64_GrpB/TOYOTA_CELICA_TA64_GrpB.sgc` | `Toyota_Celica_TCT_TA64_GrpB_ngp6` · Drivable / UsedOnly | L | 86 / 4 · 26 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Toyota GR Yaris Rally2 · `rsf-toyota-gr-yaris-rally2` | `Cars/yaris_rally2/yaris_rally2.sgc` | `Toyota_GR_Yaris_Rally2_ngp7` · Drivable / Both | — | 94 / 8 · 29 · 8 / 0 | **ERROR DETECTADO** · e_wheel_gravel_lf, e_wheel_gravel_lb |
| Toyota Yaris WRC 2018 · `rsf-toyota-yaris-wrc-2018` | `Cars/YARIS_WRC18/yaris_wrc.sgc` | `Toyota_Yaris_WRC_2018_ngp6` · Drivable / NewSalonOnly | — | 185 / 6 · 35 · 2 / 2 | **ERROR DETECTADO** · e_light_lf, e_light_rf, i_bonnet, i_bodye |
| Trabant P 800 RS GrpA · `rsf-trabant-p-800-rs-grpa` | `Cars/Trabant_P800RS/Trabant_P800RS.sgc` | `Trabant_P_800_RS_GrpA_ngp6` · VisualOnly / UsedOnly | — | 75 / 4 · 15 · 1 / 0 | **ERROR DETECTADO** · e_stoplever |
| Volvo 240 Turbo GrpA · `rsf-volvo-240-turbo-grpa` | `Cars/Volvo242LE/Volvo242LE.sgc` | `Volvo_240_Turbo_GrpA_ngp7` · VisualOnly / UsedOnly | — | 87 / 4 · 28 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| VW Golf II GTI 16V GrpA · `rsf-vw-golf-ii-gti-16v-grpa` | `Cars/vw_golf_86/vw_golf_86.sgc` | `VW_Golf_II_GTI_16V_GrpA_ngp6` · VisualOnly / UsedOnly | — | 91 / 4 · 22 · 8 / 4 | **ERROR DETECTADO** · e_steeringwheel, e_wheel_gravel_lf, i_body |
| VW Polo GTI R5 · `rsf-vw-polo-gti-r5` | `Cars/Polo_gti_r5/Polo_gti_r5.sgc` | `VW_Polo_GTI_R5_ngp6` · VisualOnly / UsedOnly | — | 85 / 4 · 25 · 2 / 7 | **ERROR DETECTADO** · e_steeringwheel, i_bonnet |
| VW Polo R WRC 2016 · `rsf-vw-polo-r-wrc-2016` | `Cars/POLO_R_WRC_2016/polo_wrc_2016.sgc` | `VW_Polo_R_WRC_2016_ngp6` · Drivable / PendingCommercialRule | L | 73 / 5 · 19 · 2 / 0 | **ERROR DETECTADO** · e_wheelgrunk_lf, e_wheelgrunk_rf |
| Wartburg 353 W 460 GrpA · `rsf-wartburg-353-w-460-grpa` | `Cars/Wartburg_353/Wartburg_353.sgc` | `Wartburg_353_W_460_GrpA_ngp6` · VisualOnly / UsedOnly | — | 79 / 4 · 13 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Subaru Impreza WRC 2000 · `subaru-impreza-2000` | `Cars/impreza00/impreza00.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 96 / 3 · 25 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Subaru Impreza WRC 2003 · `subaru-impreza-2003` | `Cars/impreza03/impreza03.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 90 / 3 · 19 · 0 / 0 | CORRECTO ESTRUCTURALMENTE |
| Toyota Corolla WRC · `toyota-corolla-wrc` | `Cars/COROLLA/corolla.sgc` | — · Drivable / UsedOnly | L sin SGC, V sin SGC | 99 / 3 · 26 · 0 / 0 | **ERROR DETECTADO** · CAR: selected model mismatch for toyota-corolla-wrc |
