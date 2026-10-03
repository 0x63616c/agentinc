// Reuse the real implementation and protocol doubles from the callback fixture.
// This executable runs a single outer AppKit loop; no sleeps or nested loops.
#define main callbackHarnessMain
#import "sparkle_driver.m"
#undef main

@interface CacheTestDriver : AincSparkleDriver
@property(copy) void (^downloaded)(void);
@end
@implementation CacheTestDriver
- (void)cacheCompleted {
    [super cacheCompleted];
    if (self.downloaded) self.downloaded();
}
@end

static AincArchiveServer *origin;
static NSURL *root;
static NSString *domain;
static NSData *archiveBytes;
static NSUInteger installReplies;

static void finish(void) {
    [origin stop];
    [sparkle.archiveServer stop];
    [sparkle.defaults removePersistentDomainForName:domain];
    removeFence(sparkle.fenceURL);
    [NSFileManager.defaultManager removeItemAtURL:root error:nil];
    puts("Sparkle inert HTTP cache passed");
    [NSApp stop:nil];
    [NSApp postEvent:[NSEvent otherEventWithType:NSEventTypeApplicationDefined location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil subtype:0 data1:0 data2:0] atStart:NO];
}

static void verifyCachedRequest(void) {
    NSCAssert(installReplies == 0 && sparkle.ready && !sparkle.prepared && !readFence(sparkle.fenceURL), @"background download has no install reply, drain, fence or extraction");
    NSCAssert(sparkle.received == archiveBytes.length && sparkle.expected == archiveBytes.length, @"real NSURLSession progress reports selected delta bytes");
    NSCAssert([[[sparkle cachedArchive] pathExtension] isEqualToString:@"delta"], @"selected delta is cached, not the full archive");
    // A fresh driver can discover the atomic persisted cache with the same item.
    AincSparkleDriver *restarted = [AincSparkleDriver new];
    restarted.cacheDirectory = sparkle.cacheDirectory;
    restarted.item = sparkle.item;
    NSCAssert([restarted hasCachedArchive], @"cache survives driver restart");
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:sparkle.item.fileURL];
    [sparkle updater:sparkle.updater willDownloadUpdate:sparkle.item withRequest:request];
    NSCAssert(![request.URL isEqual:sparkle.item.fileURL] && [request.URL.host isEqualToString:@"127.0.0.1"], @"public mutable request hook serves cache on loopback");
    TestItem *full = [TestItem new];
    full.fileURL = [NSURL URLWithString:@"https://fixture.invalid/full.zip"];
    NSMutableURLRequest *fallback = [NSMutableURLRequest requestWithURL:full.fileURL];
    [sparkle updater:sparkle.updater willDownloadUpdate:(SUAppcastItem *)full withRequest:fallback];
    NSCAssert([fallback.URL isEqual:full.fileURL], @"Sparkle's full fallback retains its original URL");
    [[[NSURLSession sharedSession] dataTaskWithRequest:request completionHandler:^(NSData *data, NSURLResponse *response, NSError *error) {
        NSCAssert(!error && [(NSHTTPURLResponse *)response statusCode] == 200 && [data isEqualToData:archiveBytes], @"real HTTP returns exact cached bytes for Sparkle verification");
        dispatch_async(dispatch_get_main_queue(), ^{
            [sparkle action:3 automatic:YES];
            NSCAssert(event == 8 && installReplies == 0 && [readFence(sparkle.fenceURL)[@"phase"] isEqualToString:@"preparing"], @"explicit click persists fence and waits for Rust drain");
            ainc_sparkle_prepared("drain failed");
            NSCAssert(installReplies == 0 && !readFence(sparkle.fenceURL), @"pre-reply drain failure safely releases only preparing fence");
            [sparkle action:6 automatic:YES];
            ainc_sparkle_prepared(NULL);
            NSCAssert(installReplies == 1 && [readFence(sparkle.fenceURL)[@"phase"] isEqualToString:@"armed"], @"successful drain durably arms before first Install reply");
            [sparkle showDownloadInitiatedWithCancellation:^{}];
            [sparkle action:4 automatic:YES];
            NSCAssert(readFence(sparkle.fenceURL) != nil, @"cancellation request alone never clears armed fence");
            [sparkle updater:sparkle.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdates error:nil];
            NSCAssert(!readFence(sparkle.fenceURL) && event == 9, @"confirmed cancellation releases fence even for Sparkle's nil-error abort");
            sparkle.choice = ^(SPUUserUpdateChoice choice) { NSCAssert(choice == SPUUserUpdateChoiceInstall, @"retry installs"); };
            [sparkle action:3 automatic:YES];
            ainc_sparkle_prepared(NULL);
            [sparkle updater:sparkle.updater willExtractUpdate:sparkle.item];
            [sparkle updater:sparkle.updater didFinishUpdateCycleForUpdateCheck:SPUUpdateCheckUpdates error:[NSError errorWithDomain:@"fixture" code:1 userInfo:nil]];
            NSCAssert(readFence(sparkle.fenceURL) && !sparkle.prepared, @"post-extraction failure cannot prove installer death and remains fenced for retry");
            releaseFenceLease();
            NSCAssert(!restoreFence(sparkle.fenceURL, @"1") && readFence(sparkle.fenceURL), @"old startup remains blocked after ambiguous installer failure");
            finish();
        });
    }] resume];
}

int main(void) {
    @autoreleasepool {
        ainc_update_smoke_init();
        root = [NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString] isDirectory:YES];
        NSURL *source = [root URLByAppendingPathComponent:@"selected.delta"];
        archiveBytes = [@"exact selected delta bytes; Sparkle has not verified them" dataUsingEncoding:NSUTF8StringEncoding];
        NSCAssert(!writePrivateData(archiveBytes, source), @"source fixture");
        origin = [[AincArchiveServer alloc] initWithArchive:source];
        NSCAssert(origin != nil, @"ephemeral HTTP source");
        CacheTestDriver *driver = [CacheTestDriver new];
        sparkle = driver;
        driver.current = @"1";
        driver.updater = (SPUUpdater *)[TestUpdater new];
        domain = [@"test.agentinc.cache." stringByAppendingString:NSUUID.UUID.UUIDString];
        driver.defaults = [[NSUserDefaults alloc] initWithSuiteName:domain];
        [driver.defaults setBool:YES forKey:@"AINCAutomaticallyDownloadUpdates"];
        driver.cacheDirectory = [root URLByAppendingPathComponent:@"cache"];
        driver.fenceURL = [root URLByAppendingPathComponent:@"install.fence"];
        driver.downloaded = ^{ verifyCachedRequest(); };
        TestItem *item = [TestItem new];
        item.displayVersionString = @"2";
        item.propertiesDictionary = @{@"deltaFrom":@"1", @"signature":@"signed-enclosure-identity"};
        item.fileURL = origin.url;
        TestState *state = [TestState new];
        state.stage = SPUUserUpdateStageNotDownloaded;
        state.userInitiated = NO;
        NSURL *cancelPath = [root URLByAppendingPathComponent:@"canceled.delta"];
        AincArchiveDownload *canceled = [AincArchiveDownload new];
        canceled.completion = ^(NSError *error) {
            NSCAssert(error.code == NSURLErrorCancelled && ![NSFileManager.defaultManager fileExistsAtPath:cancelPath.path], @"canceled real download cannot publish a cache entry");
            NSURL *failedPath = [root URLByAppendingPathComponent:@"failed.delta"];
            AincArchiveDownload *failed = [AincArchiveDownload new];
            failed.completion = ^(NSError *failure) {
                NSCAssert(failure && ![NSFileManager.defaultManager fileExistsAtPath:failedPath.path], @"HTTP rejection cannot publish a cache entry");
                [driver showUpdateFoundWithAppcastItem:(SUAppcastItem *)item state:(SPUUserUpdateState *)state reply:^(SPUUserUpdateChoice choice) {
                    NSCAssert(choice == SPUUserUpdateChoiceInstall && sparkle.prepared && [readFence(sparkle.fenceURL)[@"phase"] isEqualToString:@"armed"], @"callback order proves durable fence/drain before Install");
                    installReplies++;
                }];
                NSCAssert(installReplies == 0 && driver.download != nil, @"retry begins independent downloader, not Sparkle installation");
            };
            [failed start:[origin.url URLByAppendingPathComponent:@"missing"] destination:failedPath];
        };
        [canceled start:origin.url destination:cancelPath];
        [canceled cancel];
        [NSApp run];
    }
    return 0;
}
