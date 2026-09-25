#import <AppKit/AppKit.h>

extern void ainc_update_action(int action, bool automatic);

@interface AincUpdateUI : NSObject
@property(strong) NSWindow *offer;
@property(strong) NSWindow *progress;
@property(strong) NSButton *automatic;
@property(strong) NSProgressIndicator *bar;
@property(strong) NSTextField *bytes;
#ifdef AINC_UPGRADE_TEST
@property(strong) NSButton *installButton;
#endif
@end

@implementation AincUpdateUI
- (void)skip:(id)sender { ainc_update_action(1, self.automatic.state == NSControlStateValueOn); [self.offer close]; self.offer = nil; }
- (void)later:(id)sender { ainc_update_action(2, self.automatic.state == NSControlStateValueOn); [self.offer close]; self.offer = nil; }
- (void)install:(id)sender { ainc_update_action(3, self.automatic.state == NSControlStateValueOn); [self.offer close]; self.offer = nil; }
- (void)cancel:(id)sender { ainc_update_action(4, false); [self.progress close]; self.progress = nil; }
- (void)automaticChanged:(id)sender { ainc_update_action(5, self.automatic.state == NSControlStateValueOn); }
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
#ifndef AINC_UPGRADE_REPRO_PRE_FIX
    result.releasedWhenClosed = NO;
#endif
    result.title = title;
    [result center];
    return result;
}

void ainc_update_offer(const char *version, const char *current, const char *html, bool automatic, bool ready) {
    AincUpdateUI *state = ui();
    [state.offer close];
    [state.progress close];
    NSString *next = [NSString stringWithUTF8String:version];
    NSString *installed = [NSString stringWithUTF8String:current];
    state.offer = window(NSMakeSize(620, 450), @"Software Update", NO);
    NSView *content = state.offer.contentView;
    NSImageView *icon = [[NSImageView alloc] initWithFrame:NSMakeRect(26, 356, 64, 64)];
    icon.image = NSApp.applicationIconImage;
    [content addSubview:icon];
    [content addSubview:label(@"A new version of AgentInc is available!", NSMakeRect(108, 389, 486, 30), [NSFont boldSystemFontOfSize:17])];
    NSString *question = ready ? @"Would you like to install it now?" : @"Would you like to download it now?";
    NSString *description = [NSString stringWithFormat:@"AgentInc %@ is now available—you have %@.\n%@", next, installed, question];
    [content addSubview:label(description, NSMakeRect(108, 348, 480, 40), [NSFont systemFontOfSize:13])];

    NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:NSMakeRect(26, 94, 568, 244)];
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
    [notes.textStorage addAttribute:NSForegroundColorAttributeName value:NSColor.labelColor range:all];
    [notes.textStorage enumerateAttribute:NSLinkAttributeName inRange:all options:0 usingBlock:^(id value, NSRange range, BOOL *stop) {
        if (value) [notes.textStorage addAttribute:NSForegroundColorAttributeName value:NSColor.linkColor range:range];
        (void)stop;
    }];
    notes.verticallyResizable = YES;
    notes.textContainer.widthTracksTextView = YES;
    scroll.documentView = notes;
    [content addSubview:scroll];

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

void ainc_update_status(const char *message) {
    AincUpdateUI *state = ui();
    [state.offer close];
    state.offer = window(NSMakeSize(460, 150), @"Software Update", YES);
    [state.offer.contentView addSubview:label([NSString stringWithUTF8String:message], NSMakeRect(28, 72, 404, 52), [NSFont systemFontOfSize:14])];
    [state.offer makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

void ainc_update_progress(const char *message, unsigned long long received, unsigned long long total) {
    AincUpdateUI *state = ui();
    [state.offer close]; state.offer = nil;
    if (!state.progress) {
        state.progress = window(NSMakeSize(470, 180), @"Updating AgentInc", NO);
        NSView *content = state.progress.contentView;
        NSImageView *icon = [[NSImageView alloc] initWithFrame:NSMakeRect(24, 92, 58, 58)];
        icon.image = NSApp.applicationIconImage;
        [content addSubview:icon];
        [content addSubview:label(@"Updating AgentInc", NSMakeRect(98, 122, 350, 28), [NSFont boldSystemFontOfSize:17])];
        [content addSubview:label(@"Downloading update...", NSMakeRect(98, 94, 350, 24), [NSFont systemFontOfSize:13])];
        state.bar = [[NSProgressIndicator alloc] initWithFrame:NSMakeRect(24, 72, 422, 16)];
        state.bar.indeterminate = NO;
        state.bar.minValue = 0; state.bar.maxValue = 1;
        [content addSubview:state.bar];
        state.bytes = label(@"", NSMakeRect(25, 39, 320, 22), [NSFont systemFontOfSize:12]);
        [content addSubview:state.bytes];
        [content addSubview:button(@"Cancel", NSMakeRect(354, 20, 92, 30), state, @selector(cancel:))];
        [state.progress makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
    }
    state.bar.doubleValue = total ? (double)received / (double)total : 0;
    state.bytes.stringValue = [NSString stringWithFormat:@"%.1f MB of %.1f MB", received / 1000000.0, total / 1000000.0];
    (void)message;
}

void ainc_update_close(void) {
    [ui().offer close]; ui().offer = nil;
    [ui().progress close]; ui().progress = nil;
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
