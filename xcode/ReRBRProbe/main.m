#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <CoreMotion/CoreMotion.h>
#import <AVFoundation/AVFoundation.h>
#import <AudioToolbox/AudioToolbox.h>
#include <math.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <mach/mach.h>

// Public Mach accounting; zero means unavailable. No private API or entitlement.
uint64_t rerbr_probe_physical_footprint(void) {
    task_vm_info_data_t info = {0};
    mach_msg_type_number_t count = TASK_VM_INFO_COUNT;
    kern_return_t result = task_info(mach_task_self(), TASK_VM_INFO,
        (task_info_t)&info, &count);
    return result == KERN_SUCCESS ? info.phys_footprint : 0;
}

extern int rerbr_probe_main(void);

// C ABI audio functions exported by Rust rerbr_probe::driving::audio
extern void rerbr_audio_render(float *out_stereo, uint32_t frames, uint32_t sample_rate);
extern void rerbr_audio_set_paused(int paused);
extern void rerbr_audio_set_interrupted(int interrupted);
extern void rerbr_audio_set_volume(float volume);
extern int rerbr_audio_prepare(void);

static AudioComponentInstance g_audio_unit = NULL;

static OSStatus rerbr_probe_audio_callback(
    void *inRefCon,
    AudioUnitRenderActionFlags *ioActionFlags,
    const AudioTimeStamp *inTimeStamp,
    UInt32 inBusNumber,
    UInt32 inNumberFrames,
    AudioBufferList *ioData)
{
    (void)inRefCon;
    (void)ioActionFlags;
    (void)inTimeStamp;
    (void)inBusNumber;
    if (!ioData) return noErr;
    // RemoteIO was configured for one interleaved stereo buffer. Refuse a
    // changed/undersized layout instead of letting Rust write past its storage.
    if (ioData->mNumberBuffers != 1 || ioData->mBuffers[0].mNumberChannels != 2 ||
        inNumberFrames > 4096 ||
        ioData->mBuffers[0].mDataByteSize / (2 * sizeof(float)) < inNumberFrames) {
        for (UInt32 i = 0; i < ioData->mNumberBuffers; ++i) {
            if (ioData->mBuffers[i].mData)
                memset(ioData->mBuffers[i].mData, 0, ioData->mBuffers[i].mDataByteSize);
        }
        return noErr;
    }
    float *outL = (float *)ioData->mBuffers[0].mData;
    rerbr_audio_render(outL, inNumberFrames, 44100);
    return noErr;
}

static void rerbr_probe_audio_setup(void) {
    if (!rerbr_audio_prepare()) return;
    NSError *error = nil;
    AVAudioSession *session = [AVAudioSession sharedInstance];
    [session setCategory:AVAudioSessionCategoryPlayback
             withOptions:AVAudioSessionCategoryOptionMixWithOthers
                   error:&error];
    if (error) {
        fprintf(stderr, "audio: AVAudioSession setCategory failed: %s\n",
                error.localizedDescription.UTF8String);
        rerbr_audio_set_interrupted(1);
        return;
    }
    [session setPreferredSampleRate:44100.0 error:nil];
    if (![session setActive:YES error:&error]) {
        fprintf(stderr, "audio: AVAudioSession activation failed: %s\n", error.localizedDescription.UTF8String);
        rerbr_audio_set_interrupted(1);
        return;
    }

    [[NSNotificationCenter defaultCenter]
        addObserverForName:AVAudioSessionInterruptionNotification
                    object:session
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *notification) {
                    NSNumber *type = notification.userInfo[AVAudioSessionInterruptionTypeKey];
                    if (type.unsignedIntegerValue == AVAudioSessionInterruptionTypeBegan) {
                        fprintf(stderr, "audio: AVAudioSession interruption began\n");
                        rerbr_audio_set_interrupted(1);
                    } else if (type.unsignedIntegerValue == AVAudioSessionInterruptionTypeEnded) {
                        fprintf(stderr, "audio: AVAudioSession interruption ended\n");
                        NSError *activeErr = nil;
                        NSNumber *options = notification.userInfo[AVAudioSessionInterruptionOptionKey];
                        if ((options.unsignedIntegerValue & AVAudioSessionInterruptionOptionShouldResume) &&
                            [[AVAudioSession sharedInstance] setActive:YES error:&activeErr]) {
                            rerbr_audio_set_interrupted(0);
                        }
                    }
                }];

    AudioComponentDescription desc;
    desc.componentType = kAudioUnitType_Output;
    desc.componentSubType = kAudioUnitSubType_RemoteIO;
    desc.componentManufacturer = kAudioUnitManufacturer_Apple;
    desc.componentFlags = 0;
    desc.componentFlagsMask = 0;

    AudioComponent comp = AudioComponentFindNext(NULL, &desc);
    if (!comp) {
        fprintf(stderr, "audio: RemoteIO AudioComponentFindNext failed\n");
        return;
    }

    OSStatus status = AudioComponentInstanceNew(comp, &g_audio_unit);
    if (status != noErr || !g_audio_unit) {
        fprintf(stderr, "audio: AudioComponentInstanceNew failed: %d\n", (int)status);
        return;
    }

    UInt32 enableIO = 1;
    status = AudioUnitSetProperty(g_audio_unit,
                         kAudioOutputUnitProperty_EnableIO,
                         kAudioUnitScope_Output,
                         0,
                         &enableIO,
                         sizeof(enableIO));
    if (status != noErr) goto audio_failed;

    AudioStreamBasicDescription asbd;
    memset(&asbd, 0, sizeof(asbd));
    asbd.mSampleRate = 44100.0;
    asbd.mFormatID = kAudioFormatLinearPCM;
    asbd.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;
    asbd.mBytesPerPacket = 8;
    asbd.mFramesPerPacket = 1;
    asbd.mBytesPerFrame = 8;
    asbd.mChannelsPerFrame = 2;
    asbd.mBitsPerChannel = 32;

    status = AudioUnitSetProperty(g_audio_unit,
                                  kAudioUnitProperty_StreamFormat,
                                  kAudioUnitScope_Input,
                                  0,
                                  &asbd,
                                  sizeof(asbd));
    if (status != noErr) {
        fprintf(stderr, "audio: AudioUnitSetProperty StreamFormat failed: %d\n", (int)status);
        goto audio_failed;
    }

    AURenderCallbackStruct cb;
    cb.inputProc = rerbr_probe_audio_callback;
    cb.inputProcRefCon = NULL;
    status = AudioUnitSetProperty(g_audio_unit,
                                  kAudioUnitProperty_SetRenderCallback,
                                  kAudioUnitScope_Input,
                                  0,
                                  &cb,
                                  sizeof(cb));
    if (status != noErr) {
        fprintf(stderr, "audio: AudioUnitSetProperty RenderCallback failed: %d\n", (int)status);
        goto audio_failed;
    }

    status = AudioUnitInitialize(g_audio_unit);
    if (status != noErr) {
        fprintf(stderr, "audio: AudioUnitInitialize failed: %d\n", (int)status);
        goto audio_failed;
    }

    status = AudioOutputUnitStart(g_audio_unit);
    if (status != noErr) {
        fprintf(stderr, "audio: AudioOutputUnitStart failed: %d\n", (int)status);
        goto audio_failed;
    } else {
        fprintf(stderr, "audio: RemoteIO AudioUnit started at 44100 Hz stereo float\n");
    }
    return;
audio_failed:
    rerbr_audio_set_interrupted(1);
    AudioComponentInstanceDispose(g_audio_unit);
    g_audio_unit = NULL;
}

// Shared by device motion and layout. Polled only on the UIKit thread.
static UIWindow *rerbr_key_window(void) {
    UIWindow *keyWindow = nil;
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if ([scene isKindOfClass:UIWindowScene.class] && scene.activationState == UISceneActivationStateForegroundActive) {
            UIWindowScene *windowScene = (UIWindowScene *)scene;
            for (UIWindow *window in windowScene.windows) {
                if (window.isKeyWindow) keyWindow = window;
            }
        }
    }
    // winit 0.30 deliberately uses the legacy app lifecycle, without a scene manifest.
    // A scene-only lookup can therefore be empty. Keep the public legacy fallback.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    if (keyWindow == nil) keyWindow = UIApplication.sharedApplication.keyWindow;
#pragma clang diagnostic pop
    return keyWindow;
}

// Normalized safe-area insets: left, top, right, bottom. Zero when unavailable.
void rerbr_drive_safe_area(double out[4]) {
    for (int i = 0; i < 4; i++) out[i] = 0.0;
    UIWindow *keyWindow = rerbr_key_window();
    UIView *view = keyWindow.rootViewController.view;
    if (view == nil) return;
    CGSize bounds = view.bounds.size;
    if (bounds.width <= 0 || bounds.height <= 0) return;
    UIEdgeInsets insets = view.safeAreaInsets;
    out[0] = insets.left / bounds.width;
    out[1] = insets.top / bounds.height;
    out[2] = insets.right / bounds.width;
    out[3] = insets.bottom / bounds.height;
}

// Polled only on the UIKit thread. No callbacks into Rust or physics writes.
double rerbr_drive_tilt(int enabled) {
    static CMMotionManager *motion;
    if (!enabled) { [motion stopDeviceMotionUpdates]; return NAN; }
    if (motion == nil) { motion = [[CMMotionManager alloc] init]; motion.deviceMotionUpdateInterval = 1.0/120.0; }
    if (![motion isDeviceMotionAvailable]) return NAN;
    if (![motion isDeviceMotionActive]) [motion startDeviceMotionUpdates];
    UIWindow *keyWindow = rerbr_key_window();
    UIView *view = keyWindow.rootViewController.view;
    if (view == nil || view.bounds.size.width <= view.bounds.size.height) return NAN;
    view.multipleTouchEnabled = YES;
    CMDeviceMotion *sample = motion.deviceMotion;
    if (sample == nil || fabs(sample.timestamp - NSProcessInfo.processInfo.systemUptime) > 0.25) return NAN;
    CMAcceleration g = sample.gravity;
    // Sensor axes are portrait-up. Let UIKit perform the actual window rotation,
    // avoiding the inverse UIDeviceOrientation/UIInterfaceOrientation naming.
    id<UICoordinateSpace> fixed = keyWindow.screen.fixedCoordinateSpace;
    CGPoint zero = [view convertPoint:CGPointZero fromCoordinateSpace:fixed];
    CGPoint gravity = [view convertPoint:CGPointMake(g.x, -g.y) fromCoordinateSpace:fixed];
    double right = gravity.x-zero.x, up = zero.y-gravity.y;
    if (hypot(right, up) < 0.2) return NAN; // Flat on a table has no reliable gravity roll.
    return atan2(right, -up);
}

int main(int argc, char *argv[]) {
    (void)argc;
    (void)argv;
    @autoreleasepool {
        // Files > On My iPad > ReRBRProbe exposes this diagnostic log after launch.
        NSURL *documents = [[[NSFileManager defaultManager]
            URLsForDirectory:NSDocumentDirectory inDomains:NSUserDomainMask] firstObject];
        [[NSFileManager defaultManager] createDirectoryAtURL:documents
            withIntermediateDirectories:YES attributes:nil error:nil];
        NSURL *log = [documents URLByAppendingPathComponent:@"rerbr-probe.log"];
        if (freopen(log.fileSystemRepresentation, "a", stderr) != NULL) {
            setvbuf(stderr, NULL, _IONBF, 0);
        }
        fprintf(stderr, "\nReRBRProbe native entry; main thread: %d\n", pthread_main_np());
        // Pass Foundation's sandbox path before starting Rust/threads. No assets
        // are bundled: the optional diagnostic reads Documents/Harwood/Maps.
        const char *documentsPath = documents.fileSystemRepresentation;
        if (documentsPath == NULL || setenv("RERBR_PROBE_DOCUMENTS", documentsPath, 1) != 0) {
            fprintf(stderr, "harwood: Documents path unavailable; diagnostic will skip\n");
        }
        // Do not call UIApplicationMain here: winit invokes it and owns the UIKit loop.
        rerbr_probe_audio_setup();
        return rerbr_probe_main();
    }
}
