# Auditoría de Cockpits e Instrumentación — RBR Mobile

## Resumen Ejecutivo
Tras corregir el filtro de visibilidad inicial `shown_at_start()` en [selection.rs](file:///C:/Users/adria/Documents/GitHub/ReRBR-iOS-Probe/src/car/selection.rs) (permitiendo `Predicate = NotUsed` aunque `Switch = true` y `SwitchBox = light`) y añadir soporte para `[i_bodyi]` en [mesh.rs](file:///C:/Users/adria/Documents/GitHub/ReRBR-iOS-Probe/src/car/mesh.rs):

- **40 / 40 vehículos RSF** de la instalación completa del PC y carpeta de transferencia iPad cargan su geometría interior (`i_body` / `i_bodyi`), volante animado (`i_steeringwheel`, slot 5) y texturas `.dds` sin errores (`verify_audi_i20_bmw_c2_and_all_cockpits_if_pc_present`).
- **8 / 8 vehículos originales RBR 1.02** conservan su comportamiento original en `physics.rbz` / `Cars/`.

## Causas Corregidas en Casos Reportados
1. **BMW M3 E30 GrpA (`BMW_M3_E30`) — Recuperado (`Funcional`):**
   - Causa real: `[i_body]` en `BMW_M3_E30.ini` tenía `Switch = true`, `SwitchBox = light`, `Predicate = NotUsed`. El cargador exigía `Predicate == "off"`, descartando toda la carrocería interior y provocando `CAR: i_body missing`.
2. **Citroën C2 R2 Max (`C2_R2_Max`) — Recuperado (`Funcional`):**
   - Causa real: `[i_steeringwheel]` en `C2_R2_Max.ini` tenía `Switch = true`, `SwitchBox = light`, `Predicate = NotUsed`, provocando `COCKPIT: invalid original steering-wheel geometry/transform` y el aviso `COCKPIT ASSETS MISSING - EXTERIOR VIEW`.
3. **VW Polo R WRC 2016 (`POLO_R_WRC_2016`) — Recuperado (`Funcional`):**
   - Causa real: el nodo de carrocería interior en `polo_wrc_2016.ini` se denomina `[i_bodyi]` en lugar de `[i_body]`.

## Instrumentación Funcional en Cockpit
- **Volante 3D:** conectado en todos los vehículos mediante el nodo original `i_steeringwheel` al ángulo real de cremallera de la física NGP a 720 Hz (`cockpit::steering_transform`).
- **Agujas y pantallas en tablero 3D:** ninguno de los `.sgc` de la flota RSF analizada expone mallas separadas animables para agujas (`i_needle_*` / `i_tacho` / `i_speedo`); únicamente existen carcasas estáticas (`i_dash`, `i_body_lcd`).
- **Solución implementada sin inventar mallas ni modificar `.sgc`:** cuando `draws_interior()` está activo (vista `COCKPIT`) y `minimal_hud == false`, se renderiza la instrumentación funcional (`draw_cockpit_instruments` en [mod.rs](file:///C:/Users/adria/Documents/GitHub/ReRBR-iOS-Probe/src/driving/mod.rs)) con tacómetro analógico de aguja, velocímetro analógico de aguja y marcha/RPM digitales conectados directamente a la telemetría física a 720 Hz y al límite real `rpm_limit` de cada coche.

## Clasificación por Vehículo (Flota RSF Auditada)

| Vehículo | Modelo 3D | Interior (`i_body` + `i_steeringwheel`) | Instrumentación | Estado |
| :--- | :--- | :--- | :--- | :--- |
| Audi 200 quattro GrpA | `Cars/Audi_200_quattro/Audi_200_quattro.sgc` | 20 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Audi quattro A1 GrpB | `Cars/AUDI_QUATTRO_A1_GrpB/AUDI_QUATTRO_A1_GrpB.sgc` | 17 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Audi quattro A2 GrpB | `Cars/AUDI_QUATTRO_A2_GrpB/AUDI_QUATTRO_A2_GrpB.sgc` | 19 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Audi quattro Grp4 | `Cars/AUDI_QUATTRO_Grp4/AUDI_QUATTRO_Grp4.sgc` | 20 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Audi Sport quattro GrpB | `Cars/Audi_Sport_Quattro/Audi_Sport_Quattro.sgc` | 13 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| BMW M1 GrpB | `Cars/BMW_M1/BMW_M1.sgc` | 14 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| BMW M3 E30 GrpA | `Cars/BMW_M3_E30/BMW_M3_E30.sgc` | Mallas completas + volante 3D (antes bloqueado por `Predicate=NotUsed`) | Volante 3D + Clúster Cockpit HUD | Recuperado / Funcional |
| Citroën C2 R2 Max | `Cars/C2_R2_Max/C2_R2_Max.sgc` | 13 mallas + volante 3D (antes bloqueado por `Predicate=NotUsed`) | Volante 3D + Clúster Cockpit HUD | Recuperado / Funcional |
| Citroën C3 WRC 2017 | `Cars/Citroen_C3_WRC17/c3_wrc_17.sgc` | 16 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Citroën C4 WRC 2008 | `Cars/C4_08/c4_08.sgc` | 25 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Citroën DS3 WRC | `Cars/DS3_WRC/ds3_wrc.sgc` | 21 mallas + volante 3D (`i_dash` estático) | Volante 3D + Clúster Cockpit HUD | Funcional |
| Citroën Xsara WRC 2006 | `Cars/Xsara_2006/xsara.sgc` | 18 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Ford Fiesta Mk VIII R2 | `Cars/Fiesta_R2_2019/fiesta_r2_2019.sgc` | 16 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Ford Fiesta RS WRC 2014 | `Cars/fiesta_wrc_14/fiesta_wrc_14.sgc` | 19 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Ford Fiesta Rally2 / Rally3 / Evo / Rally4 / WRC 2019 | `Cars/fiesta_rally*` / `fiesta_wrc_19` | 17–24 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Ford Focus RS WRC 2006 | `Cars/FORD_FOCUS_2006/focus06.sgc` | Mallas completas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Hyundai i20 Coupe WRC 2017 / 2020 / 2021 | `Cars/i20_WRC_20/i20_wrc_20.sgc` (y 17/21) | 28 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Opel ADAM R2 | `Cars/Opel_Adam_R2/Opel_Adam_R2.sgc` | 13 mallas + volante 3D (`i_body_lcd` estático) | Volante 3D + Clúster Cockpit HUD | Funcional |
| Peugeot 205 T16 GrpB | `Cars/PEUGEOT_205_T16_e1/PEUGEOT_205_T16_e1.sgc` | 15 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Peugeot 208 R2 / Rally4 / T16 R5 | `Cars/PEUGEOT_208_*` | 15–24 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Porsche 911 SC RS GrpB | `Cars/Porsche_911_SC_RS/Porsche_911_SC_RS.sgc` | 19 mallas + volante 3D (`i_dash` estático) | Volante 3D + Clúster Cockpit HUD | Funcional |
| Renault Clio 16S / III R3 / IV R3T / Rally3 / Rally4 / Rally5 | `Cars/Renault_Clio*` | 24 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Škoda Fabia RS Rally2 / WRC 2006 | `Cars/fabia_rs_rally2` / `FABIA06` | 14–24 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Subaru Impreza GDA WRC2003 (S9) | `Cars/impreza_2003_kazhol/impreza03.sgc` | 14 mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| Toyota Celica 2000GT / TCT TA64 / GR Yaris Rally2 / Yaris WRC 2018 | `Cars/CELICA_*` / `yaris_*` | 16+ mallas + volante 3D | Volante 3D + Clúster Cockpit HUD | Funcional |
| VW Polo R WRC 2016 | `Cars/POLO_R_WRC_2016/polo_wrc_2016.sgc` | Mallas completas + volante 3D (`i_bodyi`) | Volante 3D + Clúster Cockpit HUD | Recuperado / Funcional |

> Nota: Si en la instalación del usuario en iPad (`Documents/RBR/Cars/...`) faltara algún archivo `.dds` o `.sgc` concreto, el motor mantiene de forma segura la vista exterior y muestra el aviso `COCKPIT ASSETS MISSING - EXTERIOR VIEW` sin cerrar la sesión ni alterar las físicas.
