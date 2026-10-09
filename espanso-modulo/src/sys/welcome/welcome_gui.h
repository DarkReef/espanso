///////////////////////////////////////////////////////////////////////////
// C++ code generated with wxFormBuilder (version Oct 26 2018)
// http://www.wxformbuilder.org/
//
// PLEASE DO *NOT* EDIT THIS FILE!
///////////////////////////////////////////////////////////////////////////

#pragma once

#include <wx/artprov.h>
#include <wx/bitmap.h>
#include <wx/button.h>
#include <wx/checkbox.h>
#include <wx/colour.h>
#include <wx/font.h>
#include <wx/frame.h>
#include <wx/gdicmn.h>
#include <wx/hyperlink.h>
#include <wx/icon.h>
#include <wx/image.h>
#include <wx/settings.h>
#include <wx/sizer.h>
#include <wx/statbmp.h>
#include <wx/stattext.h>
#include <wx/string.h>
#include <wx/textctrl.h>
#include <wx/xrc/xmlres.h>

///////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
/// Class WelcomeFrame
///////////////////////////////////////////////////////////////////////////////
class WelcomeFrame : public wxFrame {
  private:
  protected:
    wxStaticText *title_label;
    wxStaticText *tray_info_label;
    wxStaticBitmap *tray_bitmap;
    wxStaticText *test_label;
    wxTextCtrl *test_text_ctrl;
    wxStaticText *doc_label;
    wxHyperlinkCtrl *m_hyperlink1;
    wxCheckBox *dont_show_checkbox;
    wxButton *got_it_btn;

    // Virtual event handlers, overide them in your derived class
    virtual void on_dont_show_change(wxCommandEvent &event) { event.Skip(); }
    virtual void on_complete(wxCommandEvent &event) { event.Skip(); }

  public:
    WelcomeFrame(
        wxWindow *parent, wxWindowID id = wxID_ANY,
        const wxString &title =
            wxT("rEspanso \u2014 \u0442\u0435\u043A\u0441\u0442\u043E\u0432\u044B\u0439 \u044D\u043A\u0441\u043F\u0430\u043D\u0434\u0435\u0440"),
        const wxPoint &pos = wxDefaultPosition,
        const wxSize &size = wxSize(600, 640),
        long style = wxCAPTION | wxCLOSE_BOX | wxSYSTEM_MENU |
                     wxTAB_TRAVERSAL);

    ~WelcomeFrame();
};
