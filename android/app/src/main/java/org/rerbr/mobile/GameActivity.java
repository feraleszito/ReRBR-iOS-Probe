package org.rerbr.mobile;

import android.app.NativeActivity;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.os.Bundle;
import android.view.Surface;
import android.view.View;
import android.view.WindowInsets;

public final class GameActivity extends NativeActivity implements SensorEventListener {
    static { System.loadLibrary("rerbr_probe"); }

    private static native void sensorRoll(double radians);
    private static native void safeArea(double left, double top, double right, double bottom, boolean phone);
    private SensorManager sensors;
    private Sensor gravity;
    private GameAudio audio;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().setDecorFitsSystemWindows(false);
        getWindow().getInsetsController().hide(WindowInsets.Type.systemBars());
        View decor = getWindow().getDecorView();
        decor.addOnLayoutChangeListener((v,l,t,r,b,ol,ot,or,ob) -> updateInsets(v));
        decor.setOnApplyWindowInsetsListener((v, insets) -> {
            reportInsets(v, insets);
            return v.onApplyWindowInsets(insets);
        });
        sensors = getSystemService(SensorManager.class);
        gravity = sensors.getDefaultSensor(Sensor.TYPE_GRAVITY);
        if (gravity == null) gravity = sensors.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
        audio = new GameAudio(this);
    }

    private void updateInsets(View view) {
        WindowInsets insets = view.getRootWindowInsets();
        if (insets != null) reportInsets(view, insets);
    }

    private void reportInsets(View view, WindowInsets windowInsets) {
        int width = view.getWidth(), height = view.getHeight();
        if (width == 0 || height == 0) return;
        android.graphics.Insets i = windowInsets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
        boolean phone = getResources().getConfiguration().smallestScreenWidthDp < 600;
        safeArea((double)i.left / width, (double)i.top / height, (double)i.right / width, (double)i.bottom / height, phone);
    }

    @Override protected void onResume() {
        super.onResume();
        if (gravity != null) sensors.registerListener(this, gravity, SensorManager.SENSOR_DELAY_GAME);
        audio.resume();
    }

    @Override protected void onPause() {
        sensors.unregisterListener(this);
        sensorRoll(Double.NaN);
        audio.pause();
        super.onPause();
    }

    @Override protected void onDestroy() {
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
