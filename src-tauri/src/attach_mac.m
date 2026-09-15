// One-file picker for the companion composer chip.
// NSOpenPanel, files only, single selection (multi-file is forbidden in
// v0). Returns a malloc'd UTF-8 path (caller frees via
// orbitty_pick_file_free) or NULL on cancel. Runs on the main thread
// via lib.rs main_thread_sync.

#import <AppKit/AppKit.h>

char *orbitty_pick_file(void) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseFiles = YES;
    panel.canChooseDirectories = NO;
    panel.allowsMultipleSelection = NO;
    // The rail is a background accessory — pull the panel forward or it
    // opens behind whatever is focused.
    [NSApp activateIgnoringOtherApps:YES];
    if ([panel runModal] == NSModalResponseOK && panel.URL.isFileURL) {
        return strdup(panel.URL.fileSystemRepresentation);
    }
    return NULL;
}

void orbitty_pick_file_free(char *s) {
    free(s);
}
