package org.rerbr.mobile;

import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.UriPermission;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Selector e importador selectivo de recursos RBR mediante Storage Access Framework (SAF).
 *
 * - Nunca asume que un content:// Uri sea una ruta POSIX del sistema de archivos.
 * - Conserva permisos de lectura persistentes (takePersistableUriPermission).
 * - Copia unicamente los archivos necesarios al almacenamiento privado de la app (files/RBR),
 *   evitando copiar los ~10,54 GB de una instalacion completa cuando hay decenas de coches/tramos.
 * - Permite fusion incremental (por ejemplo, importar carpeta RBR y despues carpeta NGP 7.4).
 */
final class SafResourceImporter {
    static final int STATE_IDLE = 0;
    static final int STATE_RUNNING = 1;
    static final int STATE_DONE = 2;
    static final int STATE_ERROR = 3;

    private static final String PREFS_NAME = "org.rerbr.mobile.saf";
    private static final String KEY_TREE_URI = "tree_uri";
    private static final String KEY_TREE_LABEL = "tree_label";
    private static final long MAX_ZIP_IMPORT_BYTES = 900L * 1024L * 1024L;

    interface Listener {
        void onStatus(int state, int filesDone, int filesTotal, long bytesDone, String detail);
        void onPersistedUri(boolean available, String label);
    }

    private static final class DocNode {
        final String docId;
        final String name;
        final String mime;
        final long size;

        DocNode(String docId, String name, String mime, long size) {
            this.docId = docId;
            this.name = name;
            this.mime = mime != null ? mime : "";
            this.size = size;
        }

        boolean isDir() {
            return DocumentsContract.Document.MIME_TYPE_DIR.equals(mime);
        }
    }

    private static final class CopyTask {
        final Uri uri;
        final String destRel;
        final long size;

        CopyTask(Uri uri, String destRel, long size) {
            this.uri = uri;
            this.destRel = destRel;
            this.size = size;
        }
    }

    private final Context context;
    private final Listener listener;
    private final AtomicBoolean running = new AtomicBoolean(false);

    SafResourceImporter(Context context, Listener listener) {
        this.context = context.getApplicationContext();
        this.listener = listener;
    }

    File targetRbrRoot() {
        File ext = context.getExternalFilesDir(null);
        File base = ext != null ? ext : context.getFilesDir();
        return new File(base, "RBR");
    }

    void syncPersistedState() {
        SharedPreferences prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE);
        String raw = prefs.getString(KEY_TREE_URI, null);
        String label = prefs.getString(KEY_TREE_LABEL, "");
        if (raw == null || raw.isEmpty()) {
            listener.onPersistedUri(false, "");
            return;
        }
        Uri uri = Uri.parse(raw);
        if (hasReadPermission(uri)) {
            listener.onPersistedUri(true, label != null && !label.isEmpty() ? label : describeUri(uri));
        } else {
            listener.onPersistedUri(false, "");
        }
    }

    void handleTreePicked(Uri treeUri, int intentFlags) {
        if (treeUri == null) {
            return;
        }
        boolean persisted = takePersistableRead(treeUri, intentFlags);
        String label = describeUri(treeUri);
        if (persisted) {
            context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                .edit()
                .putString(KEY_TREE_URI, treeUri.toString())
                .putString(KEY_TREE_LABEL, label)
                .apply();
            listener.onPersistedUri(true, label);
        }
        startTreeImport(treeUri, label);
    }

    void handleZipPicked(Uri docUri, int intentFlags) {
        if (docUri == null) {
            return;
        }
        takePersistableRead(docUri, intentFlags);
        String label = queryDisplayName(docUri);
        startZipImport(docUri, label);
    }

    void reimportPersistedTree() {
        SharedPreferences prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE);
        String raw = prefs.getString(KEY_TREE_URI, null);
        String label = prefs.getString(KEY_TREE_LABEL, "Carpeta RBR");
        if (raw == null || raw.isEmpty()) {
            listener.onStatus(STATE_ERROR, 0, 0, 0L, "SIN CARPETA AUTORIZADA — PULSA SELECCIONAR CARPETA RBR");
            listener.onPersistedUri(false, "");
            return;
        }
        Uri treeUri = Uri.parse(raw);
        if (!hasReadPermission(treeUri)) {
            listener.onPersistedUri(false, "");
            listener.onStatus(STATE_ERROR, 0, 0, 0L, "PERMISO SAF CADUCADO — VUELVE A SELECCIONAR LA CARPETA RBR");
            return;
        }
        startTreeImport(treeUri, label != null ? label : describeUri(treeUri));
    }

    private boolean takePersistableRead(Uri uri, int intentFlags) {
        int flags = intentFlags & Intent.FLAG_GRANT_READ_URI_PERMISSION;
        if (flags == 0) {
            flags = Intent.FLAG_GRANT_READ_URI_PERMISSION;
        }
        try {
            context.getContentResolver().takePersistableUriPermission(uri, flags);
            return true;
        } catch (SecurityException | IllegalArgumentException ignored) {
            return false;
        }
    }

    private boolean hasReadPermission(Uri uri) {
        List<UriPermission> perms = context.getContentResolver().getPersistedUriPermissions();
        for (UriPermission p : perms) {
            if (p.isReadPermission() && p.getUri().equals(uri)) {
                return true;
            }
        }
        return false;
    }

    private String describeUri(Uri uri) {
        try {
            String docId = DocumentsContract.getTreeDocumentId(uri);
            if (docId != null && !docId.isEmpty()) {
                return docId;
            }
        } catch (IllegalArgumentException ignored) {
        }
        return uri.getLastPathSegment() != null ? uri.getLastPathSegment() : uri.toString();
    }

    private String queryDisplayName(Uri docUri) {
        try (Cursor c = context.getContentResolver().query(
                docUri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst() && !c.isNull(0)) {
                return c.getString(0);
            }
        } catch (RuntimeException ignored) {
        }
        return describeUri(docUri);
    }

    private void startTreeImport(Uri treeUri, String label) {
        if (!running.compareAndSet(false, true)) {
            return;
        }
        Thread worker = new Thread(() -> {
            try {
                runTreeImport(treeUri, label);
            } catch (Exception e) {
                String msg = e.getMessage() != null ? e.getMessage() : e.getClass().getSimpleName();
                listener.onStatus(STATE_ERROR, 0, 0, 0L, "ERROR SAF: " + msg);
            } finally {
                running.set(false);
            }
        }, "rerbr-saf-import");
        worker.setDaemon(true);
        worker.start();
    }

    private void startZipImport(Uri zipUri, String label) {
        if (!running.compareAndSet(false, true)) {
            return;
        }
        Thread worker = new Thread(() -> {
            try {
                runZipImport(zipUri, label);
            } catch (Exception e) {
                String msg = e.getMessage() != null ? e.getMessage() : e.getClass().getSimpleName();
                listener.onStatus(STATE_ERROR, 0, 0, 0L, "ERROR ZIP SAF: " + msg);
            } finally {
                running.set(false);
            }
        }, "rerbr-saf-zip");
        worker.setDaemon(true);
        worker.start();
    }

    private void runTreeImport(Uri treeUri, String label) throws IOException {
        ContentResolver resolver = context.getContentResolver();
        listener.onStatus(STATE_RUNNING, 0, 0, 0L, "ANALIZANDO ÁRBOL SAF: " + label);

        String treeRootDocId = DocumentsContract.getTreeDocumentId(treeUri);
        String rbrDocId = locateLogicalRbrRoot(resolver, treeUri, treeRootDocId);
        if (rbrDocId == null) {
            listener.onStatus(
                STATE_ERROR,
                0,
                0,
                0L,
                "LA CARPETA NO CONTIENE RECURSOS RBR RECONOCIBLES (Cars/, Maps/, Plugins/NGP.dll, physics.rbz)"
            );
            return;
        }

        List<DocNode> rootChildren = listChildren(resolver, treeUri, rbrDocId);
        List<CopyTask> plan = new ArrayList<>();
        Set<String> plannedLower = new HashSet<>();

        // 1. Archivos base compartidos y NGP 7.4
        for (String topFile : new String[] {"physics.rbz", "misc.rbz", "Scripts.rbz", "season.dat"}) {
            DocNode n = findIgnoreCase(rootChildren, topFile);
            if (n != null && !n.isDir()) {
                addTask(plan, plannedLower, treeUri, n, topFile);
            }
        }
        DocNode rootNgp = findIgnoreCase(rootChildren, "NGP.dll");
        if (rootNgp != null && !rootNgp.isDir()) {
            addTask(plan, plannedLower, treeUri, rootNgp, "Plugins/NGP.dll");
        }
        DocNode pluginsDir = findDirIgnoreCase(rootChildren, "Plugins");
        if (pluginsDir != null) {
            DocNode ngp = findIgnoreCase(listChildren(resolver, treeUri, pluginsDir.docId), "NGP.dll");
            if (ngp != null && !ngp.isDir()) {
                addTask(plan, plannedLower, treeUri, ngp, "Plugins/NGP.dll");
            }
        }

        // Espectadores (.sgs)
        DocNode sgsDir = findDirIgnoreCase(rootChildren, "Sgs");
        if (sgsDir != null) {
            for (DocNode s : listChildren(resolver, treeUri, sgsDir.docId)) {
                String low = s.name.toLowerCase(Locale.ROOT);
                if (!s.isDir() && low.contains("spectator_") && low.endsWith(".sgs")) {
                    addTask(plan, plannedLower, treeUri, s, "Sgs/" + s.name);
                }
            }
        }

        // Atlas de iconos visuales de copiloto (ligero: ~660 KB en lugar de todo Generic/)
        DocNode genericDir = findDirIgnoreCase(rootChildren, "Generic");
        if (genericDir != null) {
            List<DocNode> genKids = listChildren(resolver, treeUri, genericDir.docId);
            for (String res : new String[] {"1280", "1024", "800", "640"}) {
                DocNode bnk = findIgnoreCase(genKids, "PC_Generic_" + res + ".bnk");
                DocNode p01 = findIgnoreCase(genKids, "PC_" + res + "_01.dds");
                DocNode p05 = findIgnoreCase(genKids, "PC_" + res + "_05.dds");
                if (bnk != null && p01 != null && p05 != null) {
                    addTask(plan, plannedLower, treeUri, bnk, "Generic/" + bnk.name);
                    addTask(plan, plannedLower, treeUri, p01, "Generic/PC_" + res + "_01.dds");
                    addTask(plan, plannedLower, treeUri, p05, "Generic/PC_" + res + "_05.dds");
                    break;
                }
            }
        }

        // 2. Seleccion inteligente de tramo (evita copiar >6 GB de Maps si es instalacion completa)
        DocNode mapsDir = findDirIgnoreCase(rootChildren, "Maps");
        if (mapsDir == null) {
            DocNode harwoodDir = findDirIgnoreCase(rootChildren, "Harwood");
            if (harwoodDir != null) {
                mapsDir = findDirIgnoreCase(listChildren(resolver, treeUri, harwoodDir.docId), "Maps");
            }
        }
        if (mapsDir != null) {
            planStageFiles(resolver, treeUri, mapsDir, plan, plannedLower);
        }

        // 3. Seleccion inteligente de vehiculo + fisicas NGP6 + audio FMOD / .eng
        planVehicleFiles(resolver, treeUri, rootChildren, plan, plannedLower);

        if (plan.isEmpty()) {
            listener.onStatus(
                STATE_ERROR,
                0,
                0,
                0L,
                "NO SE ENCONTRARON ARCHIVOS ÚTILES DE RBR EN LA CARPETA SELECCIONADA"
            );
            return;
        }

        File destRoot = targetRbrRoot();
        if (!destRoot.exists() && !destRoot.mkdirs()) {
            throw new IOException("No se pudo crear " + destRoot.getAbsolutePath());
        }

        long bytesCopied = 0L;
        int skippedUpToDate = 0;
        int total = plan.size();
        byte[] buffer = new byte[128 * 1024];

        for (int i = 0; i < total; i++) {
            CopyTask task = plan.get(i);
            File dst = safeResolveDest(destRoot, task.destRel);
            if (dst == null) {
                continue;
            }
            boolean smallConfig = task.destRel.endsWith(".ini") || task.destRel.endsWith(".json") || task.destRel.endsWith(".lsp");
            if (!smallConfig && dst.isFile() && task.size > 0 && dst.length() == task.size) {
                bytesCopied += task.size;
                skippedUpToDate++;
                listener.onStatus(STATE_RUNNING, i + 1, total, bytesCopied, "VERIFICADO: " + task.destRel);
                continue;
            }
            listener.onStatus(STATE_RUNNING, i, total, bytesCopied, "COPIANDO: " + task.destRel);
            bytesCopied += copyUriAtomically(resolver, task.uri, dst, buffer);
            listener.onStatus(STATE_RUNNING, i + 1, total, bytesCopied, "COPIADO: " + task.destRel);
        }

        String summary = String.format(
            Locale.US,
            "IMPORTACIÓN SAF COMPLETADA: %d ARCHIVOS (%.1f MiB, %d REUTILIZADOS)",
            total,
            bytesCopied / (1024.0 * 1024.0),
            skippedUpToDate
        );
        listener.onStatus(STATE_DONE, total, total, bytesCopied, summary);
    }

    private void planStageFiles(
            ContentResolver resolver,
            Uri treeUri,
            DocNode mapsDir,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        List<DocNode> kids = listChildren(resolver, treeUri, mapsDir.docId);
        DocNode texMap = findIgnoreCase(kids, "TextureFilenameMap.dat");
        if (texMap != null && !texMap.isDir()) {
            addTask(plan, plannedLower, treeUri, texMap, "Maps/TextureFilenameMap.dat");
        }

        List<String> lbsStems = new ArrayList<>();
        for (DocNode k : kids) {
            if (!k.isDir() && k.name.toLowerCase(Locale.ROOT).endsWith(".lbs")) {
                lbsStems.add(k.name.substring(0, k.name.length() - 4));
            }
        }
        if (lbsStems.isEmpty()) {
            return;
        }

        if (lbsStems.size() <= 2) {
            for (DocNode k : kids) {
                if (!k.isDir() && isStageAssetName(k.name)) {
                    addTask(plan, plannedLower, treeUri, k, "Maps/" + k.name);
                }
            }
            return;
        }

        // Instalacion grande: elegir 1 tramo validado completo (evita copiar gigabytes de Maps/)
        String chosenStem = null;
        String[] preferred = new String[] {"track-20_O", "track-22_O", "track-41_N", "track-46_O"};
        for (String cand : preferred) {
            if (hasCompleteStage(kids, cand)) {
                chosenStem = cand;
                break;
            }
        }
        if (chosenStem == null) {
            for (String cand : lbsStems) {
                if (hasCompleteStage(kids, cand)) {
                    chosenStem = cand;
                    break;
                }
            }
        }
        if (chosenStem == null) {
            chosenStem = lbsStems.get(0);
        }

        for (String ext : new String[] {".lbs", ".col", ".trk", ".mat", ".dls", ".fnc", ".ini", "_textures.rbz"}) {
            DocNode f = findIgnoreCase(kids, chosenStem + ext);
            if (f != null && !f.isDir()) {
                addTask(plan, plannedLower, treeUri, f, "Maps/" + f.name);
            }
        }
        // Incluir archivos *_textures.rbz hermanos del mismo mapa (ej. track-20_N_textures.rbz para track-20_O)
        int underscore = chosenStem.lastIndexOf('_');
        if (underscore > 0) {
            String mapPrefix = (chosenStem.substring(0, underscore) + "_").toLowerCase(Locale.ROOT);
            for (DocNode k : kids) {
                String low = k.name.toLowerCase(Locale.ROOT);
                if (!k.isDir() && low.startsWith(mapPrefix) && low.endsWith("_textures.rbz")) {
                    addTask(plan, plannedLower, treeUri, k, "Maps/" + k.name);
                }
            }
        }
    }

    private boolean hasCompleteStage(List<DocNode> kids, String stem) {
        for (String ext : new String[] {".lbs", ".col", ".trk", ".mat", ".dls", ".fnc", ".ini", "_textures.rbz"}) {
            DocNode f = findIgnoreCase(kids, stem + ext);
            if (f == null || f.isDir()) {
                return false;
            }
        }
        return true;
    }

    private boolean isStageAssetName(String name) {
        String low = name.toLowerCase(Locale.ROOT);
        return low.equals("texturefilenamemap.dat")
            || low.endsWith(".lbs")
            || low.endsWith(".col")
            || low.endsWith(".trk")
            || low.endsWith(".mat")
            || low.endsWith(".dls")
            || low.endsWith(".fnc")
            || low.endsWith(".ini")
            || low.endsWith("_textures.rbz");
    }

    private void planVehicleFiles(
            ContentResolver resolver,
            Uri treeUri,
            List<DocNode> rootChildren,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        DocNode carsDir = findDirIgnoreCase(rootChildren, "Cars");
        DocNode rsfDataDir = findDirIgnoreCase(rootChildren, "rsfdata");
        DocNode audioFmodDir = findDirIgnoreCase(rootChildren, "AudioFMOD");
        DocNode audioDir = findDirIgnoreCase(rootChildren, "Audio");

        List<DocNode> carsKids = carsDir != null ? listChildren(resolver, treeUri, carsDir.docId) : new ArrayList<>();
        DocNode carsIni = findIgnoreCase(carsKids, "Cars.ini");
        if (carsIni != null && !carsIni.isDir()) {
            addTask(plan, plannedLower, treeUri, carsIni, "Cars/Cars.ini");
        }

        List<DocNode> carFolders = new ArrayList<>();
        for (DocNode c : carsKids) {
            if (c.isDir()) {
                carFolders.add(c);
            }
        }

        DocNode rsfCarsDir = null;
        DocNode rsfCacheDir = null;
        List<DocNode> rsfCacheKids = new ArrayList<>();
        if (rsfDataDir != null) {
            List<DocNode> rsfKids = listChildren(resolver, treeUri, rsfDataDir.docId);
            rsfCarsDir = findDirIgnoreCase(rsfKids, "cars");
            rsfCacheDir = findDirIgnoreCase(rsfKids, "cache");
            if (rsfCacheDir != null) {
                rsfCacheKids = listChildren(resolver, treeUri, rsfCacheDir.docId);
                for (String cf : new String[] {"cars.json", "carmodels.json", "cargroups.json", "audiobank.ini"}) {
                    DocNode node = findIgnoreCase(rsfCacheKids, cf);
                    if (node != null && !node.isDir()) {
                        addTask(plan, plannedLower, treeUri, node, "rsfdata/cache/" + cf);
                    }
                }
            }
        }

        List<DocNode> rsfPackKids = rsfCarsDir != null ? listChildren(resolver, treeUri, rsfCarsDir.docId) : new ArrayList<>();
        List<DocNode> fmodKids = audioFmodDir != null ? listChildren(resolver, treeUri, audioFmodDir.docId) : new ArrayList<>();

        DocNode audioCarsDir = null;
        List<DocNode> audioCarsKids = new ArrayList<>();
        if (audioDir != null) {
            audioCarsDir = findDirIgnoreCase(listChildren(resolver, treeUri, audioDir.docId), "Cars");
            if (audioCarsDir != null) {
                audioCarsKids = listChildren(resolver, treeUri, audioCarsDir.docId);
                DocNode acIni = findIgnoreCase(audioCarsKids, "Cars.ini");
                if (acIni != null && !acIni.isDir()) {
                    addTask(plan, plannedLower, treeUri, acIni, "Audio/Cars/Cars.ini");
                }
            }
        }

        // Paquete minimo (<= 2 carpetas de coches): copiar todos los coches presentes en el paquete
        if (carFolders.size() <= 2) {
            for (DocNode folder : carFolders) {
                addCarFolderTasks(resolver, treeUri, folder, plan, plannedLower);
            }
            if (rsfPackKids.size() <= 4) {
                for (DocNode pk : rsfPackKids) {
                    addRsfPackTasks(resolver, treeUri, pk, plan, plannedLower);
                }
            }
            if (fmodKids.size() <= 8) {
                for (DocNode bk : fmodKids) {
                    if (!bk.isDir() && bk.name.toLowerCase(Locale.ROOT).endsWith(".bank")) {
                        addTask(plan, plannedLower, treeUri, bk, "AudioFMOD/" + bk.name);
                    }
                }
            }
            if (audioCarsKids.size() <= 8) {
                for (DocNode ek : audioCarsKids) {
                    if (!ek.isDir() && ek.name.toLowerCase(Locale.ROOT).endsWith(".eng")) {
                        addTask(plan, plannedLower, treeUri, ek, "Audio/Cars/" + ek.name);
                    }
                }
            }
            return;
        }

        // Instalacion grande (>2 coches): elegir 1 coche conducible completo sin copiar gigabytes de Cars/
        DocNode focusFolder = findDirIgnoreCase(carFolders, "FORD_FOCUS_2006");
        DocNode focusPack = findPackIgnoreCase(rsfPackKids, "Ford_Focus_Mk_II_RS_WRC_2006_ngp6");
        DocNode focusBank = findIgnoreCase(fmodKids, "Focus_Mk2_WRC.bank");
        if (focusFolder != null && focusPack != null) {
            addCarFolderTasks(resolver, treeUri, focusFolder, plan, plannedLower);
            addRsfPackTasks(resolver, treeUri, focusPack, plan, plannedLower);
            addFmodBankAndStrings(treeUri, fmodKids, focusBank != null ? focusBank.name : null, plan, plannedLower);
            return;
        }

        // Buscar cualquier coche RSF NGP6 completo usando rsfdata/cache/cars.json + carmodels.json + audiobank.ini
        if (tryPlanFirstCompleteRsfCar(
                resolver, treeUri, rsfCacheKids, carFolders, rsfPackKids, fmodKids, plan, plannedLower)) {
            return;
        }

        // Fallback a coche original retail 1.02
        String[][] retailCandidates = new String[][] {
            {"impreza03", "subaru.eng"},
            {"TOYOTA_COROLLA", "corolla.eng"},
            {"CITROEN_XSARA", "xsara.eng"},
            {"PEUGEOT_206", "206.eng"},
            {"MITSUBISHI_LANCER", "lancer.eng"},
            {"MG_ZR", "mg_zr.eng"},
            {"HYUNDAI_ACCENT", "accent.eng"},
            {"SUBARU_IMPREZA_2000", "impreza2000.eng"},
        };
        for (String[] rc : retailCandidates) {
            DocNode cf = findDirIgnoreCase(carFolders, rc[0]);
            if (cf != null) {
                addCarFolderTasks(resolver, treeUri, cf, plan, plannedLower);
                DocNode eng = findIgnoreCase(audioCarsKids, rc[1]);
                if (eng != null && !eng.isDir()) {
                    addTask(plan, plannedLower, treeUri, eng, "Audio/Cars/" + eng.name);
                }
                return;
            }
        }
    }

    private boolean tryPlanFirstCompleteRsfCar(
            ContentResolver resolver,
            Uri treeUri,
            List<DocNode> cacheKids,
            List<DocNode> carFolders,
            List<DocNode> rsfPackKids,
            List<DocNode> fmodKids,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        DocNode carsJsonNode = findIgnoreCase(cacheKids, "cars.json");
        DocNode modelsJsonNode = findIgnoreCase(cacheKids, "carmodels.json");
        if (carsJsonNode == null || modelsJsonNode == null) {
            return false;
        }
        try {
            String carsText = readSmallUtf8(resolver, DocumentsContract.buildDocumentUriUsingTree(treeUri, carsJsonNode.docId));
            String modelsText = readSmallUtf8(resolver, DocumentsContract.buildDocumentUriUsingTree(treeUri, modelsJsonNode.docId));
            Map<String, String> audioMap = new HashMap<>();
            DocNode audioIniNode = findIgnoreCase(cacheKids, "audiobank.ini");
            if (audioIniNode != null) {
                String iniText = readSmallUtf8(resolver, DocumentsContract.buildDocumentUriUsingTree(treeUri, audioIniNode.docId));
                for (String line : iniText.split("\n")) {
                    int eq = line.indexOf('=');
                    if (eq > 0 && !line.trim().startsWith(";") && !line.trim().startsWith("[")) {
                        audioMap.put(line.substring(0, eq).trim().toLowerCase(Locale.ROOT), line.substring(eq + 1).trim());
                    }
                }
            }
            JSONArray modelsArr = new JSONArray(modelsText);
            Map<String, String> modelFolderById = new HashMap<>();
            for (int i = 0; i < modelsArr.length(); i++) {
                JSONObject m = modelsArr.optJSONObject(i);
                if (m != null) {
                    String path = m.optString("path", "").replace('\\', '/');
                    int slash = path.lastIndexOf('/');
                    String folder = slash >= 0 ? path.substring(slash + 1) : path;
                    modelFolderById.put(String.valueOf(m.opt("id")), folder);
                }
            }
            JSONArray carsArr = new JSONArray(carsText);
            for (int i = 0; i < carsArr.length(); i++) {
                JSONObject c = carsArr.optJSONObject(i);
                if (c == null) {
                    continue;
                }
                String packName = c.optString("physics_Revision", c.optString("path", "")).trim();
                String folderName = modelFolderById.get(String.valueOf(c.opt("carmodel_id")));
                if (packName.isEmpty() || folderName == null || folderName.isEmpty()) {
                    continue;
                }
                DocNode cf = findDirIgnoreCase(carFolders, folderName);
                DocNode pk = findPackIgnoreCase(rsfPackKids, packName);
                if (cf == null || pk == null) {
                    continue;
                }
                String bankRaw = audioMap.get(packName.toLowerCase(Locale.ROOT));
                if (bankRaw != null && !bankRaw.isEmpty()) {
                    String bankName = bankRaw.toLowerCase(Locale.ROOT).endsWith(".bank") ? bankRaw : bankRaw + ".bank";
                    if (findIgnoreCase(fmodKids, bankName) == null) {
                        continue;
                    }
                    addCarFolderTasks(resolver, treeUri, cf, plan, plannedLower);
                    addRsfPackTasks(resolver, treeUri, pk, plan, plannedLower);
                    addFmodBankAndStrings(treeUri, fmodKids, bankName, plan, plannedLower);
                    return true;
                }
            }
        } catch (Exception ignored) {
        }
        return false;
    }

    private void addCarFolderTasks(
            ContentResolver resolver,
            Uri treeUri,
            DocNode carFolder,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        List<DocNode> kids = listChildren(resolver, treeUri, carFolder.docId);
        for (DocNode k : kids) {
            String low = k.name.toLowerCase(Locale.ROOT);
            if (!k.isDir() && (low.endsWith(".sgc") || low.endsWith(".ini"))) {
                addTask(plan, plannedLower, treeUri, k, "Cars/" + carFolder.name + "/" + k.name);
            } else if (k.isDir() && low.equals("textures")) {
                for (DocNode t : listChildren(resolver, treeUri, k.docId)) {
                    if (!t.isDir() && t.name.toLowerCase(Locale.ROOT).endsWith(".dds")) {
                        addTask(plan, plannedLower, treeUri, t, "Cars/" + carFolder.name + "/textures/" + t.name);
                    }
                }
            }
        }
    }

    private void addRsfPackTasks(
            ContentResolver resolver,
            Uri treeUri,
            DocNode packNode,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        if (!packNode.isDir()) {
            addTask(plan, plannedLower, treeUri, packNode, "rsfdata/cars/" + packNode.name);
            return;
        }
        addRecursiveDir(resolver, treeUri, packNode.docId, "rsfdata/cars/" + packNode.name, 3, plan, plannedLower);
    }

    private void addRecursiveDir(
            ContentResolver resolver,
            Uri treeUri,
            String dirDocId,
            String relPrefix,
            int maxDepth,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        if (maxDepth < 0) {
            return;
        }
        for (DocNode k : listChildren(resolver, treeUri, dirDocId)) {
            if (k.isDir()) {
                addRecursiveDir(resolver, treeUri, k.docId, relPrefix + "/" + k.name, maxDepth - 1, plan, plannedLower);
            } else {
                addTask(plan, plannedLower, treeUri, k, relPrefix + "/" + k.name);
            }
        }
    }

    private void addFmodBankAndStrings(
            Uri treeUri,
            List<DocNode> fmodKids,
            String bankName,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        if (bankName == null || bankName.isEmpty()) {
            return;
        }
        DocNode bank = findIgnoreCase(fmodKids, bankName);
        if (bank != null && !bank.isDir()) {
            addTask(plan, plannedLower, treeUri, bank, "AudioFMOD/" + bank.name);
        }
        String stem = bankName.toLowerCase(Locale.ROOT).endsWith(".bank")
            ? bankName.substring(0, bankName.length() - 5)
            : bankName;
        DocNode strings = findIgnoreCase(fmodKids, stem + ".strings.bank");
        if (strings != null && !strings.isDir()) {
            addTask(plan, plannedLower, treeUri, strings, "AudioFMOD/" + strings.name);
        } else {
            DocNode master = findIgnoreCase(fmodKids, "Master.strings.bank");
            if (master != null && !master.isDir()) {
                addTask(plan, plannedLower, treeUri, master, "AudioFMOD/" + master.name);
            }
        }
    }

    private DocNode findPackIgnoreCase(List<DocNode> packKids, String packName) {
        DocNode dir = findDirIgnoreCase(packKids, packName);
        if (dir != null) {
            return dir;
        }
        return findIgnoreCase(packKids, packName + ".zip");
    }

    private String locateLogicalRbrRoot(ContentResolver resolver, Uri treeUri, String rootDocId) {
        List<DocNode> top = listChildren(resolver, treeUri, rootDocId);
        if (looksLikeRbrContents(top)) {
            return rootDocId;
        }
        DocNode childRbr = findDirIgnoreCase(top, "RBR");
        if (childRbr != null && looksLikeRbrContents(listChildren(resolver, treeUri, childRbr.docId))) {
            return childRbr.docId;
        }
        for (DocNode sub : top) {
            if (sub.isDir()) {
                List<DocNode> subKids = listChildren(resolver, treeUri, sub.docId);
                if (looksLikeRbrContents(subKids)) {
                    return sub.docId;
                }
                DocNode nestedRbr = findDirIgnoreCase(subKids, "RBR");
                if (nestedRbr != null && looksLikeRbrContents(listChildren(resolver, treeUri, nestedRbr.docId))) {
                    return nestedRbr.docId;
                }
            }
        }
        return null;
    }

    private boolean looksLikeRbrContents(List<DocNode> children) {
        for (DocNode c : children) {
            String low = c.name.toLowerCase(Locale.ROOT);
            if (low.equals("physics.rbz")
                    || low.equals("misc.rbz")
                    || low.equals("scripts.rbz")
                    || low.equals("ngp.dll")
                    || (c.isDir() && (low.equals("cars") || low.equals("maps") || low.equals("plugins") || low.equals("rsfdata")))) {
                return true;
            }
        }
        return false;
    }

    private List<DocNode> listChildren(ContentResolver resolver, Uri treeUri, String parentDocId) {
        List<DocNode> out = new ArrayList<>();
        Uri childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId);
        String[] projection = new String[] {
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_SIZE
        };
        try (Cursor c = resolver.query(childrenUri, projection, null, null, null)) {
            if (c == null) {
                return out;
            }
            while (c.moveToNext()) {
                String id = c.getString(0);
                String name = c.getString(1);
                String mime = c.getString(2);
                long size = c.isNull(3) ? -1L : c.getLong(3);
                if (id != null && name != null && !name.isEmpty()) {
                    out.add(new DocNode(id, name, mime, size));
                }
            }
        } catch (RuntimeException ignored) {
        }
        return out;
    }

    private DocNode findIgnoreCase(List<DocNode> list, String target) {
        for (DocNode n : list) {
            if (n.name.equalsIgnoreCase(target)) {
                return n;
            }
        }
        return null;
    }

    private DocNode findDirIgnoreCase(List<DocNode> list, String target) {
        for (DocNode n : list) {
            if (n.isDir() && n.name.equalsIgnoreCase(target)) {
                return n;
            }
        }
        return null;
    }

    private void addTask(
            List<CopyTask> plan,
            Set<String> plannedLower,
            Uri treeUri,
            DocNode node,
            String destRel) {
        String key = destRel.toLowerCase(Locale.ROOT);
        if (plannedLower.add(key)) {
            Uri docUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, node.docId);
            plan.add(new CopyTask(docUri, destRel, node.size));
        }
    }

    private File safeResolveDest(File destRoot, String relPath) throws IOException {
        String normalized = relPath.replace('\\', '/').trim();
        while (normalized.startsWith("/")) {
            normalized = normalized.substring(1);
        }
        if (normalized.isEmpty() || normalized.contains("..")) {
            return null;
        }
        File target = new File(destRoot, normalized);
        String rootCanon = destRoot.getCanonicalPath();
        String targetCanon = target.getCanonicalPath();
        if (!targetCanon.startsWith(rootCanon + File.separator) && !targetCanon.equals(rootCanon)) {
            return null;
        }
        return target;
    }

    private long copyUriAtomically(ContentResolver resolver, Uri srcUri, File dst, byte[] buffer) throws IOException {
        File parent = dst.getParentFile();
        if (parent != null && !parent.exists() && !parent.mkdirs()) {
            throw new IOException("No se pudo crear carpeta " + parent.getName());
        }
        File part = new File(dst.getParentFile(), dst.getName() + ".part");
        long total = 0L;
        try (InputStream in = resolver.openInputStream(srcUri)) {
            if (in == null) {
                throw new IOException("ContentResolver devolvió stream nulo para " + dst.getName());
            }
            try (FileOutputStream out = new FileOutputStream(part)) {
                int read;
                while ((read = in.read(buffer)) != -1) {
                    out.write(buffer, 0, read);
                    total += read;
                }
                out.getFD().sync();
            }
        } catch (IOException e) {
            part.delete();
            throw e;
        }
        if (dst.exists() && !dst.delete()) {
            part.delete();
            throw new IOException("No se pudo actualizar " + dst.getName());
        }
        if (!part.renameTo(dst)) {
            part.delete();
            throw new IOException("Fallo al confirmar archivo " + dst.getName());
        }
        return total;
    }

    private void runZipImport(Uri zipUri, String label) throws IOException {
        ContentResolver resolver = context.getContentResolver();
        File destRoot = targetRbrRoot();
        if (!destRoot.exists() && !destRoot.mkdirs()) {
            throw new IOException("No se pudo crear " + destRoot.getAbsolutePath());
        }
        listener.onStatus(STATE_RUNNING, 0, 0, 0L, "EXTRAYENDO PAQUETE ZIP SAF: " + label);

        byte[] buffer = new byte[128 * 1024];
        int filesDone = 0;
        long bytesDone = 0L;

        try (InputStream raw = resolver.openInputStream(zipUri)) {
            if (raw == null) {
                throw new IOException("No se pudo abrir el archivo ZIP seleccionado");
            }
            try (ZipInputStream zis = new ZipInputStream(raw)) {
                ZipEntry entry;
                while ((entry = zis.getNextEntry()) != null) {
                    if (entry.isDirectory()) {
                        continue;
                    }
                    String rel = normalizeZipEntryToRbrRel(entry.getName());
                    if (rel == null) {
                        continue;
                    }
                    File dst = safeResolveDest(destRoot, rel);
                    if (dst == null) {
                        continue;
                    }
                    File parent = dst.getParentFile();
                    if (parent != null && !parent.exists() && !parent.mkdirs()) {
                        throw new IOException("No se pudo crear " + parent.getName());
                    }
                    File part = new File(dst.getParentFile(), dst.getName() + ".part");
                    try (FileOutputStream out = new FileOutputStream(part)) {
                        int read;
                        while ((read = zis.read(buffer)) != -1) {
                            bytesDone += read;
                            if (bytesDone > MAX_ZIP_IMPORT_BYTES) {
                                throw new IOException("El ZIP supera el límite de paquete mínimo (900 MiB); usa SELECCIONAR CARPETA RBR");
                            }
                            out.write(buffer, 0, read);
                        }
                        out.getFD().sync();
                    } catch (IOException e) {
                        part.delete();
                        throw e;
                    }
                    if (dst.exists()) {
                        dst.delete();
                    }
                    if (!part.renameTo(dst)) {
                        part.delete();
                        throw new IOException("No se pudo guardar " + dst.getName());
                    }
                    filesDone++;
                    listener.onStatus(STATE_RUNNING, filesDone, 0, bytesDone, "EXTRAÍDO: " + rel);
                }
            }
        }

        if (filesDone == 0) {
            listener.onStatus(
                STATE_ERROR,
                0,
                0,
                0L,
                "EL ZIP NO CONTIENE ARCHIVOS RBR VÁLIDOS (ESPERADO RBR/Cars, RBR/Maps, RBR/Plugins)"
            );
            return;
        }
        String summary = String.format(
            Locale.US,
            "PAQUETE ZIP IMPORTADO: %d ARCHIVOS (%.1f MiB)",
            filesDone,
            bytesDone / (1024.0 * 1024.0)
        );
        listener.onStatus(STATE_DONE, filesDone, filesDone, bytesDone, summary);
    }

    private String normalizeZipEntryToRbrRel(String rawName) {
        String clean = rawName.replace('\\', '/').trim();
        while (clean.startsWith("/")) {
            clean = clean.substring(1);
        }
        if (clean.isEmpty() || clean.contains("..")) {
            return null;
        }
        String lower = clean.toLowerCase(Locale.ROOT);
        if (lower.startsWith("rbr/")) {
            clean = clean.substring(4);
        } else {
            int idx = lower.indexOf("/rbr/");
            if (idx >= 0) {
                clean = clean.substring(idx + 5);
            }
        }
        String cl = clean.toLowerCase(Locale.ROOT);
        if (cl.equals("ngp.dll")) {
            return "Plugins/NGP.dll";
        }
        if (cl.equals("physics.rbz")
                || cl.equals("misc.rbz")
                || cl.equals("scripts.rbz")
                || cl.equals("season.dat")
                || cl.startsWith("plugins/")
                || cl.startsWith("cars/")
                || cl.startsWith("maps/")
                || cl.startsWith("rsfdata/")
                || cl.startsWith("audiofmod/")
                || cl.startsWith("audio/")
                || cl.startsWith("sgs/")
                || cl.startsWith("generic/")) {
            return clean;
        }
        return null;
    }

    private String readSmallUtf8(ContentResolver resolver, Uri uri) throws IOException {
        try (InputStream in = resolver.openInputStream(uri)) {
            if (in == null) {
                return "";
            }
            ByteArrayOutputStream bos = new ByteArrayOutputStream();
            byte[] buf = new byte[16 * 1024];
            int n;
            int total = 0;
            while ((n = in.read(buf)) != -1) {
                total += n;
                if (total > 2 * 1024 * 1024) {
                    break;
                }
                bos.write(buf, 0, n);
            }
            return bos.toString(StandardCharsets.UTF_8.name());
        }
    }
}
