#import <AppKit/AppKit.h>
#import <CommonCrypto/CommonDigest.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/file.h>
#include <netinet/in.h>
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
extern char *ainc_update_format_notes(const char *markdown, const char *current, bool history);
extern void ainc_update_free_notes(char *text);
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
    NSString *question = ready ? @"Downloaded. Verify and install it now?" : @"Would you like to download and install it now?";
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

// The cache is inert: only Sparkle, after explicit consent and drain, verifies
// and extracts archives. Neither the downloader nor its HTTP server installs.
static NSString *digestName(NSData *data) {
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256(data.bytes, (CC_LONG)data.length, digest);
    NSMutableString *name = [NSMutableString string];
    for (NSUInteger i = 0; i < sizeof(digest); i++) [name appendFormat:@"%02x", digest[i]];
    return name;
}

static NSString *writePrivateData(NSData *data, NSURL *url) {
    if (!url) return nil; // Unbundled harness.
    NSError *error = nil;
    if (![NSFileManager.defaultManager createDirectoryAtURL:url.URLByDeletingLastPathComponent withIntermediateDirectories:YES
        attributes:@{NSFilePosixPermissions:@0700} error:&error]) return error.localizedDescription;
    char *temporary = strdup([[url.path stringByAppendingString:@".XXXXXX"] fileSystemRepresentation]);
    if (!temporary) return @"Could not allocate update record.";
    int fd = mkstemp(temporary);
    if (fd < 0) { free(temporary); return @"Could not create update record."; }
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
    int directory = open(url.URLByDeletingLastPathComponent.fileSystemRepresentation, O_RDONLY);
    if (directory < 0 || fsync(directory) != 0) success = NO;
    if (directory >= 0) close(directory);
    return success ? nil : @"Could not persist update record.";
}

static BOOL sendBytes(int socket, const void *bytes, size_t remaining) {
    while (remaining) {
        ssize_t count = send(socket, bytes, remaining, 0);
        if (count < 0 && errno == EINTR) continue;
        if (count <= 0) return NO;
        bytes = (const char *)bytes + count;
        remaining -= count;
    }
    return YES;
}

@interface AincArchiveServer : NSObject
@property(strong) dispatch_source_t listener;
@property(strong) NSURL *url;
- (instancetype)initWithArchive:(NSURL *)archive;
- (void)stop;
@end
@implementation AincArchiveServer
- (instancetype)initWithArchive:(NSURL *)archive {
    self = [super init];
    if (!self) return nil;
    int socketFD = socket(AF_INET, SOCK_STREAM, 0);
    if (socketFD < 0) return nil;
    fcntl(socketFD, F_SETFD, FD_CLOEXEC);
    fcntl(socketFD, F_SETFL, O_NONBLOCK);
    struct sockaddr_in address = {.sin_len = sizeof(address), .sin_family = AF_INET,
        .sin_port = 0, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    socklen_t size = sizeof(address);
    if (bind(socketFD, (struct sockaddr *)&address, size) || listen(socketFD, 8)
        || getsockname(socketFD, (struct sockaddr *)&address, &size)) { close(socketFD); return nil; }
    NSString *route = [@"/" stringByAppendingString:NSUUID.UUID.UUIDString];
    self.url = [[NSURL URLWithString:[NSString stringWithFormat:@"http://127.0.0.1:%u%@/",
        ntohs(address.sin_port), route]] URLByAppendingPathComponent:archive.lastPathComponent];
    NSString *requestLine = [NSString stringWithFormat:@"GET %@ HTTP/1.", [NSURLComponents componentsWithURL:self.url resolvingAgainstBaseURL:NO].percentEncodedPath];
    self.listener = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, socketFD, 0, dispatch_get_global_queue(QOS_CLASS_UTILITY, 0));
    dispatch_source_set_cancel_handler(self.listener, ^{ close(socketFD); });
    dispatch_source_set_event_handler(self.listener, ^{
        int client;
        while ((client = accept(socketFD, NULL, NULL)) >= 0) {
            int connection = client;
            fcntl(connection, F_SETFD, FD_CLOEXEC);
            fcntl(connection, F_SETFL, 0);
            int enabled = 1;
            setsockopt(connection, SOL_SOCKET, SO_NOSIGPIPE, &enabled, sizeof(enabled));
            struct timeval timeout = {.tv_sec = 30};
            setsockopt(connection, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
            setsockopt(connection, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout));
            dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
                @autoreleasepool {
                    NSMutableData *request = [NSMutableData data];
                    while (request.length < 16384) {
                        char bytes[1024];
                        ssize_t count = recv(connection, bytes, sizeof(bytes), 0);
                        if (count <= 0) break;
                        [request appendBytes:bytes length:(NSUInteger)count];
                        if ([request rangeOfData:[@"\r\n\r\n" dataUsingEncoding:NSUTF8StringEncoding] options:0 range:NSMakeRange(0, request.length)].location != NSNotFound) break;
                    }
                    NSString *text = [[NSString alloc] initWithData:request encoding:NSUTF8StringEncoding];
                    int file = [text hasPrefix:requestLine] ? open(archive.fileSystemRepresentation, O_RDONLY | O_NOFOLLOW) : -1;
                    struct stat attributes;
                    if (file >= 0 && fstat(file, &attributes) == 0 && S_ISREG(attributes.st_mode)) {
                        NSString *header = [NSString stringWithFormat:@"HTTP/1.1 200 OK\r\nContent-Length: %lld\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n", (long long)attributes.st_size];
                        if (sendBytes(connection, header.UTF8String, strlen(header.UTF8String))) {
                            char bytes[65536];
                            ssize_t count;
                            while ((count = read(file, bytes, sizeof(bytes))) > 0) if (!sendBytes(connection, bytes, (size_t)count)) break;
                        }
                    } else {
                        const char *missing = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        sendBytes(connection, missing, strlen(missing));
                    }
                    if (file >= 0) close(file);
                    close(connection);
                }
            });
        }
    });
    dispatch_resume(self.listener);
    return self;
}
- (void)stop { if (self.listener) { dispatch_source_cancel(self.listener); self.listener = nil; } }
- (void)dealloc { [self stop]; }
@end

@interface AincArchiveDownload : NSObject <NSURLSessionDownloadDelegate>
@property(strong) NSURLSession *session;
@property(strong) NSURLSessionDownloadTask *task;
@property(strong) NSURL *destination;
@property(copy) void (^progress)(uint64_t, uint64_t);
@property(copy) void (^completion)(NSError *);
@property(strong) NSError *saveError;
- (void)start:(NSURL *)url destination:(NSURL *)destination;
- (void)cancel;
@end
@implementation AincArchiveDownload
- (void)start:(NSURL *)url destination:(NSURL *)destination {
    self.destination = destination;
    NSOperationQueue *queue = [NSOperationQueue new];
    queue.maxConcurrentOperationCount = 1;
    self.session = [NSURLSession sessionWithConfiguration:NSURLSessionConfiguration.ephemeralSessionConfiguration
        delegate:self delegateQueue:queue];
    self.task = [self.session downloadTaskWithURL:url];
    [self.task resume];
}
- (void)cancel { [self.task cancel]; }
- (void)URLSession:(NSURLSession *)session downloadTask:(NSURLSessionDownloadTask *)task didWriteData:(int64_t)bytes totalBytesWritten:(int64_t)written totalBytesExpectedToWrite:(int64_t)expected {
    void (^progress)(uint64_t, uint64_t) = self.progress;
    if (progress) dispatch_async(dispatch_get_main_queue(), ^{ progress((uint64_t)MAX(0, written), (uint64_t)MAX(0, expected)); });
}
- (void)URLSession:(NSURLSession *)session downloadTask:(NSURLSessionDownloadTask *)task didFinishDownloadingToURL:(NSURL *)location {
    NSHTTPURLResponse *response = (NSHTTPURLResponse *)task.response;
    if (![response isKindOfClass:NSHTTPURLResponse.class] || response.statusCode != 200) {
        self.saveError = [NSError errorWithDomain:@"AgentInc.Cache" code:1 userInfo:@{NSLocalizedDescriptionKey:@"Update archive download was refused."}];
        return;
    }
    NSFileManager *files = NSFileManager.defaultManager;
    NSError *error = nil;
    NSURL *temporary = [self.destination URLByAppendingPathExtension:NSUUID.UUID.UUIDString];
    BOOL success = [files createDirectoryAtURL:self.destination.URLByDeletingLastPathComponent withIntermediateDirectories:YES attributes:@{NSFilePosixPermissions:@0700} error:&error]
        && [files copyItemAtURL:location toURL:temporary error:&error];
    int fd = success ? open(temporary.fileSystemRepresentation, O_RDWR | O_NOFOLLOW) : -1;
    if (fd < 0 || fchmod(fd, 0600) || fsync(fd)) success = NO;
    if (fd >= 0) close(fd);
    if (success && rename(temporary.fileSystemRepresentation, self.destination.fileSystemRepresentation)) success = NO;
    int directory = open(self.destination.URLByDeletingLastPathComponent.fileSystemRepresentation, O_RDONLY);
    if (directory < 0 || fsync(directory)) success = NO;
    if (directory >= 0) close(directory);
    if (!success) {
        [files removeItemAtURL:temporary error:nil];
        self.saveError = error ?: [NSError errorWithDomain:@"AgentInc.Cache" code:2 userInfo:@{NSLocalizedDescriptionKey:@"Could not save downloaded update."}];
    }
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task didCompleteWithError:(NSError *)error {
    void (^complete)(NSError *) = self.completion;
    self.completion = nil;
    self.progress = nil;
    [self.session finishTasksAndInvalidate];
    self.session = nil;
    self.task = nil;
    if (error) [NSFileManager.defaultManager removeItemAtURL:self.destination error:nil];
    NSError *failure = error ?: self.saveError;
    if (complete) dispatch_async(dispatch_get_main_queue(), ^{ complete(failure); });
}
@end

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
@property BOOL preparationFailed;
@property BOOL changelog;
@property BOOL historyRequested;
@property BOOL historyFailed;
@property BOOL started;
@property(strong) NSUserDefaults *defaults;
@property BOOL backgroundDownload;
@property BOOL userVisible;
@property BOOL installRequested;
@property(strong) NSURL *cacheDirectory;
@property(strong) NSURL *fenceURL;
@property(strong) AincArchiveDownload *download;
@property(strong) AincArchiveServer *archiveServer;
@property BOOL cacheFailed;
@property BOOL ownsInstallCycle;
@property BOOL cancellationRequested;
@property BOOL extractionStarted;
@property BOOL recoveringArmedFence;
@property uint64_t received;
@property uint64_t expected;
- (void)action:(int)action automatic:(BOOL)automatic;
- (void)presentOffer;
- (void)prepare;
- (void)preparedWithError:(NSString *)error;
- (void)downloadArchive;
- (void)requestHistory;
@end

static AincSparkleDriver *sparkle;

static NSString *escaped(NSString *text) {
    return [[[text stringByReplacingOccurrencesOfString:@"&" withString:@"&amp;"]
        stringByReplacingOccurrencesOfString:@"<" withString:@"&lt;"]
        stringByReplacingOccurrencesOfString:@">" withString:@"&gt;"];
}

static NSString *itemNotes(SUAppcastItem *item, NSString *current, BOOL history) {
    NSString *description = item.itemDescription ?: @"Release notes unavailable.";
    if ([item.itemDescriptionFormat isEqualToString:@"markdown"]) {
        char *html = ainc_update_format_notes(description.UTF8String, current.UTF8String, history);
        NSString *result = [NSString stringWithUTF8String:html];
        ainc_update_free_notes(html);
        return result;
    }
    return [item.itemDescriptionFormat isEqualToString:@"plain-text"]
        ? [NSString stringWithFormat:@"<p>%@</p>", escaped(description)] : description;
}

// Sparkle 2.9.6 relaunches through NSWorkspace.openURL without forwarding the
// host environment. Keep only documented profile/companion configuration,
// never keys or the rest of the caller's process environment. Fixture-only
// feed overrides below are compiled out of the shipping application.
static NSArray<NSString *> *profileEnvironmentKeys(void) {
    return @[@"AGENTINC_SESSION_PATH", @"AINC_DISCOVERY_FILE", @"AINC_DAEMON_URL",
        @"AINC_TOKEN_FILE", @"AINC_DATABASE_URL", @"DATABASE_URL", @"AINC_LEGACY_DIR",
        @"AGENTINC_CODEX_HOME", @"AGENTINC_CODEX_PATH", @"AINC_RUNTIME_CONFIG",
        @"AINC_WORKSPACE_DIR", @"AINC_TOOL_ALLOW"
#ifdef AINC_UPGRADE_TEST
        , @"AINC_UPGRADE_TEST_MODE", @"AINC_UPGRADE_TEST_FROM", @"AINC_UPGRADE_TEST_SUCCESS_FILE",
        @"AINC_UPGRADE_TEST_SPARKLE_FEED_URL", @"AINC_UPGRADE_TEST_FEED_URL",
        @"AINC_UPGRADE_TEST_DOWNLOADED_FILE", @"AINC_UPGRADE_TEST_PREPARATION_FILE"
#endif
    ];
}

static NSURL *relaunchProfileURL(NSBundle *bundle) {
    if (!bundle.bundleIdentifier.length) return nil; // Unbundled builds/tests.
    NSData *path = [bundle.bundlePath.stringByResolvingSymlinksInPath dataUsingEncoding:NSUTF8StringEncoding];
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256(path.bytes, (CC_LONG)path.length, digest);
    NSMutableString *name = [NSMutableString string];
    for (NSUInteger i = 0; i < sizeof(digest); i++) [name appendFormat:@"%02x", digest[i]];
    NSURL *support = [NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask].firstObject;
    NSString *domain = [bundle objectForInfoDictionaryKey:@"SUDefaultsDomain"] ?: bundle.bundleIdentifier;
    NSURL *directory = [[support URLByAppendingPathComponent:domain isDirectory:YES]
        URLByAppendingPathComponent:@"Updater Relaunch" isDirectory:YES];
    return [directory URLByAppendingPathComponent:[name stringByAppendingPathExtension:@"plist"]];
}

static NSURL *installationFenceURL(NSBundle *bundle) {
    return [relaunchProfileURL(bundle) URLByAppendingPathExtension:@"fence"];
}
static int fenceLease = -1;

static NSDictionary *readFence(NSURL *url) {
    return url ? [NSDictionary dictionaryWithContentsOfURL:url] : nil;
}
static void releaseFenceLease(void) {
    if (fenceLease >= 0) { close(fenceLease); fenceLease = -1; }
}
static NSString *removeFence(NSURL *url) {
    if (url && unlink(url.fileSystemRepresentation) != 0 && errno != ENOENT) return @"Could not clear the update startup fence.";
    releaseFenceLease();
    return nil;
}
static NSString *acquireFence(NSURL *url, NSString *version) {
    if (!url) return nil;
    NSDictionary *existing = readFence(url);
    if (!existing && [NSFileManager.defaultManager fileExistsAtPath:url.path]) return @"The pending update fence is unreadable. Reinstall AgentInc before restarting the local runtime.";
    if (existing && ![existing[@"version"] isEqualToString:version]) return @"An earlier update is still pending. Retry that update before starting the local runtime.";
    if (fenceLease < 0) {
        [NSFileManager.defaultManager createDirectoryAtURL:url.URLByDeletingLastPathComponent withIntermediateDirectories:YES attributes:@{NSFilePosixPermissions:@0700} error:nil];
        NSURL *lease = [url URLByAppendingPathExtension:@"lock"];
        fenceLease = open(lease.fileSystemRepresentation, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, 0600);
        if (fenceLease < 0 || flock(fenceLease, LOCK_EX | LOCK_NB) != 0) {
            releaseFenceLease();
            return @"Another AgentInc instance is preparing an update.";
        }
    }
    if (existing) return nil; // Never downgrade an armed record during recovery.
    NSData *data = [NSPropertyListSerialization dataWithPropertyList:@{@"version":version, @"phase":@"preparing"} format:NSPropertyListBinaryFormat_v1_0 options:0 error:nil];
    return writePrivateData(data, url);
}
// Each signed appcast is published beside its release's immutable assets, so
// an interrupted target remains reachable after the latest feed advances.
static NSString *pinnedFeed(SUAppcastItem *item) {
    return [item.fileURL.URLByDeletingLastPathComponent URLByAppendingPathComponent:@"appcast.xml"].absoluteString;
}
static NSString *armFence(NSURL *url, NSString *version, NSString *feed) {
    NSMutableDictionary *record = [@{@"version":version, @"phase":@"armed"} mutableCopy];
    if (feed) record[@"feed"] = feed;
    NSData *data = [NSPropertyListSerialization dataWithPropertyList:record format:NSPropertyListBinaryFormat_v1_0 options:0 error:nil];
    return writePrivateData(data, url);
}
static NSString *restoreFence(NSURL *url, NSString *version) {
    NSDictionary *record = readFence(url);
    if (!record) return nil; // Unreadable existing files still fence startup below.
    if ([record[@"version"] isEqualToString:version]) return removeFence(url);
    if ([record[@"phase"] isEqualToString:@"preparing"]) {
        // No Install reply can precede the durable transition to armed. Only
        // recover a dead owner's pre-drain record; another live UI may own it.
        NSURL *lease = [url URLByAppendingPathExtension:@"lock"];
        int fd = open(lease.fileSystemRepresentation, O_RDWR | O_NOFOLLOW | O_CLOEXEC);
        if (fd >= 0) {
            if (flock(fd, LOCK_EX | LOCK_NB) == 0) {
                NSString *error = removeFence(url);
                close(fd);
                return error;
            }
            close(fd);
        }
    }
    return nil;
}
bool ainc_sparkle_installation_fenced(void) {
    @autoreleasepool {
        NSURL *url = installationFenceURL(NSBundle.mainBundle);
        return url && [NSFileManager.defaultManager fileExistsAtPath:url.path];
    }
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
    return writePrivateData(data, url);
}

static NSString *restoreRelaunchProfileRecord(NSURL *url, NSString *version, BOOL consume) {
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
    if (consume && unlink(url.fileSystemRepresentation) != 0) return @"Could not consume the update relaunch profile.";
    return nil;
}

static NSString *restoreRelaunchProfile(NSURL *url, NSString *version) {
    return restoreRelaunchProfileRecord(url, version, YES);
}

const char *ainc_restore_relaunch_profile(const char *executableVersion) {
    static NSString *error;
    @autoreleasepool {
        NSBundle *bundle = NSBundle.mainBundle;
        NSString *version = [bundle objectForInfoDictionaryKey:@"CFBundleVersion"];
        NSDictionary *fence = readFence(installationFenceURL(bundle));
        if ([fence[@"version"] isEqualToString:version]
            && ![[bundle objectForInfoDictionaryKey:@"CFBundleShortVersionString"] isEqualToString:[NSString stringWithUTF8String:executableVersion]]) {
            error = @"The replacement bundle and running executable versions differ; local runtime startup remains fenced.";
            return error.UTF8String;
        }
        if ([fence[@"phase"] isEqualToString:@"armed"] && ![fence[@"version"] isEqualToString:version]) {
            // Recovery must return to the same profile too. Keep the handoff
            // until a matching replacement consumes it, including repeat crashes.
            error = restoreRelaunchProfileRecord(relaunchProfileURL(bundle), fence[@"version"], NO);
        } else error = restoreRelaunchProfile(relaunchProfileURL(bundle), version);
        if (!error) error = restoreFence(installationFenceURL(bundle), [bundle objectForInfoDictionaryKey:@"CFBundleVersion"]);
    }
    return error.UTF8String;
}

@implementation AincSparkleDriver
- (NSURL *)cachedArchive {
    if (!self.cacheDirectory || !self.item.fileURL) return nil;
    // The user driver's item is Sparkle's selected item (including its selected
    // delta), not the parent full release. Bind cache identity to its enclosure.
    NSData *identity = [NSJSONSerialization dataWithJSONObject:@[self.item.fileURL.absoluteString, self.item.versionString, self.item.propertiesDictionary]
        options:NSJSONWritingSortedKeys error:nil];
    if (!identity) return nil;
    NSString *name = [NSString stringWithFormat:@"%@-%@", digestName(identity), self.item.fileURL.lastPathComponent];
    return [self.cacheDirectory URLByAppendingPathComponent:name];
}
- (void)pruneArchives {
    NSFileManager *files = NSFileManager.defaultManager;
    if (!self.cacheDirectory || self.download || (self.fenceURL && [files fileExistsAtPath:self.fenceURL.path])) return;
    NSString *current = [self cachedArchive].lastPathComponent;
    for (NSURL *entry in [files contentsOfDirectoryAtURL:self.cacheDirectory includingPropertiesForKeys:nil options:0 error:nil]) {
        if (![entry.lastPathComponent isEqualToString:current]) [files removeItemAtURL:entry error:nil];
    }
}
- (BOOL)hasCachedArchive {
    NSURL *archive = [self cachedArchive];
    struct stat attributes;
    return archive && lstat(archive.fileSystemRepresentation, &attributes) == 0
        && S_ISREG(attributes.st_mode) && attributes.st_size > 0 && attributes.st_uid == geteuid();
}
- (void)cacheCompleted {
    self.ready = YES;
    self.cacheFailed = NO;
    self.message = @"Update downloaded; verification occurs when you install";
    if (!self.userVisible && [[self.defaults objectForKey:@"AINCUpdateRemindAfter"] timeIntervalSinceNow] > 0) {
        void (^reply)(SPUUserUpdateChoice) = self.choice;
        self.choice = nil;
        if (reply) reply(SPUUserUpdateChoiceDismiss);
        return;
    }
#ifdef AINC_UPGRADE_TEST
    ainc_update_action(10, false); // Rust waits for the current daemon's readiness.
    if (strcmp(getenv("AINC_UPGRADE_TEST_MODE") ?: "", "automatic") == 0) [self presentOffer];
#endif
    if (self.userVisible) [self presentOffer];
}
- (void)downloadArchive {
    if (self.download) return;
    if ([self hasCachedArchive]) { [self cacheCompleted]; return; }
    NSURL *destination = [self cachedArchive];
    if (!destination) {
        self.message = @"The update archive cannot be cached.";
        self.cacheFailed = YES;
        return;
    }
    self.cacheFailed = NO;
    self.message = @"Downloading update…";
    self.received = 0;
    self.expected = 0;
    AincArchiveDownload *download = [AincArchiveDownload new];
    self.download = download;
    __weak AincSparkleDriver *driver = self;
    __weak AincArchiveDownload *flight = download;
    self.cancellation = ^{ [driver.download cancel]; };
    download.progress = ^(uint64_t received, uint64_t expected) {
        driver.received = received;
        driver.expected = expected;
        if (driver.userVisible) ainc_update_progress(driver.message.UTF8String, received, expected);
    };
    download.completion = ^(NSError *error) {
        AincSparkleDriver *strong = driver;
        if (!strong || strong.download != flight) return;
        strong.download = nil;
        strong.cancellation = nil;
        if (error) {
            strong.ready = NO;
            strong.cacheFailed = YES;
            strong.message = error.code == NSURLErrorCancelled ? @"Update download canceled" : error.localizedDescription;
            if (strong.userVisible) ainc_update_status(strong.message.UTF8String, strong.current.UTF8String, 2);
        } else [strong cacheCompleted];
    };
    [download start:self.item.fileURL destination:destination];
}
- (void)updater:(SPUUpdater *)updater willDownloadUpdate:(SUAppcastItem *)item withRequest:(NSMutableURLRequest *)request {
    // A fallback item has a different original URL. Leave it untouched; Sparkle
    // retains responsibility for selecting and verifying full/delta archives.
    if ([item.fileURL isEqual:self.item.fileURL]
        && [item.versionString isEqualToString:self.item.versionString]
        && [item.propertiesDictionary isEqualToDictionary:self.item.propertiesDictionary]
        && [self hasCachedArchive]) {
        [self.archiveServer stop];
        self.archiveServer = [[AincArchiveServer alloc] initWithArchive:[self cachedArchive]];
        if (self.archiveServer) request.URL = self.archiveServer.url;
    }
}
- (void)updater:(SPUUpdater *)updater willExtractUpdate:(SUAppcastItem *)item {
    self.extractionStarted = YES;
#ifdef AINC_UPGRADE_TEST
    const char *marker = getenv("AINC_UPGRADE_TEST_PREPARATION_FILE");
    if (marker) {
        NSString *text = [NSString stringWithFormat:@"%@ prepared=%d fence=%@\n", item.versionString, self.prepared, readFence(self.fenceURL)[@"phase"] ?: @"missing"];
        NSString *error = writePrivateData([text dataUsingEncoding:NSUTF8StringEncoding], [NSURL fileURLWithPath:[NSString stringWithUTF8String:marker]]);
        NSCAssert(!error, @"%@", error);
    }
#endif
}
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
        (self.notes ?: itemNotes(self.item, self.current, NO)).UTF8String,
        [self.defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"], self.ready, false);
    if (self.item.informationOnlyUpdate) {
        for (NSView *view in ui().offer.contentView.subviews) {
            if ([view isKindOfClass:NSButton.class] && ((NSButton *)view).action == @selector(install:)) {
                ((NSButton *)view).title = @"Learn More";
            }
        }
    }
#ifdef AINC_UPGRADE_TEST
    const char *mode = getenv("AINC_UPGRADE_TEST_MODE") ?: "";
    if ((strcmp(mode, "manual") == 0 || (strcmp(mode, "automatic") == 0 && self.ready)) && !self.item.informationOnlyUpdate) {
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
    self.cacheFailed = NO;
    self.choice = reply;
    self.ready = state.stage != SPUUserUpdateStageNotDownloaded || [self hasCachedArchive];
    self.installArmed |= state.stage == SPUUserUpdateStageInstalling;
    self.message = [NSString stringWithFormat:@"AgentInc %@ is available", item.displayVersionString];
    self.backgroundDownload = !state.userInitiated && [self.defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"] && !item.informationOnlyUpdate;
    if (self.backgroundDownload && state.stage == SPUUserUpdateStageNotDownloaded) {
        [self downloadArchive];
        return;
    }
    NSDate *reminder = [self.defaults objectForKey:@"AINCUpdateRemindAfter"];
    if (!state.userInitiated && reminder.timeIntervalSinceNow > 0) {
        self.choice = nil;
        reply(SPUUserUpdateChoiceDismiss);
        return;
    }
    [self presentOffer];
}
- (void)showUpdateReleaseNotesWithDownloadData:(SPUDownloadData *)data {
    self.notes = [[NSString alloc] initWithData:data.data encoding:NSUTF8StringEncoding] ?: @"Release notes unavailable.";
    if (self.choice && !self.download && !self.changelog && (!self.backgroundDownload || self.userVisible)) [self presentOffer];
}
- (void)showUpdateReleaseNotesFailedToDownloadWithError:(NSError *)error {
    self.notes = escaped(error.localizedDescription);
    if (self.choice && !self.download && !self.changelog && (!self.backgroundDownload || self.userVisible)) [self presentOffer];
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
    if (self.installRequested) { [self action:3 automatic:[self.defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"]]; return; }
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
    self.choice = nil;
    self.cancellation = nil;
    self.acknowledgement = nil;
    self.retryTermination = nil;
    if (!self.preparing && !self.preparationFailed) ainc_update_close();
}
- (void)showUpdateInFocus {
    self.userVisible = YES;
    [self.defaults removeObjectForKey:@"AINCUpdateRemindAfter"];
    if (self.preparationFailed || self.cacheFailed) {
        ainc_update_status(self.message.UTF8String, self.current.UTF8String, 2);
        return;
    }
    if (self.download) {
        ainc_update_progress(self.message.UTF8String, self.received, self.expected);
        ui().cancelButton.enabled = YES;
    }
    else if (ui().offer) [ui().offer makeKeyAndOrderFront:nil];
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
    if (!self.ownsInstallCycle) self.recoveringArmedFence = [readFence(self.fenceURL)[@"phase"] isEqualToString:@"armed"];
    NSString *error = acquireFence(self.fenceURL, self.item.versionString);
    if (error) { [self preparedWithError:error]; return; }
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
        // No initial reply was released for a preparing record. Armed recovery
        // remains fenced even if a later flush/drain attempt fails.
        if (fenceLease >= 0 && [readFence(self.fenceURL)[@"phase"] isEqualToString:@"preparing"]) removeFence(self.fenceURL);
        ainc_update_status(error.UTF8String, self.current.UTF8String, 2);
        return;
    }
    self.prepared = YES;
    self.preparationFailed = NO;
    void (^continuation)(void) = self.continuation;
    self.continuation = nil;
    if (continuation) continuation();
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
    if (self.cacheFailed && action == 6) { [self downloadArchive]; return; }
    if (action == 6 && self.retryTermination) {
        self.continuation = self.retryTermination;
        [self prepare];
        return;
    }
    if ((action == 4 || action == 7) && self.cancellation) {
        void (^cancel)(void) = self.cancellation;
        self.cancellation = nil;
        if (!self.download) self.cancellationRequested = YES;
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
        if (action == 2) [self.defaults setObject:[NSDate dateWithTimeIntervalSinceNow:86400] forKey:@"AINCUpdateRemindAfter"];
        void (^reply)(SPUUserUpdateChoice) = self.choice;
        self.choice = nil;
        self.cacheFailed = NO;
        SPUUserUpdateChoice choice = action == 1 ? SPUUserUpdateChoiceSkip
            : action == 3 ? SPUUserUpdateChoiceInstall : SPUUserUpdateChoiceDismiss;
        if (choice == SPUUserUpdateChoiceInstall && self.item.informationOnlyUpdate) {
            NSURL *url = self.item.infoURL;
            if ([@[@"https", @"http"] containsObject:url.scheme.lowercaseString]) [NSWorkspace.sharedWorkspace openURL:url];
            choice = SPUUserUpdateChoiceDismiss;
        }
        if (choice == SPUUserUpdateChoiceSkip) {
            [self.download cancel];
            self.download = nil;
            self.cancellation = nil;
            NSURL *archive = [self cachedArchive];
            if (archive) [NSFileManager.defaultManager removeItemAtURL:archive error:nil];
            self.ready = NO;
            self.installArmed = NO;
        }
        if (choice == SPUUserUpdateChoiceDismiss) self.ready = NO;
        if (choice == SPUUserUpdateChoiceInstall) {
            [self.download cancel];
            self.download = nil;
            self.cancellation = nil;
            self.installRequested = YES;
            self.userVisible = YES;
        }
        if (choice == SPUUserUpdateChoiceInstall && !self.prepared) {
            __weak AincSparkleDriver *driver = self;
            self.continuation = ^{
                driver.ownsInstallCycle = YES;
                reply(SPUUserUpdateChoiceInstall);
            };
            [self prepare];
        } else reply(choice);
    }
}
- (BOOL)updater:(SPUUpdater *)updater shouldPostponeRelaunchForUpdate:(SUAppcastItem *)item untilInvokingBlock:(void (^)(void))installHandler {
    // Download/extraction can take time. Repeat the UI flush immediately before
    // Sparkle requests termination, retaining our already-held runtime locks.
    self.installArmed = YES;
    self.continuation = installHandler;
    [self prepare];
    return YES;
}
- (void)updater:(SPUUpdater *)updater willInstallUpdate:(SUAppcastItem *)item {
    self.installArmed = YES;
}
- (void)updater:(SPUUpdater *)updater didFinishUpdateCycleForUpdateCheck:(SPUUpdateCheck)check error:(NSError *)error {
    if (self.historyRequested && error) {
        self.historyRequested = NO;
        self.historyFailed = YES;
        [self showUpdaterError:error acknowledgement:^{}];
    }
    // Before willExtractUpdate no installer has launched, so a completed abort
    // proves it is safe to resume. After that callback Sparkle's public cycle
    // completion is not proof its out-of-process installer exited. Stay fenced
    // on ambiguous failures and recover by retrying the same target version.
    if ((error || self.cancellationRequested) && self.ownsInstallCycle
        && !self.extractionStarted && !self.recoveringArmedFence) {
        NSURL *profile = relaunchProfileURL(NSBundle.mainBundle);
        if (profile) [NSFileManager.defaultManager removeItemAtURL:profile error:nil];
        self.prepared = NO;
        self.installArmed = NO;
        if (!removeFence(self.fenceURL)) ainc_update_action(9, false);
    }
    self.ownsInstallCycle = NO;
    self.prepared = NO;
    self.extractionStarted = NO;
    self.cancellationRequested = NO;
    [self.archiveServer stop];
    self.archiveServer = nil;
    if (error) { self.ready = NO; self.installArmed = NO; }
    self.backgroundDownload = NO;
    self.installRequested = NO;
    self.userVisible = NO;
    [self pruneArchives];
}
- (NSString *)feedURLStringForUpdater:(SPUUpdater *)updater {
    NSDictionary *fence = readFence(self.fenceURL);
    if ([fence[@"phase"] isEqualToString:@"armed"] && [fence[@"feed"] isKindOfClass:NSString.class]) return fence[@"feed"];
#ifdef AINC_UPGRADE_TEST
    const char *feed = getenv("AINC_UPGRADE_TEST_SPARKLE_FEED_URL");
    return feed ? [NSString stringWithUTF8String:feed] : nil;
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
        [history appendString:itemNotes(item, self.current, YES)];
        if ([comparator compareVersion:item.displayVersionString toVersion:self.current] == NSOrderedDescending) {
            [notes appendString:itemNotes(item, self.current, NO)];
        }
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
        if (strcmp(getenv("AINC_UPGRADE_TEST_FROM") ?: "", version) == 0) {
            // Each gate pass starts fresh, without touching production defaults.
            [[[NSUserDefaults alloc] initWithSuiteName:domain] removePersistentDomainForName:domain];
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
    sparkle.fenceURL = installationFenceURL(host);
    sparkle.cacheDirectory = [[relaunchProfileURL(host) URLByDeletingPathExtension] URLByAppendingPathComponent:@"Archives" isDirectory:YES];
    [sparkle pruneArchives];
    // Sparkle's automatic-download switch also consents to install-on-quit.
    // AgentInc has a download-only setting, implemented through user-driver
    // an inert archive cache instead.
    sparkle.updater.automaticallyDownloadsUpdates = NO;
    if (![sparkle.updater startUpdater:&error]) {
        sparkle.message = error.localizedDescription;
        return false;
    }
    sparkle.started = YES;
    sparkle.message = [NSString stringWithFormat:@"AgentInc %@", sparkle.current];
    return true;
}

void ainc_sparkle_check(bool background) {
    if (!sparkle.started) {
        ainc_update_status((sparkle.message ?: @"Updates are available in production builds.").UTF8String,
            (sparkle.current ?: @"").UTF8String, 3);
    } else if (background) [sparkle.updater checkForUpdatesInBackground];
    else if (sparkle.choice || sparkle.preparationFailed || sparkle.cacheFailed || sparkle.download) [sparkle showUpdateInFocus];
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
        | (sparkle.ready && [[sparkle.defaults objectForKey:@"AINCUpdateRemindAfter"] timeIntervalSinceNow] <= 0 ? 8 : 0)
        | (sparkle.updater.canCheckForUpdates || sparkle.choice || sparkle.cacheFailed || sparkle.preparationFailed || sparkle.download ? 16 : 0)
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
    if (!failure) failure = armFence(sparkle.fenceURL, sparkle.item.versionString, pinnedFeed(sparkle.item));
    [sparkle preparedWithError:failure];
    if (failure && ![readFence(sparkle.fenceURL)[@"phase"] isEqualToString:@"armed"]) ainc_update_action(9, false);
}
