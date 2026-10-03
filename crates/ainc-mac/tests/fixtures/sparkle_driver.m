// Compile the actual implementation so tests exercise its callbacks and actions,
// including block lifetimes, without an installed framework or production defaults.
#import "../../src/update_window.m"
#include <stdio.h>

static int event;
void ainc_update_action(int action, bool automatic) { event = action; }
// Rust's renderer is tested through native_update::tests. These callbacks keep
// this standalone Objective-C protocol/lifetime harness independent of Rust.
char *ainc_update_format_notes(const char *markdown, const char *current, bool history) { return strdup(markdown); }
void ainc_update_free_notes(char *text) { free(text); }

@interface TestUpdater : NSObject
@property BOOL automaticallyDownloadsUpdates;
@property BOOL automaticallyChecksForUpdates;
@property NSTimeInterval updateCheckInterval;
@property BOOL canCheckForUpdates;
@end
@implementation TestUpdater
@end

@interface TestItem : NSObject
@property(copy) NSString *displayVersionString;
@property(copy) NSString *itemDescription;
@property(copy) NSString *itemDescriptionFormat;
@property BOOL informationOnlyUpdate;
@property(strong) NSURL *infoURL;
@property(strong) NSURL *fileURL;
@property(strong) NSDictionary *propertiesDictionary;
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
@property(copy) NSString *path;
@end
@implementation TestBundle
- (id)objectForInfoDictionaryKey:(NSString *)key { return [key isEqualToString:@"SUDefaultsDomain"] ? self.domain : nil; }
- (NSString *)bundleIdentifier { return self.domain; }
- (NSString *)bundlePath { return self.path ?: @"/test/AgentInc.app"; }
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
        item.propertiesDictionary = @{};
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
        NSCAssert(replies == 2 && event == 8 && sparkle.preparing, @"initial install waits for flush and drain BEFORE Sparkle download/preparation");
        [sparkle preparedWithError:@"Daemon refused drain"];
        NSCAssert(replies == 2 && !sparkle.prepared, @"failed initial drain cannot start Sparkle");
        [ui() retry:nil];
        [sparkle preparedWithError:nil];
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
        sparkle.prepared = NO;
        [sparkle showReadyToInstallAndRelaunch:reply];
        [findButton(@"Install Update") performClick:nil];
        NSCAssert(event == 8 && sparkle.preparing && replies == 3, @"ready reply waits for real shutdown gate");
        [sparkle preparedWithError:@"Daemon refused drain"];
        NSCAssert(!sparkle.prepared && replies == 3, @"failed drain never releases installation");
        [ui() dismiss:nil];
        [sparkle showUpdateInFocus];
        NSCAssert(ui().offer.visible, @"checking again restores a dismissed drain failure");
        [ui() retry:nil];
        NSCAssert(sparkle.preparing && replies == 3, @"retry re-enters gate");
        [sparkle preparedWithError:nil];
        [sparkle preparedWithError:nil];
        NSCAssert(replies == 4 && choice == SPUUserUpdateChoiceInstall, @"successful drain resumes exactly once");

        sparkle.prepared = YES;
        __block NSUInteger installs = 0;
        BOOL postponed = [sparkle updater:sparkle.updater shouldPostponeRelaunchForUpdate:(SUAppcastItem *)item untilInvokingBlock:^{ installs++; }];
        NSCAssert(postponed && installs == 0 && sparkle.preparing, @"even a drained installation repeats the final draft flush");
        [sparkle preparedWithError:nil];
        NSCAssert(installs == 1, @"delegate resumes after drain");

        __block NSUInteger terminationRetries = 0;
        [sparkle showInstallingUpdateWithApplicationTerminated:NO retryTerminatingApplication:^{ terminationRetries++; }];
        [ui().cancelButton performClick:nil];
        NSCAssert(sparkle.preparing && !sparkle.prepared && terminationRetries == 0, @"retry quit flushes work again before invoking Sparkle");
        [sparkle preparedWithError:nil];
        NSCAssert(terminationRetries == 1, @"retry quit reaches Sparkle after the barrier");

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

        // A persisted cache never needs an Install reply to become ready.
        sparkle.installRequested = NO;
        sparkle.userVisible = NO;
        [sparkle.defaults setBool:YES forKey:@"AINCAutomaticallyDownloadUpdates"];
        state.userInitiated = NO;
        NSString *cachePath = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        sparkle.cacheDirectory = [NSURL fileURLWithPath:cachePath];
        item.fileURL = [NSURL URLWithString:@"https://fixture.invalid/selected.delta"];
        NSCAssert(!writePrivateData([@"delta bytes" dataUsingEncoding:NSUTF8StringEncoding], [sparkle cachedArchive]), @"seed inert cache");
        NSUInteger beforeAutomatic = replies;
        [sparkle showUpdateFoundWithAppcastItem:(SUAppcastItem *)item state:(SPUUserUpdateState *)state reply:reply];
        NSCAssert(replies == beforeAutomatic && sparkle.ready && sparkle.choice && !sparkle.installArmed && !ui().offer, @"cached background update is inert and ready without Install");
        [sparkle action:2 automatic:YES];
        NSCAssert(!(ainc_sparkle_state() & 8) && replies == beforeAutomatic + 1 && choice == SPUUserUpdateChoiceDismiss, @"later dismisses the initial offer and persists a reminder");
        NSUserDefaults *reloaded = [[NSUserDefaults alloc] initWithSuiteName:driverDomain];
        NSCAssert([[reloaded objectForKey:@"AINCUpdateRemindAfter"] timeIntervalSinceNow] > 0, @"reminder survives driver restart");
        [sparkle showUpdateFoundWithAppcastItem:(SUAppcastItem *)item state:(SPUUserUpdateState *)state reply:reply];
        NSCAssert(replies == beforeAutomatic + 2 && !sparkle.choice, @"persisted reminder dismisses background cycles so future checks keep scheduling");
        state.userInitiated = YES;
        [sparkle showUpdateFoundWithAppcastItem:(SUAppcastItem *)item state:(SPUUserUpdateState *)state reply:reply];
        [sparkle showUpdateInFocus];
        NSCAssert((ainc_sparkle_state() & 8) && ui().offer.visible, @"explicit check restores the held ready offer");
        [findButton(@"Skip This Version") performClick:nil];
        NSCAssert(replies == beforeAutomatic + 3 && choice == SPUUserUpdateChoiceSkip, @"real initial Skip reply owns Sparkle skip persistence");
        NSCAssert(![sparkle hasCachedArchive], @"skip discards cached archive");
        [NSFileManager.defaultManager removeItemAtPath:cachePath error:nil];
        [sparkle dismissUpdateInstallation];

        // Exact-target recovery over a persisted release layout: arm X, the
        // latest feed advances to Y, and the old UI restarts after an
        // ambiguous installer failure.
        AincSparkleDriver *primary = sparkle;
        NSFileManager *files = NSFileManager.defaultManager;
        NSURL *recovery = [NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString] isDirectory:YES];
        NSURL *releaseX = [recovery URLByAppendingPathComponent:@"releases/v2" isDirectory:YES];
        NSURL *latestFeed = [recovery URLByAppendingPathComponent:@"latest/appcast.xml"];
        NSData *feedX = [@"signed appcast for 2" dataUsingEncoding:NSUTF8StringEncoding];
        NSCAssert(!writePrivateData(feedX, [releaseX URLByAppendingPathComponent:@"appcast.xml"]) && !writePrivateData(feedX, latestFeed), @"publish X");
        NSURL *recoveryCache = [recovery URLByAppendingPathComponent:@"cache" isDirectory:YES];
        NSURL *recoveryFence = [recovery URLByAppendingPathComponent:@"install.fence"];
        AincSparkleDriver *(^launch)(NSString *) = ^(NSString *current) {
            AincSparkleDriver *driver = [AincSparkleDriver new];
            driver.current = current;
            driver.updater = (SPUUpdater *)[TestUpdater new];
            driver.defaults = primary.defaults;
            driver.cacheDirectory = recoveryCache;
            driver.fenceURL = recoveryFence;
            sparkle = driver;
            return driver;
        };
        TestItem *(^releaseItem)(NSURL *, NSString *) = ^(NSURL *directory, NSString *version) {
            TestItem *target = [TestItem new];
            target.displayVersionString = version;
            target.propertiesDictionary = @{@"deltaFrom":@"1"};
            target.fileURL = [directory URLByAppendingPathComponent:@"AgentInc-2-from-1.delta"];
            return target;
        };
        NSArray<NSString *> *(^cached)(void) = ^{
            return [[files contentsOfDirectoryAtPath:recoveryCache.path error:nil] sortedArrayUsingSelector:@selector(compare:)];
        };
        [primary.defaults setBool:NO forKey:@"AINCAutomaticallyDownloadUpdates"];
        state.userInitiated = YES;
        AincSparkleDriver *old = launch(@"1");
        old.item = (SUAppcastItem *)releaseItem(releaseX, @"2");
        NSString *archiveX = [old cachedArchive].lastPathComponent;
        NSCAssert(!writePrivateData([@"cached X" dataUsingEncoding:NSUTF8StringEncoding], [old cachedArchive])
            && !writePrivateData([@"superseded" dataUsingEncoding:NSUTF8StringEncoding], [recoveryCache URLByAppendingPathComponent:@"old-AgentInc.tar.gz"])
            && !writePrivateData([@"partial" dataUsingEncoding:NSUTF8StringEncoding], [[old cachedArchive] URLByAppendingPathExtension:NSUUID.UUID.UUIDString]), @"seed cache");
        old.download = [AincArchiveDownload new];
        [old updater:old.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdatesInBackground error:nil];
        NSCAssert(cached().count == 3, @"an active download keeps every cache entry");
        old.download = nil;
        [old updater:old.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdatesInBackground error:nil];
        NSCAssert([cached() isEqualToArray:@[archiveX]], @"a resolved cycle removes superseded archives and interrupted copies, retaining the current item");
        NSCAssert(!writePrivateData([@"partial" dataUsingEncoding:NSUTF8StringEncoding], [[old cachedArchive] URLByAppendingPathExtension:NSUUID.UUID.UUIDString]), @"seed abandoned copy");
        old = launch(@"1");
        [old pruneArchives:NO];
        NSCAssert([cached() isEqualToArray:@[archiveX]], @"unfenced restart keeps downloaded X and removes only the abandoned copy");
        [old updater:old.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdatesInBackground error:[NSError errorWithDomain:NSURLErrorDomain code:NSURLErrorNotConnectedToInternet userInfo:nil]];
        NSCAssert([cached() isEqualToArray:@[archiveX]], @"an offline check keeps downloaded X");
        [old updater:old.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdateInformation error:nil];
        NSCAssert([cached() isEqualToArray:@[archiveX]], @"a history-only probe keeps downloaded X");
        old.item = (SUAppcastItem *)releaseItem(releaseX, @"2");
        NSCAssert(![old feedURLStringForUpdater:old.updater], @"unfenced checks use the bundle's latest feed");
        NSUInteger beforeRecovery = replies;
        [old showUpdateFoundWithAppcastItem:old.item state:(SPUUserUpdateState *)state reply:reply];
        event = 0;
        [findButton(@"Install Update") performClick:nil];
        NSCAssert(event == 8 && replies == beforeRecovery, @"install waits for drain");
        ainc_sparkle_prepared(NULL);
        NSString *pinned = [releaseX URLByAppendingPathComponent:@"appcast.xml"].absoluteString;
        NSDictionary *armed = readFence(recoveryFence);
        NSCAssert(replies == beforeRecovery + 1 && choice == SPUUserUpdateChoiceInstall
            && [armed[@"phase"] isEqualToString:@"armed"] && [armed[@"feed"] isEqualToString:pinned], @"immutable signed appcast is pinned before the Install reply");
        [old updater:old.updater willExtractUpdate:old.item];
        [old updater:old.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdates error:[NSError errorWithDomain:@"fixture" code:1 userInfo:nil]];
        NSCAssert([readFence(recoveryFence) isEqualToDictionary:armed] && [cached() isEqualToArray:@[archiveX]], @"ambiguous failure keeps fence, pinned feed and pending bytes");
        NSCAssert(!writePrivateData([@"signed appcast for 3" dataUsingEncoding:NSUTF8StringEncoding], latestFeed), @"latest advances to Y");
        [ui() dismiss:nil];
        releaseFenceLease(); // UI killed.

        NSCAssert(!restoreFence(recoveryFence, @"1") && [readFence(recoveryFence) isEqualToDictionary:armed], @"old UI restart stays fenced");
        AincSparkleDriver *restarted = launch(@"1");
        [restarted pruneArchives:NO];
        NSCAssert([cached() isEqualToArray:@[archiveX]], @"startup pruning retains the armed target's bytes");
        NSString *routed = [restarted feedURLStringForUpdater:restarted.updater];
        NSCAssert([routed isEqualToString:pinned] && [[NSData dataWithContentsOfURL:[NSURL URLWithString:routed]] isEqualToData:feedX], @"fenced checks obtain X's signed appcast after latest moved to Y");
        TestItem *retried = releaseItem([NSURL URLWithString:routed].URLByDeletingLastPathComponent, @"2");
        [restarted showUpdateFoundWithAppcastItem:(SUAppcastItem *)retried state:(SPUUserUpdateState *)state reply:reply];
        NSCAssert(restarted.ready, @"retried target finds its retained cache");
        event = 0;
        [findButton(@"Install Update") performClick:nil];
        NSCAssert(!restarted.preparationFailed && event == 8, @"the armed target itself can be retried");
        ainc_sparkle_prepared(NULL);
        NSCAssert(replies == beforeRecovery + 2 && choice == SPUUserUpdateChoiceInstall && [readFence(recoveryFence) isEqualToDictionary:armed], @"retry installs X under the same pinned fence");
        [restarted updater:restarted.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdates error:[NSError errorWithDomain:@"fixture" code:2 userInfo:nil]];
        NSCAssert([readFence(recoveryFence) isEqualToDictionary:armed], @"a recovery abort cannot prove the original installer exited");
        NSCAssert(acquireFence(recoveryFence, @"3") != nil, @"Y cannot supersede the armed target");
        [ui() dismiss:nil];
        releaseFenceLease(); // Sparkle terminates the old UI.

        NSCAssert(!restoreFence(recoveryFence, @"2") && ![files fileExistsAtPath:recoveryFence.path], @"matching replacement startup clears recovery metadata");
        AincSparkleDriver *replacement = launch(@"2");
        NSCAssert(![replacement feedURLStringForUpdater:replacement.updater], @"replacement checks the latest feed again, offering Y");
        [replacement pruneArchives:NO];
        NSCAssert([cached() isEqualToArray:@[archiveX]], @"replacement startup alone does not resolve the current update");
        replacement.item = (SUAppcastItem *)releaseItem([recovery URLByAppendingPathComponent:@"releases/v3" isDirectory:YES], @"3");
        NSString *archiveY = [replacement cachedArchive].lastPathComponent;
        NSCAssert(!writePrivateData([@"cached Y" dataUsingEncoding:NSUTF8StringEncoding], [replacement cachedArchive]), @"download Y");
        [replacement updater:replacement.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdatesInBackground error:nil];
        NSCAssert([cached() isEqualToArray:@[archiveY]], @"a successful newer-item cycle prunes superseded X");
        [replacement updater:replacement.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdates error:[NSError errorWithDomain:@"SUSparkleErrorDomain" code:SUNoUpdateError userInfo:nil]];
        NSCAssert(cached().count == 0 && !replacement.item, @"a resolved no-update outcome prunes every archive");
        [files removeItemAtURL:recovery error:nil];
        sparkle = primary;

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

        TestUpdater *fresh = [TestUpdater new];
        bundle.domain = [@"test.agentinc.sparkle.fresh." stringByAppendingString:NSUUID.UUID.UUIDString];
        defaults = [[NSUserDefaults alloc] initWithSuiteName:bundle.domain];
        migratePreferences((SPUUpdater *)fresh, path, bundle);
        NSCAssert(fresh.automaticallyChecksForUpdates, @"fresh installation retains AgentInc's automatic checks default");
        [defaults removePersistentDomainForName:bundle.domain];
        [defaults setBool:NO forKey:@"SUEnableAutomaticChecks"];
        fresh.automaticallyChecksForUpdates = NO;
        migratePreferences((SPUUpdater *)fresh, path, bundle);
        NSCAssert(!fresh.automaticallyChecksForUpdates, @"an explicitly disabled Sparkle preference survives migration");
        [defaults removePersistentDomainForName:bundle.domain];

        // Exercise the production handoff, simulating LaunchServices' missing
        // environment without touching the user's real profile or defaults.
        NSURL *record = [NSURL fileURLWithPath:[path stringByAppendingString:@".relaunch"]];
        setenv("AGENTINC_SESSION_PATH", "/isolated/profile/sessions.json", 1);
        setenv("AINC_DISCOVERY_FILE", "/isolated/profile/api-url", 1);
        setenv("DATABASE_URL", "postgres://fixture/password", 1);
        setenv("AINC_RUNTIME_CONFIG", "{\"endpoint\":\"fixture\"}", 1);
        setenv("AINC_UPGRADE_TEST_SPARKLE_FEED_URL", "https://test-only.invalid/feed", 1);
        setenv("AINC_UPGRADE_TEST_DOWNLOADED_FILE", "/isolated/downloaded", 1);
        setenv("AINC_UPGRADE_TEST_PREPARATION_FILE", "/isolated/preparing", 1);
        NSCAssert(!saveRelaunchProfile(record, @"2"), @"save production relaunch profile");
        NSDictionary *savedRecord = [NSPropertyListSerialization propertyListWithData:[NSData dataWithContentsOfURL:record] options:0 format:nil error:nil];
#ifdef AINC_UPGRADE_TEST
        NSCAssert([savedRecord[@"environment"][@"AINC_UPGRADE_TEST_SPARKLE_FEED_URL"] isEqualToString:@"https://test-only.invalid/feed"], @"fixture record preserves test feed without changing a signed plist");
        NSCAssert([savedRecord[@"environment"][@"AINC_UPGRADE_TEST_DOWNLOADED_FILE"] isEqualToString:@"/isolated/downloaded"]
            && [savedRecord[@"environment"][@"AINC_UPGRADE_TEST_PREPARATION_FILE"] isEqualToString:@"/isolated/preparing"], @"fixture record preserves both gate markers");
        unsetenv("AINC_UPGRADE_TEST_SPARKLE_FEED_URL");
#else
        NSCAssert(!savedRecord[@"environment"][@"AINC_UPGRADE_TEST_SPARKLE_FEED_URL"], @"production record excludes test feed overrides");
        NSCAssert(!savedRecord[@"environment"][@"AINC_UPGRADE_TEST_DOWNLOADED_FILE"] && !savedRecord[@"environment"][@"AINC_UPGRADE_TEST_PREPARATION_FILE"], @"production record excludes fixture marker overrides");
#endif
        NSDictionary *permissions = [NSFileManager.defaultManager attributesOfItemAtPath:record.path error:nil];
        NSCAssert([permissions[NSFilePosixPermissions] unsignedShortValue] == 0600, @"credentials are owner-only");
        unsetenv("AGENTINC_SESSION_PATH");
        unsetenv("DATABASE_URL");
        unsetenv("AINC_RUNTIME_CONFIG");
        setenv("AINC_DISCOVERY_FILE", "/explicit/new-launch/api-url", 1);
        NSCAssert(!restoreRelaunchProfile(record, @"1") && !getenv("AGENTINC_SESSION_PATH"), @"old-version launch does not consume a failed update's profile");
        NSCAssert(!restoreRelaunchProfile(record, @"2"), @"restore replacement profile");
#ifdef AINC_UPGRADE_TEST
        NSCAssert(strcmp(getenv("AINC_UPGRADE_TEST_SPARKLE_FEED_URL"), "https://test-only.invalid/feed") == 0, @"fixture feed survives LaunchServices relaunch");
#endif
        NSCAssert(strcmp(getenv("AGENTINC_SESSION_PATH"), "/isolated/profile/sessions.json") == 0, @"session path survives LaunchServices relaunch");
        NSCAssert(strcmp(getenv("DATABASE_URL"), "postgres://fixture/password") == 0, @"external database configuration survives");
        NSCAssert(strcmp(getenv("AINC_RUNTIME_CONFIG"), "{\"endpoint\":\"fixture\"}") == 0, @"companion runtime override survives");
        NSCAssert(strcmp(getenv("AINC_DISCOVERY_FILE"), "/explicit/new-launch/api-url") == 0, @"explicit new launch overrides win");
        NSCAssert(![NSFileManager.defaultManager fileExistsAtPath:record.path], @"handoff is consumed exactly once");
        NSURL *first = relaunchProfileURL(bundle);
        bundle.path = @"/another/AgentInc.app";
        NSCAssert(![first isEqual:relaunchProfileURL(bundle)], @"separate app copies cannot consume each other's handoff");
        NSURL *fence = [record URLByAppendingPathExtension:@"fence"];
        NSCAssert(!acquireFence(fence, @"2"), @"persist preparing fence before drain");
        NSCAssert(!restoreFence(fence, @"1") && readFence(fence), @"live preparing owner cannot be unfenced by a second UI startup");
        releaseFenceLease(); // Simulated owner crash before the initial reply.
        NSCAssert(!restoreFence(fence, @"1") && !readFence(fence), @"interrupted pre-drain is recoverable without arming Sparkle");
        NSCAssert(!acquireFence(fence, @"2") && !armFence(fence, @"2", nil), @"arm durably before replying Install");
        releaseFenceLease();
        NSCAssert(!restoreFence(fence, @"1") && readFence(fence), @"old UI restart stays fenced after armed crash");
        NSCAssert(acquireFence(fence, @"3") != nil, @"different target cannot erase the pending installation");
        NSCAssert(!acquireFence(fence, @"2") && [readFence(fence)[@"phase"] isEqualToString:@"armed"], @"same-target retry keeps the fence armed");
        releaseFenceLease();
        NSCAssert(!restoreFence(fence, @"2") && !readFence(fence), @"matching replacement clears the startup fence");
        [NSFileManager.defaultManager removeItemAtURL:[fence URLByAppendingPathExtension:@"lock"] error:nil];
        [sparkle.defaults removePersistentDomainForName:driverDomain];
        sparkle = nil;
    }
    puts("Sparkle driver callbacks passed");
    return 0;
}
