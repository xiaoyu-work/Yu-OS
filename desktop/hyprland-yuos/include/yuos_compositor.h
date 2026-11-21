// SPDX-License-Identifier: GPL-3.0-only
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct YuOsCompositor YuOsCompositor;
typedef struct YuOsSelection  YuOsSelection;

enum YuOsOriginKind {
    YUOS_ORIGIN_LOGIN    = 1,
    YUOS_ORIGIN_CREATOR  = 2,
    YUOS_ORIGIN_INSTANCE = 3,
    YUOS_ORIGIN_LOCKER   = 4,
};

enum YuOsRights {
    YUOS_SELECTION_READ  = 1,
    YUOS_SELECTION_WRITE = 2,
    YUOS_PANEL           = 4,
    YUOS_CONTEXT         = 8,
    YUOS_DESKTOP_SESSION      = 16,
    YUOS_SESSION_LOCK    = 32,
};

enum YuOsCommandKind {
    YUOS_CREATE         = 1,
    YUOS_RETIRE         = 2,
    YUOS_SET_SELECTION  = 3,
    YUOS_STOP           = 4,
    YUOS_INSTANCE_ENDED = 5,
    YUOS_DESKTOP        = 6,
    YUOS_LOCKER         = 7,
};

enum YuOsDesktopKind {
    YUOS_DESKTOP_LIST            = 1,
    YUOS_DESKTOP_FOCUS           = 2,
    YUOS_DESKTOP_CLOSE           = 3,
    YUOS_DESKTOP_CLOSE_APP       = 4,
    YUOS_DESKTOP_FOCUS_WORKSPACE = 5,
    YUOS_DESKTOP_TOGGLE_SPECIAL  = 6,
    YUOS_DESKTOP_MOVE_WINDOW     = 7,
    YUOS_DESKTOP_TOGGLE_FLOATING = 8,
    YUOS_DESKTOP_TOGGLE_PIN      = 9,
    YUOS_DESKTOP_OBSERVE         = 10,
    YUOS_DESKTOP_VISUAL_CAPTURE  = 11,
    YUOS_DESKTOP_VISUAL_CLICK    = 12,
};

typedef struct YuOsDesktopRequest {
    uint32_t kind;
    uint64_t identifier;
    uint8_t  app_id[256];
    uint8_t  workspace[80];
    uint8_t  observation[33];
    uint64_t expires_ms;
    uint32_t x;
    uint32_t y;
    uint32_t width;
    uint32_t height;
} YuOsDesktopRequest;

typedef struct YuOsCommand {
    uint64_t serial;
    uint64_t token;
    uint32_t kind;
} YuOsCommand;

/*
 * OS-private ABI, not an App interface. Handles and commands are confined to
 * the compositor event thread. Only this library may create handles/tokens.
 * All non-null pointers must remain valid for the duration of a call.
 * Creator descriptors and selection handles transfer ownership once; other
 * descriptor parameters are borrowed. MIME pointers live with their selection.
 * False, zero rights or a negative status never grants authority.
 */
YuOsCompositor* yuos_compositor_open(void);
void            yuos_compositor_close(YuOsCompositor* handle);
bool            yuos_compositor_ready(YuOsCompositor* handle, const uint8_t* display, size_t length);
bool            yuos_compositor_desktop_changed(YuOsCompositor* handle);
int32_t         yuos_compositor_next(YuOsCompositor* handle, YuOsCommand* command);
bool            yuos_compositor_acknowledge(YuOsCompositor* handle, uint64_t serial, bool accepted);
int32_t         yuos_compositor_take_descriptor(YuOsCompositor* handle, uint64_t serial);
bool            yuos_compositor_take_desktop(YuOsCompositor* handle, uint64_t serial, YuOsDesktopRequest* request);
bool            yuos_compositor_desktop_response(YuOsCompositor* handle, uint64_t serial, const uint8_t* bytes, size_t length);
uint32_t        yuos_compositor_rights(YuOsCompositor* handle, uint32_t origin, uint64_t token);
bool            yuos_compositor_accepts_login(YuOsCompositor* handle, int32_t descriptor);
bool            yuos_compositor_client_added(YuOsCompositor* handle, uint32_t origin, uint64_t token, int32_t descriptor);
bool            yuos_compositor_client_removed(YuOsCompositor* handle, uint32_t origin, uint64_t token);
bool            yuos_compositor_context_created(YuOsCompositor* handle, uint32_t origin, uint64_t token);
bool            yuos_compositor_listener_added(YuOsCompositor* handle, uint64_t token);
bool            yuos_compositor_listener_removed(YuOsCompositor* handle, uint64_t token);
bool            yuos_compositor_accepts_context(YuOsCompositor* handle, uint64_t token, int32_t descriptor);
YuOsSelection*  yuos_compositor_take_selection(YuOsCompositor* handle, uint64_t serial);
size_t          yuos_selection_count(const YuOsSelection* selection);
const char*     yuos_selection_mime(const YuOsSelection* selection, size_t index);
bool            yuos_selection_send(const YuOsSelection* selection, const uint8_t* mime, size_t length, int32_t descriptor);
void            yuos_selection_close(YuOsSelection* selection);

#ifdef __cplusplus
}
#endif
