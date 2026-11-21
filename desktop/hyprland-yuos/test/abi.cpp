// SPDX-License-Identifier: GPL-3.0-only

#include "../include/yuos_compositor.h"
#include "../YuOs.hpp"
#include <cstdlib>
#include <iostream>

int main() {
    if (::unsetenv("YUOS_DISPLAY_CONTROL_FD") != 0)
        return 1;
    if (yuos_compositor_open() != nullptr)
        return 2;
    YuOsCommand command{};
    if (yuos_compositor_next(nullptr, &command) != -1)
        return 3;
    if (yuos_compositor_rights(nullptr, YUOS_ORIGIN_INSTANCE, 1) != 0)
        return 4;
    if (yuos_compositor_context_created(nullptr, YUOS_ORIGIN_CREATOR, 1))
        return 5;
    if (yuos_compositor_take_descriptor(nullptr, 1) != -1)
        return 6;
    if (yuos_compositor_take_selection(nullptr, 1) != nullptr)
        return 7;
    if (yuos_selection_count(nullptr) != 0)
        return 8;
    if (yuos_selection_send(nullptr, nullptr, 0, -1))
        return 9;
    if (YuOs::initialize())
        return 10;
    if (YuOs::permitted(nullptr, YUOS_PANEL))
        return 11;
    YuOs::SOrigin origin{};
    if (YuOs::contextOrigin(nullptr, origin))
        return 12;
    YuOsDesktopRequest desktop{};
    if (yuos_compositor_take_desktop(nullptr, 1, &desktop))
        return 13;
    if (yuos_compositor_desktop_response(nullptr, 1, nullptr, 0))
        return 14;
    YuOs::shutdown();
    std::cout << "Native compositor ABI rejects unauthenticated startup and invalid handles\n";
    return 0;
}
