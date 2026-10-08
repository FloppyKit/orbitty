#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <AppKit/AppKit.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>

typedef void (*OrbittyThumbCb)(void *ctx, int status, const char *b64, int w, int h);

static void orbitty_thumb_finish(void *ctx, OrbittyThumbCb cb, int status, NSString *b64, int w, int h) {
    if (!cb) return;
    const char *raw = b64 ? b64.UTF8String : NULL;
    cb(ctx, status, raw, w, h);
}

int orbitty_thumb_preflight(void) {
    return CGPreflightScreenCaptureAccess() ? 1 : 0;
}

int orbitty_thumb_request(void) {
    return CGRequestScreenCaptureAccess() ? 1 : 0;
}

static BOOL orbitty_thumb_sck_ok(void) {
    NSOperatingSystemVersion v14 = {14, 0, 0};
    if (![[NSProcessInfo processInfo] isOperatingSystemAtLeastVersion:v14]) return NO;
    return NSClassFromString(@"SCScreenshotManager") != nil
        && NSClassFromString(@"SCShareableContent") != nil;
}

static BOOL orbitty_thumb_is_ours(SCWindow *window) API_AVAILABLE(macos(14.0)) {
    SCRunningApplication *app = window.owningApplication;
    if (!app) return NO;
    NSString *bid = app.bundleIdentifier ?: @"";
    NSString *name = app.applicationName ?: @"";
    if ([bid rangeOfString:@"orbitty" options:NSCaseInsensitiveSearch].location != NSNotFound) return YES;
    if ([name rangeOfString:@"orbitty" options:NSCaseInsensitiveSearch].location != NSNotFound) return YES;
    return NO;
}

void orbitty_thumb_capture(uint32_t window_id, int32_t pid, const char *title, int max_px, void *ctx, OrbittyThumbCb cb) {
    if (orbitty_thumb_sck_ok()) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
#pragma clang diagnostic ignored "-Wunguarded-availability"
        if (!CGPreflightScreenCaptureAccess()) {
            orbitty_thumb_finish(ctx, cb, 1, nil, 0, 0);
            return;
        }
        NSString *wantTitle = title ? [NSString stringWithUTF8String:title] : @"";
        int cap = max_px > 0 ? max_px : 480;
        [SCShareableContent getShareableContentExcludingDesktopWindows:YES
                                                   onScreenWindowsOnly:NO
                                                     completionHandler:^(SCShareableContent *content, NSError *error) {
            if (error || !content) {
                orbitty_thumb_finish(ctx, cb, 4, nil, 0, 0);
                return;
            }
            SCWindow *found = nil;
            if (window_id != 0) {
                for (SCWindow *w in content.windows) {
                    if (orbitty_thumb_is_ours(w)) continue;
                    if (w.windowID == window_id) {
                        found = w;
                        break;
                    }
                }
            }
            if (!found && pid > 0 && wantTitle.length > 0) {
                for (SCWindow *w in content.windows) {
                    if (orbitty_thumb_is_ours(w)) continue;
                    if (w.owningApplication.processID == pid && [w.title isEqualToString:wantTitle]) {
                        found = w;
                        break;
                    }
                }
            }
            if (!found) {
                orbitty_thumb_finish(ctx, cb, 2, nil, 0, 0);
                return;
            }
            CGRect fr = found.frame;
            CGFloat longest = MAX(fr.size.width, fr.size.height);
            CGFloat scale = 1;
            if (longest > cap) scale = (CGFloat)cap / longest;
            NSInteger tw = MAX(1, (NSInteger)llround(fr.size.width * scale));
            NSInteger th = MAX(1, (NSInteger)llround(fr.size.height * scale));
            SCContentFilter *filter = [[SCContentFilter alloc] initWithDesktopIndependentWindow:found];
            SCStreamConfiguration *cfg = [SCStreamConfiguration new];
            cfg.width = tw;
            cfg.height = th;
            cfg.showsCursor = NO;
            if ([cfg respondsToSelector:@selector(setIgnoreShadowsSingleWindow:)]) {
                cfg.ignoreShadowsSingleWindow = YES;
            }
            [SCScreenshotManager captureImageWithFilter:filter
                                          configuration:cfg
                                      completionHandler:^(CGImageRef img, NSError *capErr) {
                if (capErr || !img) {
                    orbitty_thumb_finish(ctx, cb, 4, nil, 0, 0);
                    return;
                }
                NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:img];
                NSData *jpeg = [rep representationUsingType:NSBitmapImageFileTypeJPEG
                                                 properties:@{ NSImageCompressionFactor: @0.7 }];
                if (!jpeg) {
                    orbitty_thumb_finish(ctx, cb, 4, nil, 0, 0);
                    return;
                }
                NSString *b64 = [jpeg base64EncodedStringWithOptions:0];
                orbitty_thumb_finish(ctx, cb, 0, b64, (int)CGImageGetWidth(img), (int)CGImageGetHeight(img));
            }];
        }];
#pragma clang diagnostic pop
    } else {
        orbitty_thumb_finish(ctx, cb, 3, nil, 0, 0);
    }
}
