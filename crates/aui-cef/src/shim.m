// AppKit glue for aui-cef. Each piece was proven in spike S1 (Baaz
// docs/24b-spike-s1.md); the patterns come from cmux PR #10197's stock-CEF
// embedding (a frameless Chrome-style Views window adopted as a child window).
#import <AppKit/AppKit.h>
#import <objc/runtime.h>

// MARK: - NSApp conformance (CefAppProtocol) on the host's NSApplication class

static const void *kHandlingKey = &kHandlingKey;
static IMP g_original_send_event = NULL;

static BOOL aui_is_handling(id self, SEL _cmd) {
  return [objc_getAssociatedObject(self, kHandlingKey) boolValue];
}

static void aui_set_handling(id self, SEL _cmd, BOOL handling) {
  objc_setAssociatedObject(self, kHandlingKey, @(handling), OBJC_ASSOCIATION_RETAIN_NONATOMIC);
}

static void aui_send_event(id self, SEL _cmd, NSEvent *event) {
  BOOL previous = aui_is_handling(self, _cmd);
  aui_set_handling(self, _cmd, YES);
  ((void (*)(id, SEL, NSEvent *))g_original_send_event)(self, _cmd, event);
  aui_set_handling(self, _cmd, previous);
}

// Chromium asks NSApp `isHandlingSendEvent`; gpui's GPUIApplication does not
// implement it. Add the methods and wrap `sendEvent:` on the app's own class
// (a subclass), leaving NSApplication untouched. Returns a bitmask for the
// log: 1 methods added, 2 sendEvent wrapped on the subclass, 4/8/16 the
// Cr/CrAppControl/Cef protocols found at runtime and adopted.
int aui_cef_install_app_conformance(void) {
  int report = 0;
  Class c = [NSApp class];
  if (!class_getInstanceMethod(c, @selector(isHandlingSendEvent))) {
    class_addMethod(c, @selector(isHandlingSendEvent), (IMP)aui_is_handling, "B@:");
    class_addMethod(c, @selector(setHandlingSendEvent:), (IMP)aui_set_handling, "v@:B");
    report |= 1;
    Method m = class_getInstanceMethod(c, @selector(sendEvent:));
    g_original_send_event = method_getImplementation(m);
    if (class_addMethod(c, @selector(sendEvent:), (IMP)aui_send_event, method_getTypeEncoding(m))) {
      report |= 2;
    } else {
      method_setImplementation(m, (IMP)aui_send_event);
    }
  }
  Protocol *p;
  if ((p = objc_getProtocol("CrAppProtocol"))) { class_addProtocol(c, p); report |= 4; }
  if ((p = objc_getProtocol("CrAppControlProtocol"))) { class_addProtocol(c, p); report |= 8; }
  if ((p = objc_getProtocol("CefAppProtocol"))) { class_addProtocol(c, p); report |= 16; }
  return report;
}

// MARK: - External message pump (cefclient MainMessageLoopExternalPumpMac)

static void (*g_work)(void) = NULL;
static CFRunLoopTimerRef g_timer = NULL;
static BOOL g_in_work = NO;
static const double kMaxDelay = 1.0 / 30.0;

static void timer_fired(CFRunLoopTimerRef timer, void *info) {
  if (!g_work || g_in_work) return;
  g_in_work = YES;
  g_work();
  g_in_work = NO;
}

void aui_cef_pump_start(void (*work)(void)) {
  g_work = work;
  if (g_timer) return;
  CFRunLoopTimerContext ctx = {0};
  g_timer = CFRunLoopTimerCreate(NULL, CFAbsoluteTimeGetCurrent() + kMaxDelay, kMaxDelay, 0, 0,
                                 timer_fired, &ctx);
  CFRunLoopAddTimer(CFRunLoopGetMain(), g_timer, kCFRunLoopCommonModes);
}

// Any thread (CEF's OnScheduleMessagePumpWork).
void aui_cef_pump_schedule(int64_t delay_ms) {
  double delay = delay_ms <= 0 ? 0 : MIN(delay_ms / 1000.0, kMaxDelay);
  CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopCommonModes, ^{
    if (g_timer) CFRunLoopTimerSetNextFireDate(g_timer, CFAbsoluteTimeGetCurrent() + delay);
  });
  CFRunLoopWakeUp(CFRunLoopGetMain());
}

void aui_cef_pump_stop(void) {
  if (g_timer) {
    CFRunLoopTimerInvalidate(g_timer);
    CFRelease(g_timer);
    g_timer = NULL;
  }
  g_work = NULL;
}

// MARK: - The page's child window

static NSWindow *window_of(void *view) {
  return view ? ((__bridge NSView *)view).window : nil;
}

// Forward: defined with the never-main flag below; called on adoption above.
void aui_cef_flatten_window(void *child_root_view);

// Places the page's window over `x, y, w, h` — the parent view's coordinates,
// top-left origin, logical pixels (what gpui lays out in) — adopting it as a
// child window first. Returns 1 when placed.
int aui_cef_place(void *parent_view, void *child_root_view, double x, double y, double w, double h) {
  NSView *pv = (__bridge NSView *)parent_view;
  NSWindow *pw = pv.window;
  NSWindow *cw = window_of(child_root_view);
  if (!pw || !cw || pw == cw) return 0;
  if (w < 1 || h < 1) { w = 1; h = 1; }
  NSRect r = pv.isFlipped ? NSMakeRect(x, y, w, h) : NSMakeRect(x, NSHeight(pv.bounds) - y - h, w, h);
  NSRect onScreen = [pw convertRectToScreen:[pv convertRect:r toView:nil]];
  if (cw.parentWindow != pw) {
    [pw addChildWindow:cw ordered:NSWindowAbove];
    aui_cef_flatten_window(child_root_view);
  }
  if (!NSEqualRects(cw.frame, onScreen)) [cw setFrame:onScreen display:YES];
  return 1;
}

// Hidden = invisible and click-through. CEF's own `Window::hide` does nothing
// to an adopted child window (S1, shot 7); alpha does (S1, shot 6).
void aui_cef_set_hidden(void *child_root_view, int hidden) {
  NSWindow *cw = window_of(child_root_view);
  if (!cw) return;
  cw.alphaValue = hidden ? 0.0 : 1.0;
  cw.ignoresMouseEvents = hidden ? YES : NO;
}

int aui_cef_is_key(void *child_root_view) {
  NSWindow *cw = window_of(child_root_view);
  return cw && cw.isKeyWindow ? 1 : 0;
}

// Hands the keyboard back to the host window.
void aui_cef_focus_parent(void *parent_view) {
  NSView *pv = (__bridge NSView *)parent_view;
  NSWindow *pw = pv.window;
  if (!pw) return;
  [pw makeKeyAndOrderFront:nil];
  [pw makeFirstResponder:pv];
}

// Gives the page window the keyboard, as a click into the page would. CEF's
// `set_focus` only moves focus inside the browser; the NSWindow must be key
// too, or keystrokes keep going to the host (B1b, live probe).
void aui_cef_focus_child(void *child_root_view) {
  NSWindow *cw = window_of(child_root_view);
  if (cw && !cw.isKeyWindow) [cw makeKeyWindow];
}

// Detach before closing, so the parent never holds a dying child.
void aui_cef_detach(void *child_root_view) {
  NSWindow *cw = window_of(child_root_view);
  if (cw && cw.parentWindow) [cw.parentWindow removeChildWindow:cw];
}

// Runs `f(ctx)` on the main queue's next turn: outside whatever host update
// (a gpui entity update, a render) asked for it. Creating and showing a page
// window synchronously inside a gpui update lost that update (B1a, live).
void aui_cef_defer(void (*f)(void *), void *ctx) {
  dispatch_async(dispatch_get_main_queue(), ^{ f(ctx); });
}

// MARK: - The page window never becomes main (B1b-L2)

static const void *kNeverMainKey = &kNeverMainKey;

static NSMutableSet<NSString *> *g_patched_classes = nil;
static NSMutableDictionary<NSString *, NSValue *> *g_original_imps = nil;

static BOOL aui_flagged_can_become_main(id self, SEL _cmd) {
  if ([objc_getAssociatedObject(self, kNeverMainKey) boolValue]) return NO;
  NSString *name = NSStringFromClass([self class]);
  NSValue *boxed = [g_original_imps objectForKey:name];
  IMP orig = boxed ? (IMP)[boxed pointerValue] : NULL;
  if (orig) return ((BOOL (*)(id, SEL))orig)(self, _cmd);
  Class sup = class_getSuperclass([self class]);
  Method m = sup ? class_getInstanceMethod(sup, _cmd) : NULL;
  if (m) {
    IMP imp = method_getImplementation(m);
    if (imp && imp != (IMP)aui_flagged_can_become_main) return ((BOOL (*)(id, SEL))imp)(self, _cmd);
  }
  return YES;
}

// Flags THAT window instance and swizzles its class (once per class) so
// `canBecomeMainWindow` → NO on flagged instances, so the host window keeps
// its coloured traffic lights while the page has focus. Flagged per instance:
// other CEF windows (DevTools, popups) of the same class lack the associated-
// object flag and keep the original behaviour. `canBecomeKey` is untouched,
// so typing into the page keeps working. Never calls `object_setClass`.
void aui_cef_never_main(void *child_root_view) {
  NSWindow *cw = window_of(child_root_view);
  if (!cw) return;
  objc_setAssociatedObject(cw, kNeverMainKey, @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
  Class cls = [cw class];
  NSString *name = NSStringFromClass(cls);
  if (!g_patched_classes) g_patched_classes = [NSMutableSet set];
  if (!g_original_imps) g_original_imps = [NSMutableDictionary dictionary];
  if ([g_patched_classes containsObject:name]) return;
  SEL sel = @selector(canBecomeMainWindow);
  Method m = class_getInstanceMethod(cls, sel);
  const char *types = m ? method_getTypeEncoding(m) : "B@:";
  IMP prev = class_replaceMethod(cls, sel, (IMP)aui_flagged_can_become_main, types);
  [g_original_imps setObject:[NSValue valueWithPointer:prev] forKey:name];
  [g_patched_classes addObject:name];
}

// MARK: - Flat page windows: no shadow, no corner rounding (R1)

static const void *kFlatKey = &kFlatKey;
static const void *kFlatLoggedKey = &kFlatLoggedKey;

static NSMutableSet<NSString *> *g_flat_patched = nil;
static NSMutableDictionary<NSString *, NSValue *> *g_flat_radius_imps = nil;
static NSMutableDictionary<NSString *, NSValue *> *g_flat_mask_imps = nil;

static CGFloat aui_flagged_corner_radius(id self, SEL _cmd) {
  if ([objc_getAssociatedObject(self, kFlatKey) boolValue]) return 0.0;
  NSString *name = NSStringFromClass([self class]);
  NSValue *boxed = [g_flat_radius_imps objectForKey:name];
  IMP orig = boxed ? (IMP)[boxed pointerValue] : NULL;
  if (orig) return ((CGFloat (*)(id, SEL))orig)(self, _cmd);
  return 0.0;
}

static id aui_flagged_corner_mask(id self, SEL _cmd) {
  if ([objc_getAssociatedObject(self, kFlatKey) boolValue]) return nil;
  NSString *key = [NSString stringWithFormat:@"%@:%@", NSStringFromClass([self class]), NSStringFromSelector(_cmd)];
  NSValue *boxed = [g_flat_mask_imps objectForKey:key];
  IMP orig = boxed ? (IMP)[boxed pointerValue] : NULL;
  if (orig) return ((id (*)(id, SEL))orig)(self, _cmd);
  return nil;
}

static NSMutableDictionary<NSString *, NSValue *> *g_flat_key_imps = nil;

// Borderless windows answer NO to `canBecomeKeyWindow` by default; a flat page
// window must still take the keyboard (typing into the page).
static BOOL aui_flagged_can_become_key(id self, SEL _cmd) {
  if ([objc_getAssociatedObject(self, kFlatKey) boolValue]) return YES;
  NSValue *boxed = [g_flat_key_imps objectForKey:NSStringFromClass([self class])];
  IMP orig = boxed ? (IMP)[boxed pointerValue] : NULL;
  if (orig) return ((BOOL (*)(id, SEL))orig)(self, _cmd);
  Class sup = class_getSuperclass([self class]);
  Method m = sup ? class_getInstanceMethod(sup, _cmd) : NULL;
  if (m) {
    IMP imp = method_getImplementation(m);
    if (imp && imp != (IMP)aui_flagged_can_become_key) return ((BOOL (*)(id, SEL))imp)(self, _cmd);
  }
  return NO;
}

static void aui_swizzle_can_become_key_once(Class cls, NSString *name) {
  if (!g_flat_key_imps) g_flat_key_imps = [NSMutableDictionary dictionary];
  if ([g_flat_key_imps objectForKey:name]) return;
  SEL sel = @selector(canBecomeKeyWindow);
  Method m = class_getInstanceMethod(cls, sel);
  const char *types = m ? method_getTypeEncoding(m) : "B@:";
  IMP prev = class_replaceMethod(cls, sel, (IMP)aui_flagged_can_become_key, types);
  [g_flat_key_imps setObject:[NSValue valueWithPointer:prev] forKey:name];
}

// Swizzles one private corner selector on `cls` (once per class+selector),
// guarded so a missing selector is a no-op, never a crash. Never touches the
// style mask (borderless would break `canBecomeKeyWindow` and typing) and
// never calls `object_setClass`.
static BOOL aui_swizzle_corner_once(Class cls, NSString *name, SEL sel, BOOL isRadius) {
  if (![cls instancesRespondToSelector:sel]) return NO;
  Method m = class_getInstanceMethod(cls, sel);
  if (!m) return NO;
  if (!g_flat_patched) g_flat_patched = [NSMutableSet set];
  NSString *key = isRadius ? [@"R:" stringByAppendingString:name] : [@"M:" stringByAppendingString:name];
  key = [key stringByAppendingFormat:@":%@", NSStringFromSelector(sel)];
  if ([g_flat_patched containsObject:key]) return YES;
  const char *types = method_getTypeEncoding(m);
  IMP prev;
  if (isRadius) {
    if (!g_flat_radius_imps) g_flat_radius_imps = [NSMutableDictionary dictionary];
    prev = class_replaceMethod(cls, sel, (IMP)aui_flagged_corner_radius, types ? types : "d@:");
    [g_flat_radius_imps setObject:[NSValue valueWithPointer:prev] forKey:name];
  } else {
    if (!g_flat_mask_imps) g_flat_mask_imps = [NSMutableDictionary dictionary];
    prev = class_replaceMethod(cls, sel, (IMP)aui_flagged_corner_mask, types ? types : "@@:");
    NSString *impKey = [NSString stringWithFormat:@"%@:%@", name, NSStringFromSelector(sel)];
    [g_flat_mask_imps setObject:[NSValue valueWithPointer:prev] forKey:impKey];
  }
  [g_flat_patched addObject:key];
  return YES;
}

// Flattens a page/popup window: no shadow, no system corner rounding, in
// every focus state. Focus/key behaviour is untouched (style mask kept, so
// `canBecomeKeyWindow` and typing keep working). Idempotent per instance.
void aui_cef_flatten_window(void *child_root_view) {
  NSWindow *cw = window_of(child_root_view);
  if (!cw) return;
  cw.hasShadow = NO;
  [cw invalidateShadow];
  objc_setAssociatedObject(cw, kFlatKey, @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
  Class cls = [cw class];
  NSString *name = NSStringFromClass(cls);
  // Titled windows get the system's corner mask (large on macOS 26) whatever
  // `_cornerRadius` says — found live: the swizzles alone left the corners
  // rounded. Borderless windows are plain rectangles; keep key-ability.
  aui_swizzle_can_become_key_once(cls, name);
  if (cw.styleMask & NSWindowStyleMaskTitled) {
    NSRect frame = cw.frame;
    cw.styleMask = NSWindowStyleMaskBorderless;
    [cw setFrame:frame display:YES];
  }
  SEL radiusSel = NSSelectorFromString(@"_cornerRadius");
  SEL getMaskSel = NSSelectorFromString(@"_getCornerMask");
  SEL maskSel = NSSelectorFromString(@"_cornerMask");
  BOOL radiusFound = aui_swizzle_corner_once(cls, name, radiusSel, YES);
  BOOL getMaskFound = aui_swizzle_corner_once(cls, name, getMaskSel, NO);
  BOOL maskFound = aui_swizzle_corner_once(cls, name, maskSel, NO);
  if (![objc_getAssociatedObject(cw, kFlatLoggedKey) boolValue] && getenv("AUI_CEF_TRACE")) {
    objc_setAssociatedObject(cw, kFlatLoggedKey, @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    NSLog(@"aui-cef flat: class=%s styleMask=0x%lx hasShadow=%d _cornerRadius=%d _getCornerMask=%d _cornerMask=%d",
          class_getName(cls), (unsigned long)cw.styleMask, (int)cw.hasShadow,
          (int)radiusFound, (int)getMaskFound, (int)maskFound);
  }
}

// Probe getters: the page window's `hasShadow` (1/0, -1 for nil) and what its
// `_cornerRadius` now returns (-1 when the selector is absent or nil).
int aui_cef_window_has_shadow(void *window) {
  NSWindow *w = (__bridge NSWindow *)window;
  if (!w) return -1;
  return w.hasShadow ? 1 : 0;
}

double aui_cef_window_corner_radius(void *window) {
  NSWindow *w = (__bridge NSWindow *)window;
  if (!w) return -1;
  SEL sel = NSSelectorFromString(@"_cornerRadius");
  if (![w respondsToSelector:sel]) return -1;
  IMP imp = [w methodForSelector:sel];
  if (!imp) return -1;
  return (double)((CGFloat (*)(id, SEL))imp)(w, sel);
}

// MARK: - Probe helpers (examples/probe.rs drives real key events)
#import <CoreGraphics/CoreGraphics.h>
#include <unistd.h>

// Posts a Command-modified key down/up to this process (USB HID keycode,
// e.g. 40 = k, 8 = c, 17 = t).
void aui_cef_post_cmd_key(unsigned short keycode, int down) {
  CGEventRef e = CGEventCreateKeyboardEvent(NULL, (CGKeyCode)keycode, down ? true : false);
  if (!e) return;
  CGEventSetFlags(e, kCGEventFlagMaskCommand);
  CGEventPostToPid(getpid(), e);
  CFRelease(e);
}

int aui_cef_window_count(void) { return (int)[NSApp windows].count; }

void *aui_cef_main_window(void) { return (__bridge void *)NSApp.mainWindow; }

void *aui_cef_key_window(void) { return (__bridge void *)NSApp.keyWindow; }

// Probe: the page window's `[cw class]` name (static storage, do not free)
// and whether a window can become main (1/0).
const char *aui_cef_window_class_name(void *window) {
  NSWindow *w = (__bridge NSWindow *)window;
  if (!w) return "";
  return class_getName([w class]);
}

int aui_cef_window_can_become_main(void *window) {
  NSWindow *w = (__bridge NSWindow *)window;
  if (!w) return -1;
  return w.canBecomeMainWindow ? 1 : 0;
}
