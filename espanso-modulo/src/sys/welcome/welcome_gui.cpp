///////////////////////////////////////////////////////////////////////////
// C++ code generated with wxFormBuilder (version Oct 26 2018)
// http://www.wxformbuilder.org/
//
// PLEASE DO *NOT* EDIT THIS FILE!
///////////////////////////////////////////////////////////////////////////

#define _UNICODE

#include "welcome_gui.h"

///////////////////////////////////////////////////////////////////////////

WelcomeFrame::WelcomeFrame(wxWindow *parent, wxWindowID id,
                           const wxString &title, const wxPoint &pos,
                           const wxSize &size, long style)
    : wxFrame(parent, id, title, pos, size, style) {
    this->SetSizeHints(wxDefaultSize, wxDefaultSize);
    this->SetBackgroundColour(wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer1;
    bSizer1 = new wxBoxSizer(wxVERTICAL);

    bSizer1->Add(0, 10, 0, wxEXPAND, 5);

    title_label = new wxStaticText(
        this, wxID_ANY,
        wxT("rEspanso \u0437\u0430\u043F\u0443\u0449\u0435\u043D!"),
        wxDefaultPosition, wxDefaultSize, 0);
    title_label->Wrap(-1);
    title_label->SetFont(wxFont(20, wxFONTFAMILY_DEFAULT, wxFONTSTYLE_NORMAL,
                                wxFONTWEIGHT_BOLD, false, wxEmptyString));

    bSizer1->Add(title_label, 0, wxALIGN_CENTER | wxALL, 10);

    tray_info_label = new wxStaticText(
        this, wxID_ANY,
        wxT("\u0417\u043D\u0430\u0447\u043E\u043A rEspanso \u043D\u0430\u0445\u043E\u0434\u0438\u0442\u0441\u044F \u0432 \u043E\u0431\u043B\u0430\u0441\u0442\u0438 \u0443\u0432\u0435\u0434\u043E\u043C\u043B\u0435\u043D\u0438\u0439:"),
        wxDefaultPosition, wxDefaultSize, 0);
    tray_info_label->Wrap(-1);
    bSizer1->Add(tray_info_label, 0, wxALIGN_CENTER | wxALL, 10);

    tray_bitmap = new wxStaticBitmap(this, wxID_ANY, wxNullBitmap,
                                     wxDefaultPosition, wxDefaultSize, 0);
    bSizer1->Add(tray_bitmap, 0, wxALIGN_CENTER | wxALL, 5);

    bSizer1->Add(0, 10, 0, 0, 10);

    test_label = new wxStaticText(
        this, wxID_ANY,
        wxT("\u0414\u043B\u044F \u043F\u0440\u043E\u0432\u0435\u0440\u043A\u0438 \u0432\u0432\u0435\u0434\u0438\u0442\u0435 \u043D\u0438\u0436\u0435 :respanso_example"),
        wxDefaultPosition, wxDefaultSize, 0);
    test_label->Wrap(-1);
    bSizer1->Add(test_label, 0, wxALIGN_CENTER | wxALL, 10);

    test_text_ctrl = new wxTextCtrl(this, wxID_ANY, wxEmptyString,
                                    wxDefaultPosition, wxDefaultSize, 0);
    test_text_ctrl->SetFont(wxFont(16, wxFONTFAMILY_DEFAULT, wxFONTSTYLE_NORMAL,
                                   wxFONTWEIGHT_NORMAL, false, wxEmptyString));

    bSizer1->Add(test_text_ctrl, 0, wxALL | wxEXPAND, 10);

    doc_label = new wxStaticText(
        this, wxID_ANY,
        wxT("rEspanso \u2014 \u0444\u043E\u0440\u043A Espanso. \u0410\u0432\u0442\u043E\u0440 \u0444\u043E\u0440\u043A\u0430: \u041A\u0443\u0446\u0438\u043D \u0418\u0432\u0430\u043D \u042E\u0440\u044C\u0435\u0432\u0438\u0447"),
        wxDefaultPosition, wxDefaultSize, 0);
    doc_label->Wrap(-1);
    bSizer1->Add(doc_label, 0, wxALIGN_CENTER | wxALL, 10);

    m_hyperlink1 = new wxHyperlinkCtrl(
        this, wxID_ANY, wxT("imaganate.dark@gmail.com"),
        wxT("mailto:imaganate.dark@gmail.com"), wxDefaultPosition,
        wxDefaultSize, wxHL_DEFAULT_STYLE);
    bSizer1->Add(m_hyperlink1, 0, wxALIGN_CENTER | wxALL, 10);

    bSizer1->Add(0, 0, 1, wxEXPAND, 5);

    wxBoxSizer *bSizer2;
    bSizer2 = new wxBoxSizer(wxHORIZONTAL);

    dont_show_checkbox = new wxCheckBox(
        this, wxID_ANY,
        wxT("\u0411\u043E\u043B\u044C\u0448\u0435 \u043D\u0435 \u043F\u043E\u043A\u0430\u0437\u044B\u0432\u0430\u0442\u044C"),
        wxDefaultPosition, wxDefaultSize, 0);
    bSizer2->Add(dont_show_checkbox, 0, wxALIGN_CENTER_VERTICAL | wxALL, 10);

    bSizer2->Add(0, 0, 1, wxEXPAND, 5);

    got_it_btn = new wxButton(
        this, wxID_ANY,
        wxT("\u041F\u043E\u043D\u044F\u0442\u043D\u043E"),
        wxDefaultPosition, wxDefaultSize, 0);

    got_it_btn->SetDefault();
    bSizer2->Add(got_it_btn, 0, wxALIGN_CENTER_VERTICAL | wxALL, 10);

    bSizer1->Add(bSizer2, 0, wxEXPAND, 10);

    this->SetSizer(bSizer1);
    this->Layout();

    this->Centre(wxBOTH);

    // Connect Events
    dont_show_checkbox->Connect(
        wxEVT_COMMAND_CHECKBOX_CLICKED,
        wxCommandEventHandler(WelcomeFrame::on_dont_show_change), NULL, this);
    got_it_btn->Connect(wxEVT_COMMAND_BUTTON_CLICKED,
                        wxCommandEventHandler(WelcomeFrame::on_complete), NULL,
                        this);
}

WelcomeFrame::~WelcomeFrame() {
    // Disconnect Events
    dont_show_checkbox->Disconnect(
        wxEVT_COMMAND_CHECKBOX_CLICKED,
        wxCommandEventHandler(WelcomeFrame::on_dont_show_change), NULL, this);
    got_it_btn->Disconnect(wxEVT_COMMAND_BUTTON_CLICKED,
                           wxCommandEventHandler(WelcomeFrame::on_complete),
                           NULL, this);
}
