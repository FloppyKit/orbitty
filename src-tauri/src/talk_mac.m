// macOS Speech.framework Talk engine.
// Press-to-start / press-to-stop. Partials + finals via C callback.
// Segment-final restart so Listening does not die after a short utterance.
// Never always-on. Never a wake word. Never auto-send.

#import <AVFoundation/AVFoundation.h>
#import <Foundation/Foundation.h>
#import <Speech/Speech.h>

typedef void (*orbitty_talk_cb)(const char *event, const char *text);

static NSString *join_talk(NSString *a, NSString *b) {
    NSString *left = [(a ?: @"") stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceAndNewlineCharacterSet]];
    NSString *right = [(b ?: @"") stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceAndNewlineCharacterSet]];
    if (right.length == 0) return left;
    if (left.length == 0) return right;
    return [NSString stringWithFormat:@"%@ %@", left, right];
}

@interface OrbittyTalkEngine : NSObject
@property (nonatomic, assign) orbitty_talk_cb cb;
@property (nonatomic, assign) BOOL running;
@property (nonatomic, strong) SFSpeechRecognizer *recognizer;
@property (nonatomic, strong) AVAudioEngine *engine;
@property (nonatomic, strong) SFSpeechAudioBufferRecognitionRequest *request;
@property (nonatomic, strong) SFSpeechRecognitionTask *task;
@property (nonatomic, copy) NSString *committed;
@property (nonatomic, copy) NSString *lastPartial;
- (int)startWithCb:(orbitty_talk_cb)cb;
- (void)stop;
@end

@implementation OrbittyTalkEngine

- (void)emit:(const char *)event text:(NSString *)text {
    if (!self.cb) return;
    const char *bytes = (text ?: @"").UTF8String;
    self.cb(event, bytes ? bytes : "");
}

- (NSString *)spoken {
    return join_talk(self.committed, self.lastPartial);
}

- (void)startTask {
    if (!self.running || !self.recognizer) return;
    SFSpeechAudioBufferRecognitionRequest *req = [[SFSpeechAudioBufferRecognitionRequest alloc] init];
    req.shouldReportPartialResults = YES;
    req.taskHint = SFSpeechRecognitionTaskHintDictation;
    // addsPunctuation is macOS 13+. respondsToSelector, not @available:
    // @available emits ___isPlatformVersionAtLeast (compiler-rt), which the
    // rustc-driven -nodefaultlibs link does not pull in.
    if ([req respondsToSelector:@selector(setAddsPunctuation:)]) {
        req.addsPunctuation = YES;
    }
    self.request = req;
    __weak OrbittyTalkEngine *weak = self;
    self.task = [self.recognizer recognitionTaskWithRequest:req
                                              resultHandler:^(SFSpeechRecognitionResult *result, NSError *error) {
        OrbittyTalkEngine *self = weak;
        if (!self || !self.running) return;
        if (self.request != req) return;
        if (result) {
            NSString *piece = result.bestTranscription.formattedString ?: @"";
            if (result.isFinal) {
                self.committed = join_talk(self.committed, piece);
                self.lastPartial = @"";
                [self emit:"partial" text:self.committed];
                [self restartTask];
            } else {
                self.lastPartial = piece;
                [self emit:"partial" text:[self spoken]];
            }
        }
        if (error && self.running && self.request == req) {
            // Cancel of the task we are replacing, silence, or a short
            // segment end — keep Listening. Permission failures are
            // caught in wait_auth before the engine starts.
            if (error.code == 203) return;
            [self restartTask];
        }
    }];
}

- (void)restartTask {
    if (!self.running) return;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(80 * NSEC_PER_MSEC)),
                   dispatch_get_main_queue(), ^{
        if (!self.running) return;
        SFSpeechRecognitionTask *old = self.task;
        self.task = nil;
        self.request = nil;
        [old cancel];
        [self startTask];
    });
}

- (void)teardownAudio {
    if (self.engine) {
        [self.engine stop];
        @try {
            [self.engine.inputNode removeTapOnBus:0];
        } @catch (__unused NSException *ex) {
        }
    }
    [self.task cancel];
    self.task = nil;
    [self.request endAudio];
    self.request = nil;
}

- (int)startWithCb:(orbitty_talk_cb)cb {
    if (self.running) {
        self.cb = cb;
        return 0;
    }
    SFSpeechRecognizer *rec = [[SFSpeechRecognizer alloc] init];
    if (!rec) {
        rec = [[SFSpeechRecognizer alloc] initWithLocale:[NSLocale localeWithLocaleIdentifier:@"en-US"]];
    }
    if (!rec || !rec.isAvailable) return 2;

    self.cb = cb;
    self.recognizer = rec;
    self.committed = @"";
    self.lastPartial = @"";
    self.running = YES;

    AVAudioEngine *engine = [[AVAudioEngine alloc] init];
    AVAudioInputNode *input = engine.inputNode;
    AVAudioFormat *format = [input outputFormatForBus:0];
    if (!format || format.channelCount == 0) {
        self.running = NO;
        return 3;
    }
    __weak OrbittyTalkEngine *weak = self;
    [input installTapOnBus:0
                bufferSize:1024
                    format:format
                     block:^(AVAudioPCMBuffer *buffer, __unused AVAudioTime *when) {
        OrbittyTalkEngine *self = weak;
        SFSpeechAudioBufferRecognitionRequest *req = self.request;
        if (self.running && req) {
            [req appendAudioPCMBuffer:buffer];
        }
    }];
    [engine prepare];
    NSError *err = nil;
    if (![engine startAndReturnError:&err]) {
        @try {
            [input removeTapOnBus:0];
        } @catch (__unused NSException *ex) {
        }
        self.running = NO;
        self.engine = nil;
        return 3;
    }
    self.engine = engine;
    [self startTask];
    return 0;
}

- (void)stop {
    if (!self.running) return;
    self.running = NO;
    NSString *finalText = [self spoken];
    [self teardownAudio];
    self.engine = nil;
    self.recognizer = nil;
    [self emit:"ended" text:finalText];
    self.committed = @"";
    self.lastPartial = @"";
}

@end

static OrbittyTalkEngine *g_engine(void) {
    static OrbittyTalkEngine *engine;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        engine = [[OrbittyTalkEngine alloc] init];
    });
    return engine;
}

static BOOL auth_denied(void) {
    SFSpeechRecognizerAuthorizationStatus speech = [SFSpeechRecognizer authorizationStatus];
    AVAuthorizationStatus mic = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
    return speech == SFSpeechRecognizerAuthorizationStatusDenied ||
           speech == SFSpeechRecognizerAuthorizationStatusRestricted ||
           mic == AVAuthorizationStatusDenied ||
           mic == AVAuthorizationStatusRestricted;
}

static BOOL auth_ready(void) {
    return [SFSpeechRecognizer authorizationStatus] == SFSpeechRecognizerAuthorizationStatusAuthorized &&
           [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] == AVAuthorizationStatusAuthorized;
}

// Call from a background thread only — the permission sheet needs the
// main run loop, and we wait on a semaphore for the answer.
static int wait_auth(void) {
    if (auth_denied()) return 1;
    if (auth_ready()) return 0;

    dispatch_semaphore_t sem = dispatch_semaphore_create(0);
    __block int denied = 0;
    dispatch_async(dispatch_get_main_queue(), ^{
        void (^finish)(int) = ^(int d) {
            denied = d;
            dispatch_semaphore_signal(sem);
        };
        void (^ask_mic)(void) = ^{
            AVAuthorizationStatus now = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
            if (now == AVAuthorizationStatusAuthorized) {
                finish(0);
                return;
            }
            if (now == AVAuthorizationStatusDenied || now == AVAuthorizationStatusRestricted) {
                finish(1);
                return;
            }
            [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio
                                     completionHandler:^(BOOL granted) {
                finish(granted ? 0 : 1);
            }];
        };
        SFSpeechRecognizerAuthorizationStatus speech = [SFSpeechRecognizer authorizationStatus];
        if (speech != SFSpeechRecognizerAuthorizationStatusAuthorized) {
            [SFSpeechRecognizer requestAuthorization:^(SFSpeechRecognizerAuthorizationStatus status) {
                if (status != SFSpeechRecognizerAuthorizationStatusAuthorized) {
                    finish(1);
                    return;
                }
                ask_mic();
            }];
            return;
        }
        ask_mic();
    });
    dispatch_semaphore_wait(sem, DISPATCH_TIME_FOREVER);
    return denied;
}

int orbitty_talk_engine_start(orbitty_talk_cb cb) {
    if (!cb) return 3;
    // wait_auth blocks on the permission sheet. Never do that on the
    // main thread (the sheet callback needs the run loop).
    if ([NSThread isMainThread]) {
        if (auth_denied()) return 1;
        if (!auth_ready()) {
            dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
                (void)wait_auth();
            });
            return 4;
        }
        return [g_engine() startWithCb:cb];
    }
    int auth = wait_auth();
    if (auth != 0) return 1;
    __block int rc = 3;
    dispatch_sync(dispatch_get_main_queue(), ^{
        rc = [g_engine() startWithCb:cb];
    });
    return rc;
}

void orbitty_talk_engine_stop(void) {
    void (^work)(void) = ^{
        [g_engine() stop];
    };
    if ([NSThread isMainThread]) {
        work();
    } else {
        dispatch_sync(dispatch_get_main_queue(), work);
    }
}

int orbitty_talk_engine_running(void) {
    return g_engine().running ? 1 : 0;
}
