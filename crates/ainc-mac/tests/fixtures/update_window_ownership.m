#import <AppKit/AppKit.h>
#include <stdbool.h>
#include <stdio.h>

void ainc_update_smoke_init(void);
void ainc_update_status(const char *message);
void ainc_update_offer(const char *version, const char *current, const char *html, bool automatic, bool ready);
void ainc_update_progress(const char *message, unsigned long long received, unsigned long long total);
void ainc_update_close(void);

void ainc_update_action(int action, bool automatic) {
    (void)action;
    (void)automatic;
}

int main(void) {
    @autoreleasepool {
        ainc_update_smoke_init();
        ainc_update_status("Checking for updates...");
        ainc_update_offer("0.3.2", "0.3.1", "<html><body>Update available</body></html>", false, false);
        ainc_update_progress("Downloading update...", 1, 2);
        ainc_update_status("Update verified");
        ainc_update_close();
    }
    puts("update window ownership passed");
    return 0;
}
