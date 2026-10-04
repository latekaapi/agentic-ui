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

// MARK: - The page window never becomes main (B1b-L)

static BOOL aui_never_main(id self, SEL _cmd) { return NO; }

static Class g_hosted_window_class = Nil;
static Class g_hosted_window_base = Nil;

// Isa-swizzles THAT window instance to a runtime subclass overriding
// `canBecomeMainWindow` → NO, so the host window keeps its coloured traffic
// lights while the page has focus. Per instance only: other CEF windows
// (DevTools, popups) keep their class. `canBecomeKey` is untouched, so typing
// into the page keeps working.
void aui_cef_never_main(void *child_root_view) {
  NSWindow *cw = window_of(child_root_view);
  if (!cw) return;
  Class base = object_getClass(cw);
  if (base == g_hosted_window_class) return;
  if (g_hosted_window_class == Nil) {
    g_hosted_window_base = base;
    g_hosted_window_class = objc_allocateClassPair(base, "AuiCefHostedWindow", 0);
    if (g_hosted_window_class == Nil) return;
    Method m = class_getInstanceMethod(base, @selector(canBecomeMainWindow));
    class_addMethod(g_hosted_window_class, @selector(canBecomeMainWindow),
                    (IMP)aui_never_main, m ? method_getTypeEncoding(m) : "B@:");
    objc_registerClassPair(g_hosted_window_class);
  }
  if (base != g_hosted_window_base) return;
  object_setClass(cw, g_hosted_window_class);
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
