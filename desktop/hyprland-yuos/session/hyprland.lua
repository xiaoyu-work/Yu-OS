hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })

hl.config({
    -- Keyboard focus changes only on click; hovering must not take it from an open dialog.
    input = {
        follow_mouse = 2,
        float_switch_override_focus = 0,
    },
    misc = {
        disable_hyprland_logo = true,
        disable_splash_rendering = true,
        -- The Yu OS supervisor owns compositor lifetime and the OS owns dialogs.
        disable_watchdog_warning = true,
        disable_hyprland_guiutils_check = true,
    },
})

hl.curve("yuos-ease", { type = "bezier", points = { {0.3, 0}, {0.7, 1} } })

hl.animation({ leaf = "windowsIn",   enabled = true, speed = 1.6, bezier = "yuos-ease", style = "popin 96%" })
hl.animation({ leaf = "windowsOut",  enabled = true, speed = 1.6, bezier = "yuos-ease", style = "popin 96%" })
hl.animation({ leaf = "fadeIn",      enabled = true, speed = 1.6, bezier = "yuos-ease" })
hl.animation({ leaf = "fadeOut",     enabled = true, speed = 1.6, bezier = "yuos-ease" })
hl.animation({ leaf = "windowsMove", enabled = true, speed = 1.4, bezier = "yuos-ease" })

hl.animation({ leaf = "workspacesIn",  enabled = true, speed = 1.8, bezier = "yuos-ease", style = "slidefade 12%" })
hl.animation({ leaf = "workspacesOut", enabled = true, speed = 1.8, bezier = "yuos-ease", style = "slidefade 12%" })

-- QuietMenu.qml already fades and slides the menu on open and close.
hl.layer_rule({
    name = "yuos-top-menu",
    match = { namespace = "^caelestia-yuos-top-menu$" },
    no_anim = true,
})

hl.window_rule({
    name = "yuos-permission-request",
    match = { class = "^yuos-desktop-reviews$" },
    float = true,
    center = true,
    rounding = 14,
    border_size = 0,
})

hl.window_rule({
    name = "yuos-floating-apps",
    match = { class = "^(com\\.yuos\\..*|YuOS-Agent)$" },
    float = true,
    center = true,
    size = { 980, 640 },
    rounding = 12,
})

hl.bind("SUPER + Q", hl.dsp.window.close())
for _, direction in ipairs({ "left", "right", "up", "down" }) do
    hl.bind("SUPER + " .. direction, hl.dsp.focus({ direction = direction }))
end
for workspace = 1, 9 do
    hl.bind("SUPER + " .. workspace, hl.dsp.focus({ workspace = workspace }))
    hl.bind("SUPER + SHIFT + " .. workspace, hl.dsp.window.move({ workspace = workspace }))
end
