// Catch NSException that would otherwise abort through Rust frames.
// catch_unwind does not see ObjC exceptions.

#import <Foundation/Foundation.h>

typedef void (*orbitty_try_fn)(void *);

int orbitty_objc_try(orbitty_try_fn fn, void *ctx) {
    @try {
        fn(ctx);
        return 1;
    } @catch (NSException *ex) {
        NSLog(@"orbitty: swallowed %@ %@", ex.name, ex.reason);
        return 0;
    }
}
