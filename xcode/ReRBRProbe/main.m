#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <CoreMotion/CoreMotion.h>
#include <math.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
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

// Polled only on the UIKit thread. No callbacks into Rust or physics writes.
double rerbr_drive_tilt(int enabled) {
    static CMMotionManager *motion;
    if (!enabled) { [motion stopDeviceMotionUpdates]; return NAN; }
    if (motion == nil) { motion = [[CMMotionManager alloc] init]; motion.deviceMotionUpdateInterval = 1.0/120.0; }
    if (![motion isDeviceMotionAvailable]) return NAN;
    if (![motion isDeviceMotionActive]) [motion startDeviceMotionUpdates];
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
        return rerbr_probe_main();
    }
}
