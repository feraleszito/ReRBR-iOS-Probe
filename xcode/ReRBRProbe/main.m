#import <Foundation/Foundation.h>
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
