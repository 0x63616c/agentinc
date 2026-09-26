// Reads the macOS calendar store (EventKit) for AgentInc's Calendar import.
// EventKit covers every calendar the Mac syncs, including iCloud and Google.
#import <AppKit/AppKit.h>
#import <EventKit/EventKit.h>

static EKEventStore *agentinc_store(void) {
    static EKEventStore *store;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ store = [[EKEventStore alloc] init]; });
    return store;
}

// EKAuthorizationStatus: 0 not determined, 1 restricted, 2 denied, 3 full access, 4 write only.
int agentinc_calendar_status(void) {
    return (int)[EKEventStore authorizationStatusForEntityType:EKEntityTypeEvent];
}

typedef void (*agentinc_calendar_granted)(void *context, int granted);

// Shows the system prompt once; later calls answer from the stored decision.
void agentinc_calendar_request(void *context, agentinc_calendar_granted callback) {
    [agentinc_store() requestFullAccessToEventsWithCompletion:^(BOOL granted, NSError *error) {
        (void)error;
        [agentinc_store() reset];
        callback(context, granted ? 1 : 0);
    }];
}

static NSString *agentinc_hex(NSColor *color) {
    NSColor *rgb = [color colorUsingColorSpace:[NSColorSpace sRGBColorSpace]];
    if (rgb == nil) return nil;
    return [NSString stringWithFormat:@"#%02X%02X%02X",
        (int)lround(rgb.redComponent * 255), (int)lround(rgb.greenComponent * 255),
        (int)lround(rgb.blueComponent * 255)];
}

// JSON array of events overlapping [start, end) in Unix seconds, or NULL
// without access. Free the result with agentinc_calendar_free.
char *agentinc_calendar_events(double start, double end) {
    if (agentinc_calendar_status() != EKAuthorizationStatusFullAccess) return NULL;
    @autoreleasepool {
        EKEventStore *store = agentinc_store();
        NSPredicate *predicate = [store
            predicateForEventsWithStartDate:[NSDate dateWithTimeIntervalSince1970:start]
                                    endDate:[NSDate dateWithTimeIntervalSince1970:end]
                                  calendars:nil];
        NSCalendar *gregorian = [NSCalendar currentCalendar];
        NSMutableArray *events = [NSMutableArray array];
        for (EKEvent *event in [store eventsMatchingPredicate:predicate]) {
            NSDate *from = event.startDate, *to = event.endDate;
            if (from == nil || to == nil) continue;
            if (event.allDay) {
                // EventKit ends an all-day event just before midnight; AgentInc
                // ends it at the next local midnight.
                from = [gregorian startOfDayForDate:from];
                to = [gregorian dateByAddingUnit:NSCalendarUnitDay value:1
                                          toDate:[gregorian startOfDayForDate:to] options:0];
            }
            long long startsAt = (long long)floor(from.timeIntervalSince1970);
            NSString *identifier = event.calendarItemExternalIdentifier ?: event.eventIdentifier;
            if (identifier == nil) continue;
            NSMutableDictionary *item = [@{
                @"external_id": [NSString stringWithFormat:@"%@@%lld", identifier,
                    (long long)floor(event.startDate.timeIntervalSince1970)],
                @"calendar": event.calendar.title ?: @"Calendar",
                @"title": event.title ?: @"",
                @"starts_at": @(startsAt),
                @"ends_at": @((long long)floor(to.timeIntervalSince1970)),
                @"all_day": @(event.allDay),
            } mutableCopy];
            NSString *color = event.calendar.color ? agentinc_hex(event.calendar.color) : nil;
            if (color) item[@"color"] = color;
            if (event.location.length) item[@"location"] = event.location;
            if (event.notes.length) item[@"notes"] = event.notes;
            [events addObject:item];
        }
        NSData *json = [NSJSONSerialization dataWithJSONObject:events options:0 error:nil];
        if (json == nil) return NULL;
        char *out = malloc(json.length + 1);
        memcpy(out, json.bytes, json.length);
        out[json.length] = 0;
        return out;
    }
}

void agentinc_calendar_free(char *json) { free(json); }
