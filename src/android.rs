//! NativeActivity owns Java services. Winit runs the shared game on android_main.
use super::{EventLoop, Probe};
use jni_sys::{
    jboolean, jclass, jint, jlong, jobject, jstring, JNIEnv, JavaVM, JNI_OK, JNI_VERSION_1_6,
};
use std::ffi::{c_void, CStr};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use winit::platform::android::{
    activity::{AndroidApp, AndroidAppWaker},
    EventLoopBuilderExtAndroid,
};

#[derive(Clone, Copy)]
struct JniHandles {
    vm: usize,
    activity: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SafStatus {
    pub state: i32,
    pub files_done: u32,
    pub files_total: u32,
    pub bytes_done: u64,
    pub detail: String,
    pub completion_epoch: u64,
    pub has_persisted_uri: bool,
    pub persisted_label: String,
}

static TILT: Mutex<Option<(f64, Instant)>> = Mutex::new(None);
static LAYOUT: Mutex<([f64; 4], bool)> = Mutex::new(([0.0; 4], true));
static SAF: Mutex<SafStatus> = Mutex::new(SafStatus {
    state: 0,
    files_done: 0,
    files_total: 0,
    bytes_done: 0,
    detail: String::new(),
    completion_epoch: 0,
    has_persisted_uri: false,
    persisted_label: String::new(),
});
static HANDLES: OnceLock<JniHandles> = OnceLock::new();
static WAKER: OnceLock<AndroidAppWaker> = OnceLock::new();

#[no_mangle]
fn android_main(app: AndroidApp) {
    let _ = HANDLES.set(JniHandles {
        vm: app.vm_as_ptr() as usize,
        activity: app.activity_as_ptr() as usize,
    });
    let _ = WAKER.set(app.create_waker());
    if let Some(files) = app
        .external_data_path()
        .or_else(|| app.internal_data_path())
    {
        std::env::set_var("RERBR_PROBE_DOCUMENTS", &files);
        eprintln!("Android game resources: {}", files.join("RBR").display());
    } else {
        std::env::remove_var("RERBR_PROBE_DOCUMENTS");
        eprintln!("Android app storage unavailable");
    }
    let result = EventLoop::builder()
        .with_android_app(app)
        .build()
        .and_then(|events| events.run_app(&mut Probe::default()));
    if let Err(error) = result {
        eprintln!("Android game loop: {error}");
    }
}

pub(super) fn tilt(enabled: bool) -> f64 {
    TILT.lock()
        .unwrap()
        .filter(|(_, when)| enabled && when.elapsed() < Duration::from_millis(250))
        .map_or(f64::NAN, |(angle, _)| angle)
}

pub(super) fn layout() -> ([f64; 4], bool) {
    *LAYOUT.lock().unwrap()
}

pub(crate) fn saf_status() -> SafStatus {
    SAF.lock().unwrap().clone()
}

pub(crate) fn open_rbr_tree_picker() -> bool {
    invoke_activity_method(c"requestOpenRbrTree")
}

pub(crate) fn open_rbr_zip_picker() -> bool {
    invoke_activity_method(c"requestOpenRbrZip")
}

pub(crate) fn reimport_persisted_rbr_tree() -> bool {
    invoke_activity_method(c"requestReimportRbrTree")
}

fn invoke_activity_method(method_name: &CStr) -> bool {
    let Some(&handles) = HANDLES.get() else {
        return false;
    };
    unsafe {
        let vm = handles.vm as *mut JavaVM;
        let activity = handles.activity as jobject;
        if vm.is_null() || activity.is_null() {
            return false;
        }
        let mut env_void: *mut c_void = std::ptr::null_mut();
        let mut attached = false;
        if ((**vm).GetEnv.unwrap())(vm, &mut env_void, JNI_VERSION_1_6) != JNI_OK {
            if ((**vm).AttachCurrentThread.unwrap())(vm, &mut env_void, std::ptr::null_mut())
                != JNI_OK
            {
                return false;
            }
            attached = true;
        }
        let env = env_void.cast::<JNIEnv>();
        let ok = if !env.is_null() {
            let fns = &**env;
            let cls = (fns.GetObjectClass.unwrap())(env, activity);
            if !cls.is_null() {
                let mid =
                    (fns.GetMethodID.unwrap())(env, cls, method_name.as_ptr(), c"()V".as_ptr());
                let invoked = if !mid.is_null() {
                    (fns.CallVoidMethodA.unwrap())(env, activity, mid, std::ptr::null());
                    if !(fns.ExceptionOccurred.unwrap())(env).is_null() {
                        (fns.ExceptionClear.unwrap())(env);
                        false
                    } else {
                        true
                    }
                } else {
                    if !(fns.ExceptionOccurred.unwrap())(env).is_null() {
                        (fns.ExceptionClear.unwrap())(env);
                    }
                    false
                };
                (fns.DeleteLocalRef.unwrap())(env, cls);
                invoked
            } else {
                false
            }
        } else {
            false
        };
        if attached {
            let _ = ((**vm).DetachCurrentThread.unwrap())(vm);
        }
        ok
    }
}

unsafe fn jstring_to_rust(env: *mut JNIEnv, text: jstring) -> String {
    if env.is_null() || text.is_null() {
        return String::new();
    }
    let fns = &**env;
    let chars = (fns.GetStringUTFChars.unwrap())(env, text, std::ptr::null_mut());
    if chars.is_null() {
        return String::new();
    }
    let out = CStr::from_ptr(chars).to_string_lossy().into_owned();
    (fns.ReleaseStringUTFChars.unwrap())(env, text, chars);
    out
}

#[no_mangle]
extern "system" fn Java_org_rerbr_mobile_GameActivity_sensorRoll(
    _: *mut JNIEnv,
    _: jclass,
    angle: f64,
) {
    *TILT.lock().unwrap() = angle.is_finite().then(|| (angle, Instant::now()));
}

#[no_mangle]
extern "system" fn Java_org_rerbr_mobile_GameActivity_safeArea(
    _: *mut JNIEnv,
    _: jclass,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    phone: jboolean,
) {
    *LAYOUT.lock().unwrap() = ([left, top, right, bottom], phone != 0);
}

#[no_mangle]
unsafe extern "system" fn Java_org_rerbr_mobile_GameActivity_importStatus(
    env: *mut JNIEnv,
    _: jclass,
    state: jint,
    files_done: jint,
    files_total: jint,
    bytes_done: jlong,
    detail: jstring,
) {
    let detail_str = jstring_to_rust(env, detail);
    {
        let mut saf = SAF.lock().unwrap();
        saf.state = state;
        saf.files_done = files_done.max(0) as u32;
        saf.files_total = files_total.max(0) as u32;
        saf.bytes_done = bytes_done.max(0) as u64;
        saf.detail = detail_str;
        if state == 2 {
            saf.completion_epoch = saf.completion_epoch.wrapping_add(1);
        }
    }
    if let Some(waker) = WAKER.get() {
        waker.wake();
    }
}

#[no_mangle]
unsafe extern "system" fn Java_org_rerbr_mobile_GameActivity_persistedUriState(
    env: *mut JNIEnv,
    _: jclass,
    available: jboolean,
    label: jstring,
) {
    let label_str = jstring_to_rust(env, label);
    let mut saf = SAF.lock().unwrap();
    saf.has_persisted_uri = available != 0;
    saf.persisted_label = label_str;
}

#[cfg(feature = "physics-drive")]
#[no_mangle]
unsafe extern "system" fn Java_org_rerbr_mobile_GameAudio_render(
    env: *mut JNIEnv,
    _: jclass,
    buffer: jobject,
) {
    // The audio thread owns this direct ByteBuffer until this synchronous call returns.
    let jni = &**env;
    let capacity = (jni.GetDirectBufferCapacity.unwrap())(env, buffer);
    if !(8..=4096 * 8).contains(&capacity) || capacity % 8 != 0 {
        return;
    }
    let data = (jni.GetDirectBufferAddress.unwrap())(env, buffer).cast::<f32>();
    if !data.is_null() && data.is_aligned() {
        crate::driving::audio::rerbr_audio_render(data, (capacity / 8) as u32, 44100);
    }
}
