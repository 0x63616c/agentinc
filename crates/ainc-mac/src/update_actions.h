//! Native update callback ABI; mirrored by native_update::NativeAction.
#pragma once
#import <Foundation/Foundation.h>
#include <stdbool.h>

typedef NS_ENUM(int, AincUpdateAction) {
    AincUpdateActionSkip = 1,
    AincUpdateActionLater = 2,
    AincUpdateActionInstall = 3,
    AincUpdateActionCancelDownload = 4,
    AincUpdateActionAutomaticChanged = 5,
    AincUpdateActionRetry = 6,
    AincUpdateActionDismiss = 7,
    AincUpdateActionPrepareInstall = 8,
    AincUpdateActionReleaseInstall = 9,
    AincUpdateActionDownloaded = 10,
};

extern void ainc_update_action(AincUpdateAction action, bool automatic);
