/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

#include "native.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <iostream>
#include <memory>
#include <stdio.h>
#include <string>
#include <vector>

#define UNICODE

#ifdef __MINGW32__
#ifndef WINVER
#define WINVER 0x0606
#endif
#define STRSAFE_NO_DEPRECATE
#endif

#include <string.h>
#include <strsafe.h>
#include <windows.h>
#include <winuser.h>
#pragma comment(lib, "Shell32.lib")
#include <shellapi.h>

#include <Windows.h>

#include "json/json.hpp"
using json = nlohmann::json;

namespace {
constexpr int THUNDER_ICON_SIZE = 32;
constexpr int THUNDER_FRAME_COUNT = 8;
constexpr UINT HEARTBEAT_TIMER_ID = 10001;
constexpr UINT TRAY_ANIMATION_TIMER_ID = 10002;
constexpr UINT TRAY_ANIMATION_INTERVAL_MS = 150;
constexpr int TRAY_MODE_PAUSED = 0;
constexpr int TRAY_MODE_ACTIVE = 1;
constexpr int TRAY_MODE_SYSTEM_BLOCKED = 2;

struct Color {
    uint8_t red;
    uint8_t green;
    uint8_t blue;
    uint8_t alpha;
};

uint8_t channel(uint32_t pixel, int shift) {
    return static_cast<uint8_t>((pixel >> shift) & 0xffU);
}

uint32_t pack(Color color) {
    return (static_cast<uint32_t>(color.alpha) << 24U) |
           (static_cast<uint32_t>(color.red) << 16U) |
           (static_cast<uint32_t>(color.green) << 8U) |
           static_cast<uint32_t>(color.blue);
}

void blend_pixel(std::vector<uint32_t> &pixels, int x, int y, Color source) {
    if (x < 0 || y < 0 || x >= THUNDER_ICON_SIZE || y >= THUNDER_ICON_SIZE ||
        source.alpha == 0) {
        return;
    }
    uint32_t &destination = pixels[static_cast<size_t>(y * THUNDER_ICON_SIZE + x)];
    const uint8_t destination_alpha = channel(destination, 24);
    if (source.alpha == 255 || destination_alpha == 0) {
        destination = pack(source);
        return;
    }

    const uint32_t inverse = 255U - source.alpha;
    const uint32_t output_alpha =
        source.alpha + (static_cast<uint32_t>(destination_alpha) * inverse + 127U) / 255U;
    if (output_alpha == 0) {
        destination = 0;
        return;
    }

    auto blend_channel = [&](uint8_t source_channel, int shift) {
        const uint32_t destination_channel = channel(destination, shift);
        const uint32_t premultiplied =
            static_cast<uint32_t>(source_channel) * source.alpha +
            (destination_channel * destination_alpha * inverse + 127U) / 255U;
        return static_cast<uint8_t>((premultiplied + output_alpha / 2U) / output_alpha);
    };

    destination = pack({blend_channel(source.red, 16),
                        blend_channel(source.green, 8),
                        blend_channel(source.blue, 0),
                        static_cast<uint8_t>(output_alpha)});
}

void draw_circle(std::vector<uint32_t> &pixels, int center_x, int center_y,
                 int radius, Color color) {
    const int radius_squared = radius * radius;
    for (int y = center_y - radius; y <= center_y + radius; ++y) {
        for (int x = center_x - radius; x <= center_x + radius; ++x) {
            const int dx = x - center_x;
            const int dy = y - center_y;
            if (dx * dx + dy * dy <= radius_squared) {
                blend_pixel(pixels, x, y, color);
            }
        }
    }
}

void draw_rect(std::vector<uint32_t> &pixels, int left, int top, int right,
               int bottom, Color color) {
    for (int y = top; y <= bottom; ++y) {
        for (int x = left; x <= right; ++x) {
            blend_pixel(pixels, x, y, color);
        }
    }
}

bool inside_polygon(float x, float y, const std::array<POINT, 7> &points) {
    bool inside = false;
    size_t previous = points.size() - 1;
    for (size_t current = 0; current < points.size(); ++current) {
        const float current_x = static_cast<float>(points[current].x);
        const float current_y = static_cast<float>(points[current].y);
        const float previous_x = static_cast<float>(points[previous].x);
        const float previous_y = static_cast<float>(points[previous].y);
        const bool intersects = ((current_y > y) != (previous_y > y)) &&
            (x < (previous_x - current_x) * (y - current_y) /
                         (previous_y - current_y) +
                     current_x);
        if (intersects) {
            inside = !inside;
        }
        previous = current;
    }
    return inside;
}

void draw_polygon(std::vector<uint32_t> &pixels,
                  const std::array<POINT, 7> &points, Color color) {
    LONG min_x = points[0].x;
    LONG max_x = points[0].x;
    LONG min_y = points[0].y;
    LONG max_y = points[0].y;
    for (const POINT &point : points) {
        min_x = std::min(min_x, point.x);
        max_x = std::max(max_x, point.x);
        min_y = std::min(min_y, point.y);
        max_y = std::max(max_y, point.y);
    }
    for (int y = static_cast<int>(min_y); y <= static_cast<int>(max_y); ++y) {
        for (int x = static_cast<int>(min_x); x <= static_cast<int>(max_x); ++x) {
            if (inside_polygon(static_cast<float>(x) + 0.5F,
                               static_cast<float>(y) + 0.5F, points)) {
                blend_pixel(pixels, x, y, color);
            }
        }
    }
}

std::array<POINT, 7> lightning_points(int shift_x, int shift_y) {
    return {{{15 + shift_x, 15 + shift_y},
             {23 + shift_x, 15 + shift_y},
             {19 + shift_x, 21 + shift_y},
             {23 + shift_x, 21 + shift_y},
             {11 + shift_x, 31 + shift_y},
             {15 + shift_x, 23 + shift_y},
             {11 + shift_x, 23 + shift_y}}};
}

HICON create_argb_icon(const std::vector<uint32_t> &pixels) {
    BITMAPV5HEADER header = {};
    header.bV5Size = sizeof(header);
    header.bV5Width = THUNDER_ICON_SIZE;
    header.bV5Height = -THUNDER_ICON_SIZE;
    header.bV5Planes = 1;
    header.bV5BitCount = 32;
    header.bV5Compression = BI_BITFIELDS;
    header.bV5RedMask = 0x00ff0000;
    header.bV5GreenMask = 0x0000ff00;
    header.bV5BlueMask = 0x000000ff;
    header.bV5AlphaMask = 0xff000000;

    HDC screen = GetDC(NULL);
    void *bits = nullptr;
    HBITMAP color_bitmap = CreateDIBSection(
        screen, reinterpret_cast<BITMAPINFO *>(&header), DIB_RGB_COLORS, &bits,
        NULL, 0);
    ReleaseDC(NULL, screen);
    if (!color_bitmap || !bits) {
        if (color_bitmap) {
            DeleteObject(color_bitmap);
        }
        return NULL;
    }
    memcpy(bits, pixels.data(), pixels.size() * sizeof(uint32_t));

    const size_t mask_stride =
        static_cast<size_t>(((THUNDER_ICON_SIZE + 15) / 16) * 2);
    std::vector<uint8_t> mask(mask_stride * THUNDER_ICON_SIZE, 0);
    HBITMAP mask_bitmap = CreateBitmap(THUNDER_ICON_SIZE, THUNDER_ICON_SIZE, 1,
                                       1, mask.data());
    if (!mask_bitmap) {
        DeleteObject(color_bitmap);
        return NULL;
    }

    ICONINFO icon_info = {};
    icon_info.fIcon = TRUE;
    icon_info.hbmMask = mask_bitmap;
    icon_info.hbmColor = color_bitmap;
    HICON icon = CreateIconIndirect(&icon_info);
    DeleteObject(mask_bitmap);
    DeleteObject(color_bitmap);
    return icon;
}

HICON create_thunder_icon(int frame, bool system_blocked, bool paused) {
    std::vector<uint32_t> pixels(
        static_cast<size_t>(THUNDER_ICON_SIZE * THUNDER_ICON_SIZE), 0);

    const std::array<uint8_t, THUNDER_FRAME_COUNT> pulse =
        {{118, 150, 196, 255, 210, 166, 126, 148}};
    const std::array<int, THUNDER_FRAME_COUNT> shift = {{0, 0, 1, 0, -1, 0, 0, 1}};
    const uint8_t frame_pulse = pulse[static_cast<size_t>(frame % THUNDER_FRAME_COUNT)];
    const int bolt_shift = shift[static_cast<size_t>(frame % THUNDER_FRAME_COUNT)];

    const Color aura = paused
        ? Color{96, 118, 145, 32}
        : (system_blocked ? Color{255, 72, 168, static_cast<uint8_t>(frame_pulse / 3)}
                          : Color{26, 196, 255, static_cast<uint8_t>(frame_pulse / 3)});
    const Color rim = paused ? Color{118, 134, 154, 210}
                             : (system_blocked ? Color{215, 70, 166, 230}
                                               : Color{43, 151, 230, 230});
    const Color cloud = paused ? Color{44, 50, 61, 248}
                               : (system_blocked ? Color{52, 20, 57, 248}
                                                 : Color{12, 25, 52, 248});

    draw_circle(pixels, 9, 14, 8, aura);
    draw_circle(pixels, 17, 10, 9, aura);
    draw_circle(pixels, 24, 15, 8, aura);
    draw_rect(pixels, 5, 13, 28, 21, aura);

    draw_circle(pixels, 9, 14, 7, rim);
    draw_circle(pixels, 17, 10, 8, rim);
    draw_circle(pixels, 24, 15, 7, rim);
    draw_rect(pixels, 5, 13, 28, 20, rim);
    draw_circle(pixels, 9, 14, 6, cloud);
    draw_circle(pixels, 17, 10, 7, cloud);
    draw_circle(pixels, 24, 15, 6, cloud);
    draw_rect(pixels, 6, 13, 27, 20, cloud);

    if (paused) {
        draw_rect(pixels, 12, 15, 15, 27, Color{170, 184, 201, 245});
        draw_rect(pixels, 19, 15, 22, 27, Color{170, 184, 201, 245});
        return create_argb_icon(pixels);
    }

    const auto bolt = lightning_points(bolt_shift, 0);
    for (int offset_y = -2; offset_y <= 2; ++offset_y) {
        for (int offset_x = -2; offset_x <= 2; ++offset_x) {
            if (offset_x == 0 && offset_y == 0) {
                continue;
            }
            auto glow = lightning_points(bolt_shift + offset_x, offset_y);
            const int distance = std::abs(offset_x) + std::abs(offset_y);
            const uint8_t alpha = static_cast<uint8_t>(
                std::max(18, static_cast<int>(frame_pulse) / (distance + 2)));
            draw_polygon(
                pixels, glow,
                system_blocked ? Color{255, 67, 164, alpha}
                               : Color{50, 210, 255, alpha});
        }
    }
    draw_polygon(
        pixels, bolt,
        system_blocked
            ? Color{255, static_cast<uint8_t>(128 + frame_pulse / 3), 198, 255}
            : Color{255, static_cast<uint8_t>(224 + frame_pulse / 8), 118, 255});
    if (frame == 2 || frame == 3) {
        draw_polygon(pixels, lightning_points(bolt_shift, 0),
                     Color{255, 255, 255, static_cast<uint8_t>(110 + frame_pulse / 2)});
    }

    const int rain_phase = frame % 4;
    const Color rain = system_blocked ? Color{190, 94, 255, 150}
                                      : Color{65, 186, 255, 160};
    draw_rect(pixels, 6, 23 + rain_phase, 7, 26 + rain_phase, rain);
    draw_rect(pixels, 26, 21 + ((rain_phase + 2) % 4), 27,
              24 + ((rain_phase + 2) % 4), rain);
    return create_argb_icon(pixels);
}

std::wstring utf8_to_wide(const std::string &value) {
    if (value.empty()) {
        return std::wstring();
    }

    int length = MultiByteToWideChar(
        CP_UTF8, MB_ERR_INVALID_CHARS, value.c_str(),
        static_cast<int>(value.size()), nullptr, 0);
    if (length <= 0) {
        return std::wstring();
    }

    std::wstring result(static_cast<size_t>(length), L'\0');
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.c_str(),
                        static_cast<int>(value.size()), &result[0], length);
    return result;
}

const wchar_t *builtin_menu_label(uint32_t id) {
    switch (id) {
    case 0:
        return L"\u0412\u044b\u0439\u0442\u0438 \u0438\u0437 rEspanso";
    case 1:
        return L"\u041f\u0435\u0440\u0435\u0437\u0430\u0433\u0440\u0443\u0437\u0438\u0442\u044c \u043a\u043e\u043d\u0444\u0438\u0433\u0443\u0440\u0430\u0446\u0438\u044e";
    case 2:
        return L"\u0412\u043a\u043b\u044e\u0447\u0438\u0442\u044c \u043f\u043e\u0434\u0441\u0442\u0430\u043d\u043e\u0432\u043a\u0438";
    case 3:
        return L"\u041e\u0442\u043a\u043b\u044e\u0447\u0438\u0442\u044c \u043f\u043e\u0434\u0441\u0442\u0430\u043d\u043e\u0432\u043a\u0438";
    case 4:
        return L"\u041f\u043e\u0447\u0435\u043c\u0443 rEspanso \u043d\u0435 \u0440\u0430\u0431\u043e\u0442\u0430\u0435\u0442?";
    case 5:
        return L"\u0417\u0430\u043f\u0443\u0441\u0442\u0438\u0442\u044c \u0430\u0432\u0442\u043e\u0438\u0441\u043f\u0440\u0430\u0432\u043b\u0435\u043d\u0438\u0435 SecureInput";
    case 6:
        return L"\u041e\u0442\u043a\u0440\u044b\u0442\u044c \u043f\u043e\u0438\u0441\u043a";
    case 7:
        return L"\u041f\u043e\u043a\u0430\u0437\u0430\u0442\u044c \u0436\u0443\u0440\u043d\u0430\u043b";
    case 8:
        return L"\u041e\u0442\u043a\u0440\u044b\u0442\u044c \u043f\u0430\u043f\u043a\u0443 \u043a\u043e\u043d\u0444\u0438\u0433\u0443\u0440\u0430\u0446\u0438\u0438";
    case 9:
        return L"\u041e\u0442\u043a\u0440\u044b\u0442\u044c \u0441\u0442\u0443\u0434\u0438\u044e rEspanso";
    default:
        return nullptr;
    }
}
} // namespace

#define APPWM_ICON_CLICK (WM_APP + 1)
#define APPWM_SHOW_CONTEXT_MENU (WM_APP + 2)
#define APPWM_UPDATE_TRAY_ICON (WM_APP + 3)

const wchar_t *const ui_winclass = L"rEspansoUI";

typedef struct {
    UIOptions options;
    NOTIFYICONDATA nid;
    HICON g_icons[MAX_ICON_COUNT];
    HICON active_frames[THUNDER_FRAME_COUNT];
    HICON blocked_frames[THUNDER_FRAME_COUNT];
    HICON paused_icon;
    int32_t current_icon_index;
    int32_t tray_mode;
    int32_t animation_frame;

    // Rust interop
    void *rust_instance;
    EventCallback event_callback;
} UIVariables;

UINT WM_TASKBARCREATED = RegisterWindowMessage(L"TaskbarCreated");

HICON selected_tray_icon(const UIVariables *variables) {
    if (variables->tray_mode == TRAY_MODE_ACTIVE) {
        HICON icon = variables->active_frames[variables->animation_frame];
        if (icon) {
            return icon;
        }
    } else if (variables->tray_mode == TRAY_MODE_SYSTEM_BLOCKED) {
        HICON icon = variables->blocked_frames[variables->animation_frame];
        if (icon) {
            return icon;
        }
    } else if (variables->paused_icon) {
        return variables->paused_icon;
    }

    if (variables->current_icon_index >= 0 &&
        variables->current_icon_index < variables->options.icon_paths_count) {
        return variables->g_icons[variables->current_icon_index];
    }
    return variables->g_icons[0];
}

void update_tray_tooltip(UIVariables *variables) {
    const wchar_t *tooltip = L"rEspanso";
    if (variables->tray_mode == TRAY_MODE_ACTIVE) {
        tooltip = L"rEspanso \u2014 \u043f\u043e\u0434\u0441\u0442\u0430\u043d\u043e\u0432\u043a\u0438 \u0430\u043a\u0442\u0438\u0432\u043d\u044b";
    } else if (variables->tray_mode == TRAY_MODE_SYSTEM_BLOCKED) {
        tooltip = L"rEspanso \u2014 \u0432\u0432\u043e\u0434 \u0432\u0440\u0435\u043c\u0435\u043d\u043d\u043e \u0437\u0430\u0431\u043b\u043e\u043a\u0438\u0440\u043e\u0432\u0430\u043d \u0441\u0438\u0441\u0442\u0435\u043c\u043e\u0439";
    } else {
        tooltip = L"rEspanso \u2014 \u043f\u043e\u0434\u0441\u0442\u0430\u043d\u043e\u0432\u043a\u0438 \u043f\u0440\u0438\u043e\u0441\u0442\u0430\u043d\u043e\u0432\u043b\u0435\u043d\u044b";
    }
    StringCchCopyW(variables->nid.szTip, ARRAYSIZE(variables->nid.szTip), tooltip);
}

void apply_tray_icon(UIVariables *variables) {
    variables->nid.hIcon = selected_tray_icon(variables);
    update_tray_tooltip(variables);
    if (variables->options.show_icon) {
        Shell_NotifyIcon(NIM_MODIFY, &variables->nid);
    }
}

void destroy_generated_icons(UIVariables *variables) {
    for (int frame = 0; frame < THUNDER_FRAME_COUNT; ++frame) {
        if (variables->active_frames[frame]) {
            DestroyIcon(variables->active_frames[frame]);
        }
        if (variables->blocked_frames[frame]) {
            DestroyIcon(variables->blocked_frames[frame]);
        }
    }
    if (variables->paused_icon) {
        DestroyIcon(variables->paused_icon);
    }
}

LRESULT CALLBACK ui_window_procedure(HWND window, unsigned int msg, WPARAM wp,
                                     LPARAM lp) {
    UIEvent event = {};
    UIVariables *variables = reinterpret_cast<UIVariables *>(
        GetWindowLongPtrW(window, GWLP_USERDATA));

    switch (msg) {
    case WM_DESTROY:
        PostQuitMessage(0);
        if (variables) {
            KillTimer(window, HEARTBEAT_TIMER_ID);
            KillTimer(window, TRAY_ANIMATION_TIMER_ID);
            if (variables->options.show_icon) {
                Shell_NotifyIcon(NIM_DELETE, &variables->nid);
            }
            for (int i = 0; i < variables->options.icon_paths_count; ++i) {
                if (variables->g_icons[i]) {
                    DestroyIcon(variables->g_icons[i]);
                }
            }
            destroy_generated_icons(variables);
            delete variables;
            SetWindowLongPtrW(window, GWLP_USERDATA, NULL);
        }
        return 0L;
    case WM_COMMAND: {
        UINT idItem = (UINT)LOWORD(wp);
        UINT flags = (UINT)HIWORD(wp);
        if (flags == 0 && variables) {
            event.event_type = UI_EVENT_TYPE_CONTEXT_MENU_CLICK;
            event.context_menu_id = (uint32_t)idItem;
            if (variables->event_callback && variables->rust_instance) {
                variables->event_callback(variables->rust_instance, event);
            }
        }
        break;
    }
    case APPWM_SHOW_CONTEXT_MENU: {
        HMENU menu = (HMENU)lp;
        POINT pt;
        GetCursorPos(&pt);
        SetForegroundWindow(window);
        TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0,
                       window, NULL);
        DestroyMenu(menu);
        break;
    }
    case APPWM_UPDATE_TRAY_ICON: {
        if (!variables) {
            break;
        }
        const int32_t index = static_cast<int32_t>(lp);
        const int32_t mode = static_cast<int32_t>(wp);
        if (index < 0 || index >= variables->options.icon_paths_count) {
            break;
        }
        variables->current_icon_index = index;
        variables->tray_mode = mode;
        variables->animation_frame = 0;
        apply_tray_icon(variables);
        break;
    }
    case APPWM_ICON_CLICK: {
        switch (lp) {
        case WM_LBUTTONUP:
        case WM_RBUTTONUP:
            event.event_type = UI_EVENT_TYPE_ICON_CLICK;
            if (variables && variables->event_callback &&
                variables->rust_instance) {
                variables->event_callback(variables->rust_instance, event);
            }
            break;
        }
        break;
    }
    case WM_TIMER: {
        if (!variables) {
            break;
        }
        if (wp == HEARTBEAT_TIMER_ID) {
            event.event_type = UI_EVENT_TYPE_HEARTBEAT;
            if (variables->event_callback && variables->rust_instance) {
                variables->event_callback(variables->rust_instance, event);
            }
        } else if (wp == TRAY_ANIMATION_TIMER_ID &&
                   variables->tray_mode != TRAY_MODE_PAUSED) {
            variables->animation_frame =
                (variables->animation_frame + 1) % THUNDER_FRAME_COUNT;
            apply_tray_icon(variables);
        }
        break;
    }
    default:
        if (msg == WM_TASKBARCREATED && variables &&
            variables->options.show_icon) {
            variables->nid.hIcon = selected_tray_icon(variables);
            Shell_NotifyIcon(NIM_ADD, &variables->nid);
        }
        return DefWindowProc(window, msg, wp, lp);
    }
    return 0L;
}

void *ui_initialize(void *_self, UIOptions _options, int32_t *error_code) {
    HWND window = NULL;
    SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);

    WNDCLASSEX uiwndclass = {
        sizeof(WNDCLASSEX),
        0,
        ui_window_procedure,
        0,
        0,
        GetModuleHandle(0),
        NULL,
        LoadCursor(0, IDC_ARROW),
        NULL,
        NULL,
        ui_winclass,
        NULL
    };

    if (RegisterClassEx(&uiwndclass)) {
        window = CreateWindowEx(0, ui_winclass, L"rEspanso UI Window",
                                WS_OVERLAPPEDWINDOW, CW_USEDEFAULT,
                                CW_USEDEFAULT, 100, 100, NULL, NULL,
                                GetModuleHandle(0), NULL);
        if (window) {
            UIVariables *variables = new UIVariables{};
            variables->options = _options;
            variables->rust_instance = _self;
            variables->current_icon_index = 0;
            variables->tray_mode = TRAY_MODE_ACTIVE;
            variables->animation_frame = 0;
            SetWindowLongPtrW(window, GWLP_USERDATA,
                              reinterpret_cast<::LONG_PTR>(variables));

            for (int i = 0; i < variables->options.icon_paths_count; ++i) {
                variables->g_icons[i] = (HICON)LoadImage(
                    NULL, variables->options.icon_paths[i], IMAGE_ICON, 0, 0,
                    LR_DEFAULTCOLOR | LR_DEFAULTSIZE | LR_LOADFROMFILE);
            }
            for (int frame = 0; frame < THUNDER_FRAME_COUNT; ++frame) {
                variables->active_frames[frame] =
                    create_thunder_icon(frame, false, false);
                variables->blocked_frames[frame] =
                    create_thunder_icon(frame, true, false);
            }
            variables->paused_icon = create_thunder_icon(0, false, true);

            ShowWindow(window, SW_HIDE);
            HICON initial_icon = selected_tray_icon(variables);
            SendMessage(window, WM_SETICON, ICON_BIG, (LPARAM)initial_icon);
            SendMessage(window, WM_SETICON, ICON_SMALL, (LPARAM)initial_icon);

            variables->nid.cbSize = sizeof(variables->nid);
            variables->nid.hWnd = window;
            variables->nid.uID = 1;
            variables->nid.uFlags = NIF_ICON | NIF_TIP | NIF_MESSAGE;
            variables->nid.uCallbackMessage = APPWM_ICON_CLICK;
            variables->nid.hIcon = initial_icon;
            update_tray_tooltip(variables);
            if (variables->options.show_icon) {
                Shell_NotifyIcon(NIM_ADD, &variables->nid);
            }

            SetTimer(window, HEARTBEAT_TIMER_ID, 1000, (TIMERPROC)NULL);
            SetTimer(window, TRAY_ANIMATION_TIMER_ID,
                     TRAY_ANIMATION_INTERVAL_MS, (TIMERPROC)NULL);
        } else {
            *error_code = -2;
            return nullptr;
        }
    } else {
        *error_code = -1;
        return nullptr;
    }

    return window;
}

int32_t ui_eventloop(void *window, EventCallback _callback) {
    if (window) {
        UIVariables *variables = reinterpret_cast<UIVariables *>(
            GetWindowLongPtrW((HWND)window, GWLP_USERDATA));
        variables->event_callback = _callback;
        MSG msg;
        while (GetMessage(&msg, 0, 0, 0)) {
            DispatchMessage(&msg);
        }
    }
    return 1;
}

int32_t ui_destroy(void *window) {
    if (window) {
        return DestroyWindow((HWND)window);
    }
    return -1;
}

void ui_exit(void *window) {
    if (window) {
        PostMessage((HWND)window, WM_CLOSE, 0, 0);
    }
}

void ui_update_tray_icon(void *window, int32_t index, int32_t animation_mode) {
    if (window) {
        PostMessage((HWND)window, APPWM_UPDATE_TRAY_ICON,
                    static_cast<WPARAM>(animation_mode),
                    static_cast<LPARAM>(index));
    }
}

void _insert_separator_menu(HMENU parent) {
    InsertMenu(parent, -1, MF_BYPOSITION | MF_SEPARATOR, 0, NULL);
}

void _insert_single_menu(HMENU parent, json item) {
    if (!item["label"].is_string() || !item["id"].is_number()) {
        return;
    }
    std::string label = item["label"];
    uint32_t raw_id = item["id"];
    const wchar_t *localized_label = builtin_menu_label(raw_id);
    std::wstring wide_label =
        localized_label ? std::wstring(localized_label) : utf8_to_wide(label);
    InsertMenu(parent, -1, MF_BYPOSITION | MF_STRING, raw_id,
               wide_label.c_str());
}

void _insert_sub_menu(HMENU parent, json items) {
    for (auto &item : items) {
        if (item["type"] == "simple") {
            _insert_single_menu(parent, item);
        } else if (item["type"] == "separator") {
            _insert_separator_menu(parent);
        } else if (item["type"] == "sub") {
            HMENU subMenu = CreatePopupMenu();
            std::string label = item["label"];
            std::wstring wide_label = utf8_to_wide(label);
            InsertMenu(parent, -1, MF_BYPOSITION | MF_POPUP,
                       (UINT_PTR)subMenu, wide_label.c_str());
            _insert_sub_menu(subMenu, item["items"]);
        }
    }
}

int32_t ui_show_context_menu(void *window, char *payload) {
    if (window) {
        auto j_menu = json::parse(payload);
        HMENU parentMenu = CreatePopupMenu();
        _insert_sub_menu(parentMenu, j_menu);
        PostMessage((HWND)window, APPWM_SHOW_CONTEXT_MENU, 0,
                    (LPARAM)parentMenu);
        return 0;
    }
    return -1;
}
