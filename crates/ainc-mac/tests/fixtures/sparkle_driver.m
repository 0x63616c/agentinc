// Compile the actual implementation so tests exercise its callbacks and actions,
// including block lifetimes, without an installed framework or production defaults.
#import "../../src/update_window.m"
#include <stdio.h>

static int event;
void ainc_update_action(int action, bool automatic) { event = action; }
static NSApplicationTerminateReply refuseQuit(id self, SEL selector, NSApplication *application) { return NSTerminateCancel; }

@interface TestUpdater : NSObject
@property BOOL automaticallyDownloadsUpdates;
@property BOOL automaticallyChecksForUpdates;
@property NSTimeInterval updateCheckInterval;
@end
@implementation TestUpdater
@end

@interface TestItem : NSObject
@property(copy) NSString *displayVersionString;
@property(copy) NSString *itemDescription;
@property(copy) NSString *itemDescriptionFormat;
@property BOOL informationOnlyUpdate;
@property(strong) NSURL *infoURL;
- (BOOL)isInformationOnlyUpdate;
@end
@implementation TestItem
- (BOOL)isInformationOnlyUpdate { return self.informationOnlyUpdate; }
- (NSString *)versionString { return self.displayVersionString; }
@end

@interface TestState : NSObject
@property SPUUserUpdateStage stage;
@property BOOL userInitiated;
@end
@implementation TestState
@end

@interface TestBundle : NSBundle
@property(copy) NSString *domain;
@end
@implementation TestBundle
- (id)objectForInfoDictionaryKey:(NSString *)key { return [key isEqualToString:@"SUDefaultsDomain"] ? self.domain : nil; }
- (NSString *)bundleIdentifier { return self.domain; }
@end

static NSButton *findButton(NSString *title) {
    for (NSView *view in ui().offer.contentView.subviews) {
        if ([view isKindOfClass:NSButton.class] && [((NSButton *)view).title isEqualToString:title]) return (NSButton *)view;
    }
    NSCAssert(NO, @"missing %@", title);
    return nil;
}

static void offer(AincSparkleDriver *driver, TestItem *item, TestState *state, void (^reply)(SPUUserUpdateChoice)) {
    [driver showUpdateFoundWithAppcastItem:(SUAppcastItem *)item state:(SPUUserUpdateState *)state reply:reply];
    NSCAssert(ui().offer.visible && !ui().offer.releasedWhenClosed, @"AgentInc owns the offer window");
}

int main(void) {
    @autoreleasepool {
        ainc_update_smoke_init();
        sparkle = [AincSparkleDriver new];
        sparkle.current = @"1.0.0";
        sparkle.updater = (SPUUpdater *)[TestUpdater new];
        NSString *driverDomain = [@"test.agentinc.sparkle.driver." stringByAppendingString:NSUUID.UUID.UUIDString];
        sparkle.defaults = [[NSUserDefaults alloc] initWithSuiteName:driverDomain];
        TestItem *item = [TestItem new];
        item.displayVersionString = @"1.1.0";
        item.itemDescription = @"<h2>AgentInc 1.1.0</h2><p>Native notes</p>";
        TestState *state = [TestState new];
        state.userInitiated = YES;
        __block NSUInteger replies = 0;
        __block SPUUserUpdateChoice choice;
        void (^reply)(SPUUserUpdateChoice) = ^(SPUUserUpdateChoice value) { replies++; choice = value; };

        offer(sparkle, item, state, reply);
        [findButton(@"Skip This Version") performClick:nil];
        NSCAssert(replies == 1 && choice == SPUUserUpdateChoiceSkip && !ui().offer, @"skip completes once");
        [sparkle action:1 automatic:NO];
        NSCAssert(replies == 1, @"stale controls cannot repeat replies");

        offer(sparkle, item, state, reply);
        [findButton(@"Remind Me Later") performClick:nil];
        NSCAssert(replies == 2 && choice == SPUUserUpdateChoiceDismiss, @"later uses Sparkle scheduling");

        offer(sparkle, item, state, reply);
        ui().automatic.state = NSControlStateValueOn;
        [ui().automatic performClick:nil];
        NSCAssert(!sparkle.updater.automaticallyDownloadsUpdates && replies == 2, @"checkbox does not consume offer reply");
        [findButton(@"Install Update") performClick:nil];
        NSCAssert(replies == 3 && choice == SPUUserUpdateChoiceInstall, @"download begins through Sparkle reply");

        __block NSUInteger cancellations = 0;
        [sparkle showDownloadInitiatedWithCancellation:^{ cancellations++; }];
        [sparkle showDownloadDidReceiveExpectedContentLength:10];
        [sparkle showDownloadDidReceiveDataOfLength:4];
        [sparkle showDownloadDidReceiveDataOfLength:2];
        NSCAssert(ui().progress.visible && !ui().progress.releasedWhenClosed && ui().bar.doubleValue == 0.6, @"incremental progress retains owned window");
        [ui().cancelButton performClick:nil];
        [sparkle action:4 automatic:NO];
        NSCAssert(cancellations == 1, @"download cancellation completes once");

        [sparkle showDownloadInitiatedWithCancellation:^{ cancellations++; }];
        [sparkle showDownloadDidStartExtractingUpdate];
        [sparkle action:4 automatic:NO];
        NSCAssert(cancellations == 1 && !ui().cancelButton.enabled, @"extraction cannot invoke expired cancellation");
        [sparkle showExtractionReceivedProgress:0.5];
        NSCAssert([ui().bytes.stringValue isEqualToString:@"50%"], @"extraction has meaningful progress");

        sparkle.installRequested = NO;
        [sparkle showReadyToInstallAndRelaunch:reply];
        [findButton(@"Install Update") performClick:nil];
        NSCAssert(event == 8 && sparkle.preparing && replies == 3, @"ready reply waits for real shutdown gate");
        [sparkle preparedWithError:@"Daemon refused drain"];
        NSCAssert(!sparkle.prepared && replies == 3, @"failed drain never releases installation");
        [ui() retry:nil];
        NSCAssert(sparkle.preparing && replies == 3, @"retry re-enters gate");
        [sparkle preparedWithError:nil];
        [sparkle preparedWithError:nil];
        NSCAssert(replies == 4 && choice == SPUUserUpdateChoiceInstall, @"successful drain resumes exactly once");

        sparkle.prepared = NO;
        __block NSUInteger installs = 0;
        BOOL postponed = [sparkle updater:sparkle.updater shouldPostponeRelaunchForUpdate:(SUAppcastItem *)item untilInvokingBlock:^{ installs++; }];
        NSCAssert(postponed && installs == 0, @"delegate postpones relaunch");
        [sparkle preparedWithError:nil];
        NSCAssert(installs == 1, @"delegate resumes after drain");

        sparkle.prepared = NO;
        originalShouldTerminate = (IMP)refuseQuit;
        event = 0;
        NSCAssert(shouldTerminate(nil, @selector(applicationShouldTerminate:), NSApp) == NSTerminateCancel && event == 0, @"termination barrier respects existing delegate refusal");
        originalShouldTerminate = NULL;
        NSCAssert(shouldTerminate(nil, @selector(applicationShouldTerminate:), NSApp) == NSTerminateLater && event == 8, @"ordinary quit of an armed installer uses the same drain gate");
        // The hook was called directly rather than by NSApplication in this test.
        sparkle.terminationWaiting = NO;
        sparkle.preparing = NO;

        sparkle.installArmed = NO;
        sparkle.prepared = NO;
        item.informationOnlyUpdate = YES;
        offer(sparkle, item, state, reply);
        [findButton(@"Learn More") performClick:nil];
        NSCAssert(choice == SPUUserUpdateChoiceDismiss, @"informational update never replies install");
        item.informationOnlyUpdate = NO;

        __block NSUInteger acknowledgements = 0;
        [sparkle showUpdaterError:[NSError errorWithDomain:@"test" code:1 userInfo:@{NSLocalizedDescriptionKey:@"Failed download"}]
            acknowledgement:^{ acknowledgements++; }];
        [ui() dismiss:nil];
        [ui() dismiss:nil];
        NSCAssert(acknowledgements == 1, @"error acknowledgement completes once");

        offer(sparkle, item, state, reply);
        NSUInteger beforeHistory = replies;
        sparkle.history = @"<h2>All releases</h2>";
        [sparkle showHistory];
        [findButton(@"Done") performClick:nil];
        NSCAssert(replies == beforeHistory && sparkle.choice && ui().offer.visible, @"history does not discard pending update choice");
        [sparkle dismissUpdateInstallation];
        NSCAssert(!ui().offer && !ui().progress && !sparkle.choice, @"dismiss clears windows and callbacks");

        // The automatic setting downloads through the same Sparkle reply but
        // does not consent to installation or install-on-quit.
        sparkle.installRequested = NO;
        sparkle.userVisible = NO;
        [sparkle.defaults setBool:YES forKey:@"AINCAutomaticallyDownloadUpdates"];
        state.userInitiated = NO;
        NSUInteger beforeAutomatic = replies;
        [sparkle showUpdateFoundWithAppcastItem:(SUAppcastItem *)item state:(SPUUserUpdateState *)state reply:reply];
        NSCAssert(replies == beforeAutomatic + 1 && choice == SPUUserUpdateChoiceInstall && !ui().offer, @"automatic setting accepts background download");
        [sparkle showDownloadInitiatedWithCancellation:^{ cancellations++; }];
        NSCAssert(!ui().progress, @"background download stays unobtrusive");
        [sparkle showDownloadDidStartExtractingUpdate];
        [sparkle showReadyToInstallAndRelaunch:reply];
        NSCAssert(sparkle.ready && sparkle.choice && replies == beforeAutomatic + 1 && !ui().offer, @"background preparation waits for explicit installation");
        [sparkle cancelBackgroundInstallation];
        NSCAssert(replies == beforeAutomatic + 2 && choice == SPUUserUpdateChoiceSkip, @"ordinary quit cancels prepared background install");
        NSCAssert(![sparkle.defaults stringForKey:@"SUSkippedVersion"], @"ordinary quit does not skip future offers");
        [sparkle dismissUpdateInstallation];
        state.userInitiated = YES;
        state.stage = SPUUserUpdateStageDownloaded;
        offer(sparkle, item, state, reply);
        [findButton(@"Skip This Version") performClick:nil];
        NSCAssert([[sparkle.defaults stringForKey:@"SUSkippedVersion"] isEqualToString:item.displayVersionString], @"explicit skip of a prepared update persists in Sparkle defaults");

        // One-time migration only touches this random test suite.
        TestBundle *bundle = [TestBundle new];
        bundle.domain = [@"test.agentinc.sparkle." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:bundle.domain];
        NSString *path = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSData *legacy = [NSJSONSerialization dataWithJSONObject:@{@"automatic_checks":@YES, @"automatic_download":@YES, @"interval_hours":@168, @"skipped_version":@"1.0.1", @"last_check":@100} options:0 error:nil];
        [legacy writeToFile:path atomically:YES];
        migratePreferences(sparkle.updater, path, bundle);
        NSCAssert(sparkle.updater.automaticallyChecksForUpdates && [defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"] && sparkle.updater.updateCheckInterval == 604800, @"legacy settings migrate");
        NSCAssert([[defaults stringForKey:@"SUSkippedVersion"] isEqualToString:@"1.0.1"], @"legacy skipped version migrates");
        [defaults setBool:NO forKey:@"AINCAutomaticallyDownloadUpdates"];
        migratePreferences(sparkle.updater, path, bundle);
        NSCAssert(![defaults boolForKey:@"AINCAutomaticallyDownloadUpdates"], @"migration never overwrites later settings");
        [defaults removePersistentDomainForName:bundle.domain];
        [NSFileManager.defaultManager removeItemAtPath:path error:nil];
        [sparkle.defaults removePersistentDomainForName:driverDomain];
        sparkle = nil;
    }
    puts("Sparkle driver callbacks passed");
    return 0;
}
