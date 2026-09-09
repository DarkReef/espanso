/*
 * This file is part of modulo.
 *
 * Copyright (C) 2020-2021 Federico Terzi
 *
 * modulo is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * modulo is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with modulo.  If not, see <https://www.gnu.org/licenses/>.
 */

// Mouse dragging mechanism greatly inspired by:
// https://developpaper.com/wxwidgets-implementing-the-drag-effect-of-titleless-bar-window/

#define _UNICODE

#include "../common/common.h"
#include "../interop/interop.h"

#include "wx/htmllbox.h"

#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

#ifdef __WXOSX__
// Implemented in search_mac.mm. Returns true if the key window's first
// responder currently has marked (uncommitted) text from an IME
// composition (e.g. selecting a CJK candidate).
extern bool IsImeComposingInKeyWindow();
#endif

// Platform-specific styles
#ifdef __WXMSW__
const int SEARCH_BAR_FONT_SIZE = 16;
const long DEFAULT_STYLE = wxSTAY_ON_TOP | wxFRAME_TOOL_WINDOW | wxRESIZE_BORDER;
#endif
#ifdef __WXOSX__
const int SEARCH_BAR_FONT_SIZE = 20;
const long DEFAULT_STYLE = wxSTAY_ON_TOP | wxRESIZE_BORDER;
#endif
#ifdef __LINUX__
const int SEARCH_BAR_FONT_SIZE = 20;
const long DEFAULT_STYLE = wxSTAY_ON_TOP | wxRESIZE_BORDER;
#endif

const int HELP_TEXT_FONT_SIZE = 10;

const wxColour SELECTION_LIGHT_BG = wxColour(164, 210, 253);
const wxColour SELECTION_DARK_BG = wxColour(49, 88, 126);

// https://docs.wxwidgets.org/stable/classwx_frame.html
const int MIN_WIDTH = 500;
const int MIN_HEIGHT = 80;

typedef void (*QueryCallback)(const char *query, void *app, void *data);
typedef void (*ResultCallback)(const char *id, void *data);

SearchMetadata *searchMetadata = nullptr;
QueryCallback queryCallback = nullptr;
ResultCallback resultCallback = nullptr;
void *data = nullptr;
void *resultData = nullptr;
wxArrayString wxItems;
wxArrayString wxTriggers;
wxArrayString wxIds;
wxArrayInt wxUsageCounts;
wxArrayInt wxFavorites;
std::unordered_map<std::string, bool> favoriteOverrides;

// App Code

class SearchApp : public wxApp {
  public:
    virtual bool OnInit();
};

class ResultListBox : public wxHtmlListBox {
  public:
    ResultListBox() {}
    ResultListBox(wxWindow *parent, bool isDark, const wxWindowID id,
                  const wxPoint &pos, const wxSize &size);

  protected:
    virtual wxString OnGetItem(size_t n) const;
    virtual void OnDrawBackground(wxDC &dc, const wxRect &rect, size_t n) const;

    bool isDark = false;

  public:
    wxDECLARE_NO_COPY_CLASS(ResultListBox);
    wxDECLARE_DYNAMIC_CLASS(ResultListBox);
};

wxIMPLEMENT_DYNAMIC_CLASS(ResultListBox, wxHtmlListBox);

ResultListBox::ResultListBox(wxWindow *parent, bool isDark, const wxWindowID id,
                             const wxPoint &pos, const wxSize &size)
    : wxHtmlListBox(parent, id, pos, size, 0) {
    this->isDark = isDark;
    SetMargins(5, 5);
    Refresh();
}

void ResultListBox::OnDrawBackground(wxDC &dc, const wxRect &rect,
                                     size_t n) const {
    if (IsSelected(n)) {
        if (isDark) {
            dc.SetBrush(wxBrush(SELECTION_DARK_BG));
        } else {
            dc.SetBrush(wxBrush(SELECTION_LIGHT_BG));
        }
    } else {
        dc.SetBrush(*wxTRANSPARENT_BRUSH);
    }
    dc.SetPen(*wxTRANSPARENT_PEN);
    dc.DrawRectangle(0, 0, rect.GetRight(), rect.GetBottom());
}

wxString EscapeHtml(const wxString &str) {
    wxString escaped = str;
    escaped.Replace(wxT("&"), wxT("&amp;"));
    escaped.Replace(wxT("<"), wxT("&lt;"));
    escaped.Replace(wxT(">"), wxT("&gt;"));
    escaped.Replace(wxT("\""), wxT("&quot;"));
    return escaped;
}

wxString ResultListBox::OnGetItem(size_t n) const {
    wxString textColor = isDark ? "white" : "";
    wxString shortcut =
        (n < 8) ? wxString::Format(wxT("Alt+%i"), (int)n + 1) : " ";

    wxString escapedLabel = EscapeHtml(wxItems[n]);
    wxString escapedTrigger = EscapeHtml(wxTriggers[n]);
    wxString favorite = wxFavorites[n] ? wxT("<font color='#f1c40f'>&#9733;</font> ") : wxT("");
    wxString usage = wxString::Format(wxT("%d/мес"), wxUsageCounts[n]);

    wxString result = wxString::Format(
        wxT("<font color='%s'><table width='100%%'><tr><td>%s%s</td><td "
            "align='right'><font color='#636e72'>%s</font> &nbsp; <b>%s</b> "
            "<font color='#636e72'>%s</font></td></tr></table></font>"),
        textColor, favorite, escapedLabel, usage, escapedTrigger, shortcut);

    return result;
}

class SearchFrame : public wxFrame {
  public:
    SearchFrame(const wxString &title, const wxPoint &pos, const wxSize &size);

    wxPanel *panel = nullptr;
    wxTextCtrl *searchBar = nullptr;
    wxStaticBitmap *iconPanel = nullptr;
    wxStaticText *helpText = nullptr;
    wxButton *allTab = nullptr;
    wxButton *favoritesTab = nullptr;
    ResultListBox *resultBox = nullptr;
    void SetItems(SearchItem *items, int itemSize);

  private:
    void OnCharEvent(wxKeyEvent &event);
    void OnQueryChange(wxCommandEvent &event);
    void OnItemClickEvent(wxCommandEvent &event);
    void OnActivate(wxActivateEvent &event);
    void OnAllTab(wxCommandEvent &event);
    void OnFavoritesTab(wxCommandEvent &event);
    void Requery();
    void ToggleFavorite();

    // Mouse events
    void OnMouseCaptureLost(wxMouseCaptureLostEvent &event);
    void OnMouseLeave(wxMouseEvent &event);
    void OnMouseMove(wxMouseEvent &event);
    void OnMouseLUp(wxMouseEvent &event);
    void OnMouseLDown(wxMouseEvent &event);
    wxPoint mLastPt;

    // Selection
    void SelectNext();
    void SelectPrevious();
    void Submit();

    bool favoritesOnly = false;
};

bool SearchApp::OnInit() {
    SearchFrame *frame =
        new SearchFrame(wxString::FromUTF8(searchMetadata->windowTitle),
                        wxPoint(50, 50), wxSize(450, 340));
    frame->Show(true);
    SetupWindowStyle(frame);
    Activate(frame);
    return true;
}

SearchFrame::SearchFrame(const wxString &title, const wxPoint &pos,
                         const wxSize &size)
    : wxFrame(NULL, wxID_ANY, title, pos, size, DEFAULT_STYLE) {
    wxInitAllImageHandlers();

#if wxCHECK_VERSION(3, 1, 3)
    bool isDark = wxSystemSettings::GetAppearance().IsDark();
#else
    const wxColour bg = wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW);
    const wxColour fg = wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOWTEXT);
    unsigned int bgSum = (bg.Red() + bg.Blue() + bg.Green());
    unsigned int fgSum = (fg.Red() + fg.Blue() + fg.Green());
    bool isDark = fgSum > bgSum;
#endif

    panel = new wxPanel(this, wxID_ANY);
    wxBoxSizer *vbox = new wxBoxSizer(wxVERTICAL);
    panel->SetSizer(vbox);

    wxBoxSizer *topBox = new wxBoxSizer(wxHORIZONTAL);

    int iconId = NewControlId();
    iconPanel = nullptr;
    if (searchMetadata->iconPath) {
        wxString iconPath = wxString::FromUTF8(searchMetadata->iconPath);
        if (wxFileExists(iconPath)) {
            wxBitmap bitmap = wxBitmap(iconPath, wxBITMAP_TYPE_PNG);
            if (bitmap.IsOk()) {
                wxImage image = bitmap.ConvertToImage();
                image.Rescale(32, 32, wxIMAGE_QUALITY_HIGH);
                wxBitmap resizedBitmap = wxBitmap(image, -1);
                iconPanel =
                    new wxStaticBitmap(panel, iconId, resizedBitmap,
                                       wxDefaultPosition, wxSize(32, 32));
                topBox->Add(iconPanel, 0, wxEXPAND | wxLEFT | wxUP | wxDOWN,
                            10);
            }
        }
    }

    int textId = NewControlId();
    searchBar =
        new wxTextCtrl(panel, textId, "", wxDefaultPosition, wxDefaultSize);
    wxFont font = searchBar->GetFont();
    font.SetPointSize(SEARCH_BAR_FONT_SIZE);
    searchBar->SetFont(font);
    topBox->Add(searchBar, 1, wxEXPAND | wxALL, 10);

    vbox->Add(topBox, 1, wxEXPAND);

    if (searchMetadata->hintText) {
        helpText = new wxStaticText(
            panel, wxID_ANY, wxString::FromUTF8(searchMetadata->hintText));
        vbox->Add(helpText, 0, wxEXPAND | wxLEFT | wxRIGHT | wxBOTTOM, 10);
        wxFont helpFont = helpText->GetFont();
        helpFont.SetPointSize(HELP_TEXT_FONT_SIZE);
        helpText->SetFont(helpFont);
    }

    wxBoxSizer *tabsBox = new wxBoxSizer(wxHORIZONTAL);
    int allTabId = NewControlId();
    int favoritesTabId = NewControlId();
    allTab = new wxButton(panel, allTabId, wxString::FromUTF8("Все"),
                          wxDefaultPosition, wxDefaultSize, wxBU_EXACTFIT);
    favoritesTab =
        new wxButton(panel, favoritesTabId, wxString::FromUTF8("★ Избранное"),
                     wxDefaultPosition, wxDefaultSize, wxBU_EXACTFIT);
    tabsBox->Add(allTab, 0, wxLEFT | wxBOTTOM, 8);
    tabsBox->Add(favoritesTab, 0, wxLEFT | wxBOTTOM, 6);
    tabsBox->AddStretchSpacer(1);
    wxStaticText *favoriteHint = new wxStaticText(
        panel, wxID_ANY, wxString::FromUTF8("Ctrl+D — добавить/убрать избранное"));
    tabsBox->Add(favoriteHint, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT | wxBOTTOM, 8);
    vbox->Add(tabsBox, 0, wxEXPAND);
    allTab->Disable();

    int resultId = NewControlId();
    resultBox = new ResultListBox(panel, isDark, resultId, wxDefaultPosition,
                                  wxSize(MIN_WIDTH, MIN_HEIGHT));
    vbox->Add(resultBox, 5, wxEXPAND | wxALL, 0);

    Bind(wxEVT_CHAR_HOOK, &SearchFrame::OnCharEvent, this, wxID_ANY);
    searchBar->Bind(wxEVT_CHAR, &SearchFrame::OnCharEvent, this, wxID_ANY);
    Bind(wxEVT_TEXT, &SearchFrame::OnQueryChange, this, textId);
    Bind(wxEVT_LISTBOX_DCLICK, &SearchFrame::OnItemClickEvent, this, resultId);
    Bind(wxEVT_ACTIVATE, &SearchFrame::OnActivate, this, wxID_ANY);
    Bind(wxEVT_BUTTON, &SearchFrame::OnAllTab, this, allTabId);
    Bind(wxEVT_BUTTON, &SearchFrame::OnFavoritesTab, this, favoritesTabId);

    if (iconPanel) {
        iconPanel->Bind(wxEVT_LEFT_UP, &SearchFrame::OnMouseLUp, this);
        iconPanel->Bind(wxEVT_LEFT_DOWN, &SearchFrame::OnMouseLDown, this);
        Bind(wxEVT_MOTION, &SearchFrame::OnMouseMove, this);
        Bind(wxEVT_LEFT_UP, &SearchFrame::OnMouseLUp, this);
        Bind(wxEVT_LEFT_DOWN, &SearchFrame::OnMouseLDown, this);
        Bind(wxEVT_MOUSE_CAPTURE_LOST, &SearchFrame::OnMouseCaptureLost, this);
        Bind(wxEVT_LEAVE_WINDOW, &SearchFrame::OnMouseLeave, this);
    }

    wxSize bestSize = panel->GetBestSize();
    this->SetClientSize(bestSize);
    this->SetSizeHints(MIN_WIDTH, MIN_HEIGHT);
    this->CentreOnScreen();

    queryCallback("", (void *)this, data);
}

void SearchFrame::OnCharEvent(wxKeyEvent &event) {
    if (event.GetKeyCode() == WXK_ESCAPE) {
        Close(true);
    } else if ((event.GetKeyCode() == 'D' || event.GetKeyCode() == 'd') &&
               event.RawControlDown()) {
        ToggleFavorite();
    } else if (event.GetKeyCode() == WXK_TAB) {
        if (wxGetKeyState(WXK_SHIFT)) {
            SelectPrevious();
        } else {
            SelectNext();
        }
    } else if (event.GetUnicodeKey() >= '1' &&
               event.GetUnicodeKey() <= '9') {
        int index = event.GetUnicodeKey() - '1';
        if (wxGetKeyState(WXK_ALT)) {
            if (resultBox->GetItemCount() > index) {
                resultBox->SetSelection(index);
                Submit();
            }
        } else {
            event.Skip();
        }
    } else if (event.GetKeyCode() == WXK_DOWN) {
        SelectNext();
    } else if (event.GetKeyCode() == WXK_UP) {
        SelectPrevious();
    } else if (event.GetKeyCode() == 'N' && event.RawControlDown()) {
        SelectNext();
    } else if (event.GetKeyCode() == 'P' && event.RawControlDown()) {
        SelectPrevious();
    } else if (event.GetKeyCode() == WXK_RETURN ||
               event.GetKeyCode() == WXK_NUMPAD_ENTER) {
#ifdef __WXOSX__
        if (IsImeComposingInKeyWindow()) {
            event.Skip();
            return;
        }
#endif
        Submit();
    } else {
        event.Skip();
    }
}

void SearchFrame::Requery() {
    wxString queryString = searchBar->GetValue();
    const char *query = queryString.ToUTF8();
    queryCallback(query, (void *)this, data);
}

void SearchFrame::OnQueryChange(wxCommandEvent &event) {
    if (helpText != nullptr) {
        helpText->Destroy();
        panel->Layout();
        helpText = nullptr;
    }
    Requery();
}

void SearchFrame::OnAllTab(wxCommandEvent &event) {
    favoritesOnly = false;
    allTab->Disable();
    favoritesTab->Enable();
    Requery();
    searchBar->SetFocus();
}

void SearchFrame::OnFavoritesTab(wxCommandEvent &event) {
    favoritesOnly = true;
    favoritesTab->Disable();
    allTab->Enable();
    Requery();
    searchBar->SetFocus();
}

void SearchFrame::OnItemClickEvent(wxCommandEvent &event) {
    resultBox->SetSelection(event.GetInt());
    Submit();
}

void SearchFrame::OnActivate(wxActivateEvent &event) {
    if (!event.GetActive()) {
        Close(true);
    }
    event.Skip();
}

void SearchFrame::OnMouseMove(wxMouseEvent &event) {
    if (event.LeftIsDown() && event.Dragging()) {
        wxPoint pt = event.GetPosition();
        wxPoint wndLeftTopPt = GetPosition();
        int distanceX = pt.x - mLastPt.x;
        int distanceY = pt.y - mLastPt.y;

        wxPoint desPt;
        desPt.x = distanceX + wndLeftTopPt.x - 24;
        desPt.y = distanceY + wndLeftTopPt.y - 24;
        this->Move(desPt);
    }

    if (event.LeftDown()) {
        this->CaptureMouse();
        mLastPt = event.GetPosition();
    }
}

void SearchFrame::OnMouseLeave(wxMouseEvent &event) {
    if (event.LeftIsDown() && event.Dragging()) {
        wxPoint pt = event.GetPosition();
        wxPoint wndLeftTopPt = GetPosition();
        int distanceX = pt.x - mLastPt.x;
        int distanceY = pt.y - mLastPt.y;

        wxPoint desPt;
        desPt.x = distanceX + wndLeftTopPt.x - 24;
        desPt.y = distanceY + wndLeftTopPt.y - 24;
        this->Move(desPt);
    }
}

void SearchFrame::OnMouseLDown(wxMouseEvent &event) {
    if (!HasCapture())
        this->CaptureMouse();
}

void SearchFrame::OnMouseLUp(wxMouseEvent &event) {
    if (HasCapture())
        ReleaseMouse();
}

void SearchFrame::OnMouseCaptureLost(wxMouseCaptureLostEvent &event) {
    if (HasCapture())
        ReleaseMouse();
}

void SearchFrame::SetItems(SearchItem *items, int itemSize) {
    wxItems.Clear();
    wxIds.Clear();
    wxTriggers.Clear();
    wxUsageCounts.Clear();
    wxFavorites.Clear();

    for (int i = 0; i < itemSize; i++) {
        std::string key = items[i].id ? items[i].id : "";
        bool favorite = items[i].favorite != 0;
        std::unordered_map<std::string, bool>::const_iterator overrideIt =
            favoriteOverrides.find(key);
        if (overrideIt != favoriteOverrides.end()) {
            favorite = overrideIt->second;
        }

        if (favoritesOnly && !favorite) {
            continue;
        }

        wxItems.Add(wxString::FromUTF8(items[i].label));
        wxIds.Add(wxString::FromUTF8(items[i].id));
        wxTriggers.Add(wxString::FromUTF8(items[i].trigger));
        wxUsageCounts.Add(items[i].usageCount < 0 ? 0 : items[i].usageCount);
        wxFavorites.Add(favorite ? 1 : 0);
    }

    resultBox->SetItemCount(wxItems.GetCount());

    if (!wxItems.IsEmpty()) {
        resultBox->SetSelection(0);
    } else {
        resultBox->SetSelection(wxNOT_FOUND);
    }
    resultBox->RefreshAll();
    resultBox->Refresh();
}

void SearchFrame::ToggleFavorite() {
    if (resultBox->GetItemCount() == 0 ||
        resultBox->GetSelection() == wxNOT_FOUND) {
        return;
    }

    long index = resultBox->GetSelection();
    bool favorite = wxFavorites[index] == 0;
    wxFavorites[index] = favorite ? 1 : 0;

    wxString id = wxIds[index];
    std::string key = std::string(id.ToUTF8());
    favoriteOverrides[key] = favorite;

    if (resultCallback) {
        wxString callbackId = wxString::FromUTF8("__respanso_favorite__:") + id +
                              (favorite ? wxT(":1") : wxT(":0"));
        resultCallback(callbackId.ToUTF8(), resultData);
    }

    if (favoritesOnly && !favorite) {
        Requery();
    } else {
        resultBox->RefreshAll();
        resultBox->Refresh();
    }
}

void SearchFrame::SelectNext() {
    if (resultBox->GetItemCount() > 0 &&
        resultBox->GetSelection() != wxNOT_FOUND) {
        int newSelected = 0;
        if (resultBox->GetSelection() < (resultBox->GetItemCount() - 1)) {
            newSelected = resultBox->GetSelection() + 1;
        }

        resultBox->SetSelection(newSelected);
    }
}

void SearchFrame::SelectPrevious() {
    if (resultBox->GetItemCount() > 0 &&
        resultBox->GetSelection() != wxNOT_FOUND) {
        int newSelected = resultBox->GetItemCount() - 1;
        if (resultBox->GetSelection() > 0) {
            newSelected = resultBox->GetSelection() - 1;
        }

        resultBox->SetSelection(newSelected);
    }
}

void SearchFrame::Submit() {
    if (resultBox->GetItemCount() > 0 &&
        resultBox->GetSelection() != wxNOT_FOUND) {
        long index = resultBox->GetSelection();
        wxString id = wxIds[index];
        if (resultCallback) {
            resultCallback(id.ToUTF8(), resultData);
        }

        Close(true);
    }
}

extern "C" void interop_show_search(SearchMetadata *_metadata,
                                    QueryCallback _queryCallback, void *_data,
                                    ResultCallback _resultCallback,
                                    void *_resultData) {
#ifdef __WXMSW__
    SetProcessDPIAware();
#endif

    searchMetadata = _metadata;
    queryCallback = _queryCallback;
    resultCallback = _resultCallback;
    data = _data;
    resultData = _resultData;
    favoriteOverrides.clear();

    wxApp::SetInstance(new SearchApp());
    int argc = 0;
    wxEntry(argc, (char **)nullptr);
}

extern "C" void update_items(void *app, SearchItem *items, int itemSize) {
    SearchFrame *frame = (SearchFrame *)app;
    frame->SetItems(items, itemSize);
}
