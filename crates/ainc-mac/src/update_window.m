#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#import <CommonCrypto/CommonDigest.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#import "SPUUpdater.h"
#import "SPUUpdaterDelegate.h"
#import "SPUUserDriver.h"
#import "SUAppcast.h"
#import "SUAppcastItem.h"
#import "SUUpdatePermissionResponse.h"
#import "SPUDownloadData.h"
#import "SUStandardVersionComparator.h"
#import "SUErrors.h"

extern void ainc_update_action(int action, bool automatic);
static void updateAction(int action, bool automatic);

@interface AincUpdateUI : NSObject <NSWindowDelegate>
@property(strong) NSWindow *offer;
@property(strong) NSAlert *alert;
@property(strong) NSWindow *progress;
@property(strong) NSButton *automatic;
@property(strong) NSProgressIndicator *bar;
@property(strong) NSTextField *bytes;
@property(strong) NSTextField *phase;
@property(strong) NSButton *cancelButton;
#ifdef AINC_UPGRADE_TEST
@property(strong) NSButton *installButton;
#endif
@end

@implementation AincUpdateUI
- (void)skip:(id)sender { [self.offer close]; self.offer = nil; updateAction(1, self.automatic.state == NSControlStateValueOn); }
- (void)later:(id)sender { [self.offer close]; self.offer = nil; updateAction(2, self.automatic.state == NSControlStateValueOn); }
- (void)install:(id)sender { [self.offer close]; self.offer = nil; updateAction(3, self.automatic.state == NSControlStateValueOn); }
- (void)cancel:(id)sender { [self.progress close]; self.progress = nil; updateAction(4, false); }
- (void)automaticChanged:(id)sender { updateAction(5, self.automatic.state == NSControlStateValueOn); }
- (void)dismiss:(id)sender { [self.offer close]; self.offer = nil; self.alert = nil; updateAction(7, false); }
- (void)retry:(id)sender { [self.offer close]; self.offer = nil; self.alert = nil; updateAction(6, false); }
- (BOOL)windowShouldClose:(NSWindow *)sender { updateAction(7, false); return YES; }
@end

static AincUpdateUI *ui(void) {
    static AincUpdateUI *shared;
    if (!shared) shared = [AincUpdateUI new];
    return shared;
}

static NSTextField *label(NSString *text, NSRect frame, NSFont *font) {
    NSTextField *field = [NSTextField labelWithString:text];
    field.frame = frame;
    field.font = font;
    field.lineBreakMode = NSLineBreakByWordWrapping;
    field.maximumNumberOfLines = 0;
    return field;
}

static NSButton *button(NSString *title, NSRect frame, id target, SEL action) {
    NSButton *result = [[NSButton alloc] initWithFrame:frame];
    result.title = title;
    result.bezelStyle = NSBezelStyleRounded;
    result.target = target;
    result.action = action;
    return result;
}

static NSWindow *window(NSSize size, NSString *title, BOOL closable) {
    NSRect rect = NSMakeRect(0, 0, size.width, size.height);
    NSWindow *result = [[NSWindow alloc] initWithContentRect:rect
        styleMask:NSWindowStyleMaskTitled | (closable ? NSWindowStyleMaskClosable : 0)
        backing:NSBackingStoreBuffered defer:NO];
    // AincUpdateUI owns each window through a strong property. Closing must not
    // release it a second time before that property is cleared or replaced.
    result.releasedWhenClosed = NO;
    result.title = title;
    [result center];
    return result;
}

void ainc_update_offer(const char *version, const char *current, const char *html, bool automatic, bool ready, bool changelog) {
    AincUpdateUI *state = ui();
    [state.offer close];
    state.alert = nil;
    [state.progress close];
    state.progress = nil;
    NSString *next = [NSString stringWithUTF8String:version];
    NSString *installed = [NSString stringWithUTF8String:current];
    state.offer = window(NSMakeSize(620, 450), changelog ? @"Release Notes" : @"Software Update", NO);
    NSView *content = state.offer.contentView;
    NSImageView *icon = [[NSImageView alloc] initWithFrame:NSMakeRect(26, 356, 64, 64)];
    icon.image = NSApp.applicationIconImage;
    [content addSubview:icon];
    [content addSubview:label(changelog ? @"AgentInc release history" : @"A new version of AgentInc is available!", NSMakeRect(108, 389, 486, 30), [NSFont boldSystemFontOfSize:17])];
    NSString *question = ready ? @"Would you like to install it now?" : @"Would you like to download it now?";
    NSString *description = changelog
        ? [NSString stringWithFormat:@"All published releases.\nYou have AgentInc %@.", installed]
        : [NSString stringWithFormat:@"AgentInc %@ is now available—you have %@.\n%@", next, installed, question];
    [content addSubview:label(description, NSMakeRect(108, 348, 480, 40), [NSFont systemFontOfSize:13])];

    CGFloat notesBottom = changelog ? 64 : 94;
    NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:NSMakeRect(26, notesBottom, 568, 338 - notesBottom)];
    scroll.hasVerticalScroller = YES;
    scroll.borderType = NSBezelBorder;
    NSTextView *notes = [[NSTextView alloc] initWithFrame:NSMakeRect(0, 0, 568, 244)];
    notes.editable = NO;
    notes.selectable = YES;
    notes.drawsBackground = YES;
    notes.backgroundColor = NSColor.textBackgroundColor;
    notes.textContainerInset = NSMakeSize(12, 10);
    NSData *data = [[NSString stringWithUTF8String:html] dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *options = @{NSDocumentTypeDocumentAttribute: NSHTMLTextDocumentType,
                              NSCharacterEncodingDocumentAttribute: @(NSUTF8StringEncoding)};
    NSAttributedString *formatted = [[NSAttributedString alloc] initWithData:data options:options documentAttributes:nil error:nil];
    [notes.textStorage setAttributedString:formatted ?: [[NSAttributedString alloc] initWithString:@"Release notes unavailable"]];
    NSRange all = NSMakeRange(0, notes.textStorage.length);
    // AppKit's HTML importer falls back to Times for CSS system-font aliases.
    // Use real system fonts while retaining heading sizes, emphasis and code styling.
    [notes.textStorage enumerateAttribute:NSFontAttributeName inRange:all options:0 usingBlock:^(id value, NSRange range, BOOL *stop) {
        NSFont *font = value ?: [NSFont systemFontOfSize:13];
        NSFontDescriptorSymbolicTraits traits = font.fontDescriptor.symbolicTraits;
        NSFont *base = (traits & NSFontDescriptorTraitMonoSpace)
            ? [NSFont monospacedSystemFontOfSize:font.pointSize weight:NSFontWeightRegular]
            : [NSFont systemFontOfSize:font.pointSize];
        NSFontDescriptor *descriptor = [base.fontDescriptor fontDescriptorWithSymbolicTraits:traits & (NSFontDescriptorTraitBold | NSFontDescriptorTraitItalic | NSFontDescriptorTraitMonoSpace)];
        NSFont *system = [NSFont fontWithDescriptor:descriptor size:font.pointSize] ?: base;
        [notes.textStorage addAttribute:NSFontAttributeName value:system range:range];
        (void)stop;
    }];
    [notes.textStorage addAttribute:NSForegroundColorAttributeName value:NSColor.labelColor range:all];
    [notes.textStorage enumerateAttribute:NSLinkAttributeName inRange:all options:0 usingBlock:^(id value, NSRange range, BOOL *stop) {
        if (value) [notes.textStorage addAttribute:NSForegroundColorAttributeName value:NSColor.linkColor range:range];
        (void)stop;
    }];
    notes.verticallyResizable = YES;
    notes.textContainer.widthTracksTextView = YES;
    scroll.documentView = notes;
    [content addSubview:scroll];

    if (changelog) {
        NSButton *done = button(@"Done", NSMakeRect(494, 19, 100, 30), state, @selector(dismiss:));
        done.keyEquivalent = @"\r";
        [content addSubview:done];
        [state.offer makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
        return;
    }

    state.automatic = [NSButton checkboxWithTitle:@"Automatically download updates in the future" target:state action:@selector(automaticChanged:)];
    state.automatic.frame = NSMakeRect(27, 60, 450, 24);
    state.automatic.state = automatic ? NSControlStateValueOn : NSControlStateValueOff;
    [content addSubview:state.automatic];
    [content addSubview:button(@"Skip This Version", NSMakeRect(26, 19, 145, 30), state, @selector(skip:))];
    [content addSubview:button(@"Remind Me Later", NSMakeRect(294, 19, 137, 30), state, @selector(later:))];
    NSButton *install = button(@"Install Update", NSMakeRect(443, 19, 151, 30), state, @selector(install:));
    install.keyEquivalent = @"\r";
    install.bezelColor = NSColor.controlAccentColor;
    install.contentTintColor = NSColor.whiteColor;
    [content addSubview:install];
#ifdef AINC_UPGRADE_TEST
    state.installButton = install;
#endif
    [state.offer makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

#ifdef AINC_UPGRADE_TEST
void ainc_update_test_click_install(void) {
    AincUpdateUI *state = ui();
    NSCAssert(state.offer && state.installButton, @"upgrade offer must be visible");
    [state.offer displayIfNeeded];
    [state.installButton performClick:nil];
}
#endif

// 0: checking, 1: up to date, 2: retryable error, 3: informational.
void ainc_update_status(const char *message, const char *current, int kind) {
    AincUpdateUI *state = ui();
    [state.offer close];
    [state.progress close]; state.progress = nil;
    state.alert = nil;
    NSString *text = [NSString stringWithUTF8String:message];
    NSString *installed = [NSString stringWithUTF8String:current];
    if (kind == 0) {
        state.offer = window(NSMakeSize(360, 112), @"Software Update", YES);
        state.offer.delegate = state;
        NSView *content = state.offer.contentView;
        NSImageView *icon = [[NSImageView alloc] initWithFrame:NSMakeRect(24, 32, 48, 48)];
        icon.image = NSApp.applicationIconImage;
        [content addSubview:icon];
        [content addSubview:label(text, NSMakeRect(92, 58, 244, 22), [NSFont boldSystemFontOfSize:13])];
        NSProgressIndicator *spinner = [[NSProgressIndicator alloc] initWithFrame:NSMakeRect(92, 32, 16, 16)];
        spinner.style = NSProgressIndicatorStyleSpinning;
        spinner.indeterminate = YES;
        [spinner startAnimation:nil];
        [content addSubview:spinner];
        [content addSubview:label(@"Looking for a newer version…", NSMakeRect(116, 30, 220, 20), [NSFont systemFontOfSize:12])];
    } else {
        state.alert = [NSAlert new];
        state.alert.alertStyle = kind == 2 ? NSAlertStyleWarning : NSAlertStyleInformational;
        state.alert.icon = NSApp.applicationIconImage;
        state.alert.messageText = kind == 1 ? @"You’re up to date" : (kind == 2 ? @"Couldn’t update AgentInc" : @"Software Update");
        state.alert.informativeText = kind == 1
            ? [NSString stringWithFormat:@"AgentInc %@ is the latest version available.", installed]
            : text;
        NSButton *primary = [state.alert addButtonWithTitle:kind == 2 ? @"Retry" : @"OK"];
        primary.target = state;
        primary.action = kind == 2 ? @selector(retry:) : @selector(dismiss:);
        if (kind == 2) {
            NSButton *cancel = [state.alert addButtonWithTitle:@"Cancel"];
            cancel.target = state;
            cancel.action = @selector(dismiss:);
        }
        [state.alert layout];
        state.offer = state.alert.window;
        state.offer.releasedWhenClosed = NO;
        state.offer.title = @"Software Update";
        [state.offer center];
    }
    [state.offer makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

void ainc_update_progress(const char *message, unsigned long long received, unsigned long long total) {
    AincUpdateUI *state = ui();
    [state.offer close]; state.offer = nil;
    state.alert = nil;
    if (!state.progress) {
        state.progress = window(NSMakeSize(470, 180), @"Updating AgentInc", NO);
        NSView *content = state.progress.contentView;
        NSImageView *icon = [[NSImageView alloc] initWithFrame:NSMakeRect(24, 92, 58, 58)];
        icon.image = NSApp.applicationIconImage;
        [content addSubview:icon];
        [content addSubview:label(@"Updating AgentInc", NSMakeRect(98, 122, 350, 28), [NSFont boldSystemFontOfSize:17])];
        state.phase = label(@"", NSMakeRect(98, 94, 350, 24), [NSFont systemFontOfSize:13]);
        [content addSubview:state.phase];
        state.bar = [[NSProgressIndicator alloc] initWithFrame:NSMakeRect(24, 72, 422, 16)];
        state.bar.indeterminate = NO;
        state.bar.minValue = 0; state.bar.maxValue = 1;
        [content addSubview:state.bar];
        state.bytes = label(@"", NSMakeRect(25, 39, 320, 22), [NSFont systemFontOfSize:12]);
        [content addSubview:state.bytes];
        state.cancelButton = button(@"Cancel", NSMakeRect(354, 20, 92, 30), state, @selector(cancel:));
        [content addSubview:state.cancelButton];
        [state.progress makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
    }
    state.phase.stringValue = [NSString stringWithUTF8String:message];
    state.bar.indeterminate = total == 0;
    if (total == 0) [state.bar startAnimation:nil];
    else [state.bar stopAnimation:nil];
    state.bar.doubleValue = total ? MIN(1.0, (double)received / (double)total) : 0;
    state.bytes.stringValue = [NSString stringWithFormat:@"%.1f MB of %.1f MB", received / 1000000.0, total / 1000000.0];
}

void ainc_update_close(void) {
    [ui().offer close]; ui().offer = nil;
    [ui().progress close]; ui().progress = nil;
    ui().alert = nil;
}

bool ainc_update_capture(const char *path, bool progress) {
    NSWindow *window = progress ? ui().progress : ui().offer;
    if (!window) return false;
    [window displayIfNeeded];
    [NSApp updateWindows];
    NSView *view = window.contentView;
    [view displayIfNeeded];
    NSBitmapImageRep *image = [view bitmapImageRepForCachingDisplayInRect:view.bounds];
    [window.effectiveAppearance performAsCurrentDrawingAppearance:^{
        NSGraphicsContext *context = [NSGraphicsContext graphicsContextWithBitmapImageRep:image];
        [NSGraphicsContext saveGraphicsState];
        [NSGraphicsContext setCurrentContext:context];
        [NSColor.windowBackgroundColor setFill];
        NSRectFill(view.bounds);
        [NSGraphicsContext restoreGraphicsState];
        [view cacheDisplayInRect:view.bounds toBitmapImageRep:image];
    }];
    NSData *png = [image representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    return [png writeToFile:[NSString stringWithUTF8String:path] atomically:YES];
}

void ainc_update_smoke_init(void) {
    [NSApplication sharedApplication];
    [NSApp finishLaunching];
}

// Sparkle owns the update state machine, downloads, verification and installer.
// This driver owns only presentation, replies, and the app's shutdown barrier.
@interface AincSparkleDriver : NSObject <SPUUserDriver, SPUUpdaterDelegate>
@property(strong) SPUUpdater *updater;
@property(strong) SUAppcastItem *item;
@property(copy) NSString *message;
@property(copy) NSString *notes;
@property(copy) NSString *history;
@property(copy) NSString *current;
@property(copy) void (^choice)(SPUUserUpdateChoice);
@property(copy) void (^cancellation)(void);
@property(copy) void (^acknowledgement)(void);
@property(copy) void (^continuation)(void);
@property(copy) void (^retryTermination)(void);
@property BOOL ready;
@property BOOL installArmed;
@property BOOL preparing;
@property BOOL prepared;
@property BOOL terminationWaiting;
@property BOOL retryQuit;
@property BOOL preparationFailed;
@property BOOL changelog;
@property BOOL historyRequested;
@property BOOL historyFailed;
@property BOOL started;
@property(strong) NSUserDefaults *defaults;
@property BOOL backgroundDownload;
@property BOOL userVisible;
@property BOOL installRequested;
@property BOOL cancelBackgroundOnQuit;
@property(strong) NSTimer *reminder;
@property uint64_t received;
@property uint64_t expected;
- (void)action:(int)action automatic:(BOOL)automatic;
- (void)presentOffer;
- (void)prepare;
- (void)preparedWithError:(NSString *)error;
- (void)cancelBackgroundInstallation;
- (void)requestHistory;
@end

static AincSparkleDriver *sparkle;
static IMP originalShouldTerminate;

static NSApplicationTerminateReply shouldTerminate(id self, SEL selector, NSApplication *application) {
    NSApplicationTerminateReply original = originalShouldTerminate
        ? ((NSApplicationTerminateReply (*)(id, SEL, NSApplication *))originalShouldTerminate)(self, selector, application)
        : NSTerminateNow;
    if (original != NSTerminateNow) return original;
    if (sparkle.backgroundDownload && !sparkle.installRequested) {
        // Download-only means quitting must not silently consent to installation.
        // Wait for Sparkle to acknowledge cancellation before the host can exit.
        sparkle.cancelBackgroundOnQuit = YES;
        dispatch_async(dispatch_get_main_queue(), ^{ [sparkle cancelBackgroundInstallation]; });
        return NSTerminateLater;
    }
    if (!sparkle.installArmed || sparkle.prepared) {
        return NSTerminateNow;
    }
    // Keep GPUI's actual delegate in place: GPUI accesses its ivars on shutdown.
    // AppKit will resume termination only after Rust flushes and drains off-thread.
    sparkle.terminationWaiting = YES;
    [sparkle prepare];
    return NSTerminateLater;
}

static void installTerminationBarrier(void) {
    Class delegate = object_getClass(NSApp.delegate);
    SEL selector = @selector(applicationShouldTerminate:);
    Method method = class_getInstanceMethod(delegate, selector);
    originalShouldTerminate = method ? method_getImplementation(method) : NULL;
    if (!class_addMethod(delegate, selector, (IMP)shouldTerminate, "Q@:@")) {
        method_setImplementation(class_getInstanceMethod(delegate, selector), (IMP)shouldTerminate);
    }
}

static NSString *escaped(NSString *text) {
    return [[[text stringByReplacingOccurrencesOfString:@"&" withString:@"&amp;"]
        stringByReplacingOccurrencesOfString:@"<" withString:@"&lt;"]
        stringByReplacingOccurrencesOfString:@">" withString:@"&gt;"];
}

static NSString *itemNotes(SUAppcastItem *item) {
    NSString *description = item.itemDescription ?: @"Release notes unavailable.";
    return [item.itemDescriptionFormat isEqualToString:@"plain-text"]
        ? [NSString stringWithFormat:@"<p>%@</p>", escaped(description)] : description;
}

// Sparkle 2.9.6 relaunches through NSWorkspace.openURL without forwarding the
// host environment. Keep only documented profile/companion configuration,
// never test feeds/keys or the rest of the caller's process environment.
static NSArray<NSString *> *profileEnvironmentKeys(void) {
    return @[@"AGENTINC_SESSION_PATH", @"AINC_DISCOVERY_FILE", @"AINC_DAEMON_URL",
        @"AINC_TOKEN_FILE", @"AINC_DATABASE_URL", @"DATABASE_URL", @"AINC_LEGACY_DIR",
        @"AGENTINC_CODEX_HOME", @"AGENTINC_CODEX_PATH", @"AINC_RUNTIME_CONFIG",
        @"AINC_WORKSPACE_DIR", @"AINC_TOOL_ALLOW"];
}

static NSURL *relaunchProfileURL(NSBundle *bundle) {
    if (!bundle.bundleIdentifier.length) return nil; // Unbundled builds/tests.
    NSData *path = [bundle.bundlePath.stringByResolvingSymlinksInPath dataUsingEncoding:NSUTF8StringEncoding];
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256(path.bytes, (CC_LONG)path.length, digest);
    NSMutableString *name = [NSMutableString string];
    for (NSUInteger i = 0; i < sizeof(digest); i++) [name appendFormat:@"%02x", digest[i]];
    NSURL *support = [NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask].firstObject;
    NSURL *directory = [[support URLByAppendingPathComponent:bundle.bundleIdentifier isDirectory:YES]
        URLByAppendingPathComponent:@"Updater Relaunch" isDirectory:YES];
    return [directory URLByAppendingPathComponent:[name stringByAppendingPathExtension:@"plist"]];
}

static NSString *saveRelaunchProfile(NSURL *url, NSString *targetVersion) {
    if (!url) return nil;
    NSMutableDictionary *environment = [NSMutableDictionary dictionary];
    for (NSString *key in profileEnvironmentKeys()) {
        const char *value = getenv(key.UTF8String);
        if (value) {
            NSString *text = [NSString stringWithUTF8String:value];
            if (!text) return @"An update profile override is not valid UTF-8.";
            environment[key] = text;
        }
    }
    NSError *error = nil;
    NSData *data = [NSPropertyListSerialization dataWithPropertyList:@{@"version":targetVersion ?: @"", @"environment":environment}
        format:NSPropertyListBinaryFormat_v1_0 options:0 error:&error];
    if (!data) return error.localizedDescription;
    if (![NSFileManager.defaultManager createDirectoryAtURL:url.URLByDeletingLastPathComponent withIntermediateDirectories:YES
        attributes:@{NSFilePosixPermissions:@0700} error:&error]) return error.localizedDescription;
    char *temporary = strdup([[url.path stringByAppendingString:@".XXXXXX"] fileSystemRepresentation]);
    if (!temporary) return @"Could not save the update relaunch profile.";
    int fd = mkstemp(temporary); // Creates the potentially credential-bearing file mode 0600.
    if (fd < 0) { free(temporary); return @"Could not save the update relaunch profile."; }
    const uint8_t *bytes = data.bytes;
    NSUInteger remaining = data.length;
    BOOL success = YES;
    while (remaining) {
        ssize_t written = write(fd, bytes, remaining);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) { success = NO; break; }
        bytes += written;
        remaining -= (NSUInteger)written;
    }
    if (success && fsync(fd) != 0) success = NO;
    if (close(fd) != 0) success = NO;
    if (success && rename(temporary, url.fileSystemRepresentation) != 0) success = NO;
    if (!success) unlink(temporary);
    free(temporary);
    return success ? nil : @"Could not save the update relaunch profile.";
}

static NSString *restoreRelaunchProfile(NSURL *url, NSString *version) {
    if (!url) return nil;
    int fd = open(url.fileSystemRepresentation, O_RDONLY | O_NOFOLLOW);
    if (fd < 0) return errno == ENOENT ? nil : @"Could not read the update relaunch profile.";
    struct stat attributes;
    if (fstat(fd, &attributes) != 0 || !S_ISREG(attributes.st_mode) || attributes.st_uid != geteuid()
        || (attributes.st_mode & 0077) != 0) {
        close(fd);
        return @"The update relaunch profile must be an owner-only regular file.";
    }
    NSFileHandle *file = [[NSFileHandle alloc] initWithFileDescriptor:fd closeOnDealloc:YES];
    NSError *error = nil;
    NSData *data = [file readDataToEndOfFileAndReturnError:&error];
    if (!data) return error.localizedDescription;
    NSDictionary *record = [NSPropertyListSerialization propertyListWithData:data options:NSPropertyListImmutable format:nil error:&error];
    if (![record isKindOfClass:NSDictionary.class] || ![record[@"environment"] isKindOfClass:NSDictionary.class]
        || ![record[@"version"] isKindOfClass:NSString.class]) return @"The update relaunch profile is invalid.";
    // A canceled/failed update must not change the profile of the old version.
    if (![record[@"version"] isEqualToString:version]) return nil;
    NSDictionary *environment = record[@"environment"];
    for (NSString *key in profileEnvironmentKeys()) {
        if (environment[key] && ![environment[key] isKindOfClass:NSString.class]) return @"The update relaunch profile is invalid.";
    }
    for (NSString *key in profileEnvironmentKeys()) {
        NSString *value = environment[key];
        // An explicit launch environment takes precedence over the handoff.
        if (value && !getenv(key.UTF8String) && setenv(key.UTF8String, value.UTF8String, 0) != 0) {
            return @"Could not restore the update relaunch profile.";
        }
    }
    if (unlink(url.fileSystemRepresentation) != 0) return @"Could not consume the update relaunch profile.";
    return nil;
}

const char *ainc_restore_relaunch_profile(void) {
    static NSString *error;
    @autoreleasepool {
        NSBundle *bundle = NSBundle.mainBundle;
        error = restoreRelaunchProfile(relaunchProfileURL(bundle), [bundle objectForInfoDictionaryKey:@"CFBundleVersion"]);
    }
    return error.UTF8String;
}

@implementation AincSparkleDriver
- (void)showUpdatePermissionRequest:(SPUUpdatePermissionRequest *)request reply:(void (^)(SUUpdatePermissionResponse *))reply {
    // Normally suppressed by the shipped SUEnableAutomaticChecks default.
    NSAlert *alert = [NSAlert new];
    alert.messageText = @"Check for AgentInc updates automatically?";
    alert.informativeText = @"You can change this in Software Updates settings.";
    [alert addButtonWithTitle:@"Check Automatically"];
    [alert addButtonWithTitle:@"Not Now"];
    BOOL allowed = [alert runModal] == NSAlertFirstButtonReturn;
    Class response = NSClassFromString(@"SUUpdatePermissionResponse");
    reply([[response alloc] initWithAutomaticUpdateChecks:allowed sendSystemProfile:NO]);
}
- (void)showUserInitiatedUpdateCheckWithCancellation:(void (^)(void))cancellation {
    self.userVisible = YES;
    self.cancellation = cancellation;
    self.message = @"Checking for updates…";
    ainc_update_status(self.message.UTF8String, self.current.UTF8String, 0);
}
- (void)presentOffer {
    if (!self.item) return;
    self.changelog = NO;
    ainc_update_offer(self.item.displayVersionString.UTF8String, self.current.UTF8String,
        (self.notes ?: itemNotes(self.item)).UTF8String,
        [self.defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"], self.ready, false);
    if (self.item.informationOnlyUpdate) {
        for (NSView *view in ui().offer.contentView.subviews) {
            if ([view isKindOfClass:NSButton.class] && ((NSButton *)view).action == @selector(install:)) {
                ((NSButton *)view).title = @"Learn More";
            }
        }
    }
#ifdef AINC_UPGRADE_TEST
    if (getenv("AINC_UPGRADE_TEST_MODE") && !self.item.informationOnlyUpdate) {
        // Exercise the real retained NSButton target/action, not a parallel path.
        NSWindow *offeredWindow = ui().offer;
        dispatch_async(dispatch_get_main_queue(), ^{
            if (self.choice && ui().offer == offeredWindow && offeredWindow.visible) ainc_update_test_click_install();
        });
    }
#endif
}
- (void)showUpdateFoundWithAppcastItem:(SUAppcastItem *)item state:(SPUUserUpdateState *)state reply:(void (^)(SPUUserUpdateChoice))reply {
    self.cancellation = nil;
    self.item = item;
    self.choice = reply;
    self.ready = state.stage != SPUUserUpdateStageNotDownloaded;
    self.installArmed |= state.stage == SPUUserUpdateStageInstalling;
    self.message = [NSString stringWithFormat:@"AgentInc %@ is available", item.displayVersionString];
    self.backgroundDownload = !state.userInitiated && [self.defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"] && !item.informationOnlyUpdate;
    if (self.backgroundDownload && state.stage == SPUUserUpdateStageNotDownloaded) {
        self.choice = nil;
        reply(SPUUserUpdateChoiceInstall);
        return;
    }
    [self presentOffer];
}
- (void)showUpdateReleaseNotesWithDownloadData:(SPUDownloadData *)data {
    self.notes = [[NSString alloc] initWithData:data.data encoding:NSUTF8StringEncoding] ?: @"Release notes unavailable.";
    if (self.choice && !self.changelog && (!self.backgroundDownload || self.userVisible)) [self presentOffer];
}
- (void)showUpdateReleaseNotesFailedToDownloadWithError:(NSError *)error {
    self.notes = escaped(error.localizedDescription);
    if (self.choice && !self.changelog && (!self.backgroundDownload || self.userVisible)) [self presentOffer];
}
- (void)showUpdateNotFoundWithError:(NSError *)error acknowledgement:(void (^)(void))acknowledgement {
    self.cancellation = nil;
    self.acknowledgement = acknowledgement;
    self.message = error.localizedDescription;
    // Sparkle distinguishes latest-version from incompatible OS / other reasons.
    NSNumber *reason = error.userInfo[@"SUNoUpdateFoundReason"];
    BOOL latest = reason && (reason.integerValue == SPUNoUpdateFoundReasonOnLatestVersion || reason.integerValue == SPUNoUpdateFoundReasonOnNewerThanLatestVersion);
    ainc_update_status(self.message.UTF8String, self.current.UTF8String, latest ? 1 : 3);
}
- (void)showUpdaterError:(NSError *)error acknowledgement:(void (^)(void))acknowledgement {
    self.cancellation = nil;
    self.choice = nil;
    self.retryTermination = nil;
    self.acknowledgement = acknowledgement;
    self.message = error.localizedDescription;
    ainc_update_status(self.message.UTF8String, self.current.UTF8String, 2);
}
- (void)showDownloadInitiatedWithCancellation:(void (^)(void))cancellation {
    self.choice = nil;
    self.cancellation = cancellation;
    self.received = 0;
    self.expected = 0;
    self.message = @"Downloading update…";
    if (self.backgroundDownload && !self.userVisible) return;
    ainc_update_progress(self.message.UTF8String, 0, 0);
    ui().cancelButton.title = @"Cancel";
    ui().cancelButton.action = @selector(cancel:);
    ui().cancelButton.enabled = YES;
}
- (void)showDownloadDidReceiveExpectedContentLength:(uint64_t)length {
    self.expected = length;
    if (self.backgroundDownload && !self.userVisible) return;
    ainc_update_progress(self.message.UTF8String, self.received, self.expected);
}
- (void)showDownloadDidReceiveDataOfLength:(uint64_t)length {
    self.received += length;
    if (self.backgroundDownload && !self.userVisible) return;
    ainc_update_progress(self.message.UTF8String, self.received, self.expected);
}
- (void)showDownloadDidStartExtractingUpdate {
    self.cancellation = nil;
    self.message = @"Preparing update…";
    if (self.backgroundDownload && !self.userVisible) return;
    ainc_update_progress(self.message.UTF8String, 0, 0);
    ui().cancelButton.enabled = NO;
}
- (void)showExtractionReceivedProgress:(double)progress {
    if (self.backgroundDownload && !self.userVisible) return;
    ainc_update_progress(self.message.UTF8String, (uint64_t)(MAX(0, MIN(1, progress)) * 1000), 1000);
    ui().bytes.stringValue = [NSString stringWithFormat:@"%.0f%%", MAX(0, MIN(1, progress)) * 100];
}
- (void)showReadyToInstallAndRelaunch:(void (^)(SPUUserUpdateChoice))reply {
    self.ready = YES;
    self.installArmed = YES;
    self.choice = reply;
    self.message = @"Update verified and ready to install";
    if (self.cancelBackgroundOnQuit) { [self cancelBackgroundInstallation]; return; }
    if (self.installRequested) { [self action:3 automatic:[self.defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"]]; return; }
    if (self.backgroundDownload && !self.userVisible) {
#ifdef AINC_UPGRADE_TEST
        if (getenv("AINC_UPGRADE_TEST_MODE")) [self presentOffer];
#endif
        return;
    }
    [self presentOffer];
}
- (void)showInstallingUpdateWithApplicationTerminated:(BOOL)terminated retryTerminatingApplication:(void (^)(void))retry {
    self.retryTermination = terminated ? nil : retry;
    self.message = @"Installing update…";
    ainc_update_progress(self.message.UTF8String, 0, 0);
    ui().cancelButton.title = @"Retry Quit";
    ui().cancelButton.action = @selector(retry:);
    ui().cancelButton.enabled = !terminated;
}
- (void)showUpdateInstalledAndRelaunched:(BOOL)relaunched acknowledgement:(void (^)(void))acknowledgement {
    acknowledgement();
}
- (void)dismissUpdateInstallation {
    [self.reminder invalidate];
    self.reminder = nil;
    self.choice = nil;
    self.cancellation = nil;
    self.acknowledgement = nil;
    self.retryTermination = nil;
    if (!self.preparing && !self.preparationFailed) ainc_update_close();
}
- (void)showUpdateInFocus {
    self.userVisible = YES;
    [self.reminder invalidate];
    self.reminder = nil;
    if (self.preparationFailed) {
        ainc_update_status(self.message.UTF8String, self.current.UTF8String, 2);
        return;
    }
    if (ui().offer) [ui().offer makeKeyAndOrderFront:nil];
    else if (ui().progress) [ui().progress makeKeyAndOrderFront:nil];
    else if (self.choice) [self presentOffer];
    else if (self.backgroundDownload) {
        ainc_update_progress(self.message.UTF8String, self.received, self.expected);
        ui().cancelButton.enabled = self.cancellation != nil;
    }
    [NSApp activateIgnoringOtherApps:YES];
}
- (void)prepare {
    if (self.preparing) return;
    self.preparing = YES;
    self.prepared = NO;
    self.preparationFailed = NO;
    self.message = @"Saving work and stopping the local runtime…";
    ainc_update_progress(self.message.UTF8String, 0, 0);
    ui().cancelButton.enabled = NO;
    // Rust handles action 8 on its next foreground turn, then drains off-thread.
    ainc_update_action(8, false);
}
- (void)preparedWithError:(NSString *)error {
    self.preparing = NO;
    if (error) {
        self.preparationFailed = YES;
        self.message = error;
        if (self.terminationWaiting) {
            self.terminationWaiting = NO;
            self.retryQuit = YES;
            [NSApp replyToApplicationShouldTerminate:NO];
        }
        ainc_update_status(error.UTF8String, self.current.UTF8String, 2);
        return;
    }
    self.prepared = YES;
    self.preparationFailed = NO;
    void (^continuation)(void) = self.continuation;
    self.continuation = nil;
    if (continuation) continuation();
    if (self.terminationWaiting) {
        self.terminationWaiting = NO;
        [NSApp replyToApplicationShouldTerminate:YES];
    } else if (self.retryQuit) {
        self.retryQuit = NO;
        [NSApp terminate:nil];
    }
}
- (void)action:(int)action automatic:(BOOL)automatic {
    if (action == 5) { [self.defaults setBool:automatic forKey:@"AINCAutomaticallyDownloadUpdates"]; return; }
    if (self.historyRequested && action == 7) { self.historyRequested = NO; return; }
    if (self.changelog) {
        self.changelog = NO;
        if (self.choice) [self presentOffer];
        return;
    }
    if (self.preparationFailed) {
        if (action == 6) [self prepare];
        // Closing the error never resumes an un-drained installation.
        return;
    }
    if (action == 6 && self.retryTermination) {
        self.continuation = self.retryTermination;
        [self prepare];
        return;
    }
    if ((action == 4 || action == 7) && self.cancellation) {
        void (^cancel)(void) = self.cancellation;
        self.cancellation = nil;
        cancel();
        return;
    }
    if (self.acknowledgement) {
        void (^acknowledge)(void) = self.acknowledgement;
        self.acknowledgement = nil;
        acknowledge();
        if (action == 6) {
            BOOL history = self.historyFailed;
            self.historyFailed = NO;
            dispatch_async(dispatch_get_main_queue(), ^{
                if (history) [self requestHistory];
                else [self.updater checkForUpdates];
            });
        }
        return;
    }
    if (self.choice && (action == 1 || action == 2 || action == 3 || action == 7)) {
        if (action != 7) [self.defaults setBool:automatic forKey:@"AINCAutomaticallyDownloadUpdates"];
        if (self.backgroundDownload && (action == 2 || action == 7)) {
            // Retain the ready reply. Dismiss would arm install-on-quit, which
            // is not what the download-only setting promises.
            self.userVisible = NO;
            if (action == 2) {
                [self.reminder invalidate];
                __weak AincSparkleDriver *driver = self;
                // Restore the existing one-day ready badge reminder. This is
                // presentation only; Sparkle still owns checking/downloading.
                self.reminder = [NSTimer scheduledTimerWithTimeInterval:86400 repeats:NO block:^(NSTimer *timer) {
                    (void)timer;
                    driver.reminder = nil;
                }];
            }
            return;
        }
        void (^reply)(SPUUserUpdateChoice) = self.choice;
        self.choice = nil;
        SPUUserUpdateChoice choice = action == 1 ? SPUUserUpdateChoiceSkip
            : action == 3 ? SPUUserUpdateChoiceInstall : SPUUserUpdateChoiceDismiss;
        if (choice == SPUUserUpdateChoiceInstall && self.item.informationOnlyUpdate) {
            NSURL *url = self.item.infoURL;
            if ([@[@"https", @"http"] containsObject:url.scheme.lowercaseString]) [NSWorkspace.sharedWorkspace openURL:url];
            choice = SPUUserUpdateChoiceDismiss;
        }
        if (choice == SPUUserUpdateChoiceSkip) {
            // The ready-to-relaunch reply cancels installation but, unlike the
            // initial offer reply, does not remember a skipped version. Keep
            // this existing AgentInc button's meaning in Sparkle's own domain.
            if (self.ready) [self.defaults setObject:self.item.versionString forKey:@"SUSkippedVersion"];
            self.ready = NO;
            self.installArmed = NO;
        }
        if (choice == SPUUserUpdateChoiceDismiss) self.ready = NO;
        if (choice == SPUUserUpdateChoiceInstall) self.installRequested = YES;
        if (choice == SPUUserUpdateChoiceInstall && self.installArmed && !self.prepared) {
            self.continuation = ^{ reply(SPUUserUpdateChoiceInstall); };
            [self prepare];
        } else reply(choice);
    }
}
- (void)cancelBackgroundInstallation {
    if (self.choice) {
        void (^reply)(SPUUserUpdateChoice) = self.choice;
        self.choice = nil;
        reply(SPUUserUpdateChoiceSkip);
    } else if (self.cancellation) {
        void (^cancel)(void) = self.cancellation;
        self.cancellation = nil;
        cancel();
    } else if (self.acknowledgement) {
        void (^acknowledge)(void) = self.acknowledgement;
        self.acknowledgement = nil;
        acknowledge();
    }
    // Extraction has no cancellation callback; showReady... will cancel it.
}
- (BOOL)updater:(SPUUpdater *)updater shouldPostponeRelaunchForUpdate:(SUAppcastItem *)item untilInvokingBlock:(void (^)(void))installHandler {
    if (self.prepared) return NO;
    self.installArmed = YES;
    self.continuation = installHandler;
    [self prepare];
    return YES;
}
- (void)updater:(SPUUpdater *)updater willInstallUpdate:(SUAppcastItem *)item {
    self.installArmed = YES;
}
- (BOOL)updater:(SPUUpdater *)updater willInstallUpdateOnQuit:(SUAppcastItem *)item immediateInstallationBlock:(void (^)(void))installHandler {
    self.item = item;
    self.ready = YES;
    self.installArmed = YES;
    self.message = @"Update verified and ready to install";
    // The ordinary-quit barrier also protects Sparkle's install-on-quit path.
    return NO;
}
- (void)updater:(SPUUpdater *)updater didFinishUpdateCycleForUpdateCheck:(SPUUpdateCheck)check error:(NSError *)error {
    if (self.historyRequested && error) {
        self.historyRequested = NO;
        self.historyFailed = YES;
        [self showUpdaterError:error acknowledgement:^{}];
    }
    if (self.cancelBackgroundOnQuit) {
        self.cancelBackgroundOnQuit = NO;
        self.backgroundDownload = NO;
        self.installArmed = NO;
        self.ready = NO;
        [NSApp replyToApplicationShouldTerminate:YES];
    }
    if (error && self.prepared) {
        NSURL *profile = relaunchProfileURL(NSBundle.mainBundle);
        if (profile) [NSFileManager.defaultManager removeItemAtURL:profile error:nil];
        self.prepared = NO;
        self.installArmed = NO;
        ainc_update_action(9, false);
    }
    if (error) { self.ready = NO; self.installArmed = NO; }
    self.backgroundDownload = NO;
    self.installRequested = NO;
    self.userVisible = NO;
}
- (NSString *)feedURLStringForUpdater:(SPUUpdater *)updater {
#ifdef AINC_UPGRADE_TEST
    return NSProcessInfo.processInfo.environment[@"AINC_UPGRADE_TEST_SPARKLE_FEED_URL"];
#else
    return nil;
#endif
}
- (void)showHistory {
    self.changelog = YES;
    ainc_update_offer(self.current.UTF8String, self.current.UTF8String,
        (self.history ?: @"<p>Release history is unavailable.</p>").UTF8String,
        false, false, true);
}
- (void)requestHistory {
    if (self.history) [self showHistory];
    else if (!self.updater.sessionInProgress) {
        self.historyRequested = YES;
        ainc_update_status("Loading release history…", self.current.UTF8String, 0);
        [self.updater checkForUpdateInformation];
    } else [self showUpdateInFocus];
}
- (void)updater:(SPUUpdater *)updater didFinishLoadingAppcast:(SUAppcast *)appcast {
    NSMutableString *history = [NSMutableString string];
    NSMutableString *notes = [NSMutableString string];
    id<SUVersionComparison> comparator = [NSClassFromString(@"SUStandardVersionComparator") defaultComparator];
    for (SUAppcastItem *item in appcast.items) {
        NSString *section = [NSString stringWithFormat:@"<h2>AgentInc %@</h2>%@", escaped(item.displayVersionString), itemNotes(item)];
        [history appendString:section];
        if ([comparator compareVersion:item.versionString toVersion:self.current] == NSOrderedDescending) [notes appendString:section];
    }
    self.history = history;
    self.notes = notes.length ? notes : nil;
    if (self.historyRequested) {
        self.historyRequested = NO;
        [self showHistory];
    }
}
@end

static void updateAction(int action, bool automatic) {
    if (sparkle) [sparkle action:action automatic:automatic];
    else ainc_update_action(action, automatic);
}

static void migratePreferences(SPUUpdater *updater, NSString *path, NSBundle *host) {
    NSString *domain = [host objectForInfoDictionaryKey:@"SUDefaultsDomain"] ?: host.bundleIdentifier;
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:domain];
    NSDictionary *saved = [defaults persistentDomainForName:domain];
    if (!saved[@"SUEnableAutomaticChecks"]) {
        // Preserve AgentInc's existing opt-out default even when packaging uses
        // SUEnableAutomaticChecks=NO to suppress Sparkle's permission prompt.
        updater.automaticallyChecksForUpdates = YES;
    }
    if ([defaults boolForKey:@"AINCSparklePreferencesMigrated"]) return;
    NSData *data = [NSData dataWithContentsOfFile:path];
    NSDictionary *legacy = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    if ([legacy isKindOfClass:NSDictionary.class]) {
        // Do not overwrite settings already owned by Sparkle.
        if (!saved[@"SUEnableAutomaticChecks"] && legacy[@"automatic_checks"]) updater.automaticallyChecksForUpdates = [legacy[@"automatic_checks"] boolValue];
        if (!saved[@"AINCAutomaticallyDownloadUpdates"] && legacy[@"automatic_download"]) [defaults setBool:[legacy[@"automatic_download"] boolValue] forKey:@"AINCAutomaticallyDownloadUpdates"];
        if (!saved[@"SUScheduledCheckInterval"] && legacy[@"interval_hours"]) updater.updateCheckInterval = [legacy[@"interval_hours"] doubleValue] * 3600;
        if (!saved[@"SUSkippedVersion"] && [legacy[@"skipped_version"] isKindOfClass:NSString.class]) [defaults setObject:legacy[@"skipped_version"] forKey:@"SUSkippedVersion"];
        NSTimeInterval last = [legacy[@"last_check"] doubleValue];
        NSTimeInterval reminder = [legacy[@"remind_after"] doubleValue] - updater.updateCheckInterval;
        if (!saved[@"SULastCheckTime"] && MAX(last, reminder) > 0) [defaults setObject:[NSDate dateWithTimeIntervalSince1970:MAX(last, reminder)] forKey:@"SULastCheckTime"];
    }
    [defaults setBool:YES forKey:@"AINCSparklePreferencesMigrated"];
}

bool ainc_sparkle_start(const char *legacy, const char *version) {
    if (sparkle) return sparkle.started;
    sparkle = [AincSparkleDriver new];
    sparkle.current = [NSString stringWithUTF8String:version];
    NSBundle *host = NSBundle.mainBundle;
#ifdef AINC_UPGRADE_TEST
    if (getenv("AINC_UPGRADE_TEST_MODE")) {
        NSString *domain = [host objectForInfoDictionaryKey:@"SUDefaultsDomain"];
        if (!domain.length || [domain isEqualToString:host.bundleIdentifier]) {
            sparkle.message = @"Upgrade tests require an isolated SUDefaultsDomain.";
            return false;
        }
    }
#endif
    NSString *path = [host.privateFrameworksPath stringByAppendingPathComponent:@"Sparkle.framework"];
    NSBundle *framework = path ? [NSBundle bundleWithPath:path] : nil;
    NSError *error = nil;
    if (!framework || ![framework loadAndReturnError:&error]) {
        sparkle.message = error.localizedDescription ?: @"Sparkle.framework is missing. Reinstall AgentInc to enable updates.";
        return false;
    }
    Class updater = NSClassFromString(@"SPUUpdater");
    sparkle.updater = [[updater alloc] initWithHostBundle:host applicationBundle:host userDriver:sparkle delegate:sparkle];
    migratePreferences(sparkle.updater, [NSString stringWithUTF8String:legacy], host);
    NSString *domain = [host objectForInfoDictionaryKey:@"SUDefaultsDomain"] ?: host.bundleIdentifier;
    sparkle.defaults = [[NSUserDefaults alloc] initWithSuiteName:domain];
    // Sparkle's automatic-download switch also consents to install-on-quit.
    // AgentInc has a download-only setting, implemented through user-driver
    // replies and explicit cancellation on ordinary quit instead.
    sparkle.updater.automaticallyDownloadsUpdates = NO;
    if (![sparkle.updater startUpdater:&error]) {
        sparkle.message = error.localizedDescription;
        return false;
    }
    sparkle.started = YES;
    sparkle.message = [NSString stringWithFormat:@"AgentInc %@", sparkle.current];
    installTerminationBarrier();
    return true;
}

void ainc_sparkle_check(bool background) {
    if (!sparkle.started) {
        ainc_update_status((sparkle.message ?: @"Updates are available in production builds.").UTF8String,
            (sparkle.current ?: @"").UTF8String, 3);
    } else if (background) [sparkle.updater checkForUpdatesInBackground];
    else [sparkle.updater checkForUpdates];
}

void ainc_sparkle_changelog(void) {
    if (!sparkle.started) { ainc_sparkle_check(false); return; }
    [sparkle requestHistory];
}

const char *ainc_sparkle_message(void) { return sparkle.message.UTF8String ?: "Updates are available in production builds."; }
unsigned int ainc_sparkle_state(void) {
    return (sparkle.updater.automaticallyChecksForUpdates ? 1 : 0)
        | ([sparkle.defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"] ? 2 : 0)
        | (sparkle.updater.updateCheckInterval > 86400 ? 4 : 0)
        | (sparkle.ready && !sparkle.reminder ? 8 : 0)
        | (sparkle.updater.canCheckForUpdates ? 16 : 0)
        | (sparkle.started ? 32 : 0);
}
void ainc_sparkle_setting(int setting, bool enabled) {
    if (setting == 0) sparkle.updater.automaticallyChecksForUpdates = enabled;
    if (setting == 1) [sparkle.defaults setBool:enabled forKey:@"AINCAutomaticallyDownloadUpdates"];
    if (setting == 2) sparkle.updater.updateCheckInterval = enabled ? 604800 : 86400;
}
void ainc_sparkle_prepared(const char *error) {
    NSString *failure = error ? [NSString stringWithUTF8String:error] : nil;
    if (!failure && sparkle.started) failure = saveRelaunchProfile(relaunchProfileURL(NSBundle.mainBundle), sparkle.item.versionString);
    [sparkle preparedWithError:failure];
}

#ifdef AINC_UPGRADE_TEST
void ainc_sparkle_restore_test_environment(void) {
    NSDictionary *environment = [NSBundle.mainBundle objectForInfoDictionaryKey:@"AINCUpgradeTestEnvironment"];
    NSArray *keys = @[@"AINC_UPGRADE_TEST_MODE", @"AINC_UPGRADE_TEST_FROM", @"AINC_UPGRADE_TEST_SUCCESS_FILE",
        @"AINC_UPGRADE_TEST_SPARKLE_FEED_URL", @"AINC_UPGRADE_TEST_FEED_URL", @"AINC_DISCOVERY_FILE",
        @"AINC_TOKEN_FILE", @"AGENTINC_SESSION_PATH", @"AINC_DATABASE_URL", @"AINC_DAEMON_URL",
        @"AINC_LEGACY_DIR", @"AGENTINC_CODEX_HOME"];
    for (NSString *key in keys) {
        NSString *value = environment[key];
        if ([value isKindOfClass:NSString.class] && !getenv(key.UTF8String)) setenv(key.UTF8String, value.UTF8String, 0);
    }
}
#endif
