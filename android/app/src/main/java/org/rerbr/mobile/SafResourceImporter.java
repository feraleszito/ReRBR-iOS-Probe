package org.rerbr.mobile;

import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.UriPermission;
import android.database.Cursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Adaptador Android Storage Access Framework (SAF) sobre la capa universal de importación en Rust.
 *
 * - Nunca asume que un content:// Uri sea una ruta POSIX convencional.
 * - Conserva permisos persistentes de lectura (takePersistableUriPermission).
 * - Los archivos .zip se delegan al extractor Rust compartido (`nativeImportZipFd`) mediante
 *   descriptor de archivo (`ParcelFileDescriptor`), con respaldo por stream si el proveedor es remoto.
 * - Las reglas de rutas permitidas (`nativeNormalizeRbrRel`) y la protección de NGP 7.4.778
 *   (`nativeCheckPreserveCoreFile`) viven íntegramente en Rust para paridad 1:1 con iOS.
 */
final class SafResourceImporter {
    static final int STATE_IDLE = 0;
    static final int STATE_RUNNING = 1;
    static final int STATE_DONE = 2;
    static final int STATE_ERROR = 3;

    private static final String PREFS_NAME = "org.rerbr.mobile.saf";
    private static final String KEY_TREE_URI = "tree_uri";
    private static final String KEY_TREE_LABEL = "tree_label";

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

    File targetDocumentsRoot() {
        File ext = context.getExternalFilesDir(null);
        return ext != null ? ext : context.getFilesDir();
    }

    File targetRbrRoot() {
        return new File(targetDocumentsRoot(), "RBR");
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

    void handleZipsPicked(List<Uri> docUris, int intentFlags) {
        if (docUris == null || docUris.isEmpty()) {
            return;
        }
        for (Uri u : docUris) {
            takePersistableRead(u, intentFlags);
        }
        startZipBatchImport(new ArrayList<>(docUris));
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

    private void startZipBatchImport(List<Uri> zipUris) {
        if (!running.compareAndSet(false, true)) {
            return;
        }
        Thread worker = new Thread(() -> {
            try {
                runZipBatchImport(zipUris);
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
        listener.onStatus(STATE_RUNNING, 0, 0, 0L, "ANALIZANDO ÁRBOL SAF UNIVERSAL: " + label);

        String treeRootDocId = DocumentsContract.getTreeDocumentId(treeUri);
        RootSelection rootSel = locateLogicalRoot(resolver, treeUri, treeRootDocId);
        if (rootSel == null) {
            listener.onStatus(
                STATE_ERROR,
                0,
                0,
                0L,
                "LA CARPETA NO CONTIENE RECURSOS RBR/RSF RECONOCIBLES (Cars/, Maps/, Plugins/NGP.dll, physics.rbz)"
            );
            return;
        }

        List<CopyTask> plan = new ArrayList<>();
        Set<String> plannedLower = new HashSet<>();
        collectUniversalTreeTasks(
            resolver,
            treeUri,
            rootSel.docId,
            rootSel.virtualPrefix,
            0,
            plan,
            plannedLower
        );

        if (plan.isEmpty()) {
            listener.onStatus(
                STATE_ERROR,
                0,
                0,
                0L,
                "NO SE ENCONTRARON ARCHIVOS ÚTILES DE RBR/RSF EN LA CARPETA SELECCIONADA"
            );
            return;
        }

        File destRoot = targetRbrRoot();
        if (!destRoot.exists() && !destRoot.mkdirs()) {
            throw new IOException("No se pudo crear " + destRoot.getAbsolutePath());
        }

        long bytesProcessed = 0L;
        int copiedFiles = 0;
        int reusedFiles = 0;
        int protectedFiles = 0;
        int total = plan.size();
        byte[] buffer = new byte[128 * 1024];

        for (int i = 0; i < total; i++) {
            CopyTask task = plan.get(i);
            File dst = resolveCasePreservingDest(destRoot, task.destRel);
            if (dst == null) {
                continue;
            }
            boolean isCore = task.destRel.equalsIgnoreCase("Plugins/NGP.dll")
                || task.destRel.equalsIgnoreCase("physics.rbz");
            boolean isSmallCfg = task.destRel.endsWith(".ini")
                || task.destRel.endsWith(".json")
                || task.destRel.endsWith(".lsp");
            if (!isCore && !isSmallCfg && dst.isFile() && task.size > 0 && dst.length() == task.size) {
                bytesProcessed += task.size;
                reusedFiles++;
                listener.onStatus(STATE_RUNNING, i + 1, total, bytesProcessed, "REUTILIZADO: " + task.destRel);
                continue;
            }

            listener.onStatus(STATE_RUNNING, i, total, bytesProcessed, "IMPORTANDO: " + task.destRel);
            CopyOutcome out = copyUriWithCoreGuard(resolver, task.uri, destRoot, dst, task.destRel, buffer);
            if (out.protectedNote != null) {
                protectedFiles++;
                listener.onStatus(STATE_RUNNING, i + 1, total, bytesProcessed, out.protectedNote);
            } else if (out.reused) {
                reusedFiles++;
                bytesProcessed += out.bytes;
                listener.onStatus(STATE_RUNNING, i + 1, total, bytesProcessed, "REUTILIZADO: " + task.destRel);
            } else {
                copiedFiles++;
                bytesProcessed += out.bytes;
                listener.onStatus(STATE_RUNNING, i + 1, total, bytesProcessed, "INSTALADO: " + task.destRel);
            }
        }

        String summary = String.format(
            Locale.US,
            "IMPORTACIÓN SAF COMPLETADA: %d NUEVOS · %d REUTILIZADOS · %d PROTEGIDOS (%.1f MiB)",
            copiedFiles,
            reusedFiles,
            protectedFiles,
            bytesProcessed / (1024.0 * 1024.0)
        );
        listener.onStatus(STATE_DONE, total, total, bytesProcessed, summary);
    }

    private void collectUniversalTreeTasks(
            ContentResolver resolver,
            Uri treeUri,
            String dirDocId,
            String relPrefix,
            int depth,
            List<CopyTask> plan,
            Set<String> plannedLower) {
        if (depth > 6) {
            return;
        }
        List<DocNode> children = listChildren(resolver, treeUri, dirDocId);
        for (DocNode child : children) {
            String name = child.name;
            String low = name.toLowerCase(Locale.ROOT);
            if (child.isDir()) {
                // Podar directorios irrelevantes antes de hacer consultas SAF adicionales
                if (depth == 0 && (relPrefix == null || relPrefix.isEmpty())) {
                    if (!low.equals("plugins")
                            && !low.equals("cars")
                            && !low.equals("maps")
                            && !low.equals("harwood")
                            && !low.equals("rsfdata")
                            && !low.equals("audiofmod")
                            && !low.equals("audio")
                            && !low.equals("sgs")
                            && !low.equals("generic")) {
                        continue;
                    }
                }
                String curRel = (relPrefix == null || relPrefix.isEmpty()) ? name : relPrefix + "/" + name;
                String curLow = curRel.toLowerCase(Locale.ROOT);
                // Dentro de Cars/<Modelo>/ solo existe `textures/` útil para el motor 3D (evita miles de queries en `carparts/` o `skins/`)
                if (curLow.startsWith("cars/")) {
                    int slashes = countSlashes(curLow);
                    if (slashes >= 2 && !low.equals("textures")) {
                        continue;
                    }
                }
                if (curLow.startsWith("rsfdata/") && slashesEq(curLow, 1) && !low.equals("cache") && !low.equals("cars")) {
                    continue;
                }
                if (curLow.startsWith("audio/") && slashesEq(curLow, 1)
                        && !low.equals("cars") && !low.equals("speech") && !low.equals("music")) {
                    continue;
                }
                collectUniversalTreeTasks(resolver, treeUri, child.docId, curRel, depth + 1, plan, plannedLower);
            } else {
                String candidateRel = (relPrefix == null || relPrefix.isEmpty()) ? name : relPrefix + "/" + name;
                String canon = GameActivity.nativeNormalizeRbrRel(candidateRel);
                if (canon != null && !canon.isEmpty()) {
                    String key = canon.toLowerCase(Locale.ROOT);
                    if (plannedLower.add(key)) {
                        Uri docUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, child.docId);
                        plan.add(new CopyTask(docUri, canon, child.size));
                    }
                }
            }
        }
    }

    private static int countSlashes(String s) {
        int c = 0;
        for (int i = 0; i < s.length(); i++) {
            if (s.charAt(i) == '/') c++;
        }
        return c;
    }

    private static boolean slashesEq(String s, int expected) {
        return countSlashes(s) == expected;
    }

    private static final class RootSelection {
        final String docId;
        final String virtualPrefix;

        RootSelection(String docId, String virtualPrefix) {
            this.docId = docId;
            this.virtualPrefix = virtualPrefix;
        }
    }

    private RootSelection locateLogicalRoot(ContentResolver resolver, Uri treeUri, String rootDocId) {
        List<DocNode> top = listChildren(resolver, treeUri, rootDocId);
        if (looksLikeRbrContents(top)) {
            return new RootSelection(rootDocId, "");
        }
        DocNode childRbr = findDirIgnoreCase(top, "RBR");
        if (childRbr != null && looksLikeRbrContents(listChildren(resolver, treeUri, childRbr.docId))) {
            return new RootSelection(childRbr.docId, "");
        }
        if (hasExt(top, ".lbs") || findIgnoreCase(top, "TextureFilenameMap.dat") != null) {
            return new RootSelection(rootDocId, "Maps");
        }
        if (findIgnoreCase(top, "Cars.ini") != null) {
            return new RootSelection(rootDocId, "Cars");
        }
        if (hasExt(top, ".bank")) {
            return new RootSelection(rootDocId, "AudioFMOD");
        }
        if (hasExt(top, ".sgc")) {
            String folder = sanitizeLastFolderName(describeUri(treeUri));
            return new RootSelection(rootDocId, "Cars/" + folder);
        }
        for (DocNode sub : top) {
            if (sub.isDir()) {
                List<DocNode> subKids = listChildren(resolver, treeUri, sub.docId);
                if (looksLikeRbrContents(subKids)) {
                    return new RootSelection(sub.docId, "");
                }
                DocNode nestedRbr = findDirIgnoreCase(subKids, "RBR");
                if (nestedRbr != null && looksLikeRbrContents(listChildren(resolver, treeUri, nestedRbr.docId))) {
                    return new RootSelection(nestedRbr.docId, "");
                }
            }
        }
        return null;
    }

    private String sanitizeLastFolderName(String raw) {
        String clean = raw.replace('\\', '/').replace(':', '/');
        int slash = clean.lastIndexOf('/');
        String tail = slash >= 0 ? clean.substring(slash + 1).trim() : clean.trim();
        return tail.isEmpty() ? "ImportedCar" : tail;
    }

    private boolean hasExt(List<DocNode> nodes, String extLower) {
        for (DocNode n : nodes) {
            if (!n.isDir() && n.name.toLowerCase(Locale.ROOT).endsWith(extLower)) {
                return true;
            }
        }
        return false;
    }

    private boolean looksLikeRbrContents(List<DocNode> children) {
        for (DocNode c : children) {
            String low = c.name.toLowerCase(Locale.ROOT);
            if (low.equals("physics.rbz")
                    || low.equals("misc.rbz")
                    || low.equals("scripts.rbz")
                    || low.equals("ngp.dll")
                    || (c.isDir()
                        && (low.equals("cars")
                            || low.equals("maps")
                            || low.equals("plugins")
                            || low.equals("rsfdata")
                            || low.equals("audiofmod")
                            || low.equals("audio")
                            || low.equals("sgs")
                            || low.equals("generic")))) {
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

    private File resolveCasePreservingDest(File destRoot, String canonicalRel) throws IOException {
        String normalized = canonicalRel.replace('\\', '/').trim();
        while (normalized.startsWith("/")) {
            normalized = normalized.substring(1);
        }
        if (normalized.isEmpty() || normalized.contains("..")) {
            return null;
        }
        File cur = destRoot;
        String[] parts = normalized.split("/");
        for (String seg : parts) {
            if (seg.isEmpty()) continue;
            File matched = null;
            File[] existing = cur.listFiles();
            if (existing != null) {
                for (File f : existing) {
                    if (f.getName().equalsIgnoreCase(seg)) {
                        matched = f;
                        break;
                    }
                }
            }
            cur = matched != null ? matched : new File(cur, seg);
        }
        String rootCanon = destRoot.getCanonicalPath();
        String targetCanon = cur.getCanonicalPath();
        if (!targetCanon.startsWith(rootCanon + File.separator) && !targetCanon.equals(rootCanon)) {
            return null;
        }
        return cur;
    }

    private static final class CopyOutcome {
        final long bytes;
        final boolean reused;
        final String protectedNote;

        CopyOutcome(long bytes, boolean reused, String protectedNote) {
            this.bytes = bytes;
            this.reused = reused;
            this.protectedNote = protectedNote;
        }
    }

    private CopyOutcome copyUriWithCoreGuard(
            ContentResolver resolver,
            Uri srcUri,
            File destRbrRoot,
            File dst,
            String canonicalRel,
            byte[] buffer) throws IOException {
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

        String preserveNote = GameActivity.nativeCheckPreserveCoreFile(
            destRbrRoot.getAbsolutePath(),
            canonicalRel,
            part.getAbsolutePath()
        );
        if (preserveNote != null && !preserveNote.isEmpty()) {
            part.delete();
            return new CopyOutcome(0L, false, preserveNote);
        }

        if (dst.exists() && !dst.delete()) {
            part.delete();
            throw new IOException("No se pudo actualizar " + dst.getName());
        }
        if (!part.renameTo(dst)) {
            part.delete();
            throw new IOException("Fallo al confirmar archivo " + dst.getName());
        }
        return new CopyOutcome(total, false, null);
    }

    private void runZipBatchImport(List<Uri> zipUris) throws IOException {
        ContentResolver resolver = context.getContentResolver();
        String docsPath = targetDocumentsRoot().getAbsolutePath();
        int batchCount = zipUris.size();

        for (int i = 0; i < batchCount; i++) {
            Uri zipUri = zipUris.get(i);
            String label = queryDisplayName(zipUri);
            boolean handledByRust = false;
            try {
                ParcelFileDescriptor pfd = resolver.openFileDescriptor(zipUri, "r");
                if (pfd != null) {
                    int detached = pfd.detachFd();
                    handledByRust = GameActivity.nativeImportZipFd(detached, label, docsPath);
                }
            } catch (Exception ignored) {
                handledByRust = false;
            }
            if (!handledByRust) {
                runStreamingZipFallback(resolver, zipUri, label);
            }
        }
    }

    /** Respaldo por stream únicamente cuando el proveedor SAF remoto no entrega un FD con Seek. */
    private void runStreamingZipFallback(ContentResolver resolver, Uri zipUri, String label) throws IOException {
        File destRoot = targetRbrRoot();
        if (!destRoot.exists() && !destRoot.mkdirs()) {
            throw new IOException("No se pudo crear " + destRoot.getAbsolutePath());
        }
        listener.onStatus(STATE_RUNNING, 0, 0, 0L, "EXTRAYENDO STREAM ZIP SAF: " + label);

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
                    String rel = GameActivity.nativeNormalizeRbrRel(entry.getName());
                    if (rel == null || rel.isEmpty()) {
                        continue;
                    }
                    File dst = resolveCasePreservingDest(destRoot, rel);
                    if (dst == null) {
                        continue;
                    }
                    long entrySz = entry.getSize();
                    boolean isCore = rel.equalsIgnoreCase("Plugins/NGP.dll") || rel.equalsIgnoreCase("physics.rbz");
                    if (!isCore && entrySz > 4L * 1024L * 1024L && dst.isFile() && dst.length() == entrySz) {
                        filesDone++;
                        bytesDone += entrySz;
                        listener.onStatus(STATE_RUNNING, filesDone, 0, bytesDone, "REUTILIZADO: " + rel);
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
                            out.write(buffer, 0, read);
                        }
                        out.getFD().sync();
                    } catch (IOException e) {
                        part.delete();
                        throw e;
                    }
                    String preserve = GameActivity.nativeCheckPreserveCoreFile(
                        destRoot.getAbsolutePath(), rel, part.getAbsolutePath()
                    );
                    if (preserve != null && !preserve.isEmpty()) {
                        part.delete();
                        filesDone++;
                        listener.onStatus(STATE_RUNNING, filesDone, 0, bytesDone, preserve);
                        continue;
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
                "EL ZIP NO CONTIENE ARCHIVOS RBR/RSF VÁLIDOS (ESPERADO RBR/Cars, RBR/Maps, RBR/Plugins)"
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
}
