package org.rerbr.mobile;

import android.app.Activity;
import android.app.NativeActivity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.os.Bundle;
import android.view.Surface;
import android.view.View;
import android.view.WindowInsets;

public final class GameActivity extends NativeActivity implements SensorEventListener {
    private OnlineBoard board;
    public void showOnlineBoard() { runOnUiThread(() -> board.show()); }
    static { System.loadLibrary("rerbr_probe"); }

    private static final int REQ_OPEN_RBR_TREE = 4101;
    private static final int REQ_OPEN_RBR_ZIP = 4102;

    private static native void sensorRoll(double radians);
    private static native void safeArea(double left, double top, double right, double bottom, boolean phone);
    private static native void importStatus(int state, int filesDone, int filesTotal, long bytesDone, String detail);
    private static native void persistedUriState(boolean available, String label);
    static native boolean nativeImportZipFd(int detachedFd, String label, String destDocumentsDir);
    static native String nativeNormalizeRbrRel(String rawPath);
    static native String nativeCheckPreserveCoreFile(String destRbrDir, String canonicalRel, String candidatePartPath);

    private SensorManager sensors;
    private Sensor gravity;
    private GameAudio audio;
    private SafResourceImporter safImporter;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        board = new OnlineBoard(this);
    if (android.os.Build.VERSION.SDK_INT >= 30) {
    getWindow().setDecorFitsSystemWindows(false);
}
hideSystemBars();
View decor = getWindow().getDecorView();
decor.addOnLayoutChangeListener((v, l, t, r, b, ol, ot, or, ob) -> updateInsets(v));
decor.setOnApplyWindowInsetsListener((v, insets) -> {
    reportInsets(v, insets);
    return v.onApplyWindowInsets(insets);
});
        sensors = getSystemService(SensorManager.class);
        gravity = sensors.getDefaultSensor(Sensor.TYPE_GRAVITY);
        if (gravity == null) gravity = sensors.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
        audio = new GameAudio(this);
        safImporter = new SafResourceImporter(this, new SafResourceImporter.Listener() {
            @Override public void onStatus(int st, int done, int total, long bytes, String detail) {
                importStatus(st, done, total, bytes, detail != null ? detail : "");
            }
            @Override public void onPersistedUri(boolean available, String label) {
                persistedUriState(available, label != null ? label : "");
            }
        });
        safImporter.syncPersistedState();
    }

    private void hideSystemBars() {
    if (android.os.Build.VERSION.SDK_INT >= 30) {
        android.view.WindowInsetsController controller =
                getWindow().getInsetsController();

        if (controller != null) {
            controller.hide(WindowInsets.Type.systemBars());
        }
    } else {
        getWindow().getDecorView().setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_FULLSCREEN
                | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
        );
    }
}
    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemBars();
        }
    }

    /** Invocado desde Rust (JNI) cuando el jugador pulsa SELECCIONAR CARPETA RBR (SAF). */
    @SuppressWarnings("unused")
    public void requestOpenRbrTree() {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
                intent.addFlags(
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                        | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
                        | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION
                );
                startActivityForResult(intent, REQ_OPEN_RBR_TREE);
            } catch (ActivityNotFoundException e) {
                importStatus(
                    SafResourceImporter.STATE_ERROR,
                    0,
                    0,
                    0L,
                    "ESTE DISPOSITIVO ANDROID NO DISPONE DE SELECTOR DE ARCHIVOS SAF"
                );
            }
        });
    }

    /** Invocado desde Rust (JNI) cuando el jugador pulsa IMPORTAR PAQUETE .ZIP (SAF). */
    @SuppressWarnings("unused")
    public void requestOpenRbrZip() {
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("*/*");
                intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                intent.putExtra(
                    Intent.EXTRA_MIME_TYPES,
                    new String[] {"application/zip", "application/x-zip-compressed", "application/octet-stream"}
                );
                intent.addFlags(
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                        | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
                );
                startActivityForResult(intent, REQ_OPEN_RBR_ZIP);
            } catch (ActivityNotFoundException e) {
                importStatus(
                    SafResourceImporter.STATE_ERROR,
                    0,
                    0,
                    0L,
                    "ESTE DISPOSITIVO ANDROID NO DISPONE DE SELECTOR DE DOCUMENTOS SAF"
                );
            }
        });
    }

    /** Invocado desde Rust (JNI) para reutilizar la carpeta autorizada persistentemente. */
    @SuppressWarnings("unused")
    public void requestReimportRbrTree() {
        if (safImporter != null) {
            safImporter.reimportPersistedTree();
        }
    }

    @Override protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (safImporter == null) {
            return;
        }
        if (requestCode == REQ_OPEN_RBR_TREE) {
            if (resultCode == Activity.RESULT_OK && data != null && data.getData() != null) {
                safImporter.handleTreePicked(data.getData(), data.getFlags());
            } else {
                importStatus(SafResourceImporter.STATE_IDLE, 0, 0, 0L, "SELECCIÓN DE CARPETA CANCELADA");
            }
        } else if (requestCode == REQ_OPEN_RBR_ZIP) {
            if (resultCode == Activity.RESULT_OK && data != null) {
                java.util.List<android.net.Uri> uris = new java.util.ArrayList<>();
                if (data.getClipData() != null) {
                    android.content.ClipData clip = data.getClipData();
                    for (int i = 0; i < clip.getItemCount(); i++) {
                        android.net.Uri u = clip.getItemAt(i).getUri();
                        if (u != null) uris.add(u);
                    }
                } else if (data.getData() != null) {
                    uris.add(data.getData());
                }
                if (!uris.isEmpty()) {
                    safImporter.handleZipsPicked(uris, data.getFlags());
                } else {
                    importStatus(SafResourceImporter.STATE_IDLE, 0, 0, 0L, "SELECCIÓN DE ARCHIVO ZIP CANCELADA");
                }
            } else {
                importStatus(SafResourceImporter.STATE_IDLE, 0, 0, 0L, "SELECCIÓN DE ARCHIVO ZIP CANCELADA");
            }
        }
    }

    private void updateInsets(View view) {
        WindowInsets insets = view.getRootWindowInsets();
        if (insets != null) reportInsets(view, insets);
    }
private void reportInsets(View view, WindowInsets windowInsets) {
    int width = view.getWidth(), height = view.getHeight();
    if (width == 0 || height == 0) return;

    android.graphics.Insets i;

    if (android.os.Build.VERSION.SDK_INT >= 30) {
        i = windowInsets.getInsets(
                WindowInsets.Type.systemBars()
                | WindowInsets.Type.displayCutout()
        );
    } else {
        i = android.graphics.Insets.of(
                windowInsets.getSystemWindowInsetLeft(),
                windowInsets.getSystemWindowInsetTop(),
                windowInsets.getSystemWindowInsetRight(),
                windowInsets.getSystemWindowInsetBottom()
        );
    }

    boolean phone = getResources().getConfiguration().smallestScreenWidthDp < 600;

    safeArea(
            (double) i.left / width,
            (double) i.top / height,
            (double) i.right / width,
            (double) i.bottom / height,
            phone
    );
}
    

    @Override protected void onResume() {
        super.onResume();
        hideSystemBars();
        if (gravity != null) sensors.registerListener(this, gravity, SensorManager.SENSOR_DELAY_GAME);
        if (safImporter != null) safImporter.syncPersistedState();
        audio.resume();
    }

    @Override protected void onPause() {
        sensors.unregisterListener(this);
        sensorRoll(Double.NaN);
        audio.pause();
        super.onPause();
    }

    @Override protected void onDestroy() {
        board.destroy();
        audio.close();
        super.onDestroy();
    }

    @Override public void onAccuracyChanged(Sensor sensor, int accuracy) {}

    @Override public void onSensorChanged(SensorEvent sample) {
        float x = sample.values[0], y = sample.values[1];
        double roll;
        switch (getDisplay().getRotation()) {
            case Surface.ROTATION_90: roll = Math.atan2(y, x); break;
            case Surface.ROTATION_180: roll = Math.atan2(x, -y); break;
            case Surface.ROTATION_270: roll = Math.atan2(-y, -x); break;
            default: roll = Math.atan2(-x, y);
        }
        sensorRoll(Math.hypot(x, y) >= 0.2 * SensorManager.GRAVITY_EARTH ? roll : Double.NaN);
    }
}
