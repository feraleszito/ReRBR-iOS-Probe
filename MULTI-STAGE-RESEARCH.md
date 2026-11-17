# MULTI-STAGE-RESEARCH.md — Harwood Forest, Chirdonhead y Côte d'Arbroz

Investigación de solo lectura sobre la instalación local `C:\Richard Burns Rally` (RBR + RallySimFans), sin modificar ningún archivo original.

## 1. Identificación interna y archivos en `Maps\`

| Tramo | Superficie | ID interno (`[MapNN]`) | Stem base (`_O` seco) | Variantes presentes en `Maps\` | Longitud (`stages_data.json`) |
| --- | --- | --- | --- | --- | --- |
| **Harwood Forest** (referencia) | Tierra (gravel) | `20` | `track-20_O` | `_N` (Noon), `_O` (Overcast) | 6100 m |
| **Chirdonhead** | Tierra (gravel) | `22` | `track-22_O` | `_N` (Noon), `_O` (Overcast) | 7000 m |
| **Côte d'Arbroz** | Asfalto (tarmac) | `41` | `track-41_O` | `_M` (Morning), `_O` (Overcast) | 4500 m |
| **Côte d'Arbroz II** | Asfalto (tarmac) | `46` | `track-46_O` | `_E` (Evening), `_O` (Overcast) | 4300 m |

Cada tramo requiere en `Maps\` los 6 archivos de física/escenario (`PHYSICS_STAGE_EXTENSIONS`) más el `.ini` de texturas, los archivos `*_textures.rbz` y `TextureFilenameMap.dat`:

- Geometría y objetos: `<stem>.lbs`
- Colisiones físicas: `<stem>.col`
- Terreno físico: `<stem>.trk`
- Materiales/superficies físicas: `<stem>.mat`
- Driveline: `<stem>.dls`
- Vallas/cercas: `<stem>.fnc`
- Definición de texturas: `<stem>.ini` + `TextureFilenameMap.dat` + `track-<id>_<variante>_textures.rbz`

### Resolución de texturas y variantes de hora del día

En `stages_tracksettings.json`:
- `20` (Harwood) y `22` (Chirdonhead) usan `timeofday = 1` (`_N`) y `3` (`_O`).
- `41` (Côte d'Arbroz) usa `timeofday = 0` (`_M`) y `3` (`_O`).
- `46` (Côte d'Arbroz II) usa `timeofday = 2` (`_E`) y `3` (`_O`).

`TextureFilenameMap.dat` redirige las peticiones `track-<id>_O` de cada tramo hacia su paquete principal de texturas (`track-20_N_textures.rbz`, `track-22_N_textures.rbz`, `track-41_M_textures.rbz`, `track-46_E_textures.rbz`), además de los DDS específicos en `track-<id>_O_textures.rbz`.

Conteos declarados en `<stem>.ini`:
- `track-20_O.ini`: `NumTextures=39`, `NumSpecularTextures=5`, `NumShadowTextures=0`
- `track-22_O.ini`: `NumTextures=39`, `NumSpecularTextures=5`, `NumShadowTextures=0`
- `track-41_O.ini`: `NumTextures=34`, `NumSpecularTextures=1`, `NumShadowTextures=0`
- `track-46_O.ini`: `NumTextures=34`, `NumSpecularTextures=1`, `NumShadowTextures=0`

## 2. Validación real con `harwood::stage::load_selected`

Ejecutado sobre `C:\Richard Burns Rally\Maps` con los parsers reales de `rerbr-asset-formats` y colisiones de `rerbr-sim`:

| Tramo (`stem`) | Bloques terreno | Objetos estáticos | Instancias interactivas | Retenidas (`withheld`) | Puntos ruta (`dls` / colisión) | Gap máximo terreno/COL | Texturas únicas | Memoria residente estimada |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `track-20_O` | 282 | 442 | 604 | 0 | 568 / 568 | 0.0001 m | 28 | 34 MiB |
| `track-22_O` | 229 | 308 | 306 | 0 | 202 / 202 | 0.0001 m | 27 | 31 MiB |
| `track-41_O` | 147 | 270 | 457 | 0 | 310 / 310 | 0.0001 m | 26 | 28 MiB |
| `track-46_O` | 144 | 262 | 454 | 0 | 310 / 310 | 0.0001 m | 26 | 28 MiB |

### Conclusiones de compatibilidad
- **Chirdonhead (`22`)**, **Côte d'Arbroz (`41`)** y **Côte d'Arbroz II (`46`)** cargan con **0 objetos retenidos**, **concordancia de alturas entre terreno `.lbs` y colisión `.col` en el 100 % de los puntos de ruta muestreados (no de toda la malla)**, y un consumo residente inferior al de Harwood Forest (28–31 MiB vs 34 MiB; muy por debajo del presupuesto de 192 MiB).
- **Superficies**: `rbr-gameplay::weather` y `rerbr-sim` distinguen `gravel` (mapas 20 y 22) y `tarmac` (mapas 41 y 46) a partir de la tabla de temporada en `misc.rbz` y los archivos `.mat` / `.trk` originales del tramo.
