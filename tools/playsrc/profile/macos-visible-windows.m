#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>

int main(void) {
  @autoreleasepool {
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
    NSMutableArray *output = [NSMutableArray array];
    for (NSDictionary *window in windows) {
      [output addObject:@{ @"id": window[(id)kCGWindowNumber], @"pid": window[(id)kCGWindowOwnerPID],
        @"owner": window[(id)kCGWindowOwnerName] ?: @"", @"layer": window[(id)kCGWindowLayer],
        @"alpha": window[(id)kCGWindowAlpha], @"bounds": window[(id)kCGWindowBounds] }];
    }
    NSMutableArray *screens = [NSMutableArray array];
    for (NSScreen *screen in NSScreen.screens) {
      CGDirectDisplayID display = [screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue];
      CGRect rect = CGDisplayBounds(display);
      [screens addObject:@{ @"X": @(rect.origin.x), @"Y": @(rect.origin.y), @"Width": @(rect.size.width), @"Height": @(rect.size.height) }];
    }
    NSData *json = [NSJSONSerialization dataWithJSONObject:@{ @"windows": output, @"screens": screens } options:0 error:nil];
    if (!json) return 1;
    fwrite(json.bytes, 1, json.length, stdout);
  }
  return 0;
}
