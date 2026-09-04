// rEspanso portable KDE/X11 tray helper for Astra Linux 1.7.
//
// Kept intentionally small: it uses wxWidgets/GTK already required by the
// portable Match Studio, so the workstation needs no extra packages or sudo.

#include <wx/dcmemory.h>
#include <wx/filename.h>
#include <wx/menu.h>
#include <wx/stdpaths.h>
#include <wx/taskbar.h>
#include <wx/wx.h>

#include <signal.h>
#include <unistd.h>

#include <string>

namespace {

enum {
    ID_OPEN_STUDIO = wxID_HIGHEST + 101,
    ID_STOP_RESPANSO,
};

std::string to_utf8(const wxString &value) {
    const wxScopedCharBuffer buffer = value.utf8_str();
    return buffer.data() ? std::string(buffer.data()) : std::string();
}

void spawn_script(const wxString &root, const char *name) {
    const std::string path = to_utf8(root + wxFILE_SEP_PATH + wxString::FromUTF8(name));
    if (path.empty()) {
        return;
    }

    const pid_t pid = fork();
    if (pid == 0) {
        // Detach GUI/menu actions from the tray process. The child inherits the
        // current DISPLAY and portable LD_LIBRARY_PATH, which is exactly what
        // studio.sh/stop.sh need.
        setsid();
        execl(path.c_str(), path.c_str(), static_cast<char *>(nullptr));
        _exit(127);
    }
}

wxIcon make_icon() {
    // Draw the icon at runtime to keep the portable bundle free of icon-theme
    // installation requirements. KDE/GTK can scale this 32px bitmap as needed.
    wxBitmap bitmap(32, 32, 32);
    wxMemoryDC dc;
    dc.SelectObject(bitmap);

    dc.SetBackground(wxBrush(wxColour(38, 42, 50)));
    dc.Clear();
    dc.SetPen(*wxTRANSPARENT_PEN);
    dc.SetBrush(wxBrush(wxColour(74, 111, 255)));
    dc.DrawRoundedRectangle(2, 2, 28, 28, 7);

    const wxString label = wxString::FromUTF8("E");
    dc.SetTextForeground(*wxWHITE);
    dc.SetFont(wxFont(18, wxFONTFAMILY_SWISS, wxFONTSTYLE_NORMAL,
                      wxFONTWEIGHT_BOLD));
    wxCoord text_w = 0;
    wxCoord text_h = 0;
    dc.GetTextExtent(label, &text_w, &text_h);
    dc.DrawText(label, (32 - text_w) / 2, (32 - text_h) / 2 - 1);

    dc.SelectObject(wxNullBitmap);

    wxIcon icon;
    icon.CopyFromBitmap(bitmap);
    return icon;
}

class RespansoTrayIcon final : public wxTaskBarIcon {
public:
    explicit RespansoTrayIcon(const wxString &root) : root_(root) {
        Bind(wxEVT_TASKBAR_LEFT_UP, &RespansoTrayIcon::on_left_click, this);
        Bind(wxEVT_MENU, &RespansoTrayIcon::on_open_studio, this, ID_OPEN_STUDIO);
        Bind(wxEVT_MENU, &RespansoTrayIcon::on_stop, this, ID_STOP_RESPANSO);
    }

    bool install() {
        return SetIcon(make_icon(), wxString::FromUTF8("rEspanso — работает"));
    }

protected:
    wxMenu *CreatePopupMenu() override {
        wxMenu *menu = new wxMenu();
        menu->Append(ID_OPEN_STUDIO, wxString::FromUTF8("Открыть Match Studio"));
        menu->AppendSeparator();
        menu->Append(ID_STOP_RESPANSO, wxString::FromUTF8("Остановить rEspanso"));
        return menu;
    }

private:
    void on_left_click(wxTaskBarIconEvent &) {
        spawn_script(root_, "studio.sh");
    }

    void on_open_studio(wxCommandEvent &) {
        spawn_script(root_, "studio.sh");
    }

    void on_stop(wxCommandEvent &) {
        // stop.sh also removes the tray PID file and terminates this helper.
        // Remove the icon first so Plasma does not keep a stale entry while the
        // service shutdown finishes.
        RemoveIcon();
        spawn_script(root_, "stop.sh");
        wxTheApp->ExitMainLoop();
    }

    wxString root_;
};

class RespansoTrayApp final : public wxApp {
public:
    bool OnInit() override {
        signal(SIGCHLD, SIG_IGN);
        SetExitOnFrameDelete(false);

        wxString root;
        if (argc >= 2) {
            root = wxString(argv[1]);
        } else {
            root = wxFileName(wxStandardPaths::Get().GetExecutablePath()).GetPath();
        }
        root = wxFileName(root, wxEmptyString).GetPath();

        tray_ = new RespansoTrayIcon(root);
        if (!tray_->install()) {
            wxFprintf(stderr,
                      wxString::FromUTF8("rEspanso: KDE/GTK system tray is unavailable\n"));
            delete tray_;
            tray_ = nullptr;
            return false;
        }
        return true;
    }

    int OnExit() override {
        if (tray_) {
            tray_->RemoveIcon();
            delete tray_;
            tray_ = nullptr;
        }
        return 0;
    }

private:
    RespansoTrayIcon *tray_ = nullptr;
};

} // namespace

wxIMPLEMENT_APP(RespansoTrayApp);
