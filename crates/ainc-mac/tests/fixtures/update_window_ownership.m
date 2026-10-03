#import <AppKit/AppKit.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void ainc_update_smoke_init(void);
void ainc_update_status(const char *message, const char *current, int kind);
void ainc_update_offer(const char *version, const char *current, const char *html, bool automatic, bool ready, bool changelog);
void ainc_update_progress(const char *message, unsigned long long received, unsigned long long total);
void ainc_update_close(void);
bool ainc_update_capture(const char *path, bool progress);

static int lastAction;

char *ainc_update_format_notes(const char *markdown, const char *current, bool history) { return strdup(markdown); }
void ainc_update_free_notes(char *text) { free(text); }

void ainc_update_action(int action, bool automatic) {
    lastAction = action;
    (void)automatic;
}

static NSView *find(NSView *view, Class type, NSString *text) {
    if ([view isKindOfClass:type]) {
        if (!text) return view;
        if ([view isKindOfClass:NSButton.class] && [[(NSButton *)view title] isEqualToString:text]) return view;
        if ([view isKindOfClass:NSTextField.class] && [[(NSTextField *)view stringValue] isEqualToString:text]) return view;
    }
    for (NSView *child in view.subviews) {
        NSView *match = find(child, type, text);
        if (match) return match;
    }
    return nil;
}

static NSWindow *visibleWindow(void) {
    for (NSWindow *window in NSApp.windows) {
        if (window.visible) return window;
    }
    NSCAssert(NO, @"update window must be visible");
    return nil;
}

static void capture(const char *directory, NSString *name, bool progress) {
    if (!directory) return;
    NSString *path = [[NSString stringWithUTF8String:directory] stringByAppendingPathComponent:name];
    NSCAssert(ainc_update_capture(path.UTF8String, progress), @"capture update window");
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        const char *directory = argc == 2 ? argv[1] : NULL;
        ainc_update_smoke_init();
        NSString *iconPath = NSProcessInfo.processInfo.environment[@"AINC_UPDATE_TEST_ICON"];
        if (iconPath) NSApp.applicationIconImage = [[NSImage alloc] initWithContentsOfFile:iconPath];
        ainc_update_status("Checking for updates…", "0.5.0", 0);
        NSProgressIndicator *spinner = (NSProgressIndicator *)find(visibleWindow().contentView, NSProgressIndicator.class, nil);
        NSCAssert(spinner && spinner.indeterminate && spinner.style == NSProgressIndicatorStyleSpinning, @"checking shows a spinner");
        capture(directory, @"checking.png", false);
        ainc_update_offer("0.5.0", "0.3.5", "<html><body><h2>AgentInc 0.5.0</h2><p>• Better updates</p><h2>AgentInc 0.4.0</h2><p>• Earlier changes</p></body></html>", false, false, false);
        NSCAssert(find(visibleWindow().contentView, NSButton.class, @"Install Update"), @"offer retains install controls");
        NSScrollView *scroll = (NSScrollView *)find(visibleWindow().contentView, NSScrollView.class, nil);
        NSTextView *notes = (NSTextView *)scroll.documentView;
        NSFont *notesFont = [notes.textStorage attribute:NSFontAttributeName atIndex:0 effectiveRange:nil];
        NSCAssert([notesFont.familyName isEqualToString:[NSFont systemFontOfSize:13].familyName], @"notes use a system font rather than the HTML importer's serif fallback");
        capture(directory, @"offer.png", false);
        ainc_update_progress("Downloading update...", 1, 2);
        capture(directory, @"progress.png", true);
        ainc_update_status("You’re up to date", "0.5.0", 1);
        NSWindow *window = visibleWindow();
        NSCAssert(find(window.contentView, NSTextField.class, @"AgentInc 0.5.0 is the latest version available."), @"up-to-date alert shows installed version");
        NSButton *ok = (NSButton *)find(window.contentView, NSButton.class, @"OK");
        NSCAssert(ok, @"up-to-date alert has an OK action");
        capture(directory, @"up-to-date.png", false);
        [ok performClick:nil];
        NSCAssert(lastAction == 7 && !window.visible, @"OK dismisses without changing update preferences");
        ainc_update_status("Could not check for updates: network unavailable.", "0.5.0", 2);
        window = visibleWindow();
        NSCAssert(find(window.contentView, NSButton.class, @"Cancel"), @"failure has a cancel action");
        NSButton *retry = (NSButton *)find(window.contentView, NSButton.class, @"Retry");
        NSCAssert(retry, @"failure has a retry action");
        capture(directory, @"error.png", false);
        [retry performClick:nil];
        NSCAssert(lastAction == 6 && !window.visible, @"retry reaches the Rust updater");
        ainc_update_offer("0.5.0", "0.5.0", "<html><body><h2>AgentInc 0.5.0</h2><p>• Better updates</p><h2>AgentInc 0.4.0</h2><p>• Earlier changes</p></body></html>", false, false, true);
        window = visibleWindow();
        NSCAssert(!find(window.contentView, NSButton.class, @"Install Update"), @"changelog is read-only even when up to date");
        NSButton *done = (NSButton *)find(window.contentView, NSButton.class, @"Done");
        NSCAssert(done, @"changelog has a Done action");
        capture(directory, @"changelog.png", false);
        [done performClick:nil];
        NSCAssert(lastAction == 7 && !window.visible, @"Done closes release history");
        ainc_update_close();
    }
    puts("update window ownership passed");
    return 0;
}
