// SPDX-License-Identifier: GPL-3.0-only
#pragma once

#include "include/yuos_compositor.h"
#include <optional>
#include <string>

namespace YuOs {
    bool                       installSelection(YuOsSelection* selection);
    void                       clearSelection();
    std::optional<std::string> desktop(const YuOsDesktopRequest& request);
}
