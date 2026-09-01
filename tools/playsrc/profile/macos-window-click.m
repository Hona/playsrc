#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    if (argc != 7 || !CGPreflightPostEventAccess()) return 2;
    pid_t pid = (pid_t)strtol(argv[1], NULL, 10);
    CGWindowID identity = (CGWindowID)strtoul(argv[2], NULL, 10);
    CGRect expected = CGRectMake(strtod(argv[3], NULL), strtod(argv[4], NULL), strtod(argv[5], NULL), strtod(argv[6], NULL));
    if (pid <= 0 || !identity || !isfinite(expected.origin.x) || !isfinite(expected.origin.y) ||
        !isfinite(expected.size.width) || !isfinite(expected.size.height) || expected.size.width <= 0 || expected.size.height <= 0) return 3;
    NSDictionary *session = CFBridgingRelease(CGSessionCopyCurrentDictionary());
    if (![session[(id)kCGSessionOnConsoleKey] boolValue] || [session[@"CGSSessionScreenIsLocked"] boolValue] ||
        NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != pid) return 4;
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
    BOOL found = NO;
    for (NSDictionary *window in windows) {
      CGRect bounds;
      if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)window[(id)kCGWindowBounds], &bounds)) return 5;
      if ([window[(id)kCGWindowNumber] unsignedIntValue] == identity) {
        if ([window[(id)kCGWindowOwnerPID] intValue] != pid || !CGRectEqualToRect(bounds, expected)) return 6;
        found = YES;
        break;
      }
      BOOL cursor = [window[(id)kCGWindowLayer] intValue] == CGWindowLevelForKey(kCGCursorWindowLevelKey) &&
          [window[(id)kCGWindowOwnerName] isEqualToString:@"Window Server"];
      if (!cursor && [window[(id)kCGWindowAlpha] doubleValue] > 0 && CGRectIntersectsRect(bounds, expected)) return 7;
    }
    if (!found) return 8;
    CGPoint point = CGPointMake(CGRectGetMidX(expected), CGRectGetMidY(expected));
    for (int up = 0; up < 2; up++) {
      CGEventRef event = CGEventCreateMouseEvent(NULL, up ? kCGEventLeftMouseUp : kCGEventLeftMouseDown, point, kCGMouseButtonLeft);
      if (!event) return 9;
      CGEventSetFlags(event, 0);
      CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
      CGEventPost(kCGHIDEventTap, event);
      CFRelease(event);
    }
    printf("{\"pid\":%d,\"windowId\":%u,\"x\":%.17g,\"y\":%.17g,\"posted\":true}", pid, identity, point.x, point.y);
  }
  return 0;
}
