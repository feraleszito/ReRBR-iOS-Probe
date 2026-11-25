#!/usr/bin/env python3
"""Extrae un paquete minimo de prueba (1 vehiculo + 1 tramo) desde una instalacion legitima de RBR.

Reglas estrictas:
- Opera exclusivamente en modo SOLO LECTURA sobre las carpetas de origen.
- Nunca modifica, mueve ni altera archivos originales del usuario.
- Rechaza escribir dentro del arbol del repositorio git (salvo `.validation/`).
- Verifica SHA-256 del runtime NGP 7.4.778.507 y todas las dependencias reales:
  geometria 3D (.sgc/.ini/shaders), texturas (.dds), pack NGP6 (.lsp/repair.ini),
  bancos FMOD (.bank/.strings.bank), geometria/colision/texturas compartidas del tramo,
  espectadores (.sgs), scripts de arranque y atlas de iconos de copiloto (.bnk/.dds).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import sys
import zipfile

CERTIFIED_NGP74_SHA256 = "95b6dbf5e8957357d857a8e919cff177a6bfb832f5324da90467d43732edaaa4"
KNOWN_NGP75_SHA256 = "cf351cdf99f880631ed2cb1d9b6b5c9db7d7c233ad905740393c37df967801ec"

CURVE_SPRITE_PROBE = 0x503
HAZARD_SPRITE_PROBE = 0x4C3

STAGE_PRESETS: dict[str, dict[str, object]] = {
    "harwood-forest": {
        "name": "Harwood Forest",
        "stem": "track-20_O",
        "map": 20,
        "country_prefix": "br",
        "rally_script_dir": "pirelli",
        "sibling_texture_rbz": ["track-20_N_textures.rbz"],
    },
    "chirdonhead": {
        "name": "Chirdonhead",
        "stem": "track-22_O",
        "map": 22,
        "country_prefix": "br",
        "rally_script_dir": "pirelli",
        "sibling_texture_rbz": ["track-22_N_textures.rbz"],
    },
    "cote-darbroz": {
        "name": "Cote d'Arbroz",
        "stem": "track-41_N",
        "map": 41,
        "country_prefix": "mo",
        "rally_script_dir": "montblanc",
        "sibling_texture_rbz": ["track-41_O_textures.rbz"],
    },
    "cote-darbroz-ii": {
        "name": "Cote d'Arbroz II",
        "stem": "track-46_O",
        "map": 46,
        "country_prefix": "mo",
        "rally_script_dir": "montblanc",
        "sibling_texture_rbz": ["track-46_N_textures.rbz"],
    },
}

RETAIL_VEHICLES: dict[str, dict[str, object]] = {
    "citroen-xsara": {"slot": 0, "folder": "CITROEN_XSARA", "stem": "xsara", "eng": "xsara.eng"},
    "hyundai-accent": {"slot": 1, "folder": "HYUNDAI_ACCENT", "stem": "accent", "eng": "accent.eng"},
    "mg-zr-s1600": {"slot": 2, "folder": "MG_ZR", "stem": "mg_zr", "eng": "mg_zr.eng"},
    "mitsubishi-lancer-evo-vii": {"slot": 3, "folder": "MITSUBISHI_LANCER", "stem": "lancer", "eng": "lancer.eng"},
    "peugeot-206": {"slot": 4, "folder": "PEUGEOT_206", "stem": "206", "eng": "206.eng"},
    "subaru-impreza-2003": {"slot": 5, "folder": "impreza03", "stem": "impreza03", "eng": "subaru.eng"},
    "toyota-corolla-wrc": {"slot": 6, "folder": "TOYOTA_COROLLA", "stem": "corolla", "eng": "corolla.eng"},
    "subaru-impreza-2000": {"slot": 7, "folder": "SUBARU_IMPREZA_2000", "stem": "impreza00", "eng": "impreza2000.eng"},
}

DEFAULT_FOCUS_2006 = {
    "id": "ford-focus-rs-wrc-2006",
    "name": "Ford Focus RS WRC 2006",
    "folder": "FORD_FOCUS_2006",
    "stem": "focus06",
    "rsf_pack": "Ford_Focus_Mk_II_RS_WRC_2006_ngp6",
    "fmod_bank": "Focus_Mk2_WRC.bank",
}


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def resolve_ci(root: Path, rel: str) -> Path | None:
    """Resolucion insensible a mayusculas/minusculas sin tocar el origen."""
    direct = root / rel
    if direct.exists():
        return direct
    cur = root
    for part in Path(rel).parts:
        if not cur.is_dir():
            return None
        target = part.lower()
        found = None
        for child in cur.iterdir():
            if child.name.lower() == target:
                found = child
                break
        if found is None:
            return None
        cur = found
    return cur


def slugify_rsf_name(name: str) -> str:
    lower = name.strip().lower()
    if lower in ("ford focus mk ii rs wrc 2006", "ford focus rs wrc 2006"):
        return "ford-focus-rs-wrc-2006"
    slug = re.sub(r"[^a-z0-9]+", "-", lower).strip("-")
    return f"rsf-{slug}"


def parse_bnk_pacenote_pages(bnk_path: Path) -> list[str]:
    data = bnk_path.read_bytes()
    if len(data) < 8:
        return []
    page_count, sprite_count = struct.unpack_from("<II", data, 0)
    sprites_at = 8 + page_count * 64
    if len(data) != sprites_at + sprite_count * 36:
        return []
    pages: list[str] = []
    for idx in range(page_count):
        raw = data[8 + idx * 64 : 8 + (idx + 1) * 64]
        end = raw.find(0)
        if end != -1:
            raw = raw[:end]
        pages.append(raw.decode("utf-8", errors="replace"))
    needed: list[str] = []
    for probe in (CURVE_SPRITE_PROBE, HAZARD_SPRITE_PROBE):
        if probe < sprite_count:
            (page_idx,) = struct.unpack_from("<I", data, sprites_at + probe * 36)
            if page_idx < len(pages):
                page_name = pages[page_idx]
                if page_name not in needed:
                    needed.append(page_name)
    return needed


def parse_audiobank_ini(path: Path) -> dict[str, str]:
    mapping: dict[str, str] = {}
    if not path.is_file():
        return mapping
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line or line.startswith((";", "#", "[")) or "=" not in line:
            continue
        k, v = line.split("=", 1)
        mapping[k.strip().lower()] = v.strip()
    return mapping


def resolve_rsf_vehicle(rbr_root: Path, vehicle_arg: str) -> dict[str, str] | None:
    if vehicle_arg == DEFAULT_FOCUS_2006["id"]:
        return dict(DEFAULT_FOCUS_2006)
    cars_json = resolve_ci(rbr_root, "rsfdata/cache/cars.json")
    models_json = resolve_ci(rbr_root, "rsfdata/cache/carmodels.json")
    audiobank_ini = resolve_ci(rbr_root, "rsfdata/cache/audiobank.ini")
    if not cars_json or not models_json:
        return None
    cars_data = json.loads(cars_json.read_text(encoding="utf-8", errors="replace"))
    models_data = json.loads(models_json.read_text(encoding="utf-8", errors="replace"))
    models_by_id = {str(m.get("id")): m for m in models_data if isinstance(m, dict)}
    audiobanks = parse_audiobank_ini(audiobank_ini) if audiobank_ini else {}

    target = vehicle_arg.strip().lower()
    for entry in cars_data:
        if not isinstance(entry, dict):
            continue
        name = str(entry.get("name", "")).strip()
        phys = str(entry.get("physics_Revision", entry.get("path", ""))).strip()
        if not phys and entry.get("folder"):
            phys = str(entry["folder"]).strip()
        car_id = slugify_rsf_name(name)
        if target not in (car_id.lower(), name.lower(), phys.lower()):
            continue
        model_info = models_by_id.get(str(entry.get("carmodel_id", "")), {})
        rel_path = str(model_info.get("path", "")).replace("\\", "/").strip("/")
        folder = rel_path.split("/")[-1] if rel_path else ""
        ini_file = str(model_info.get("iniFile", "")).strip()
        stem = Path(ini_file).stem if ini_file else ""
        bank = audiobanks.get(phys.lower(), "")
        if bank and not bank.lower().endswith(".bank"):
            bank += ".bank"
        return {
            "id": car_id,
            "name": name,
            "folder": folder,
            "stem": stem,
            "rsf_pack": phys,
            "fmod_bank": bank,
        }
    return None


def resolve_stage(rbr_root: Path, stage_arg: str) -> dict[str, object]:
    key = stage_arg.strip().lower()
    if key in STAGE_PRESETS:
        return dict(STAGE_PRESETS[key])
    m = re.match(r"^track-(\d+)_([onb])$", key)
    if not m:
        raise SystemExit(
            f"Tramo no reconocido: '{stage_arg}'. Usa uno de {sorted(STAGE_PRESETS)} o formato track-<num>_<O|N|B>."
        )
    map_num = int(m.group(1))
    weather = m.group(2).upper()
    stem = f"track-{map_num}_{weather}"
    siblings: list[str] = []
    maps_dir = resolve_ci(rbr_root, "Maps")
    if maps_dir and maps_dir.is_dir():
        for w in ("N", "O", "B"):
            if w != weather and resolve_ci(maps_dir, f"track-{map_num}_{w}_textures.rbz"):
                siblings.append(f"track-{map_num}_{w}_textures.rbz")
    rally_by_decade = {
        1: ("ar", "arctic"),
        2: ("br", "pirelli"),
        3: ("au", "canberra"),
        4: ("mo", "montblanc"),
        5: ("ho", "hokkaido"),
        6: ("us", "ramada"),
    }
    country_prefix, rally_script_dir = rally_by_decade.get(map_num // 10, ("br", ""))
    return {
        "name": stem,
        "stem": stem,
        "map": map_num,
        "country_prefix": country_prefix,
        "rally_script_dir": rally_script_dir,
        "sibling_texture_rbz": siblings,
    }


def check_zip_entries(zip_path: Path, required_entries: list[str]) -> list[str]:
    missing: list[str] = []
    try:
        with zipfile.ZipFile(zip_path, "r") as zf:
            names = {n.replace("\\", "/").lower() for n in zf.namelist()}
            for req in required_entries:
                if req.replace("\\", "/").lower() not in names:
                    missing.append(f"{zip_path.name} -> {req}")
    except zipfile.BadZipFile:
        missing.append(f"{zip_path.name} (archivo RBZ/ZIP corrupto)")
    return missing


def ensure_outside_repo(output_dir: Path) -> None:
    repo_root = Path(__file__).resolve().parents[1]
    out_resolved = output_dir.resolve()
    val_dir = (repo_root / ".validation").resolve()
    if out_resolved == repo_root or (
        repo_root in out_resolved.parents and out_resolved != val_dir and val_dir not in out_resolved.parents
    ):
        raise SystemExit(
            f"SEGURIDAD: --output ({out_resolved}) apunta dentro del repositorio ({repo_root}).\n"
            "Elige una carpeta fuera del repositorio (por ejemplo en Descargas o Escritorio) "
            "para no contaminar git con recursos comerciales de RBR."
        )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Extrae un paquete minimo de prueba (1 coche + 1 tramo) en modo solo lectura."
    )
    parser.add_argument("--rbr-root", required=True, type=Path, help="Ruta a la instalacion legitima de RBR")
    parser.add_argument(
        "--ngp74-root",
        type=Path,
        default=None,
        help="Ruta opcional al paquete NGP 7.4.778.507 (Plugins/NGP.dll + physics.rbz) si --rbr-root tiene NGP 7.5",
    )
    parser.add_argument("--output", required=True, type=Path, help="Carpeta destino donde crear RBR/")
    parser.add_argument(
        "--vehicle",
        default="ford-focus-rs-wrc-2006",
        help="ID del vehiculo (por defecto: ford-focus-rs-wrc-2006, o ej. subaru-impreza-2003)",
    )
    parser.add_argument(
        "--stage",
        default="harwood-forest",
        help="ID o stem del tramo (por defecto: harwood-forest, chirdonhead, cote-darbroz, cote-darbroz-ii)",
    )
    parser.add_argument(
        "--zip",
        action="store_true",
        help="Generar ademas un archivo ZIP listo para subir a Android / BrowserStack",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Solo verificar dependencias y emitir informe sin copiar archivos",
    )
    args = parser.parse_args()

    rbr_root: Path = args.rbr_root.resolve()
    ngp74_root: Path | None = args.ngp74_root.resolve() if args.ngp74_root else None
    output_dir: Path = args.output.resolve()

    if not rbr_root.is_dir():
        print(f"ERROR: No existe la carpeta --rbr-root: {rbr_root}", file=sys.stderr)
        return 2
    if not args.dry_run:
        ensure_outside_repo(output_dir)

    stage = resolve_stage(rbr_root, args.stage)
    copy_plan: list[tuple[Path, str, str]] = []  # (src_path, dest_rel_under_RBR, category)
    missing: list[dict[str, str]] = []
    warnings: list[str] = []

    def add_required(roots: list[Path], rel: str, category: str, dest_rel: str | None = None) -> Path | None:
        target_rel = dest_rel or rel
        for root in roots:
            found = resolve_ci(root, rel)
            if found and found.is_file():
                copy_plan.append((found, target_rel, category))
                return found
        missing.append({"path": f"RBR/{target_rel}", "category": category, "reason": "Archivo no encontrado"})
        return None

    # 1. Runtime NGP 7.4.778.507 + archivos base compartidos
    ngp_roots = ([ngp74_root] if ngp74_root else []) + [rbr_root]
    ngp_dll: Path | None = None
    ngp_bad_detail: str | None = None
    for cand_root in ngp_roots:
        for rel_cand in ("Plugins/NGP.dll", "NGP.dll"):
            cand = resolve_ci(cand_root, rel_cand)
            if cand and cand.is_file():
                digest = sha256_file(cand)
                if digest == CERTIFIED_NGP74_SHA256:
                    ngp_dll = cand
                    break
                if digest == KNOWN_NGP75_SHA256:
                    ngp_bad_detail = (
                        f"Detectado NGP 7.5.779.508 en {cand} (SHA-256 {digest[:12]}...). "
                        "El motor exige exactamente NGP 7.4.778.507. Pasa --ngp74-root apuntando a tu carpeta NGP 7.4."
                    )
                else:
                    ngp_bad_detail = (
                        f"NGP.dll en {cand} tiene SHA-256 no soportado ({digest[:16]}...); "
                        f"se requiere {CERTIFIED_NGP74_SHA256[:16]}... (NGP 7.4.778.507)."
                    )
        if ngp_dll:
            break

    if ngp_dll:
        copy_plan.append((ngp_dll, "Plugins/NGP.dll", "core_ngp"))
    else:
        missing.append(
            {
                "path": "RBR/Plugins/NGP.dll",
                "category": "core_ngp",
                "reason": ngp_bad_detail or "No se encontro Plugins/NGP.dll (NGP 7.4.778.507)",
            }
        )

    phys_rbz = add_required(ngp_roots, "physics.rbz", "core_physics")
    if phys_rbz:
        for bad_entry in check_zip_entries(
            phys_rbz,
            ["Physics/physics.lsp", "Physics/pirelli/tyres.lsp", "Physics/michelin/tyres.lsp"],
        ):
            missing.append({"path": f"RBR/{bad_entry}", "category": "core_physics", "reason": "Entrada ausente en RBZ"})

    misc_rbz = add_required([rbr_root], "misc.rbz", "core_boot")
    if misc_rbz:
        for bad_entry in check_zip_entries(misc_rbz, ["Misc/prime.lsp", "Misc/nice_w.ini", "Misc/bad_w.ini"]):
            missing.append({"path": f"RBR/{bad_entry}", "category": "core_boot", "reason": "Entrada ausente en RBZ"})

    scripts_rbz = add_required([rbr_root], "Scripts.rbz", "core_boot")
    if scripts_rbz:
        rally_dir = str(stage["rally_script_dir"])
        sub = f"Scripts/{rally_dir}/" if rally_dir else "Scripts/"
        for idx in range(1, 6):
            for bad_entry in check_zip_entries(scripts_rbz, [f"{sub}load_car_flip{idx}.lua"]):
                missing.append(
                    {"path": f"RBR/{bad_entry}", "category": "core_boot", "reason": "Script Lua ausente en Scripts.rbz"}
                )

    add_required([rbr_root], "Cars/Cars.ini", "core_boot")

    # Espectadores requeridos por boot::inputs y por el pais del tramo
    prefixes = ["br"]
    stage_prefix = str(stage["country_prefix"])
    if stage_prefix not in prefixes:
        prefixes.append(stage_prefix)
    for prefix in prefixes:
        for idx in range(4):
            add_required([rbr_root], f"Sgs/{prefix}_spectator_0{idx}.sgs", "core_spectators")

    # Opcionales recomendados ligeros: iconos visuales del copiloto y calendario
    season_dat = resolve_ci(rbr_root, "season.dat")
    if season_dat and season_dat.is_file():
        copy_plan.append((season_dat, "season.dat", "shared_ui"))

    for width in (1280, 1024, 800, 640):
        bnk = resolve_ci(rbr_root, f"Generic/PC_Generic_{width}.bnk")
        if bnk and bnk.is_file():
            pages = parse_bnk_pacenote_pages(bnk)
            resolved_pages = [(pg, resolve_ci(rbr_root, f"Generic/{pg}")) for pg in pages]
            if pages and all(p and p.is_file() for _, p in resolved_pages):
                copy_plan.append((bnk, f"Generic/{bnk.name}", "shared_pacenote_hud"))
                for _, p in resolved_pages:
                    assert p is not None
                    copy_plan.append((p, f"Generic/{p.name}", "shared_pacenote_hud"))
                break
    else:
        warnings.append("Generic/PC_Generic_*.bnk no encontrado; las notas visuales usaran texto del .dls.")

    # 2. Vehiculo (RSF NGP6 + FMOD o Retail 1.02 + .eng)
    veh_key = args.vehicle.strip().lower()
    vehicle_summary: dict[str, object] = {}
    if veh_key in RETAIL_VEHICLES:
        spec = RETAIL_VEHICLES[veh_key]
        folder = str(spec["folder"])
        stem = str(spec["stem"])
        eng = str(spec["eng"])
        vehicle_summary = {"id": veh_key, "mode": "retail_102", "folder": folder, "audio": f"Audio/Cars/{eng}"}
        car_dir = resolve_ci(rbr_root, f"Cars/{folder}")
        if not car_dir or not car_dir.is_dir():
            missing.append(
                {"path": f"RBR/Cars/{folder}", "category": "vehicle_model", "reason": "Carpeta del modelo 3D ausente"}
            )
        else:
            add_required([rbr_root], f"Cars/{folder}/{stem}.sgc", "vehicle_model")
            add_required([rbr_root], f"Cars/{folder}/{stem}.ini", "vehicle_model")
            add_required([rbr_root], f"Cars/{folder}/{stem}_shaders.ini", "vehicle_model")
            for f in sorted(car_dir.iterdir()):
                if f.is_file() and f.suffix.lower() in (".ini", ".sgc") and f.name.lower() not in (
                    f"{stem}.sgc".lower(),
                    f"{stem}.ini".lower(),
                    f"{stem}_shaders.ini".lower(),
                ):
                    copy_plan.append((f, f"Cars/{folder}/{f.name}", "vehicle_model"))
            tex_dir = resolve_ci(car_dir, "textures")
            if tex_dir and tex_dir.is_dir():
                dds_files = sorted(f for f in tex_dir.iterdir() if f.is_file() and f.suffix.lower() == ".dds")
                if not dds_files:
                    missing.append(
                        {
                            "path": f"RBR/Cars/{folder}/textures/*.dds",
                            "category": "vehicle_textures",
                            "reason": "Sin texturas DDS",
                        }
                    )
                for dds in dds_files:
                    copy_plan.append((dds, f"Cars/{folder}/textures/{dds.name}", "vehicle_textures"))
            else:
                missing.append(
                    {
                        "path": f"RBR/Cars/{folder}/textures",
                        "category": "vehicle_textures",
                        "reason": "Carpeta textures ausente",
                    }
                )
        add_required([rbr_root], "Audio/Cars/Cars.ini", "vehicle_audio")
        add_required([rbr_root], f"Audio/Cars/{eng}", "vehicle_audio")
    else:
        rsf = resolve_rsf_vehicle(rbr_root, args.vehicle)
        if not rsf:
            print(
                f"ERROR: No se pudo resolver el vehiculo '{args.vehicle}' en rsfdata/cache/cars.json ni presets.",
                file=sys.stderr,
            )
            return 2
        folder = rsf["folder"]
        stem = rsf["stem"]
        pack = rsf["rsf_pack"]
        bank = rsf["fmod_bank"]
        vehicle_summary = {
            "id": rsf["id"],
            "name": rsf["name"],
            "mode": "rsf_ngp6",
            "folder": folder,
            "rsf_pack": pack,
            "fmod_bank": bank,
        }
        car_dir = resolve_ci(rbr_root, f"Cars/{folder}")
        if not car_dir or not car_dir.is_dir():
            missing.append(
                {"path": f"RBR/Cars/{folder}", "category": "vehicle_model", "reason": "Carpeta del modelo 3D ausente"}
            )
        else:
            add_required([rbr_root], f"Cars/{folder}/{stem}.sgc", "vehicle_model")
            add_required([rbr_root], f"Cars/{folder}/{stem}.ini", "vehicle_model")
            add_required([rbr_root], f"Cars/{folder}/{stem}_shaders.ini", "vehicle_model")
            for f in sorted(car_dir.iterdir()):
                if f.is_file() and f.suffix.lower() in (".ini", ".sgc"):
                    rel_f = f"Cars/{folder}/{f.name}"
                    if not any(dst.lower() == rel_f.lower() for _, dst, _ in copy_plan):
                        copy_plan.append((f, rel_f, "vehicle_model"))
            tex_dir = resolve_ci(car_dir, "textures")
            if tex_dir and tex_dir.is_dir():
                dds_files = sorted(f for f in tex_dir.iterdir() if f.is_file() and f.suffix.lower() == ".dds")
                if not dds_files:
                    missing.append(
                        {
                            "path": f"RBR/Cars/{folder}/textures/*.dds",
                            "category": "vehicle_textures",
                            "reason": "Sin texturas DDS",
                        }
                    )
                for dds in dds_files:
                    copy_plan.append((dds, f"Cars/{folder}/textures/{dds.name}", "vehicle_textures"))
            else:
                missing.append(
                    {
                        "path": f"RBR/Cars/{folder}/textures",
                        "category": "vehicle_textures",
                        "reason": "Carpeta textures ausente",
                    }
                )

        # Pack de fisicas NGP6 (directorio o .zip)
        pack_dir = resolve_ci(rbr_root, f"rsfdata/cars/{pack}")
        pack_zip = resolve_ci(rbr_root, f"rsfdata/cars/{pack}.zip")
        required_pack_files = [
            "common.lsp",
            "gravel.lsp",
            "tarmac.lsp",
            "snow.lsp",
            "r_gravel.lsp",
            "r_tarmac.lsp",
            "r_snow.lsp",
            "repair.ini",
            "setups/d_gravel.lsp",
            "setups/d_tarmac.lsp",
            "setups/d_snow.lsp",
        ]
        if pack_dir and pack_dir.is_dir():
            for req in required_pack_files:
                add_required([rbr_root], f"rsfdata/cars/{pack}/{req}", "vehicle_ngp_pack")
            for extra in sorted(pack_dir.rglob("*")):
                if extra.is_file():
                    rel_extra = f"rsfdata/cars/{pack}/{extra.relative_to(pack_dir).as_posix()}"
                    if not any(dst.lower() == rel_extra.lower() for _, dst, _ in copy_plan):
                        copy_plan.append((extra, rel_extra, "vehicle_ngp_pack"))
        elif pack_zip and pack_zip.is_file():
            copy_plan.append((pack_zip, f"rsfdata/cars/{pack}.zip", "vehicle_ngp_pack"))
            for bad_entry in check_zip_entries(pack_zip, required_pack_files):
                missing.append(
                    {"path": f"RBR/rsfdata/cars/{bad_entry}", "category": "vehicle_ngp_pack", "reason": "Falta en pack"}
                )
        else:
            missing.append(
                {
                    "path": f"RBR/rsfdata/cars/{pack}",
                    "category": "vehicle_ngp_pack",
                    "reason": "Pack de fisicas NGP6 ausente",
                }
            )

        # Metadatos ligeros RSF cache
        for cache_file in ("cars.json", "carmodels.json", "cargroups.json", "audiobank.ini"):
            cf = resolve_ci(rbr_root, f"rsfdata/cache/{cache_file}")
            if cf and cf.is_file():
                copy_plan.append((cf, f"rsfdata/cache/{cache_file}", "rsf_metadata"))

        # Banco FMOD Studio
        if bank:
            add_required([rbr_root], f"AudioFMOD/{bank}", "vehicle_fmod_audio")
            strings_stem = bank[: -len(".bank")] if bank.lower().endswith(".bank") else bank
            strings_bank = resolve_ci(rbr_root, f"AudioFMOD/{strings_stem}.strings.bank")
            master_strings = resolve_ci(rbr_root, "AudioFMOD/Master.strings.bank")
            if strings_bank and strings_bank.is_file():
                copy_plan.append(
                    (strings_bank, f"AudioFMOD/{strings_bank.name}", "vehicle_fmod_audio")
                )
            elif master_strings and master_strings.is_file():
                copy_plan.append(
                    (master_strings, "AudioFMOD/Master.strings.bank", "vehicle_fmod_audio")
                )
            else:
                missing.append(
                    {
                        "path": f"RBR/AudioFMOD/{strings_stem}.strings.bank",
                        "category": "vehicle_fmod_audio",
                        "reason": "Falta banco .strings.bank (o Master.strings.bank) requerido por FMOD",
                    }
                )
        else:
            warnings.append(f"Sin banco FMOD asignado para {rsf['name']}.")

    # 3. Tramo completo en RBR/Maps/
    stage_stem = str(stage["stem"])
    add_required([rbr_root], "Maps/TextureFilenameMap.dat", "stage_maps", "Maps/TextureFilenameMap.dat")
    for ext in ("lbs", "col", "trk", "mat", "dls", "fnc", "ini"):
        add_required([rbr_root], f"Maps/{stage_stem}.{ext}", "stage_maps", f"Maps/{stage_stem}.{ext}")
    add_required(
        [rbr_root],
        f"Maps/{stage_stem}_textures.rbz",
        "stage_textures",
        f"Maps/{stage_stem}_textures.rbz",
    )
    for sib in stage["sibling_texture_rbz"]:  # type: ignore[index]
        add_required([rbr_root], f"Maps/{sib}", "stage_shared_textures", f"Maps/{sib}")

    # Deduplicar manteniendo orden
    deduped_plan: list[tuple[Path, str, str]] = []
    seen_dest: set[str] = set()
    for src, dst, cat in copy_plan:
        key = dst.lower()
        if key not in seen_dest:
            seen_dest.add(key)
            deduped_plan.append((src, dst, cat))

    total_bytes = sum(src.stat().st_size for src, _, _ in deduped_plan)

    if missing:
        report_lines = [
            "=================================================================",
            "INFORME DE ARCHIVOS FALTANTES O INCOMPATIBLES (PAQUETE INCOMPLETO)",
            "=================================================================",
        ]
        for item in missing:
            report_lines.append(f"- [{item['category']}] {item['path']}: {item['reason']}")
        report_text = "\n".join(report_lines) + "\n"
        print(report_text, file=sys.stderr)
        if not args.dry_run:
            output_dir.mkdir(parents=True, exist_ok=True)
            (output_dir / "MISSING_FILES_REPORT.txt").write_text(report_text, encoding="utf-8")
        return 1

    # Capturar huella de origen (tamano + mtime_ns) para demostrar lectura 100% inmutable
    source_stat_before = {src: (src.stat().st_size, src.stat().st_mtime_ns) for src, _, _ in deduped_plan}

    if not args.dry_run:
        rbr_out = output_dir / "RBR"
        if rbr_out.exists():
            shutil.rmtree(rbr_out)
        rbr_out.mkdir(parents=True, exist_ok=True)

        manifest_files: list[dict[str, object]] = []
        for src, dst_rel, cat in deduped_plan:
            dst_path = rbr_out / Path(dst_rel)
            dst_path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dst_path)
            manifest_files.append(
                {
                    "relative_path": f"RBR/{dst_rel}",
                    "category": cat,
                    "bytes": dst_path.stat().st_size,
                    "sha256": sha256_file(dst_path),
                }
            )

        # Verificar que ningun archivo original fue modificado ni tocado
        for src, before in source_stat_before.items():
            after = (src.stat().st_size, src.stat().st_mtime_ns)
            if after != before:
                raise RuntimeError(f"Invariante de solo-lectura violado en {src}")

        manifest = {
            "package": "RBR-Mobile-Android-Minimal-OneCar-OneStage",
            "vehicle": vehicle_summary,
            "stage": {
                "id": args.stage,
                "name": stage["name"],
                "stem": stage["stem"],
                "original_map": stage["map"],
            },
            "ngp74_sha256": CERTIFIED_NGP74_SHA256,
            "file_count": len(manifest_files),
            "total_bytes": total_bytes,
            "total_mib": round(total_bytes / (1024 * 1024), 2),
            "warnings": warnings,
            "files": manifest_files,
        }
        manifest_path = output_dir / "MANIFEST.json"
        manifest_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

        zip_path: Path | None = None
        if args.zip:
            zip_path = output_dir.parent / f"{output_dir.name}.zip"
            with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
                for src, dst_rel, _ in deduped_plan:
                    dst_path = rbr_out / Path(dst_rel)
                    zf.write(dst_path, arcname=f"RBR/{dst_rel}")

        print(
            f"OK: Paquete minimo creado en {rbr_out}\n"
            f"  Vehiculo : {vehicle_summary.get('name', vehicle_summary.get('id'))}\n"
            f"  Tramo    : {stage['name']} ({stage['stem']})\n"
            f"  Archivos : {len(manifest_files)}\n"
            f"  Tamano   : {total_bytes:,} bytes ({total_bytes / (1024 * 1024):.2f} MiB)\n"
            f"  Manifest : {manifest_path}"
        )
        if zip_path:
            print(
                f"  ZIP      : {zip_path} ({zip_path.stat().st_size / (1024 * 1024):.2f} MiB)"
            )
    else:
        print(
            f"DRY-RUN OK: {len(deduped_plan)} archivos verificados, "
            f"{total_bytes:,} bytes ({total_bytes / (1024 * 1024):.2f} MiB)"
        )
    for w in warnings:
        print(f"AVISO: {w}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
