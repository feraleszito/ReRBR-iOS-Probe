#import <WebKit/WebKit.h>
#import <Security/Security.h>

@interface RBRBoard : NSObject <WKScriptMessageHandlerWithReply, WKNavigationDelegate>
@property(nonatomic,strong) WKWebView *web;
@property(nonatomic,strong) UIViewController *panel;
@property(nonatomic,strong) NSURL *entry;
@end
@implementation RBRBoard
- (instancetype)init {
    if ((self = [super init])) {
        WKWebViewConfiguration *config = [WKWebViewConfiguration new];
        config.websiteDataStore = WKWebsiteDataStore.nonPersistentDataStore;
        [config.userContentController addScriptMessageHandlerWithReply:self contentWorld:WKContentWorld.pageWorld name:@"board"];
        _web = [[WKWebView alloc] initWithFrame:CGRectZero configuration:config];
        _web.navigationDelegate = self;
        _entry = [[NSBundle.mainBundle.resourceURL URLByAppendingPathComponent:@"leaderboard"] URLByAppendingPathComponent:@"index.html"];
        [_web loadFileURL:_entry allowingReadAccessToURL:_entry.URLByDeletingLastPathComponent];
    }
    return self;
}
- (void)webView:(WKWebView *)view decidePolicyForNavigationAction:(WKNavigationAction *)action decisionHandler:(void (^)(WKNavigationActionPolicy))decide {
    decide(action.targetFrame.isMainFrame && [action.request.URL.path isEqual:self.entry.path] && action.request.URL.isFileURL ? WKNavigationActionPolicyAllow : WKNavigationActionPolicyCancel);
}
- (void)webView:(WKWebView *)web didFinishNavigation:(WKNavigation *)navigation {
    if(self.panel.presentingViewController) [web evaluateJavaScript:@"window.boardOpened()" completionHandler:nil];
}
- (void)show {
    UIViewController *parent = rerbr_key_window().rootViewController;
    while(parent.presentedViewController) parent = parent.presentedViewController;
    if (!parent || parent == self.panel) return;
    self.panel = [UIViewController new];
    self.panel.view = self.web;
    self.panel.modalPresentationStyle = UIModalPresentationFullScreen;
    [parent presentViewController:self.panel animated:YES completion:^{
        [self.web evaluateJavaScript:@"window.boardOpened()" completionHandler:nil];
    }];
}
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)msg
                 replyHandler:(void (^)(id, NSString *))reply {
    if (!msg.frameInfo.mainFrame || ![self.web.URL.path isEqual:self.entry.path] || ![msg.body isKindOfClass:NSDictionary.class]) { reply(nil,@"Invalid origin"); return; }
    NSString *op=msg.body[@"op"], *value=msg.body[@"value"];
    if (![op isKindOfClass:NSString.class] || ![value isKindOfClass:NSString.class]) { reply(nil,@"Invalid message"); return; }
    if ([op isEqual:@"close"]) { [self.panel dismissViewControllerAnimated:YES completion:nil]; reply(@"ok",nil); return; }
    if ([op isEqual:@"session"] || [op isEqual:@"persist"]) {
        NSMutableDictionary *q=[@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,
          (__bridge id)kSecAttrService:@"org.rerbr.online-board",(__bridge id)kSecAttrAccount:@"session-v1"} mutableCopy];
        if ([op isEqual:@"session"]) {
            q[(__bridge id)kSecReturnData]=@YES;
            CFTypeRef out=NULL;
            OSStatus status=SecItemCopyMatching((__bridge CFDictionaryRef)q,&out);
            NSData *data=CFBridgingRelease(out);
            if(status==errSecItemNotFound) reply(@"",nil);
            else if(status==errSecSuccess) reply([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding],nil);
            else reply(nil,@"Keychain unavailable");
        } else {
            NSDictionary *attrs=@{(__bridge id)kSecValueData:[value dataUsingEncoding:NSUTF8StringEncoding],
              (__bridge id)kSecAttrAccessible:(__bridge id)kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly};
            OSStatus status=SecItemUpdate((__bridge CFDictionaryRef)q,(__bridge CFDictionaryRef)attrs);
            if(status==errSecItemNotFound) { [q addEntriesFromDictionary:attrs]; status=SecItemAdd((__bridge CFDictionaryRef)q,NULL); }
            reply(status==errSecSuccess ? @"ok":nil,status==errSecSuccess ? nil:@"Keychain unavailable");
        }
        return;
    }
    NSFileManager *fs=NSFileManager.defaultManager;
    NSURL *root=[[[fs URLsForDirectory:NSDocumentDirectory inDomains:NSUserDomainMask] firstObject] URLByAppendingPathComponent:@"RBR/online-board"];
    NSURL *pending=[root URLByAppendingPathComponent:@"pending"];
    if ([op isEqual:@"selection"]) {
        reply([NSString stringWithContentsOfURL:[root URLByAppendingPathComponent:@"selection.json"] encoding:NSUTF8StringEncoding error:nil] ?: @"{}",nil);
    } else if ([op isEqual:@"pending"]) {
        NSMutableArray *rows=[NSMutableArray array];
        for (NSURL *f in [fs contentsOfDirectoryAtURL:pending includingPropertiesForKeys:nil options:0 error:nil]) {
            if(![f.pathExtension isEqual:@"json"]) continue;
            NSData *data=[NSData dataWithContentsOfURL:f];
            id obj=data && data.length<=8192 ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
            if([obj isKindOfClass:NSDictionary.class]) [rows addObject:obj];
        }
        reply([[NSString alloc] initWithData:[NSJSONSerialization dataWithJSONObject:rows options:0 error:nil] encoding:NSUTF8StringEncoding],nil);
    } else if ([op isEqual:@"ack"] && [value rangeOfString:@"^lb1-[0-9a-f]{20}$" options:NSRegularExpressionSearch].length==24 && value.length==24) {
        NSURL *sent=[root URLByAppendingPathComponent:@"sent"];
        [fs createDirectoryAtURL:sent withIntermediateDirectories:YES attributes:nil error:nil];
        NSString *file=[value stringByAppendingPathExtension:@"json"];
        BOOL ok=[fs moveItemAtURL:[pending URLByAppendingPathComponent:file] toURL:[sent URLByAppendingPathComponent:file] error:nil];
        reply(ok ? @"ok":nil,ok ? nil:@"Receipt write failed");
    } else reply(nil,@"Unknown board operation");
}
@end
static RBRBoard *rbr_online_board(void) {
    static RBRBoard *board;
    if (!board) board=[RBRBoard new];
    return board;
}
void rerbr_show_board(void) { dispatch_async(dispatch_get_main_queue(), ^{ [rbr_online_board() show]; }); }
