#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <CoreMotion/CoreMotion.h>
#import <AVFoundation/AVFoundation.h>
#import <AudioToolbox/AudioToolbox.h>
#import <GameController/GameController.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
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

// Normalized safe-area insets: left, top, right, bottom. Returns the phone idiom.
int rerbr_drive_safe_area(double out[4]) {
    for (int i = 0; i < 4; i++) out[i] = 0.0;
    int phone = UIDevice.currentDevice.userInterfaceIdiom == UIUserInterfaceIdiomPhone;
    UIWindow *keyWindow = rerbr_key_window();
    UIView *view = keyWindow.rootViewController.view;
    if (view == nil) return phone;
    CGSize bounds = view.bounds.size;
    if (bounds.width <= 0 || bounds.height <= 0) return phone;
    UIEdgeInsets insets = view.safeAreaInsets;
    out[0] = insets.left / bounds.width;
    out[1] = insets.top / bounds.height;
    out[2] = insets.right / bounds.width;
    out[3] = insets.bottom / bounds.height;
    return phone;
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

typedef struct {
    uint8_t connected;
    uint8_t _pad[3];
    float left_x;
    float left_y;
    float right_x;
    float right_y;
    float left_trigger;
    float right_trigger;
    uint32_t buttons_mask;
    char name[64];
} RerbrGamepadSnapshot;

void rerbr_gamepad_poll(RerbrGamepadSnapshot *out) {
    if (!out) return;
    memset(out, 0, sizeof(*out));
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        [GCController startWirelessControllerDiscoveryWithCompletionHandler:nil];
    });
    GCController *active = [GCController current];
    if (active == nil || active.extendedGamepad == nil) {
        for (GCController *c in [GCController controllers]) {
            if (c.extendedGamepad != nil) {
                active = c;
                break;
            }
        }
    }
    if (active == nil) return;
    GCExtendedGamepad *pad = active.extendedGamepad;
    if (pad == nil) return;
    out->connected = 1;
    out->left_x = pad.leftThumbstick.xAxis.value;
    out->left_y = pad.leftThumbstick.yAxis.value;
    out->right_x = pad.rightThumbstick.xAxis.value;
    out->right_y = pad.rightThumbstick.yAxis.value;
    out->left_trigger = pad.leftTrigger.value;
    out->right_trigger = pad.rightTrigger.value;
    uint32_t mask = 0;
    if (pad.buttonA.isPressed) mask |= (1u << 0);
    if (pad.buttonB.isPressed) mask |= (1u << 1);
    if (pad.buttonX.isPressed) mask |= (1u << 2);
    if (pad.buttonY.isPressed) mask |= (1u << 3);
    if (pad.leftShoulder.isPressed) mask |= (1u << 4);
    if (pad.rightShoulder.isPressed) mask |= (1u << 5);
    if (pad.dpad.up.isPressed) mask |= (1u << 6);
    if (pad.dpad.down.isPressed) mask |= (1u << 7);
    if (pad.dpad.left.isPressed) mask |= (1u << 8);
    if (pad.dpad.right.isPressed) mask |= (1u << 9);
    if (pad.buttonMenu.isPressed) mask |= (1u << 10);
    if (pad.buttonOptions != nil && pad.buttonOptions.isPressed) mask |= (1u << 11);
    if (pad.leftThumbstickButton != nil && pad.leftThumbstickButton.isPressed) mask |= (1u << 12);
    if (pad.rightThumbstickButton != nil && pad.rightThumbstickButton.isPressed) mask |= (1u << 13);
    out->buttons_mask = mask;
    const char *cname = active.vendorName ? active.vendorName.UTF8String : "Gamepad";
    if (cname) {
        strlcpy(out->name, cname, sizeof(out->name));
    }
}

static char g_pilot_name_result[64] = {0};
static int g_pilot_name_ready = 0;

void rerbr_prompt_pilot_name(const char *initial) {
    NSString *initStr = initial ? [NSString stringWithUTF8String:initial] : @"";
    dispatch_async(dispatch_get_main_queue(), ^{
        UIWindow *keyWindow = rerbr_key_window();
        UIViewController *vc = keyWindow.rootViewController;
        while (vc.presentedViewController != nil) {
            vc = vc.presentedViewController;
        }
        if (vc == nil) return;
        UIAlertController *alert = [UIAlertController
            alertControllerWithTitle:@"Nombre del piloto"
            message:@"Introduce tu nombre de competición (máx. 18 caracteres)"
            preferredStyle:UIAlertControllerStyleAlert];
        [alert addTextFieldWithConfigurationHandler:^(UITextField *tf) {
            tf.text = initStr;
            tf.placeholder = @"PILOTO RBR";
            tf.autocapitalizationType = UITextAutocapitalizationTypeAllCharacters;
            tf.autocorrectionType = UITextAutocorrectionTypeNo;
            tf.clearButtonMode = UITextFieldViewModeWhileEditing;
        }];
        [alert addAction:[UIAlertAction actionWithTitle:@"Cancelar"
                                                  style:UIAlertActionStyleCancel
                                                handler:nil]];
        [alert addAction:[UIAlertAction actionWithTitle:@"Aceptar"
                                                  style:UIAlertActionStyleDefault
                                                handler:^(UIAlertAction *action) {
            (void)action;
            NSString *txt = alert.textFields.firstObject.text ?: @"";
            const char *utf8 = txt.UTF8String;
            if (utf8 != NULL) {
                strlcpy(g_pilot_name_result, utf8, sizeof(g_pilot_name_result));
                g_pilot_name_ready = 1;
            }
        }]];
        [vc presentViewController:alert animated:YES completion:nil];
    });
}

int rerbr_poll_pilot_name(char *out, int cap) {
    if (!g_pilot_name_ready || !out || cap <= 0) return 0;
    strlcpy(out, g_pilot_name_result, (size_t)cap);
    g_pilot_name_ready = 0;
    return 1;
}

// ============================================================================
// Adaptador nativo iOS/iPadOS para importación universal de recursos RBR/RSF
// mediante UIDocumentPickerViewController + URLs de seguridad + NSFileCoordinator.
// La lógica de validación, filtrado, deduplicación y descompresión ZIP vive en Rust.
// ============================================================================
extern void rerbr_ios_import_begin_batch(uint32_t count, const char *utf8_label);
extern void rerbr_ios_import_process_path(
    const char *utf8_path,
    int is_directory,
    uint32_t batch_index,
    uint32_t batch_total
);
extern void rerbr_ios_import_finish_batch(void);
extern void rerbr_ios_import_cancel_or_fail(int is_error, const char *utf8_message);
extern void rerbr_ios_set_persisted_folder_state(int available, const char *utf8_label);

static NSString *const kRerbrFolderBookmarkKey = @"org.rerbr.probe.rbr_folder_bookmark";
static NSString *const kRerbrFolderLabelKey = @"org.rerbr.probe.rbr_folder_label";

@interface RerbrDocumentPickerCoordinator : NSObject <UIDocumentPickerDelegate>
@property (nonatomic, assign) BOOL pickingFolder;
@property (nonatomic, assign) BOOL busy;
@end

@implementation RerbrDocumentPickerCoordinator

+ (instancetype)shared {
    static RerbrDocumentPickerCoordinator *instance = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[RerbrDocumentPickerCoordinator alloc] init];
    });
    return instance;
}

- (UIViewController *)topPresenter {
    UIWindow *keyWindow = rerbr_key_window();
    UIViewController *vc = keyWindow.rootViewController;
    while (vc.presentedViewController != nil) {
        vc = vc.presentedViewController;
    }
    return vc;
}

- (void)syncSavedBookmarkState {
    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
    NSData *bookmark = [defaults dataForKey:kRerbrFolderBookmarkKey];
    NSString *label = [defaults stringForKey:kRerbrFolderLabelKey] ?: @"Carpeta RBR";
    if (bookmark != nil && bookmark.length > 0) {
        rerbr_ios_set_persisted_folder_state(1, label.UTF8String ?: "Carpeta RBR");
    } else {
        rerbr_ios_set_persisted_folder_state(0, "");
    }
}

- (void)presentPickerForFolder:(BOOL)folder {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (self.busy) {
            return;
        }
        UIViewController *presenter = [self topPresenter];
        if (presenter == nil) {
            rerbr_ios_import_cancel_or_fail(1, "VENTANA DE iOS AÚN NO DISPONIBLE PARA PRESENTAR ARCHIVOS");
            return;
        }
        self.pickingFolder = folder;
        NSArray<UTType *> *types = folder
            ? @[UTTypeFolder]
            : @[UTTypeZIP, UTTypeArchive];
        UIDocumentPickerViewController *picker =
            [[UIDocumentPickerViewController alloc] initForOpeningContentTypes:types];
        picker.delegate = self;
        picker.allowsMultipleSelection = !folder;
        picker.shouldShowFileExtensions = YES;
        [presenter presentViewController:picker animated:YES completion:nil];
    });
}

- (void)documentPickerWasCancelled:(UIDocumentPickerViewController *)controller {
    (void)controller;
    rerbr_ios_import_cancel_or_fail(0, "SELECCIÓN EN ARCHIVOS DE iOS CANCELADA");
}

- (void)documentPicker:(UIDocumentPickerViewController *)controller
    didPickDocumentsAtURLs:(NSArray<NSURL *> *)urls
{
    (void)controller;
    if (urls.count == 0) {
        rerbr_ios_import_cancel_or_fail(0, "NINGÚN RECURSO SELECCIONADO");
        return;
    }
    if (self.busy) {
        return;
    }
    self.busy = YES;
    BOOL folderMode = self.pickingFolder;
    NSArray<NSURL *> *pickedUrls = [urls copy];

    if (folderMode) {
        NSURL *folderUrl = pickedUrls.firstObject;
        BOOL scoped = [folderUrl startAccessingSecurityScopedResource];
        NSError *bmErr = nil;
        NSData *bookmark = [folderUrl bookmarkDataWithOptions:0
                               includingResourceValuesForKeys:nil
                                                relativeToURL:nil
                                                        error:&bmErr];
        NSString *label = folderUrl.lastPathComponent ?: @"Carpeta RBR";
        if (bookmark != nil && bmErr == nil) {
            NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
            [defaults setObject:bookmark forKey:kRerbrFolderBookmarkKey];
            [defaults setObject:label forKey:kRerbrFolderLabelKey];
            rerbr_ios_set_persisted_folder_state(1, label.UTF8String ?: "Carpeta RBR");
        }
        if (scoped) {
            [folderUrl stopAccessingSecurityScopedResource];
        }
    }

    NSString *firstLabel = pickedUrls.firstObject.lastPathComponent ?: @"Recursos RBR";
    rerbr_ios_import_begin_batch((uint32_t)pickedUrls.count, firstLabel.UTF8String ?: "Recursos RBR");

    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        @autoreleasepool {
            uint32_t total = (uint32_t)pickedUrls.count;
            BOOL hadFatalCoordError = NO;
            for (uint32_t i = 0; i < total; i++) {
                NSURL *url = pickedUrls[i];
                BOOL scoped = [url startAccessingSecurityScopedResource];
                NSError *coordError = nil;
                __block BOOL invoked = NO;
                NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
                [coordinator coordinateReadingItemAtURL:url
                                                options:0
                                                  error:&coordError
                                             byAccessor:^(NSURL *newURL) {
                    invoked = YES;
                    NSNumber *isDirValue = nil;
                    [newURL getResourceValue:&isDirValue forKey:NSURLIsDirectoryKey error:nil];
                    BOOL isDir = isDirValue != nil ? isDirValue.boolValue : folderMode;
                    const char *fsPath = newURL.fileSystemRepresentation;
                    if (fsPath != NULL) {
                        rerbr_ios_import_process_path(fsPath, isDir ? 1 : 0, i, total);
                    } else {
                        rerbr_ios_import_cancel_or_fail(1, "NO SE PUDO OBTENER RUTA POSIX DEL ARCHIVO EN iOS");
                    }
                }];
                if (!invoked && coordError != nil) {
                    hadFatalCoordError = YES;
                    NSString *errText = [NSString stringWithFormat:@"ERROR DE ACCESO iOS/iCLOUD: %@",
                                         coordError.localizedDescription ?: @"No disponible"];
                    rerbr_ios_import_cancel_or_fail(1, errText.UTF8String ?: "ERROR DE ACCESO iOS");
                }
                if (scoped) {
                    [url stopAccessingSecurityScopedResource];
                }
                if (hadFatalCoordError) {
                    break;
                }
            }
            if (!hadFatalCoordError) {
                rerbr_ios_import_finish_batch();
            }
            dispatch_async(dispatch_get_main_queue(), ^{
                self.busy = NO;
            });
        }
    });
}

- (void)reimportSavedFolder {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (self.busy) {
            return;
        }
        NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
        NSData *bookmark = [defaults dataForKey:kRerbrFolderBookmarkKey];
        if (bookmark == nil || bookmark.length == 0) {
            rerbr_ios_set_persisted_folder_state(0, "");
            rerbr_ios_import_cancel_or_fail(1, "SIN CARPETA GUARDADA — SELECCIONA CARPETA EN ARCHIVOS DE iOS");
            return;
        }
        BOOL stale = NO;
        NSError *resolveErr = nil;
        NSURL *resolvedUrl = [NSURL URLByResolvingBookmarkData:bookmark
                                                       options:0
                                                 relativeToURL:nil
                                           bookmarkDataIsStale:&stale
                                                         error:&resolveErr];
        if (resolvedUrl == nil || resolveErr != nil) {
            rerbr_ios_set_persisted_folder_state(0, "");
            rerbr_ios_import_cancel_or_fail(1, "EL MARCADOR DE SEGURIDAD DE iOS CADUCÓ — SELECCIONA LA CARPETA DE NUEVO");
            return;
        }
        self.busy = YES;
        NSString *label = resolvedUrl.lastPathComponent ?: @"Carpeta RBR";
        if (stale) {
            BOOL scopedRefresh = [resolvedUrl startAccessingSecurityScopedResource];
            NSData *fresh = [resolvedUrl bookmarkDataWithOptions:0
                                  includingResourceValuesForKeys:nil
                                                   relativeToURL:nil
                                                           error:nil];
            if (fresh != nil) {
                [defaults setObject:fresh forKey:kRerbrFolderBookmarkKey];
            }
            if (scopedRefresh) {
                [resolvedUrl stopAccessingSecurityScopedResource];
            }
        }
        rerbr_ios_import_begin_batch(1, label.UTF8String ?: "Carpeta RBR");
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            @autoreleasepool {
                BOOL scoped = [resolvedUrl startAccessingSecurityScopedResource];
                NSError *coordError = nil;
                __block BOOL invoked = NO;
                NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
                [coordinator coordinateReadingItemAtURL:resolvedUrl
                                                options:0
                                                  error:&coordError
                                             byAccessor:^(NSURL *newURL) {
                    invoked = YES;
                    const char *fsPath = newURL.fileSystemRepresentation;
                    if (fsPath != NULL) {
                        rerbr_ios_import_process_path(fsPath, 1, 0, 1);
                    } else {
                        rerbr_ios_import_cancel_or_fail(1, "NO SE PUDO ABRIR LA CARPETA AUTORIZADA EN iOS");
                    }
                }];
                if (!invoked && coordError != nil) {
                    NSString *errText = [NSString stringWithFormat:@"ERROR REIMPORTANDO DESDE iOS/iCLOUD: %@",
                                         coordError.localizedDescription ?: @"No disponible"];
                    rerbr_ios_import_cancel_or_fail(1, errText.UTF8String ?: "ERROR REIMPORTANDO");
                } else {
                    rerbr_ios_import_finish_batch();
                }
                if (scoped) {
                    [resolvedUrl stopAccessingSecurityScopedResource];
                }
                dispatch_async(dispatch_get_main_queue(), ^{
                    self.busy = NO;
                });
            }
        });
    });
}

@end

int rerbr_ios_pick_rbr_folder(void) {
    [[RerbrDocumentPickerCoordinator shared] presentPickerForFolder:YES];
    return 1;
}

int rerbr_ios_pick_rbr_zip(void) {
    [[RerbrDocumentPickerCoordinator shared] presentPickerForFolder:NO];
    return 1;
}

int rerbr_ios_reimport_persisted_folder(void) {
    [[RerbrDocumentPickerCoordinator shared] reimportSavedFolder];
    return 1;
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
        [[RerbrDocumentPickerCoordinator shared] syncSavedBookmarkState];
        rerbr_probe_audio_setup();
        return rerbr_probe_main();
    }
}
