# Inventario de vehículos y físicas RSF

21 de noviembre de 2026 · Rama `audit/rsf-vehicle-visuals` · Base de esta revisión `7bf5ef2`

## Fuentes existentes

| Origen | Contenido |
| --- | --- |
| `Desktop/Respaldo RBR iPad` | Accesible. 1.975 carpetas y 6.863 archivos por debajo de la raíz. |
| `Desktop/Respaldo RBR iPad/Cars 2` | Accesible. 1.974 carpetas y los mismos 6.863 archivos. La carpeta interior `Cars 2` tiene 45 directorios de primer nivel y 3.431 archivos. `__MACOSX` contiene 3.432 metadatos AppleDouble, incluido el del directorio exportado. |
| Instalación PC `C:/Richard Burns Rally` | Referencia de 111 registros Mobile, 110 SGC en disco, caché RSF y paquetes NGP locales. |
| Transferencia `RBR Mobile - Autos Listos para iPad` | Referencia complementaria. 30 SGC repartidos entre las dos raíces exportadas y 30 carpetas de física en la exportación de conducción. |
| Repositorio `ReRBR-iOS-Probe` | Descriptores, cargadores, catálogo y pruebas. La consulta a Git devolvió cero archivos versionados con extensión SGC, DDS o LSP. |

El contenido real del respaldo comprende 2.038 INI, 723 DDS, 464 JPG, 117 TXT, 47 SGC, 26 BMP, ocho BAK, cuatro DB, dos EXE y dos GIF. La auditoría registró los ejecutables como archivos y leyó los recursos del coche como datos. Los 47 SGC coinciden por SHA-256 con archivos de PC y representan 42 contenidos de geometría distintos. Hay 31 rutas SGC del respaldo cuyo contenido coincide con alguna de las 30 rutas de las exportaciones, porque el respaldo incluye copias adicionales. El directorio `fiesta_rally2` contiene presets de piezas y carece de SGC propio. Los ocho slots de `Cars.ini` del respaldo apuntan a los modelos originales pertinentes.

La inspección visual de CPU ejecutó el cargador Mobile sobre una copia de trabajo de los SGC, INI y DDS del respaldo. El inventario de hashes se obtuvo directamente de la fuente original. El respaldo carece de LSP, ZIP de física y caché RSF. Las disponibilidades de física se midieron en sus respectivas raíces, PC y exportaciones. Los perfiles de Carrera del iPad siguen pendientes de cotejo.

## Duplicados, variantes y altas

| Grupo | Evidencia y resultado |
| --- | --- |
| Citroën DS3 WRC | `DS3_WRC` y `DS3_WRC 2` tienen SGC, INI activos y DDS planos idénticos. El catálogo agrupa ambos IDs en una fila. La copia `without caps` comparte SGC. `with caps` añade cuatro submallas de 464 triángulos y se conserva como variante de asfalto. |
| Citroën Xsara WRC 2006 | `Xsara_2006` y `Xsara_2006 2` coinciden en SGC, INI activos y DDS planos. El catálogo agrupa ambos IDs. El Xsara original de `Car00` es otro descriptor y se conserva. |
| Ford Fiesta Rally2 2023 | Las carpetas `fiesta_rally2_23` y `fiesta_rally2_23 2` tienen el mismo SGC e INI activo. Los DDS del segundo directorio completan un conjunto distinto del primero. Se conservan los dos orígenes. El showroom carga la copia completa mediante sus DDS locales cuando falta la ruta declarada del paquete gemelo. |
| Škoda Fabia RS Rally2 | `fabia_rs_rally2 2.sgc` tiene el mismo SHA-256 que el SGC principal. La búsqueda del primer SGC podía dejar fuera la carpeta al no encontrar los INI con el nombre derivado de esa copia. El catálogo ahora prefiere el SGC asociado a una configuración utilizable. |
| Lancer Evo IX, Impreza N14/N15 y Clio Rally4/Rally5 | Cada par comparte modelo visual y declara packs distintos en RSF. Se conservaron los seis IDs como variantes físicas. |
| Lancer Evo X V1.21 | Las dos rutas PC contienen el mismo SGC y los mismos INI. Una entrada anuncia un pack propio y la otra es una entrada visual. Las seis rutas DDS declaradas se resuelven como se verificó en la auditoría anterior. Se conservaron ambas referencias de catálogo. |

El garaje de Carrera aplica la misma agrupación que el concesionario y prefiere un ID que el perfil ya posea. Los IDs anteriores siguen resolviendo sus descriptores y se conservan como claves de guardado. Una entrada ofrecida en un concesionario y también visible como propiedad en el garaje representa el mismo coche en dos pantallas. El informe de física distingue ese caso del uso de un modelo con varios packs.

El catálogo PC tiene 111 descriptores y 111 filas visibles. En el respaldo se identificaron 44 descriptores y quedaron 42 filas visibles, con **dos duplicados reales agrupados**. Las otras tres repeticiones físicas de SGC son la copia interna del Fabia, la opción de tapas `without caps` y la carpeta parcial del Fiesta. Estas repeticiones se mantienen en el inventario de archivos. La comparación incluyó rutas y nombres, bytes de SGC, INI y DDS, y la existencia de una referencia NGP independiente.

## Significado de los estados de física

- **Verificada**. Están disponibles y son legibles `common.lsp`, los tres LSP de superficie, sus tres opciones `r_*.lsp`, sus tres setups predeterminados y `repair.ini`. La caché local enlaza un pack único con el ID RSF, el ID de modelo y la ruta SGC exacta, y está presente el modelo propio. Se trata de una verificación de asociación de archivos.
- **Pendiente**. Los once archivos están disponibles y queda por demostrar la asociación exacta entre modelo y pack en esa raíz.
- **Incompleta**. Falta un elemento de ese conjunto o el lector devuelve un error de legibilidad o sintaxis. El diagnóstico imprime la causa y los archivos afectados.
- **No encontrada**. El nombre del pack se conoce, pero el resolver local carece de un paquete para esa referencia.
- **Desconocida**. La entrada es visual y carece de un pack identificado en la evidencia de esa raíz.
- **Retail**. La asociación se comprueba con el slot de `Cars.ini`. Los ocho vehículos originales se mantienen fuera de los totales NGP.

El diagnóstico conserva los `CarId` de las superficies y los SHA-256 de `common`, del LSP de superficie y de sus opciones. Un `CarId` almacenado sirve como evidencia comparativa. La ejecución de NGP y la verificación en el dispositivo quedan pendientes. La clasificación de catálogo ahora exige el conjunto mínimo de grava que ya comprueba el resolver. El recuento de inventario comprueba además asfalto, nieve y reparación.

| Origen | Entradas | Retail | NGP verificada | NGP pendiente | NGP incompleta | NGP no encontrada | Desconocida |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| PC | 111 | 8 | 41 | 0 | 0 | 61 | 1 |
| Transferencia para conducir | 39 | 8 | 27 | 3 | 0 | 1 | 0 |
| Transferencia visual | 11 | 8 | 0 | 0 | 0 | 1 | 2 |
| Respaldo | 44 | 8 | 0 | 0 | 0 | 1 | 35 |

Las 61 referencias NGP ausentes en PC son las candidatas a conseguir en el futuro si se desea conducir esas entradas. La lista detallada conserva el nombre de cada pack. En la exportación para conducir están los packs de Fiesta Rally2, Fiesta Mk VIII R2 y Porsche 911 SC RS GrpB, pero falta allí su asociación exacta con el SGC del descriptor. En el respaldo, la entrada Focus 2006 ahora identifica el pack como ausente. Buscar por «Focus» podía tomar la carpeta visual `Cars/FORD_FOCUS_2006` como una física incompleta, aun sin LSP. El caso está cubierto por una prueba de regresión. El Corolla tiene su slot correcto en `Cars.ini` del respaldo y el slot PC apunta al Celica, como documentó el barrido original.

## Resultado visual y pruebas

El diagnóstico cargó 43 de 44 exteriores y 43 de 44 interiores del respaldo. Hay 21 entradas correctas estructuralmente, 22 con caras sin asociación y una con DDS insuficientes, `fiesta_rally2_23`. Esa carpeta carece del conjunto activo `textures` del Fiesta. La copia `fiesta_rally2_23 2` conserva el conjunto y carga en CPU. El SGC numerado del Fabia coincide con el principal y se inventarió por separado. El catálogo usa el conjunto completo del Fabia.

El BMW E30 del respaldo pasó la auditoría de materiales y cockpit. Los dos DS3 del respaldo conservaron la misma incidencia única en la antena. Los presets `antenna_on` y `antenna_off` del respaldo también declaran un solo FaceBlock para esa pieza. El Evo X se comprobó en la instalación PC. Las regresiones reales de BMW, DS3 y Evo X pasaron. Las verificaciones de render y guardados en el iPad siguen pendientes.

- Suite de biblioteca con 194 aprobadas y 16 ignoradas, seguida de la regresión específica del Focus con el último ajuste.
- Formato, Clippy con `physics-stage,rerbr-3d`, comprobación `aarch64-apple-ios` y prueba del manifiesto con altas, cambios, bajas y archivos sin cambios.
- `1f70500` corrige el DDS local del Fiesta. `124fe98` agrupa copias verificadas, recupera el Fabia y exige las dependencias de grava. `b320f6e` corrige la detección del pack Focus. `6d62043` añade el inventario repetible.

## Repetir la auditoría

Desde el repositorio, con PowerShell. Elegir una ruta de salida de trabajo fuera del respaldo.

```powershell
New-Item -ItemType Directory -Force .validation/rsf-inventory | Out-Null
$inventory = [scriptblock]::Create((Get-Content scripts/inventory-vehicle-files.ps1 -Raw))
& $inventory -Root 'C:/Users/adria/Desktop/Respaldo RBR iPad' |
  Set-Content -Encoding UTF8 .validation/rsf-inventory/inventory.csv
& $inventory -Root 'C:/Users/adria/Desktop/Respaldo RBR iPad' -Previous .validation/rsf-inventory/inventory.csv |
  Set-Content -Encoding UTF8 .validation/rsf-inventory/inventory-next.csv
```

El segundo comando marca `Added`, `Changed`, `Removed` y `Unchanged` por ruta y SHA-256. Un cambio de nombre aparece como alta y baja con el mismo hash. Comparar las altas de SGC y DDS con los grupos del inventario y después comprobar los metadatos y las físicas para decidir si corresponde un vehículo nuevo, una variante o una copia. La herramienta sólo lee el origen y escribe al flujo de salida.

Para cotejar el mismo conjunto con los lectores Mobile, apuntar `VEHICLE_AUDIT_DOCUMENTS` al padre de `RBR` en cada origen de trabajo.

```powershell
cargo test --features physics-stage --lib audit_rsf_physics_inventory -- --ignored --nocapture
cargo test --features physics-stage --lib audit_vehicle_resources -- --ignored --nocapture
cargo test --features physics-stage --lib audit_vehicle_interiors -- --ignored --nocapture
```

La salida contiene la ruta escogida, estado de visibilidad en el catálogo, dependencias ausentes, IDs RSF/modelo y hashes de física. Los volcados locales de esta revisión están en `.validation/rsf-inventory/`. Las 6.863 filas del manifiesto contienen el SHA-256 y tamaño de cada archivo, incluidas las variantes y los metadatos AppleDouble.

## Catálogo PC cotejado con el respaldo y la transferencia

«Respaldo» indica coincidencia SHA-256 con un SGC de esa fuente. La columna Texturas cuenta los DDS de exterior preparados en PC. «Incidencias» muestra las caras sin asociación de exterior/interior del barrido visual previo. La columna física muestra el estado PC seguido por el de la transferencia para conducir. Cuando hay IDs, el primer par es coche RSF/modelo RSF. El trío siguiente son los CarId de grava, asfalto y nieve del pack PC.

| Vehículo e ID | Modelo | Texturas | Ruedas | Física NGP | Duplicado | Estado visual CPU |
| --- | --- | ---: | --- | --- | --- | --- |
| Citroen Xsara (Car00) · `citroen-xsara` | `Cars/xsara/xsara.sgc` · respaldo | 3 | 4 grava | Retail 0 | — | Estructural |
| Ford Focus Mk II RS WRC 2006 · `ford-focus-rs-wrc-2006` | `Cars/FORD_FOCUS_2006/focus06.sgc` · respaldo | 6 | 4 grava | Verificada / No encontrada · `Ford_Focus_Mk_II_RS_WRC_2006_ngp6` (55/17) · 745426673/2929182514/2933989622 | — | Estructural |
| Hyundai Accent WRC · `hyundai-accent-wrc` | `Cars/accent/accent.sgc` · respaldo | 3 | 4 grava | Retail 1 | — | Estructural |
| MG ZR Super 1600 · `mg-zr-s1600` | `Cars/zr/zr.sgc` · respaldo | 3 | 4 grava | Retail 2 | — | Estructural |
| Mitsubishi Lancer Evo VII · `mitsubishi-lancer-evo-vii` | `Cars/Lancer_EVO_VII/lancer_evo_vII.sgc` · respaldo | 3 | 4 grava | Retail 3 | — | Estructural |
| Peugeot 206 WRC · `peugeot-206-wrc` | `Cars/Peugeot_206/Peugeot_206.sgc` · respaldo | 3 | 4 grava | Retail 4 | — | Estructural |
| Abarth Grande Punto S2000 · `rsf-abarth-grande-punto-s2000` | `Cars/g.punto_s2000/g.punto_s2000.sgc` | 3 | 4 grava | No encontrada / — · `Abarth_Grande_Punto_S2000_ngp6` | — | Incidencias 4/1 |
| Alpine A110 Rally RGT · `rsf-alpine-a110-rally-rgt` | `Cars/Alpine_A110_2020/Alpine_A110_2020.sgc` | 4 | 4 grava | No encontrada / — · `Alpine_A110_Rally_RGT_ngp6` | — | Incidencias 1/0 |
| Aston Martin Vantage RGT · `rsf-aston-martin-vantage-rgt` | `Cars/AstonMartin_Vantage_RGT/AstonMartin_Vantage_RGT.sgc` | 4 | 4 grava | No encontrada / — · `Aston_Martin_Vantage_RGT_ngp6` | — | Incidencias 2/0 |
| Audi 200 quattro GrpA · `rsf-audi-200-quattro-grpa` | `Cars/Audi_200_quattro/Audi_200_quattro.sgc` · respaldo | 3 | 4 grava | Verificada / Verificada · `Audi_200_quattro_GrpA_ngp6` (72/165) · 1302218286/1626991612/4192566596 | — | Incidencias 0/2 |
| Audi quattro A1 GrpB · `rsf-audi-quattro-a1-grpb` | `Cars/AUDI_QUATTRO_A1_GrpB/AUDI_QUATTRO_A1_GrpB.sgc` · respaldo | 3 | 4 grava | Verificada / Verificada · `Audi_quattro_A1_GrpB_ngp6` (66/192) · 3973610360/786674529/4045343425 | — | Estructural |
| Audi quattro A2 GrpB · `rsf-audi-quattro-a2-grpb` | `Cars/AUDI_QUATTRO_A2_GrpB/AUDI_QUATTRO_A2_GrpB.sgc` · respaldo | 6 | 4 grava | Verificada / Verificada · `Audi_quattro_A2_GrpB_ngp6` (67/236) · 4168722626/3105715918/3854489979 | — | Estructural |
| Audi quattro Grp4 · `rsf-audi-quattro-grp4` | `Cars/AUDI_QUATTRO_Grp4/AUDI_QUATTRO_Grp4.sgc` | 3 | 4 grava | No encontrada / — · `Audi_quattro_Grp4_ngp6` | — | Estructural |
| Audi Sport quattro GrpB · `rsf-audi-sport-quattro-grpb` | `Cars/Audi_Sport_Quattro/Audi_Sport_Quattro.sgc` · respaldo | 4 | 4 grava | Verificada / Verificada · `Audi_Sport_quattro_GrpB_ngp6` (68/131) · 2617269856/1168891795/775465168 | — | Estructural |
| BMW M1 GrpB · `rsf-bmw-m1-grpb` | `Cars/BMW_M1/BMW_M1.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `BMW_M1_GrpB_ngp6` (99/256) · 215003388/1496829852/599322509 | — | Estructural |
| BMW M3 E30 GrpA · `rsf-bmw-m3-e30-grpa` | `Cars/BMW_M3_E30/BMW_M3_E30.sgc` · respaldo | 20 | 4 grava | Verificada / Verificada · `BMW_M3_E30_GrpA_ngp6` (87/97) · 2140769171/2941775201/3835131910 | — | Estructural |
| BMW M3 E36 GrpA · `rsf-bmw-m3-e36-grpa` | `Cars/bmw_m3_e36/bmw_m3_e36.sgc` | 6 | 4 grava | No encontrada / — · `BMW_M3_E36_GrpA_ngp6` | — | Incidencias 1/2 |
| Citroen C2 GT S1600 · `rsf-citroen-c2-gt-s1600` | `Cars/CITROEN_C2_s1600/C2s1600.sgc` | 4 | 4 grava | No encontrada / — · `Citroen_C2_GT_S1600_ngp6` | — | Estructural |
| Citroen C2 R2 Max · `rsf-citroen-c2-r2-max` | `Cars/C2_R2_Max/C2_R2_Max.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `Citroen_C2_R2_Max_ngp6` (27/115) · 3067020584/3556554519/4027730251 | — | Incidencias 0/3 |
| Citroen C3 R5 · `rsf-citroen-c3-r5` | `Cars/C3_r5/C3_r5.sgc` | 6 | 4 grava | No encontrada / — · `Citroen_C3_R5_ngp6` | — | Incidencias 2/3 |
| Citroen C3 WRC 2017 · `rsf-citroen-c3-wrc-2017` | `Cars/Citroen_C3_WRC17/Citroen_C3_WRC17.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `Citroen_C3_WRC_2017_ngp6` (14/191) · 3612858988/1283615833/1440265323 | — | Estructural |
| Citroen C4 WRC 2008 · `rsf-citroen-c4-wrc-2008` | `Cars/C4_08/c4_08.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `Citroen_C4_WRC_2008_ngp6` (12/257) · 3341473789/3684603128/1168488954 | — | Incidencias 1/1 |
| Citroen DS3 R1 · `rsf-citroen-ds3-r1` | `Cars/Citroen_DS3_R1/Citroen_DS3_R1.sgc` | 5 | 4 grava | No encontrada / — · `Citroen_DS3_R1_ngp6` | — | Incidencias 4/0 |
| Citroen DS3 R3-MAX · `rsf-citroen-ds3-r3-max` | `Cars/Citroen_DS3_R3/Citroen_DS3_R3.sgc` | 5 | 4 grava | No encontrada / — · `Citroen_DS3_R3-MAX_ngp6` | — | Incidencias 4/0 |
| Citroen DS3 R5 · `rsf-citroen-ds3-r5` | `Cars/DS3_R5/ds3_r5.sgc` | 5 | 4 grava | No encontrada / — · `Citroen_DS3_R5_ngp6` | — | Incidencias 10/1 |
| Citroen DS3 WRC · `rsf-citroen-ds3-wrc` | `Cars/DS3_WRC/ds3_wrc.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `Citroen_DS3_WRC_ngp6` (8/78) · 108427400/3313572964/2230916751 | Copia numerada en respaldo | Incidencias 1/0 |
| Citroen Xsara Kit Car · `rsf-citroen-xsara-kit-car` | `Cars/Citroen_Xsara_KitCar/Citroen_Xsara_KitCar.sgc` | 4 | 4 grava | No encontrada / — · `Citroen_Xsara_Kit_Car_ngp6` | — | Estructural |
| Citroen Xsara WRC 2006 · `rsf-citroen-xsara-wrc-2006` | `Cars/Xsara_2006/xsara.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `Citroen_Xsara_WRC_2006_ngp6` (13/18) · 2711024201/272024919/588880974 | Copia numerada en respaldo | Estructural |
| Fiat 124 Abarth Rally RGT · `rsf-fiat-124-abarth-rally-rgt` | `Cars/124_Abarth_RGT/124_Abarth_RGT.sgc` | 5 | 4 grava | No encontrada / — · `Fiat_124_Abarth_Rally_RGT_b_ngp6` | — | Incidencias 1/0 |
| Fiat 126 Grp2 · `rsf-fiat-126-grp2` | `Cars/126P/126p.sgc` | 4 | 4 grava | No encontrada / — · `Fiat_126_Grp2_ngp6` | — | Incidencias 0/1 |
| Fiat 131 Abarth Grp4 · `rsf-fiat-131-abarth-grp4` | `Cars/131_Abarth_Gr4/131_Abarth_Gr4.sgc` | 5 | 4 grava | No encontrada / — · `Fiat_131_Abarth_Grp4_ngp6` | — | Incidencias 0/4 |
| Fiat Abarth 500 R3T · `rsf-fiat-abarth-500-r3t` | `Cars/500_Abarth_R3T/500_Abarth_R3T.sgc` | 4 | 4 grava | No encontrada / — · `Fiat_Abarth_500_R3T_ngp6` | — | Incidencias 1/0 |
| Ford Escort Mk II RS Grp4 · `rsf-ford-escort-mk-ii-rs-grp4` | `Cars/Escort_mk_2/Escort_mk_2.sgc` | 4 | 4 grava | No encontrada / — · `Ford_Escort_Mk_II_RS_Grp4_ngp6` | — | Incidencias 14/2 |
| Ford Escort Mk V RS Cosworth Grp · `rsf-ford-escort-mk-v-rs-cosworth-grp` | `Cars/ESCORT_COSWORTH_GRA/Escort_Cosworth_GrA.sgc` | 6 | 4 grava | Verificada / — · `Ford_Escort_Mk_V_RS_Cosworth_GrpA_ngp6` (43/40) · 113807539/187692327/3335676137 | — | Estructural |
| Ford Fiesta Mk VIII R2 · `rsf-ford-fiesta-mk-viii-r2` | `Cars/Fiesta_R2_2019/Fiesta_R2_2019.sgc` · respaldo | 6 | 4 grava | Verificada / Pendiente · `Ford_Fiesta_Mk_VIII_R2_ngp6` (71/237) · 684254075/3657367308/2628520853 | — | Incidencias 0/5 |
| Ford Fiesta Mk VI S2000 · `rsf-ford-fiesta-mk-vi-s2000` | `Cars/Fiesta_S2000/fiesta_s2000.sgc` | 5 | 4 grava | No encontrada / — · `Ford_Fiesta_Mk_VI_S2000_ngp6` | — | Incidencias 3/3 |
| Ford Fiesta R2 · `rsf-ford-fiesta-r2` | `Cars/fiesta_r2/fiesta_r2.sgc` | 5 | 4 grava | No encontrada / — · `Ford_Fiesta_R2_ngp6` | — | Incidencias 0/1 |
| Ford Fiesta R5 · `rsf-ford-fiesta-r5` | `Cars/F_Fiesta_R5/F_Fiesta_R5.sgc` | 7 | 4 grava | No encontrada / — · `Ford_Fiesta_R5_ngp6` | — | Estructural |
| Ford Fiesta Rally2 · `rsf-ford-fiesta-rally2` | `Cars/fiesta_rally2_23/fiesta_rally2.sgc` · respaldo | 7 | 4 grava | Verificada / Pendiente · `Ford_Fiesta_Rally2_ngp7` (81/262) · 101712350/1728254095/3267675554 | — | Incidencias 4/8 |
| Ford Fiesta Rally3 · `rsf-ford-fiesta-rally3` | `Cars/Fiesta_rally3/Fiesta_rally3.sgc` | 6 | 4 grava | Verificada / — · `Ford_Fiesta_Rally3_ngp6` (88/251) · 172146992/2685473672/365364476 | — | Incidencias 1/1 |
| Ford Fiesta rally3 evo · `rsf-ford-fiesta-rally3-evo` | `Cars/fiesta_rally3_evo/fiesta_rally3_evo.sgc` · respaldo | 8 | 4 grava | Verificada / Verificada · `Ford_Fiesta_Rally3_Evo_ngp7` (109/266) · 3499885757/2463236954/1495708472 | — | Incidencias 0/2 |
| Ford Fiesta Rally4 · `rsf-ford-fiesta-rally4` | `Cars/Fiesta_rally4/Fiesta_rally4.sgc` · respaldo | 6 | 4 grava | Verificada / Verificada · `Ford_Fiesta_Rally4_ngp6` (80/244) · 920865160/1200057977/2189926758 | — | Incidencias 1/1 |
| Ford Fiesta RS WRC 2014 · `rsf-ford-fiesta-rs-wrc-2014` | `Cars/fiesta_wrc_14/fiesta_wrc_14.sgc` · respaldo | 6 | 4 grava | Verificada / Verificada · `Ford_Fiesta_RS_WRC_2014_ngp6` (9/157) · 1548576775/2679770347/3737257472 | — | Incidencias 2/1 |
| Ford Fiesta WRC 2019 · `rsf-ford-fiesta-wrc-2019` | `Cars/fiesta_wrc_19/fiesta_wrc_19.sgc` · respaldo | 6 | 4 grava | Verificada / Verificada · `Ford_Fiesta_WRC_2019_ngp6` (11/226) · 998256140/4223343362/3104784395 | — | Incidencias 3/3 |
| Honda Civic Type R(FN2) R3 · `rsf-honda-civic-type-r-fn2-r3` | `Cars/HONDA_Civic_R3/Honda_Civic_R3.sgc` | 5 | 4 grava | No encontrada / — · `Honda_Civic_Type_R_FN2_R3_ngp6` | — | Incidencias 0/4 |
| Hyundai i20 Coupe WRC 2017 · `rsf-hyundai-i20-coupe-wrc-2017` | `Cars/i20_WRC_17/i20_wrc_17.sgc` | 11 | 4 grava | No encontrada / — · `Hyundai_i20_Coupe_WRC_2017_ngp6` | — | Incidencias 6/1 |
| Hyundai i20 Coupe WRC 2020 · `rsf-hyundai-i20-coupe-wrc-2020` | `Cars/i20_WRC_20/i20_wrc_20.sgc` · respaldo | 13 | 4 grava | Verificada / Verificada · `Hyundai_i20_Coupe_WRC_2020_ngp6` (76/239) · 2952307149/1875088579/762826698 | — | Incidencias 8/0 |
| Hyundai i20 Coupe WRC 2021 · `rsf-hyundai-i20-coupe-wrc-2021` | `Cars/i20_WRC_21/i20_wrc_21.sgc` | 16 | 4 grava | No encontrada / — · `Hyundai_i20_Coupe_WRC_2021_ngp7` | — | Estructural |
| Hyundai i20 N Rally2 · `rsf-hyundai-i20-n-rally2` | `Cars/i20_rally2/i20_rally2.sgc` | 14 | 4 grava | No encontrada / — · `Hyundai_i20_N_Rally2_ngp7` | — | Incidencias 6/0 |
| Hyundai i20 R5 · `rsf-hyundai-i20-r5` | `Cars/i20_R5/i20_R5.sgc` | 11 | 4 grava | No encontrada / — · `Hyundai_i20_R5_ngp6` | — | Incidencias 6/0 |
| Lada Kalina RC2 GrpA · `rsf-lada-kalina-rc2-grpa` | `Cars/kalina_rc2/kalina_rc2.sgc` | 5 | 4 grava | No encontrada / — · `Lada_Kalina_RC2_GrpA_ngp6` | — | Incidencias 0/1 |
| Lada VFTS GrpB · `rsf-lada-vfts-grpb` | `Cars/Lada_VFTS_NGP/Lada_VFTS_NGP.sgc` | 7 | 4 grava | Verificada / — · `Lada_VFTS_GrpB_ngp6` (41/134) · 804276484/1479243279/1764558183 | — | Incidencias 1/0 |
| LANCER EVO X · `rsf-lancer-evo-x` | `Cars/LANCER_EVO_X/lancer_evo_X.sgc` | 6 | 4 grava | Desconocida / — | SGC gemelo · IDs distintos | Incidencias 4/0 |
| Lancia Delta HF 4WD GrpA · `rsf-lancia-delta-hf-4wd-grpa` | `Cars/delta_4wd/delta_4wd.sgc` | 5 | 4 grava | Verificada / — · `Lancia_Delta_HF_4WD_GrpA_ngp6` (73/166) · 2130515654/1798113535/1035259342 | — | Incidencias 3/2 |
| Lancia Stratos HF Grp4 · `rsf-lancia-stratos-hf-grp4` | `Cars/Lancia_stratos/Lancia_Stratos.sgc` | 4 | 4 grava | No encontrada / — · `Lancia_Stratos_HF_Grp4_ngp6` | — | Incidencias 6/0 |
| Lotus Exige S RGT · `rsf-lotus-exige-s-rgt` | `Cars/Lotus_Exige/Lotus_Exige.sgc` | 4 | 4 grava | No encontrada / — · `Lotus_Exige_S_RGT_ngp6` | — | Incidencias 0/2 |
| Mazda 323 BF 4WD Turbo GrpA · `rsf-mazda-323-bf-4wd-turbo-grpa` | `Cars/MAZDA_323_BFMR/MAZDA_323_BFMR.sgc` | 4 | 4 grava | No encontrada / — · `Mazda_323_BF_4WD_Turbo_GrpA_ngp6` | — | Incidencias 1/0 |
| Mini JCW WRC · `rsf-mini-jcw-wrc` | `Cars/MINI_WRC01B/mini_wrc01b.sgc` | 7 | 4 grava | Verificada / — · `Mini_JCW_WRC_ngp6` (101/160) · 438493153/3720470751/572235235 | — | Incidencias 2/0 |
| Mitsubishi Lancer Evo II GrpA · `rsf-mitsubishi-lancer-evo-ii-grpa` | `Cars/LANCER_EVO_II/LANCER_EVO_II.sgc` | 4 | 4 grava | Verificada / — · `Mitsubishi_Lancer_Evo_II_GrpA_ngp6` (49/173) · 2082326903/2670508454/3154589997 | — | Incidencias 0/4 |
| Mitsubishi Lancer Evo IX N4 · `rsf-mitsubishi-lancer-evo-ix-n4` | `Cars/evo_IX/evo_IX.sgc` | 5 | 4 grava | No encontrada / — · `Mitsubishi_Lancer_Evo_IX_N4_ngp6` | Variante con pack propio | Estructural |
| Mitsubishi Lancer Evo IX R4 · `rsf-mitsubishi-lancer-evo-ix-r4` | `Cars/evo_IX/evo_IX.sgc` | 5 | 4 grava | No encontrada / — · `Mitsubishi_Lancer_Evo_IX_R4_ngp6` | Variante con pack propio | Estructural |
| Mitsubishi Lancer Evo X R4 · `rsf-mitsubishi-lancer-evo-x-r4` | `Cars/LANCER_EVO_X_v1_21/lancer_evo_X.sgc` | 6 | 4 grava | No encontrada / — · `Mitsubishi_Lancer_Evo_X_R4_V1.2_ngp6` | SGC gemelo · IDs distintos | Incidencias 4/0 |
| Opel ADAM R2 · `rsf-opel-adam-r2` | `Cars/Opel_Adam_R2/Opel_Adam_R2.sgc` · respaldo | 4 | 4 grava | Verificada / Verificada · `Opel_ADAM_R2_ngp6` (29/145) · 1984340682/3129611095/815525545 | — | Incidencias 1/0 |
| Opel Ascona 400 Grp4 · `rsf-opel-ascona-400-grp4` | `Cars/Opel_Ascona_400/Opel_Ascona_400.sgc` | 4 | 4 grava | Verificada / — · `Opel_Ascona_400_Grp4_ngp6` (19/137) · 1760736679/3394418942/1924036083 | — | Estructural |
| Opel Manta 400 GrpB · `rsf-opel-manta-400-grpb` | `Cars/OPEL_MANTA_400/OPEL_MANTA_400.sgc` | 3 | 4 grava | No encontrada / — · `Opel_Manta_400_GrpB_ngp6` | — | Incidencias 2/0 |
| Peugeot 106 Rallye S20 GrpA · `rsf-peugeot-106-rallye-s20-grpa` | `Cars/Peugeot106_rally/Peugeot106_rally.sgc` | 22 | 4 grava | No encontrada / — · `Peugeot_106_Rallye_S20_GrpA_ngp6` | — | Estructural |
| Peugeot 205 T16 GrpB · `rsf-peugeot-205-t16-grpb` | `Cars/PEUGEOT_205_T16_e1/PEUGEOT_205_T16_e1.sgc` · respaldo | 4 | 4 grava | Verificada / Verificada · `Peugeot_205_T16_GrpB_ngp6` (70/139) · 2706450074/1699660905/2051992516 | — | Estructural |
| Peugeot 207 S2000 Evolution Plus · `rsf-peugeot-207-s2000-evolution-plus` | `Cars/207_s2000_2012/207_s2000_2012.sgc` | 4 | 4 grava | No encontrada / — · `Peugeot_207_S2000_Evolution_Plus_ngp6` | — | Estructural |
| Peugeot 208 R2 · `rsf-peugeot-208-r2` | `Cars/PEUGEOT_208_R_2/PEUGEOT_208_R_2.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `Peugeot_208_R2_ngp6` (30/229) · 1329754390/4217721543/161430901 | — | Incidencias 2/0 |
| Peugeot 208 Rally4 · `rsf-peugeot-208-rally4` | `Cars/PEUGEOT_208_Rally4/PEUGEOT_208_Rally4.sgc` · respaldo | 4 | 4 grava | Verificada / Verificada · `Peugeot_208_Rally4_ngp6` (85/249) · 2529122630/1889560338/3496088869 | — | Incidencias 3/0 |
| Peugeot 208 T16 R5 · `rsf-peugeot-208-t16-r5` | `Cars/208_T16_R5_evo/208_T16_R5_evo.sgc` | 9 | 4 grava | No encontrada / — · `Peugeot_208_T16_R5_ngp7` | — | Estructural |
| Peugeot 306 Maxi Kit Car · `rsf-peugeot-306-maxi-kit-car` | `Cars/p306maxi/p306maxi.sgc` · respaldo | 5 | 4 grava | Verificada / — · `Peugeot_306_Maxi_Kit_Car_ngp6` (95/48) · 3818958421/376756276/2714609810 | — | Incidencias 0/1 |
| Porsche 911 GT3 RS (2007) RGT · `rsf-porsche-911-gt3-rs-2007-rgt` | `Cars/PORSCHE911GT3RS/PORSCHE911GT3RS.sgc` | 3 | 4 grava | No encontrada / — · `Porsche_911_GT3_RS_2007_RGT_ngp6` | — | Estructural |
| Porsche 911 GT3 RS (2010) RGT · `rsf-porsche-911-gt3-rs-2010-rgt` | `Cars/PORSCHE_GT3_RGT/PORSCHE_GT3_RGT.sgc` | 4 | 4 grava | No encontrada / — · `Porsche_911_GT3_RS_2010_RGT_ngp7` | — | Incidencias 2/2 |
| Porsche 911 SC 3.0 Grp4 · `rsf-porsche-911-sc-3-0-grp4` | `Cars/Porsche_911SC/Porsche_911SC.sgc` | 4 | 4 grava | No encontrada / — · `Porsche_911_SC_3.0_Grp4_ngp6` | — | Estructural |
| Porsche 911 SC RS GrpB · `rsf-porsche-911-sc-rs-grpb` | `Cars/PORSCHE911_SCRS_GrpB/PORSCHE911_SCRS_GrpB.sgc` · respaldo | 3 | 4 grava | Verificada / Pendiente · `Porsche_911_SC_RS_GrpB_ngp6` (23/141) · 605758134/431570091/307314733 | — | Estructural |
| Renault 5 GT Turbo GrpA · `rsf-renault-5-gt-turbo-grpa` | `Cars/5GTT/5gtt.sgc` | 5 | 4 grava | No encontrada / — · `Renault_5_GT_Turbo_GrpA_ngp6` | — | Incidencias 4/1 |
| Renault Clio 16S Williams GrpA · `rsf-renault-clio-16s-williams-grpa` | `Cars/RENAULT_CLIO/Renault_Clio.sgc` | 7 | 4 grava | No encontrada / — · `Renault_Clio_16S_Williams_GrpA_ngp6` | — | Estructural |
| Renault Clio III R3 · `rsf-renault-clio-iii-r3` | `Cars/Clio_R3/Clio_R3.sgc` | 5 | 4 grava | No encontrada / — · `Renault_Clio_III_R3_ngp6` | — | Estructural |
| Renault Clio IV R3T · `rsf-renault-clio-iv-r3t` | `Cars/Clio4_R3T/Clio4_R3T.sgc` | 4 | 4 grava | No encontrada / — · `Renault_Clio_IV_R3T_ngp6` | — | Incidencias 1/0 |
| Renault Clio Rally3 · `rsf-renault-clio-rally3` | `Cars/Renault_ClioV_Rally3/Renault_ClioV_Rally3.sgc` | 4 | 4 grava | No encontrada / — · `Renault_Clio_Rally3_ngp7` | — | Incidencias 2/0 |
| Renault Clio Rally4 · `rsf-renault-clio-rally4` | `Cars/Renault_ClioV_Rally4/Renault_ClioV_Rally4.sgc` · respaldo | 4 | 4 grava | Verificada / Verificada · `Renault_Clio_Rally4_ngp6` (93/254) · 2168549195/2173472442/3348898600 | Variante con pack propio | Incidencias 2/0 |
| Renault Clio Rally5 · `rsf-renault-clio-rally5` | `Cars/Renault_ClioV_Rally4/Renault_ClioV_Rally4.sgc` · respaldo | 4 | 4 grava | Verificada / Verificada · `Renault_Clio_Rally5_ngp6` (96/254) · 2754613235/2949458848/3807200144 | Variante con pack propio | Incidencias 2/0 |
| Renault Twingo R1 · `rsf-renault-twingo-r1` | `Cars/Twingo_R1/Twingo_R1.sgc` | 5 | 4 grava | No encontrada / — · `Renault_Twingo_R1_ngp6` | — | Estructural |
| Renault Twingo R2 Evo · `rsf-renault-twingo-r2-evo` | `Cars/Twingo_R2/Twingo_R2.sgc` | 5 | 4 grava | No encontrada / — · `Renault_Twingo_R2_Evo_ngp6` | — | Estructural |
| Seat Leon Cupra R GrpN · `rsf-seat-leon-cupra-r-grpn` | `Cars/leon_cupra_grn/leon_cupra_grn.sgc` | 4 | 4 grava | No encontrada / — · `Seat_Leon_Cupra_R_GrpN_ngp6` | — | Estructural |
| Skoda 130 LR GrpB · `rsf-skoda-130-lr-grpb` | `Cars/skoda_130LR/skoda_130LR.sgc` | 4 | 4 grava | No encontrada / — · `Skoda_130_LR_GrpB_ngp6` | — | Estructural |
| Skoda 130 RS Grp2 · `rsf-skoda-130-rs-grp2` | `Cars/Skoda_130RS/Skoda_130RS.sgc` | 4 | 4 grava | No encontrada / — · `Skoda_130_RS_Grp2_ngp6` | — | Estructural |
| Skoda Fabia R5 · `rsf-skoda-fabia-r5` | `Cars/Fabia_r5_2018/Fabia_r5_2018.sgc` | 6 | 4 grava | No encontrada / — · `Skoda_Fabia_R5_b_ngp6` | — | Incidencias 2/1 |
| Skoda Fabia R5 evo · `rsf-skoda-fabia-r5-evo` | `Cars/Fabia_R5_evo/Fabia_R5_evo.sgc` | 4 | 4 grava | No encontrada / — · `Skoda_Fabia_R5_evo_ngp6` | — | Incidencias 2/5 |
| Skoda Fabia RS Rally2 · `rsf-skoda-fabia-rs-rally2` | `Cars/fabia_rs_rally2/fabia_rs_rally2.sgc` · respaldo | 7 | 4 grava | Verificada / Verificada · `Skoda_Fabia_RS_Rally2_ngp7` (106/263) · 3647815146/1984195306/498727318 | — | Incidencias 18/8 |
| Skoda Fabia S2000 Evo 2 · `rsf-skoda-fabia-s2000-evo-2` | `Cars/Fabia_S2000_2012/Fabia_S2000_2012.sgc` | 8 | 4 grava | No encontrada / — · `Skoda_Fabia_S2000_Evo_2_ngp6` | — | Incidencias 2/0 |
| Skoda Fabia WRC 2006 · `rsf-skoda-fabia-wrc-2006` | `Cars/FABIA06/fabia06.sgc` · respaldo | 3 | 4 grava | Verificada / Verificada · `Skoda_Fabia_WRC_2006_ngp6` (82/246) · 1338675042/1635006510/3443908965 | — | Incidencias 1/0 |
| Skoda Favorit 136 L GrpA · `rsf-skoda-favorit-136-l-grpa` | `Cars/Favorit/Favorit.sgc` | 5 | 4 grava | No encontrada / — · `Skoda_Favorit_136_L_GrpA_ngp6` | — | Estructural |
| Subaru Impreza GC8 555 GrpA · `rsf-subaru-impreza-gc8-555-grpa` | `Cars/impreza95/impreza95.sgc` | 4 | 4 grava | No encontrada / — · `Subaru_Impreza_GC8_555_GrpA_ngp6` | — | Incidencias 1/1 |
| Subaru Impreza GDA WRC2003 (S9) · `rsf-subaru-impreza-gda-wrc2003-s9` | `Cars/impreza_2003_kazhol/impreza03.sgc` · respaldo | 3 | 4 grava | Verificada / Verificada · `Subaru_Impreza_GDA_WRC2003_S9_2006_ngp6` (57/105) · 3571803401/773117028/1449790222 | — | Estructural |
| Subaru Impreza N14 N4 · `rsf-subaru-impreza-n14-n4` | `Cars/Impreza_N14_Pro/impreza_N14.sgc` | 3 | 4 grava | No encontrada / — · `Subaru_Impreza_N14_N4_ngp6` | Variante con pack propio | Incidencias 2/0 |
| Subaru Impreza N15 R4 · `rsf-subaru-impreza-n15-r4` | `Cars/Impreza_N14_Pro/impreza_N14.sgc` | 3 | 4 grava | No encontrada / — · `Subaru_Impreza_N15_R4_ngp6` | Variante con pack propio | Incidencias 2/0 |
| Toyota Celica 2000GT(ST185) GrpA · `rsf-toyota-celica-2000gt-st185-grpa` | `Cars/CELICA_st185/CELICA_st185.sgc` | 6 | 4 grava | No encontrada / — · `Toyota_Celica_2000GT-FOUR_RC_ST185_GrpA_ngp6` | — | Estructural |
| Toyota Celica TCT (TA64) GrpB · `rsf-toyota-celica-tct-ta64-grpb` | `Cars/TOYOTA_CELICA_TA64_GrpB/TOYOTA_CELICA_TA64_GrpB.sgc` · respaldo | 4 | 4 grava | Verificada / Verificada · `Toyota_Celica_TCT_TA64_GrpB_ngp6` (77/240) · 2300425828/2533933195/3497555416 | — | Estructural |
| Toyota GR Yaris Rally2 · `rsf-toyota-gr-yaris-rally2` | `Cars/yaris_rally2/yaris_rally2.sgc` · respaldo | 8 | 4 grava | Verificada / — · `Toyota_GR_Yaris_Rally2_ngp7` (107/264) · 3841637995/897590838/539916823 | — | Incidencias 8/0 |
| Toyota Yaris WRC 2018 · `rsf-toyota-yaris-wrc-2018` | `Cars/YARIS_WRC18/yaris_wrc.sgc` · respaldo | 6 | 4 grava | Verificada / — · `Toyota_Yaris_WRC_2018_ngp6` (16/227) · 3140711559/2064138633/968628864 | — | Incidencias 2/2 |
| Trabant P 800 RS GrpA · `rsf-trabant-p-800-rs-grpa` | `Cars/Trabant_P800RS/Trabant_P800RS.sgc` | 4 | 4 grava | No encontrada / — · `Trabant_P_800_RS_GrpA_ngp6` | — | Incidencias 1/0 |
| Volvo 240 Turbo GrpA · `rsf-volvo-240-turbo-grpa` | `Cars/Volvo242LE/Volvo242LE.sgc` | 4 | 4 grava | No encontrada / — · `Volvo_240_Turbo_GrpA_ngp7` | — | Estructural |
| VW Golf II GTI 16V GrpA · `rsf-vw-golf-ii-gti-16v-grpa` | `Cars/vw_golf_86/vw_golf_86.sgc` | 4 | 4 grava | No encontrada / — · `VW_Golf_II_GTI_16V_GrpA_ngp6` | — | Incidencias 8/4 |
| VW Polo GTI R5 · `rsf-vw-polo-gti-r5` | `Cars/Polo_gti_r5/Polo_gti_r5.sgc` | 4 | 4 grava | No encontrada / — · `VW_Polo_GTI_R5_ngp6` | — | Incidencias 2/7 |
| VW Polo R WRC 2016 · `rsf-vw-polo-r-wrc-2016` | `Cars/POLO_R_WRC_2016/polo_wrc_2016.sgc` · respaldo | 5 | 4 grava | Verificada / Verificada · `VW_Polo_R_WRC_2016_ngp6` (10/162) · 1301876470/2389600794/3474350321 | — | Incidencias 2/0 |
| Wartburg 353 W 460 GrpA · `rsf-wartburg-353-w-460-grpa` | `Cars/Wartburg_353/Wartburg_353.sgc` | 4 | 4 grava | No encontrada / — · `Wartburg_353_W_460_GrpA_ngp6` | — | Estructural |
| Subaru Impreza WRC 2000 · `subaru-impreza-2000` | `Cars/impreza00/impreza00.sgc` · respaldo | 3 | 4 grava | Retail 7 | — | Estructural |
| Subaru Impreza WRC 2003 · `subaru-impreza-2003` | `Cars/impreza03/impreza03.sgc` · respaldo | 3 | 4 grava | Retail 5 | — | Estructural |
| Toyota Corolla WRC · `toyota-corolla-wrc` | `Cars/COROLLA/corolla.sgc` · respaldo | 3 | 4 grava | Retail 6 | — | Error de slot PC |

## Catálogo reconstruido desde el respaldo

Los IDs de respaldo se generan a partir de sus carpetas, porque el respaldo carece de caché RSF. Las equivalencias PC de la tabla usan la ruta exacta. Los alias numerados y la coincidencia SHA-256 se detallan arriba. Los resultados corresponden a los 44 descriptores, incluidos los dos alias que siguen accesibles para guardados.

| Vehículo e ID en respaldo | Modelo | ID PC por ruta | Fila visible | DDS exterior / draws cockpit | Estado visual CPU |
| --- | --- | --- | --- | ---: | --- |
| Citroen Xsara (Car00) · `citroen-xsara` | `Cars/xsara/xsara.sgc` | `citroen-xsara` | Sí | 3 / 27 | Estructural |
| Ford Focus RS WRC 2006 · `ford-focus-rs-wrc-2006` | `Cars/FORD_FOCUS_2006/focus06.sgc` | `ford-focus-rs-wrc-2006` | Sí | 6 / 23 | Estructural |
| Hyundai Accent WRC · `hyundai-accent-wrc` | `Cars/accent/accent.sgc` | `hyundai-accent-wrc` | Sí | 3 / 26 | Estructural |
| MG ZR Super 1600 · `mg-zr-s1600` | `Cars/zr/zr.sgc` | `mg-zr-s1600` | Sí | 3 / 26 | Estructural |
| Mitsubishi Lancer Evo VII · `mitsubishi-lancer-evo-vii` | `Cars/Lancer_EVO_VII/lancer_evo_vII.sgc` | `mitsubishi-lancer-evo-vii` | Sí | 3 / 28 | Estructural |
| Peugeot 206 WRC · `peugeot-206-wrc` | `Cars/Peugeot_206/Peugeot_206.sgc` | `peugeot-206-wrc` | Sí | 3 / 21 | Estructural |
| Audi 200 Quattro · `rsf-audi-200-quattro` | `Cars/Audi_200_quattro/Audi_200_quattro.sgc` | `rsf-audi-200-quattro-grpa` | Sí | 3 / 50 | Incidencias 0/2 |
| AUDI QUATTRO A1 GrpB · `rsf-audi-quattro-a1-grpb` | `Cars/AUDI_QUATTRO_A1_GrpB/AUDI_QUATTRO_A1_GrpB.sgc` | `rsf-audi-quattro-a1-grpb` | Sí | 3 / 21 | Estructural |
| AUDI QUATTRO A2 GrpB · `rsf-audi-quattro-a2-grpb` | `Cars/AUDI_QUATTRO_A2_GrpB/AUDI_QUATTRO_A2_GrpB.sgc` | `rsf-audi-quattro-a2-grpb` | Sí | 6 / 24 | Estructural |
| Audi Sport Quattro · `rsf-audi-sport-quattro` | `Cars/Audi_Sport_Quattro/Audi_Sport_Quattro.sgc` | `rsf-audi-sport-quattro-grpb` | Sí | 4 / 13 | Estructural |
| BMW M1 · `rsf-bmw-m1` | `Cars/BMW_M1/BMW_M1.sgc` | `rsf-bmw-m1-grpb` | Sí | 5 / 15 | Estructural |
| BMW M3 E30 · `rsf-bmw-m3-e30` | `Cars/BMW_M3_E30/BMW_M3_E30.sgc` | `rsf-bmw-m3-e30-grpa` | Sí | 20 / 40 | Estructural |
| C2 R2 Max · `rsf-c2-r2-max` | `Cars/C2_R2_Max/C2_R2_Max.sgc` | `rsf-citroen-c2-r2-max` | Sí | 5 / 24 | Incidencias 0/3 |
| C4 08 · `rsf-c4-08` | `Cars/C4_08/c4_08.sgc` | `rsf-citroen-c4-wrc-2008` | Sí | 5 / 38 | Incidencias 1/1 |
| Citroen C3 WRC17 · `rsf-citroen-c3-wrc17` | `Cars/Citroen_C3_WRC17/Citroen_C3_WRC17.sgc` | `rsf-citroen-c3-wrc-2017` | Sí | 5 / 17 | Estructural |
| DS3 WRC · `rsf-ds3-wrc` | `Cars/DS3_WRC/ds3_wrc.sgc` | `rsf-citroen-ds3-wrc` | Sí | 5 / 23 | Incidencias 1/0 |
| DS3 WRC 2 · `rsf-ds3-wrc-2` | `Cars/DS3_WRC 2/ds3_wrc.sgc` | — | Alias conservado | 5 / 23 | Incidencias 1/0 |
| FABIA06 · `rsf-fabia06` | `Cars/FABIA06/fabia06.sgc` | `rsf-skoda-fabia-wrc-2006` | Sí | 3 / 18 | Incidencias 1/0 |
| Fabia Rs Rally2 · `rsf-fabia-rs-rally2` | `Cars/fabia_rs_rally2/fabia_rs_rally2.sgc` | `rsf-skoda-fabia-rs-rally2` | Sí | 7 / 35 | Incidencias 18/8 |
| Fiesta R2 2019 · `rsf-fiesta-r2-2019` | `Cars/Fiesta_R2_2019/Fiesta_R2_2019.sgc` | `rsf-ford-fiesta-mk-viii-r2` | Sí | 6 / 23 | Incidencias 0/5 |
| Fiesta Rally2 23 · `rsf-fiesta-rally2-23` | `Cars/fiesta_rally2_23/fiesta_rally2.sgc` | `rsf-ford-fiesta-rally2` | Sí | — / — | SHOWROOM DDS: cars\fiesta_rally2_23\textures\fiesta_rally2_drivers.dds for Cars/fiesta_rally2_23: SHOWROOM READ: missing Cars/fiesta_rally2_23/textures/fiesta_rally2_drivers.dds; SHOWROOM DDS: cars\fiesta_rally2_23\textures\fiesta_rally2_external.dds for Cars/fiesta_rally2_23: SHOWROOM READ: missing Cars/fiesta_rally2_23/textures/fiesta_rally2_external.dds |
| Fiesta Rally2 23 2 · `rsf-fiesta-rally2-23-2` | `Cars/fiesta_rally2_23 2/fiesta_rally2.sgc` | — | Sí | 7 / 20 | Incidencias 4/8 |
| Fiesta Rally3 Evo · `rsf-fiesta-rally3-evo` | `Cars/fiesta_rally3_evo/fiesta_rally3_evo.sgc` | `rsf-ford-fiesta-rally3-evo` | Sí | 8 / 25 | Incidencias 0/2 |
| Fiesta Rally4 · `rsf-fiesta-rally4` | `Cars/Fiesta_rally4/Fiesta_rally4.sgc` | `rsf-ford-fiesta-rally4` | Sí | 6 / 26 | Incidencias 1/1 |
| Fiesta Wrc 14 · `rsf-fiesta-wrc-14` | `Cars/fiesta_wrc_14/fiesta_wrc_14.sgc` | `rsf-ford-fiesta-rs-wrc-2014` | Sí | 6 / 20 | Incidencias 2/1 |
| Fiesta Wrc 19 · `rsf-fiesta-wrc-19` | `Cars/fiesta_wrc_19/fiesta_wrc_19.sgc` | `rsf-ford-fiesta-wrc-2019` | Sí | 6 / 21 | Incidencias 3/3 |
| I20 WRC 20 · `rsf-i20-wrc-20` | `Cars/i20_WRC_20/i20_wrc_20.sgc` | `rsf-hyundai-i20-coupe-wrc-2020` | Sí | 13 / 29 | Incidencias 8/0 |
| Impreza 2003 Kazhol · `rsf-impreza-2003-kazhol` | `Cars/impreza_2003_kazhol/impreza03.sgc` | `rsf-subaru-impreza-gda-wrc2003-s9` | Sí | 3 / 19 | Estructural |
| Opel Adam R2 · `rsf-opel-adam-r2` | `Cars/Opel_Adam_R2/Opel_Adam_R2.sgc` | `rsf-opel-adam-r2` | Sí | 4 / 13 | Incidencias 1/0 |
| P306maxi · `rsf-p306maxi` | `Cars/p306maxi/p306maxi.sgc` | `rsf-peugeot-306-maxi-kit-car` | Sí | 5 / 21 | Incidencias 0/1 |
| PEUGEOT 205 T16 E1 · `rsf-peugeot-205-t16-e1` | `Cars/PEUGEOT_205_T16_e1/PEUGEOT_205_T16_e1.sgc` | `rsf-peugeot-205-t16-grpb` | Sí | 4 / 15 | Estructural |
| PEUGEOT 208 R 2 · `rsf-peugeot-208-r-2` | `Cars/PEUGEOT_208_R_2/PEUGEOT_208_R_2.sgc` | `rsf-peugeot-208-r2` | Sí | 5 / 23 | Incidencias 2/0 |
| PEUGEOT 208 Rally4 · `rsf-peugeot-208-rally4` | `Cars/PEUGEOT_208_Rally4/PEUGEOT_208_Rally4.sgc` | `rsf-peugeot-208-rally4` | Sí | 4 / 27 | Incidencias 3/0 |
| POLO R WRC 2016 · `rsf-polo-r-wrc-2016` | `Cars/POLO_R_WRC_2016/polo_wrc_2016.sgc` | `rsf-vw-polo-r-wrc-2016` | Sí | 5 / 19 | Incidencias 2/0 |
| PORSCHE911 SCRS GrpB · `rsf-porsche911-scrs-grpb` | `Cars/PORSCHE911_SCRS_GrpB/PORSCHE911_SCRS_GrpB.sgc` | `rsf-porsche-911-sc-rs-grpb` | Sí | 3 / 22 | Estructural |
| Renault ClioV Rally4 · `rsf-renault-cliov-rally4` | `Cars/Renault_ClioV_Rally4/Renault_ClioV_Rally4.sgc` | `rsf-renault-clio-rally5`, `rsf-renault-clio-rally4` | Sí | 4 / 31 | Incidencias 2/0 |
| TOYOTA CELICA TA64 GrpB · `rsf-toyota-celica-ta64-grpb` | `Cars/TOYOTA_CELICA_TA64_GrpB/TOYOTA_CELICA_TA64_GrpB.sgc` | `rsf-toyota-celica-tct-ta64-grpb` | Sí | 4 / 26 | Estructural |
| Xsara 2006 · `rsf-xsara-2006` | `Cars/Xsara_2006/xsara.sgc` | `rsf-citroen-xsara-wrc-2006` | Sí | 5 / 22 | Estructural |
| Xsara 2006 2 · `rsf-xsara-2006-2` | `Cars/Xsara_2006 2/xsara.sgc` | — | Alias conservado | 5 / 22 | Estructural |
| Yaris Rally2 · `rsf-yaris-rally2` | `Cars/yaris_rally2/yaris_rally2.sgc` | `rsf-toyota-gr-yaris-rally2` | Sí | 8 / 29 | Incidencias 8/0 |
| YARIS WRC18 · `rsf-yaris-wrc18` | `Cars/YARIS_WRC18/yaris_wrc.sgc` | `rsf-toyota-yaris-wrc-2018` | Sí | 6 / 35 | Incidencias 2/2 |
| Subaru Impreza WRC 2000 · `subaru-impreza-2000` | `Cars/impreza00/impreza00.sgc` | `subaru-impreza-2000` | Sí | 3 / 25 | Estructural |
| Subaru Impreza WRC 2003 · `subaru-impreza-2003` | `Cars/impreza03/impreza03.sgc` | `subaru-impreza-2003` | Sí | 3 / 19 | Estructural |
| Toyota Corolla WRC · `toyota-corolla-wrc` | `Cars/COROLLA/corolla.sgc` | `toyota-corolla-wrc` | Sí | 3 / 26 | Estructural |

## Recursos faltantes por vehículo y origen

Las rutas declaradas se cotejaron dentro de cada origen. Un montado conjunto del dispositivo puede aportar los archivos ausentes en una exportación. En el Fiesta Rally2 numerado, el diagnóstico visual confirma que los DDS locales satisfacen las referencias externas ausentes.

- **PC / rsf-abarth-grande-punto-s2000** — rsfdata/cars/Abarth_Grande_Punto_S2000_ngp6 Pack esperado: Abarth_Grande_Punto_S2000_ngp6.
- **PC / rsf-alpine-a110-rally-rgt** — rsfdata/cars/Alpine_A110_Rally_RGT_ngp6 Pack esperado: Alpine_A110_Rally_RGT_ngp6.
- **PC / rsf-aston-martin-vantage-rgt** — rsfdata/cars/Aston_Martin_Vantage_RGT_ngp6 Pack esperado: Aston_Martin_Vantage_RGT_ngp6.
- **PC / rsf-audi-quattro-grp4** — rsfdata/cars/Audi_quattro_Grp4_ngp6 Pack esperado: Audi_quattro_Grp4_ngp6.
- **PC / rsf-bmw-m3-e36-grpa** — rsfdata/cars/BMW_M3_E36_GrpA_ngp6 Pack esperado: BMW_M3_E36_GrpA_ngp6.
- **PC / rsf-citroen-c2-gt-s1600** — rsfdata/cars/Citroen_C2_GT_S1600_ngp6 Pack esperado: Citroen_C2_GT_S1600_ngp6.
- **PC / rsf-citroen-c3-r5** — rsfdata/cars/Citroen_C3_R5_ngp6 Pack esperado: Citroen_C3_R5_ngp6.
- **PC / rsf-citroen-ds3-r1** — rsfdata/cars/Citroen_DS3_R1_ngp6 Pack esperado: Citroen_DS3_R1_ngp6.
- **PC / rsf-citroen-ds3-r3-max** — rsfdata/cars/Citroen_DS3_R3-MAX_ngp6 Pack esperado: Citroen_DS3_R3-MAX_ngp6.
- **PC / rsf-citroen-ds3-r5** — rsfdata/cars/Citroen_DS3_R5_ngp6 Pack esperado: Citroen_DS3_R5_ngp6.
- **PC / rsf-citroen-xsara-kit-car** — rsfdata/cars/Citroen_Xsara_Kit_Car_ngp6 Pack esperado: Citroen_Xsara_Kit_Car_ngp6.
- **PC / rsf-fiat-124-abarth-rally-rgt** — rsfdata/cars/Fiat_124_Abarth_Rally_RGT_b_ngp6 Pack esperado: Fiat_124_Abarth_Rally_RGT_b_ngp6.
- **PC / rsf-fiat-126-grp2** — cars/126p/textures/126p_internal_kwindscreen.dds; rsfdata/cars/Fiat_126_Grp2_ngp6 Pack esperado: Fiat_126_Grp2_ngp6.
- **PC / rsf-fiat-131-abarth-grp4** — rsfdata/cars/Fiat_131_Abarth_Grp4_ngp6 Pack esperado: Fiat_131_Abarth_Grp4_ngp6.
- **PC / rsf-fiat-abarth-500-r3t** — rsfdata/cars/Fiat_Abarth_500_R3T_ngp6 Pack esperado: Fiat_Abarth_500_R3T_ngp6.
- **PC / rsf-ford-escort-mk-ii-rs-grp4** — rsfdata/cars/Ford_Escort_Mk_II_RS_Grp4_ngp6 Pack esperado: Ford_Escort_Mk_II_RS_Grp4_ngp6.
- **PC / rsf-ford-fiesta-mk-vi-s2000** — rsfdata/cars/Ford_Fiesta_Mk_VI_S2000_ngp6 Pack esperado: Ford_Fiesta_Mk_VI_S2000_ngp6.
- **PC / rsf-ford-fiesta-r2** — rsfdata/cars/Ford_Fiesta_R2_ngp6 Pack esperado: Ford_Fiesta_R2_ngp6.
- **PC / rsf-ford-fiesta-r5** — rsfdata/cars/Ford_Fiesta_R5_ngp6 Pack esperado: Ford_Fiesta_R5_ngp6.
- **PC / rsf-honda-civic-type-r-fn2-r3** — rsfdata/cars/Honda_Civic_Type_R_FN2_R3_ngp6 Pack esperado: Honda_Civic_Type_R_FN2_R3_ngp6.
- **PC / rsf-hyundai-i20-coupe-wrc-2017** — rsfdata/cars/Hyundai_i20_Coupe_WRC_2017_ngp6 Pack esperado: Hyundai_i20_Coupe_WRC_2017_ngp6.
- **PC / rsf-hyundai-i20-coupe-wrc-2021** — rsfdata/cars/Hyundai_i20_Coupe_WRC_2021_ngp7 Pack esperado: Hyundai_i20_Coupe_WRC_2021_ngp7.
- **PC / rsf-hyundai-i20-n-rally2** — rsfdata/cars/Hyundai_i20_N_Rally2_ngp7 Pack esperado: Hyundai_i20_N_Rally2_ngp7.
- **PC / rsf-hyundai-i20-r5** — rsfdata/cars/Hyundai_i20_R5_ngp6 Pack esperado: Hyundai_i20_R5_ngp6.
- **PC / rsf-lada-kalina-rc2-grpa** — rsfdata/cars/Lada_Kalina_RC2_GrpA_ngp6 Pack esperado: Lada_Kalina_RC2_GrpA_ngp6.
- **PC / rsf-lada-vfts-grpb** — cars/Lada_VFTS_NGP/textures/impreza03_internal_kwindscreen_snow.dds; rsfdata/cars/Lada_VFTS_GrpB_ngp6
- **PC / rsf-lancia-stratos-hf-grp4** — rsfdata/cars/Lancia_Stratos_HF_Grp4_ngp6 Pack esperado: Lancia_Stratos_HF_Grp4_ngp6.
- **PC / rsf-lotus-exige-s-rgt** — rsfdata/cars/Lotus_Exige_S_RGT_ngp6 Pack esperado: Lotus_Exige_S_RGT_ngp6.
- **PC / rsf-mazda-323-bf-4wd-turbo-grpa** — rsfdata/cars/Mazda_323_BF_4WD_Turbo_GrpA_ngp6 Pack esperado: Mazda_323_BF_4WD_Turbo_GrpA_ngp6.
- **PC / rsf-mitsubishi-lancer-evo-ix-n4** — rsfdata/cars/Mitsubishi_Lancer_Evo_IX_N4_ngp6 Pack esperado: Mitsubishi_Lancer_Evo_IX_N4_ngp6.
- **PC / rsf-mitsubishi-lancer-evo-ix-r4** — rsfdata/cars/Mitsubishi_Lancer_Evo_IX_R4_ngp6 Pack esperado: Mitsubishi_Lancer_Evo_IX_R4_ngp6.
- **PC / rsf-mitsubishi-lancer-evo-x-r4** — rsfdata/cars/Mitsubishi_Lancer_Evo_X_R4_V1.2_ngp6 Pack esperado: Mitsubishi_Lancer_Evo_X_R4_V1.2_ngp6.
- **PC / rsf-opel-manta-400-grpb** — rsfdata/cars/Opel_Manta_400_GrpB_ngp6 Pack esperado: Opel_Manta_400_GrpB_ngp6.
- **PC / rsf-peugeot-106-rallye-s20-grpa** — rsfdata/cars/Peugeot_106_Rallye_S20_GrpA_ngp6 Pack esperado: Peugeot_106_Rallye_S20_GrpA_ngp6.
- **PC / rsf-peugeot-207-s2000-evolution-plus** — rsfdata/cars/Peugeot_207_S2000_Evolution_Plus_ngp6 Pack esperado: Peugeot_207_S2000_Evolution_Plus_ngp6.
- **PC / rsf-peugeot-208-t16-r5** — rsfdata/cars/Peugeot_208_T16_R5_ngp7 Pack esperado: Peugeot_208_T16_R5_ngp7.
- **PC / rsf-porsche-911-gt3-rs-2007-rgt** — rsfdata/cars/Porsche_911_GT3_RS_2007_RGT_ngp6 Pack esperado: Porsche_911_GT3_RS_2007_RGT_ngp6.
- **PC / rsf-porsche-911-gt3-rs-2010-rgt** — rsfdata/cars/Porsche_911_GT3_RS_2010_RGT_ngp7 Pack esperado: Porsche_911_GT3_RS_2010_RGT_ngp7.
- **PC / rsf-porsche-911-sc-3-0-grp4** — rsfdata/cars/Porsche_911_SC_3.0_Grp4_ngp6 Pack esperado: Porsche_911_SC_3.0_Grp4_ngp6.
- **PC / rsf-renault-5-gt-turbo-grpa** — cars/5gtt/textures/impreza03_internal_kwindscreen.dds; rsfdata/cars/Renault_5_GT_Turbo_GrpA_ngp6 Pack esperado: Renault_5_GT_Turbo_GrpA_ngp6.
- **PC / rsf-renault-clio-16s-williams-grpa** — rsfdata/cars/Renault_Clio_16S_Williams_GrpA_ngp6 Pack esperado: Renault_Clio_16S_Williams_GrpA_ngp6.
- **PC / rsf-renault-clio-iii-r3** — rsfdata/cars/Renault_Clio_III_R3_ngp6 Pack esperado: Renault_Clio_III_R3_ngp6.
- **PC / rsf-renault-clio-iv-r3t** — rsfdata/cars/Renault_Clio_IV_R3T_ngp6 Pack esperado: Renault_Clio_IV_R3T_ngp6.
- **PC / rsf-renault-clio-rally3** — rsfdata/cars/Renault_Clio_Rally3_ngp7 Pack esperado: Renault_Clio_Rally3_ngp7.
- **PC / rsf-renault-twingo-r1** — rsfdata/cars/Renault_Twingo_R1_ngp6 Pack esperado: Renault_Twingo_R1_ngp6.
- **PC / rsf-renault-twingo-r2-evo** — rsfdata/cars/Renault_Twingo_R2_Evo_ngp6 Pack esperado: Renault_Twingo_R2_Evo_ngp6.
- **PC / rsf-seat-leon-cupra-r-grpn** — rsfdata/cars/Seat_Leon_Cupra_R_GrpN_ngp6 Pack esperado: Seat_Leon_Cupra_R_GrpN_ngp6.
- **PC / rsf-skoda-130-lr-grpb** — rsfdata/cars/Skoda_130_LR_GrpB_ngp6 Pack esperado: Skoda_130_LR_GrpB_ngp6.
- **PC / rsf-skoda-130-rs-grp2** — rsfdata/cars/Skoda_130_RS_Grp2_ngp6 Pack esperado: Skoda_130_RS_Grp2_ngp6.
- **PC / rsf-skoda-fabia-r5** — rsfdata/cars/Skoda_Fabia_R5_b_ngp6 Pack esperado: Skoda_Fabia_R5_b_ngp6.
- **PC / rsf-skoda-fabia-r5-evo** — rsfdata/cars/Skoda_Fabia_R5_evo_ngp6 Pack esperado: Skoda_Fabia_R5_evo_ngp6.
- **PC / rsf-skoda-fabia-s2000-evo-2** — rsfdata/cars/Skoda_Fabia_S2000_Evo_2_ngp6 Pack esperado: Skoda_Fabia_S2000_Evo_2_ngp6.
- **PC / rsf-skoda-fabia-wrc-2006** — cars/fabia06/textures/e_p206_internal_kwindscreen.dds
- **PC / rsf-skoda-favorit-136-l-grpa** — rsfdata/cars/Skoda_Favorit_136_L_GrpA_ngp6 Pack esperado: Skoda_Favorit_136_L_GrpA_ngp6.
- **PC / rsf-subaru-impreza-gc8-555-grpa** — rsfdata/cars/Subaru_Impreza_GC8_555_GrpA_ngp6 Pack esperado: Subaru_Impreza_GC8_555_GrpA_ngp6.
- **PC / rsf-subaru-impreza-n14-n4** — rsfdata/cars/Subaru_Impreza_N14_N4_ngp6 Pack esperado: Subaru_Impreza_N14_N4_ngp6.
- **PC / rsf-subaru-impreza-n15-r4** — rsfdata/cars/Subaru_Impreza_N15_R4_ngp6 Pack esperado: Subaru_Impreza_N15_R4_ngp6.
- **PC / rsf-toyota-celica-2000gt-st185-grpa** — rsfdata/cars/Toyota_Celica_2000GT-FOUR_RC_ST185_GrpA_ngp6 Pack esperado: Toyota_Celica_2000GT-FOUR_RC_ST185_GrpA_ngp6.
- **PC / rsf-trabant-p-800-rs-grpa** — rsfdata/cars/Trabant_P_800_RS_GrpA_ngp6 Pack esperado: Trabant_P_800_RS_GrpA_ngp6.
- **PC / rsf-volvo-240-turbo-grpa** — rsfdata/cars/Volvo_240_Turbo_GrpA_ngp7 Pack esperado: Volvo_240_Turbo_GrpA_ngp7.
- **PC / rsf-vw-golf-ii-gti-16v-grpa** — rsfdata/cars/VW_Golf_II_GTI_16V_GrpA_ngp6 Pack esperado: VW_Golf_II_GTI_16V_GrpA_ngp6.
- **PC / rsf-vw-polo-gti-r5** — rsfdata/cars/VW_Polo_GTI_R5_ngp6 Pack esperado: VW_Polo_GTI_R5_ngp6.
- **PC / rsf-wartburg-353-w-460-grpa** — rsfdata/cars/Wartburg_353_W_460_GrpA_ngp6 Pack esperado: Wartburg_353_W_460_GrpA_ngp6.
- **Transferencia de conducción / citroen-xsara** — Cars/Cars.ini; Cars/xsara/xsara.sgc; Cars/xsara/xsara.ini; Cars/xsara/xsara_shaders.ini; Cars/xsara/xsara_shader_settings_clear.ini; Cars/xsara/Textures/xsara_external.dds; Cars/xsara/Textures/xsara_external_dynamic.dds; Cars/xsara/Textures/xsara_external_transparent.dds; Cars/xsara/Textures/xsara_internal.dds; Cars/xsara/Textures/xsara_internal_transparent.dds; Cars/xsara/Textures/xsara_internal_scrape.dds
- **Transferencia de conducción / ford-focus-rs-wrc-2006** — Cars/Cars.ini; Cars/FORD_FOCUS_2006/focus06.sgc; Cars/FORD_FOCUS_2006/focus06.ini; Cars/FORD_FOCUS_2006/focus06_shaders.ini; Cars/FORD_FOCUS_2006/focus06_shader_settings_clear.ini; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_dynamic.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_transparent.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_transparent.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_scrape.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_kwindscreen.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_parts.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_dynamic_mud.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_mud.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_scrape.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_extra_lights.dds Pack esperado: Ford_Focus_Mk_II_RS_WRC_2006_ngp6.
- **Transferencia de conducción / hyundai-accent-wrc** — Cars/Cars.ini; Cars/accent/accent.sgc; Cars/accent/accent.ini; Cars/accent/accent_shaders.ini; Cars/accent/accent_shader_settings_clear.ini; Cars/accent/Textures/accent_external.dds; Cars/accent/Textures/accent_external_dynamic.dds; Cars/accent/Textures/accent_external_transparent.dds; Cars/accent/Textures/accent_internal.dds; Cars/accent/Textures/accent_internal_transparent.dds; Cars/accent/Textures/accent_internal_scrape.dds
- **Transferencia de conducción / mg-zr-s1600** — Cars/Cars.ini; Cars/zr/zr.sgc; Cars/zr/zr.ini; Cars/zr/zr_shaders.ini; Cars/zr/zr_shader_settings_clear.ini; Cars/zr/Textures/zr_external.dds; Cars/zr/Textures/zr_external_dynamic.dds; Cars/zr/Textures/zr_external_transparent.dds; Cars/zr/Textures/zr_internal.dds; Cars/zr/Textures/zr_internal_transparent.dds; Cars/zr/Textures/zr_internal_scrape.dds
- **Transferencia de conducción / mitsubishi-lancer-evo-vii** — Cars/Cars.ini; Cars/Lancer_EVO_VII/lancer_evo_vII.sgc; Cars/Lancer_EVO_VII/lancer_evo_vII.ini; Cars/Lancer_EVO_VII/lancer_evo_vII_shaders.ini; Cars/Lancer_EVO_VII/lancer_evo_vII_shader_settings_clear.ini; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_external.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_external_dynamic.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_external_transparent.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_internal.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_internal_transparent.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_internal_scrape.dds
- **Transferencia de conducción / peugeot-206-wrc** — Cars/Cars.ini; Cars/Peugeot_206/Peugeot_206.sgc; Cars/Peugeot_206/Peugeot_206.ini; Cars/Peugeot_206/Peugeot_206_shaders.ini; Cars/Peugeot_206/Peugeot_206_shader_settings_clear.ini; Cars/Peugeot_206/Textures/e_p206_external.dds; Cars/Peugeot_206/Textures/e_p206_external_dynamic.dds; Cars/Peugeot_206/Textures/e_p206_external_transparent.dds; Cars/Peugeot_206/Textures/e_p206_internal.dds; Cars/Peugeot_206/Textures/e_p206_internal_transparent.dds; Cars/Peugeot_206/Textures/e_p206_internal_scrape.dds
- **Transferencia de conducción / rsf-audi-200-quattro-grpa** — Cars/Cars.ini
- **Transferencia de conducción / rsf-audi-quattro-a1-grpb** — Cars/Cars.ini
- **Transferencia de conducción / rsf-audi-quattro-a2-grpb** — Cars/Cars.ini
- **Transferencia de conducción / rsf-audi-sport-quattro-grpb** — Cars/Cars.ini
- **Transferencia de conducción / rsf-bmw-m1-grpb** — Cars/Cars.ini
- **Transferencia de conducción / rsf-bmw-m3-e30-grpa** — Cars/Cars.ini
- **Transferencia de conducción / rsf-citroen-c2-r2-max** — Cars/Cars.ini
- **Transferencia de conducción / rsf-citroen-c3-wrc-2017** — Cars/Cars.ini
- **Transferencia de conducción / rsf-citroen-c4-wrc-2008** — Cars/Cars.ini
- **Transferencia de conducción / rsf-citroen-ds3-wrc** — Cars/Cars.ini
- **Transferencia de conducción / rsf-citroen-xsara-wrc-2006** — Cars/Cars.ini
- **Transferencia de conducción / rsf-ford-fiesta-mk-viii-r2** — Cars/Cars.ini; Cars/Fiesta_R2_2019/Fiesta_R2_2019.sgc; Cars/Fiesta_R2_2019/Fiesta_R2_2019.ini; Cars/Fiesta_R2_2019/Fiesta_R2_2019_shaders.ini; Cars/Fiesta_R2_2019/Fiesta_R2_2019_shader_settings_clear.ini; Cars/Fiesta_R2_2019/Textures/Fiesta_R2_2019_external.dds; Cars/Fiesta_R2_2019/Textures/Fiesta_R2_2019_external_dynamic.dds; Cars/Fiesta_R2_2019/Textures/Fiesta_R2_2019_external_transparent.dds
- **Transferencia de conducción / rsf-ford-fiesta-rally2** — Cars/Cars.ini; Cars/fiesta_rally2_23/fiesta_rally2.sgc; Cars/fiesta_rally2_23/fiesta_rally2.ini; Cars/fiesta_rally2_23/fiesta_rally2_shaders.ini; Cars/fiesta_rally2_23/fiesta_rally2_shader_settings_clear.ini; Cars/fiesta_rally2_23/Textures/fiesta_rally2_external.dds; Cars/fiesta_rally2_23/Textures/fiesta_rally2_external_dynamic.dds; Cars/fiesta_rally2_23/Textures/fiesta_rally2_external_transparent.dds
- **Transferencia de conducción / rsf-ford-fiesta-rally3-evo** — Cars/Cars.ini
- **Transferencia de conducción / rsf-ford-fiesta-rally4** — Cars/Cars.ini
- **Transferencia de conducción / rsf-ford-fiesta-rs-wrc-2014** — Cars/Cars.ini
- **Transferencia de conducción / rsf-ford-fiesta-wrc-2019** — Cars/Cars.ini
- **Transferencia de conducción / rsf-hyundai-i20-coupe-wrc-2020** — Cars/Cars.ini
- **Transferencia de conducción / rsf-opel-adam-r2** — Cars/Cars.ini
- **Transferencia de conducción / rsf-peugeot-205-t16-grpb** — Cars/Cars.ini
- **Transferencia de conducción / rsf-peugeot-208-r2** — Cars/Cars.ini
- **Transferencia de conducción / rsf-peugeot-208-rally4** — Cars/Cars.ini
- **Transferencia de conducción / rsf-porsche-911-sc-rs-grpb** — Cars/Cars.ini; Cars/PORSCHE911_SCRS_GrpB/PORSCHE911_SCRS_GrpB.sgc; Cars/PORSCHE911_SCRS_GrpB/PORSCHE911_SCRS_GrpB.ini; Cars/PORSCHE911_SCRS_GrpB/PORSCHE911_SCRS_GrpB_shaders.ini; Cars/PORSCHE911_SCRS_GrpB/PORSCHE911_SCRS_GrpB_shader_settings_clear.ini; Cars/PORSCHE911_SCRS_GrpB/Textures/PORSCHE911_SCRS_GrpB_external.dds; Cars/PORSCHE911_SCRS_GrpB/Textures/PORSCHE911_SCRS_GrpB_external_dynamic.dds; Cars/PORSCHE911_SCRS_GrpB/Textures/PORSCHE911_SCRS_GrpB_external_transparent.dds
- **Transferencia de conducción / rsf-renault-clio-rally4** — Cars/Cars.ini
- **Transferencia de conducción / rsf-renault-clio-rally5** — Cars/Cars.ini
- **Transferencia de conducción / rsf-skoda-fabia-rs-rally2** — Cars/Cars.ini
- **Transferencia de conducción / rsf-skoda-fabia-wrc-2006** — Cars/Cars.ini; cars/fabia06/textures/e_p206_internal_kwindscreen.dds
- **Transferencia de conducción / rsf-subaru-impreza-gda-wrc2003-s9** — Cars/Cars.ini
- **Transferencia de conducción / rsf-toyota-celica-tct-ta64-grpb** — Cars/Cars.ini
- **Transferencia de conducción / rsf-vw-polo-r-wrc-2016** — Cars/Cars.ini
- **Transferencia de conducción / subaru-impreza-2000** — Cars/Cars.ini; Cars/impreza00/impreza00.sgc; Cars/impreza00/impreza00.ini; Cars/impreza00/impreza00_shaders.ini; Cars/impreza00/impreza00_shader_settings_clear.ini; Cars/impreza00/Textures/impreza00_external.dds; Cars/impreza00/Textures/impreza00_external_dynamic.dds; Cars/impreza00/Textures/impreza00_external_transparent.dds; Cars/impreza00/Textures/impreza00_internal.dds; Cars/impreza00/Textures/impreza00_internal_transparent.dds; Cars/impreza00/Textures/impreza00_internal_scrape.dds
- **Transferencia de conducción / subaru-impreza-2003** — Cars/Cars.ini; Cars/impreza03/impreza03.sgc; Cars/impreza03/impreza03.ini; Cars/impreza03/impreza03_shaders.ini; Cars/impreza03/impreza03_shader_settings_clear.ini; Cars/impreza03/Textures/impreza03_external.dds; Cars/impreza03/Textures/impreza03_external_dynamic.dds; Cars/impreza03/Textures/impreza03_external_transparent.dds; Cars/impreza03/Textures/impreza03_internal.dds; Cars/impreza03/Textures/impreza03_internal_transparent.dds; Cars/impreza03/Textures/impreza03_internal_scrape.dds
- **Transferencia de conducción / toyota-corolla-wrc** — Cars/Cars.ini; Cars/COROLLA/corolla.sgc; Cars/COROLLA/corolla.ini; Cars/COROLLA/corolla_shaders.ini; Cars/COROLLA/corolla_shader_settings_clear.ini; Cars/COROLLA/textures/corolla_external.dds; Cars/COROLLA/textures/corolla_external_dynamic.dds; Cars/COROLLA/textures/corolla_external_transparent.dds; Cars/COROLLA/textures/corolla_internal.dds; Cars/COROLLA/textures/corolla_internal_transparent.dds; Cars/COROLLA/textures/corolla_internal_scrape.dds; Cars/COROLLA/textures/e_p206_internal_kwindscreen.dds
- **Transferencia visual / citroen-xsara** — Cars/Cars.ini; Cars/xsara/xsara.sgc; Cars/xsara/xsara.ini; Cars/xsara/xsara_shaders.ini; Cars/xsara/xsara_shader_settings_clear.ini; Cars/xsara/Textures/xsara_external.dds; Cars/xsara/Textures/xsara_external_dynamic.dds; Cars/xsara/Textures/xsara_external_transparent.dds; Cars/xsara/Textures/xsara_internal.dds; Cars/xsara/Textures/xsara_internal_transparent.dds; Cars/xsara/Textures/xsara_internal_scrape.dds
- **Transferencia visual / ford-focus-rs-wrc-2006** — Cars/Cars.ini; Cars/FORD_FOCUS_2006/focus06.sgc; Cars/FORD_FOCUS_2006/focus06.ini; Cars/FORD_FOCUS_2006/focus06_shaders.ini; Cars/FORD_FOCUS_2006/focus06_shader_settings_clear.ini; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_dynamic.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_transparent.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_transparent.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_scrape.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_internal_kwindscreen.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_parts.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_dynamic_mud.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_mud.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_external_scrape.dds; Cars/FORD_FOCUS_2006/textures/focus_wrc06_extra_lights.dds Pack esperado: Ford_Focus_Mk_II_RS_WRC_2006_ngp6.
- **Transferencia visual / hyundai-accent-wrc** — Cars/Cars.ini; Cars/accent/accent.sgc; Cars/accent/accent.ini; Cars/accent/accent_shaders.ini; Cars/accent/accent_shader_settings_clear.ini; Cars/accent/Textures/accent_external.dds; Cars/accent/Textures/accent_external_dynamic.dds; Cars/accent/Textures/accent_external_transparent.dds; Cars/accent/Textures/accent_internal.dds; Cars/accent/Textures/accent_internal_transparent.dds; Cars/accent/Textures/accent_internal_scrape.dds
- **Transferencia visual / mg-zr-s1600** — Cars/Cars.ini; Cars/zr/zr.sgc; Cars/zr/zr.ini; Cars/zr/zr_shaders.ini; Cars/zr/zr_shader_settings_clear.ini; Cars/zr/Textures/zr_external.dds; Cars/zr/Textures/zr_external_dynamic.dds; Cars/zr/Textures/zr_external_transparent.dds; Cars/zr/Textures/zr_internal.dds; Cars/zr/Textures/zr_internal_transparent.dds; Cars/zr/Textures/zr_internal_scrape.dds
- **Transferencia visual / mitsubishi-lancer-evo-vii** — Cars/Cars.ini; Cars/Lancer_EVO_VII/lancer_evo_vII.sgc; Cars/Lancer_EVO_VII/lancer_evo_vII.ini; Cars/Lancer_EVO_VII/lancer_evo_vII_shaders.ini; Cars/Lancer_EVO_VII/lancer_evo_vII_shader_settings_clear.ini; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_external.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_external_dynamic.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_external_transparent.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_internal.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_internal_transparent.dds; Cars/Lancer_EVO_VII/Textures/Lancer_wrc_internal_scrape.dds
- **Transferencia visual / peugeot-206-wrc** — Cars/Cars.ini; Cars/Peugeot_206/Peugeot_206.sgc; Cars/Peugeot_206/Peugeot_206.ini; Cars/Peugeot_206/Peugeot_206_shaders.ini; Cars/Peugeot_206/Peugeot_206_shader_settings_clear.ini; Cars/Peugeot_206/Textures/e_p206_external.dds; Cars/Peugeot_206/Textures/e_p206_external_dynamic.dds; Cars/Peugeot_206/Textures/e_p206_external_transparent.dds; Cars/Peugeot_206/Textures/e_p206_internal.dds; Cars/Peugeot_206/Textures/e_p206_internal_transparent.dds; Cars/Peugeot_206/Textures/e_p206_internal_scrape.dds
- **Transferencia visual / rsf-fiesta-rally3** — Cars/Cars.ini
- **Transferencia visual / rsf-renault-cliov-rally3** — Cars/Cars.ini
- **Transferencia visual / subaru-impreza-2000** — Cars/Cars.ini; Cars/impreza00/impreza00.sgc; Cars/impreza00/impreza00.ini; Cars/impreza00/impreza00_shaders.ini; Cars/impreza00/impreza00_shader_settings_clear.ini; Cars/impreza00/Textures/impreza00_external.dds; Cars/impreza00/Textures/impreza00_external_dynamic.dds; Cars/impreza00/Textures/impreza00_external_transparent.dds; Cars/impreza00/Textures/impreza00_internal.dds; Cars/impreza00/Textures/impreza00_internal_transparent.dds; Cars/impreza00/Textures/impreza00_internal_scrape.dds
- **Transferencia visual / subaru-impreza-2003** — Cars/Cars.ini; Cars/impreza03/impreza03.sgc; Cars/impreza03/impreza03.ini; Cars/impreza03/impreza03_shaders.ini; Cars/impreza03/impreza03_shader_settings_clear.ini; Cars/impreza03/Textures/impreza03_external.dds; Cars/impreza03/Textures/impreza03_external_dynamic.dds; Cars/impreza03/Textures/impreza03_external_transparent.dds; Cars/impreza03/Textures/impreza03_internal.dds; Cars/impreza03/Textures/impreza03_internal_transparent.dds; Cars/impreza03/Textures/impreza03_internal_scrape.dds
- **Transferencia visual / toyota-corolla-wrc** — Cars/Cars.ini; Cars/COROLLA/corolla.sgc; Cars/COROLLA/corolla.ini; Cars/COROLLA/corolla_shaders.ini; Cars/COROLLA/corolla_shader_settings_clear.ini; Cars/COROLLA/textures/corolla_external.dds; Cars/COROLLA/textures/corolla_external_dynamic.dds; Cars/COROLLA/textures/corolla_external_transparent.dds; Cars/COROLLA/textures/corolla_internal.dds; Cars/COROLLA/textures/corolla_internal_transparent.dds; Cars/COROLLA/textures/corolla_internal_scrape.dds; Cars/COROLLA/textures/e_p206_internal_kwindscreen.dds
- **Respaldo / ford-focus-rs-wrc-2006** — Pack esperado: Ford_Focus_Mk_II_RS_WRC_2006_ngp6.
- **Respaldo / rsf-fabia06** — cars/fabia06/textures/e_p206_internal_kwindscreen.dds
- **Respaldo / rsf-fiesta-rally2-23** — cars/fiesta_rally2_23/textures/fiesta_rally2_external_mud.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_dynamic_mud.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_scrape.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_dynamic.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_transparent.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_ext.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_parts.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_drivers.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_parts.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_transparent.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_kwindscreen.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_scrape.dds
- **Respaldo / rsf-fiesta-rally2-23-2** — cars/fiesta_rally2_23/textures/fiesta_rally2_external_mud.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_dynamic_mud.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_scrape.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_dynamic.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_transparent.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_ext.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_parts.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_drivers.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_parts.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_transparent.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_internal_kwindscreen.dds; cars/fiesta_rally2_23/textures/fiesta_rally2_external_scrape.dds
