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

#define _UNICODE

#ifdef _MSC_VER
#pragma execution_character_set("utf-8")
#endif

#include "../common/common.h"
#include "../interop/interop.h"

#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

// https://docs.wxwidgets.org/stable/classwx_frame.html
const long DEFAULT_STYLE = wxSTAY_ON_TOP | wxCLOSE_BOX | wxCAPTION;

const int PADDING = 5;
const int MULTILINE_MIN_HEIGHT = 100;
const int MULTILINE_MIN_WIDTH = 100;
const int PREVIEW_MIN_HEIGHT = 120;
const char *PREVIEW_SENTINEL_ID = "__respanso_preview__";
const int PREVIEW_MODE_LAYOUT = 0;
const int PREVIEW_MODE_LIVE = 1;
const int PREVIEW_MODE_MANUAL = 2;
const int PREVIEW_MODE_SUBMIT = 3;
const int PREVIEW_REQUEST_LIVE = 1;
const int PREVIEW_REQUEST_MANUAL = 2;
const int PREVIEW_REQUEST_SUBMIT = 3;

typedef const char *(*PreviewCallback)(const ValuePair *, int, int, int *,
                                       void *);

FormMetadata *formMetadata = nullptr;
std::vector<ValuePair> values;
PreviewCallback previewCallback = nullptr;
void *previewData = nullptr;

// Field Wrappers

class FieldWrapper {
  public:
    virtual ~FieldWrapper() = default;
    virtual wxString getValue() = 0;
};

class TextFieldWrapper : public FieldWrapper {
    wxTextCtrl *control;

  public:
    explicit TextFieldWrapper(wxTextCtrl *control) : control(control) {}

    wxString getValue() override { return control->GetValue(); }
};

class ChoiceFieldWrapper : public FieldWrapper {
    wxChoice *control;

  public:
    explicit ChoiceFieldWrapper(wxChoice *control) : control(control) {}

    wxString getValue() override { return control->GetStringSelection(); }
};

class ListFieldWrapper : public FieldWrapper {
    wxListBox *control;
    wxString separator;

  public:
    explicit ListFieldWrapper(wxListBox *control, wxString separator)
        : control(control), separator(separator) {}

    wxString getValue() override {
      wxArrayInt selections;
      control->GetSelections(selections);

      wxString value = "";
      for (unsigned int i = 0; i < selections.size(); i++) {
        if (i > 0) value.Append(separator);
        value.Append(control->GetString(selections[i]));
      }

      return value;
    }
};

// App Code

class FormApp : public wxApp {
  public:
    virtual bool OnInit();
};
enum {
    ID_Submit = 20000,
    ID_Preview = 20001,
    ID_PreviewTimer = 20002,
};

class FormFrame : public wxFrame {
  public:
    FormFrame(const wxString &title, const wxPoint &pos, const wxSize &size);

    wxPanel *panel;
    std::vector<void *> fields;
    std::unordered_map<std::string, std::unique_ptr<FieldWrapper>> idMap;
    wxButton *submit;
    wxButton *previewButton;
    wxStaticText *helpText;
    wxStaticText *previewLabel;
    wxTextCtrl *previewControl;
    wxTimer previewTimer;
    bool hasFocusedMultilineControl;
    bool previewEnabled;
    bool computedPreviewEnabled;
    int previewMode;
    int previewDebounceMs;
    int lastPreviewStatus;

  private:
    void AddComponent(wxPanel *parent, wxBoxSizer *sizer, FieldMetadata meta);
    void Submit();
    void OnSubmitBtn(wxCommandEvent &event);
    void OnPreviewBtn(wxCommandEvent &event);
    void OnPreviewTimer(wxTimerEvent &event);
    void OnCharHook(wxKeyEvent &event);
    void OnListBoxEvent(wxCommandEvent &event);
    void OnFieldChanged(wxCommandEvent &event);
    void UpdateHelpText();
    void UpdatePreview();
    bool RequestComputedPreview(int request);
    std::vector<ValuePair> CollectCurrentValues(
        std::vector<std::string> &ids,
        std::vector<std::string> &fieldValues);
    wxString RenderPreview();
    wxString RenderField(FieldMetadata meta);
    void HandleNormalFocus(wxFocusEvent &event);
    void HandleMultilineFocus(wxFocusEvent &event);
};

bool FormApp::OnInit() {
    const wxSize &maxFormSize =
        wxSize(formMetadata->maxWindowWidth, formMetadata->maxWindowHeight);
    FormFrame *frame =
        new FormFrame(wxString::FromUTF8(formMetadata->windowTitle),
                      wxPoint(50, 50), maxFormSize);
    frame->SetMaxSize(maxFormSize);
    setFrameIcon(wxString::FromUTF8(formMetadata->iconPath), frame);
    frame->Show(true);

    Activate(frame);

    return true;
}
FormFrame::FormFrame(const wxString &title, const wxPoint &pos,
                     const wxSize &size)
    : wxFrame(NULL, wxID_ANY, title, pos, size, DEFAULT_STYLE) {
    hasFocusedMultilineControl = false;
    previewEnabled = false;
    previewLabel = nullptr;
    previewControl = nullptr;
    previewButton = nullptr;
    computedPreviewEnabled = formMetadata->computedPreviewEnabled != 0;
    previewMode = formMetadata->previewMode;
    previewDebounceMs = formMetadata->previewDebounceMs;
    lastPreviewStatus = 0;
    previewTimer.SetOwner(this, ID_PreviewTimer);

    panel = new wxPanel(this, wxID_ANY);
    wxBoxSizer *vbox = new wxBoxSizer(wxVERTICAL);
    panel->SetSizer(vbox);

    for (int field = 0; field < formMetadata->fieldSize; field++) {
        FieldMetadata meta = formMetadata->fields[field];
        if (meta.id != nullptr && strcmp(meta.id, PREVIEW_SENTINEL_ID) == 0) {
            previewEnabled = true;
            continue;
        }
        AddComponent(panel, vbox, meta);
    }

    if (previewEnabled) {
        previewLabel = new wxStaticText(
            panel, wxID_ANY, wxString::FromUTF8("Предпросмотр"));
        wxFont previewFont = previewLabel->GetFont();
        previewFont.SetWeight(wxFONTWEIGHT_BOLD);
        previewLabel->SetFont(previewFont);
        vbox->Add(previewLabel, 0, wxLEFT | wxRIGHT | wxTOP, PADDING);

        if (computedPreviewEnabled && previewMode == PREVIEW_MODE_MANUAL) {
            previewButton = new wxButton(
                panel, ID_Preview, wxString::FromUTF8("Рассчитать"));
            vbox->Add(previewButton, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP,
                      PADDING);
        }

        previewControl = new wxTextCtrl(
            panel, wxID_ANY, "", wxDefaultPosition, wxDefaultSize,
            wxTE_MULTILINE | wxTE_READONLY);
        previewControl->SetMinSize(
            wxSize(MULTILINE_MIN_WIDTH, PREVIEW_MIN_HEIGHT));
        vbox->Add(previewControl, 1, wxEXPAND | wxALL, PADDING);
    }

    submit = new wxButton(panel, ID_Submit, wxString::FromUTF8("Вставить"));
    vbox->Add(submit, 0, wxEXPAND | wxALL, PADDING);

    helpText =
        new wxStaticText(panel, wxID_ANY, "", wxDefaultPosition, wxDefaultSize);
    wxFont helpFont = helpText->GetFont();
    helpFont.SetPointSize(8);
    helpText->SetFont(helpFont);
    vbox->Add(helpText, 0, wxLEFT | wxRIGHT | wxBOTTOM, PADDING);
    UpdateHelpText();

    Bind(wxEVT_BUTTON, &FormFrame::OnSubmitBtn, this, ID_Submit);
    Bind(wxEVT_BUTTON, &FormFrame::OnPreviewBtn, this, ID_Preview);
    Bind(wxEVT_TIMER, &FormFrame::OnPreviewTimer, this, ID_PreviewTimer);
    Bind(wxEVT_CHAR_HOOK, &FormFrame::OnCharHook, this, wxID_ANY);

    if (previewEnabled) {
        if (!computedPreviewEnabled || previewMode == PREVIEW_MODE_LAYOUT) {
            UpdatePreview();
        } else if (previewMode == PREVIEW_MODE_LIVE) {
            previewControl->ChangeValue(wxString::FromUTF8("Расчёт…"));
            previewTimer.StartOnce(10);
        } else if (previewMode == PREVIEW_MODE_MANUAL) {
            previewControl->ChangeValue(
                wxString::FromUTF8("Нажмите «Рассчитать», чтобы увидеть результат."));
        } else {
            previewControl->ChangeValue(wxString::FromUTF8(
                "Результат будет рассчитан перед вставкой."));
        }
    }

    this->SetClientSize(panel->GetBestSize());
    this->CentreOnScreen();
}

void FormFrame::AddComponent(wxPanel *parent, wxBoxSizer *sizer,
                             FieldMetadata meta) {
    void *control = nullptr;

    switch (meta.fieldType) {
    case FieldType::LABEL: {
        const LabelMetadata *labelMeta =
            static_cast<const LabelMetadata *>(meta.specific);
        const long style = wxST_ELLIPSIZE_END;
        auto label = new wxStaticText(parent, wxID_ANY,
                                      wxString::FromUTF8(labelMeta->text),
                                      wxDefaultPosition, wxDefaultSize, style);

        label->Wrap(this->GetClientSize().GetWidth());
        control = label;
        fields.push_back(label);
        break;
    }
    case FieldType::TEXT: {
        const TextMetadata *textMeta =
            static_cast<const TextMetadata *>(meta.specific);
        long style = 0;
        if (textMeta->multiline) {
            style |= wxTE_MULTILINE;
        }

        auto textControl = new wxTextCtrl(
            parent, NewControlId(), wxString::FromUTF8(textMeta->defaultText),
            wxDefaultPosition, wxDefaultSize, style);

        if (textMeta->multiline) {
            textControl->SetMinSize(
                wxSize(MULTILINE_MIN_WIDTH, MULTILINE_MIN_HEIGHT));
            textControl->Bind(wxEVT_SET_FOCUS, &FormFrame::HandleMultilineFocus,
                              this, wxID_ANY);
        } else {
            textControl->Bind(wxEVT_SET_FOCUS, &FormFrame::HandleNormalFocus,
                              this, wxID_ANY);
        }

        textControl->Bind(wxEVT_TEXT, &FormFrame::OnFieldChanged, this,
                          wxID_ANY);

        std::unique_ptr<FieldWrapper> field(
            new TextFieldWrapper(textControl));
        idMap[std::string(meta.id)] = std::move(field);
        control = textControl;
        fields.push_back(textControl);
        break;
    }
    case FieldType::CHOICE: {
        const ChoiceMetadata *choiceMeta =
            static_cast<const ChoiceMetadata *>(meta.specific);

        int selectedItem = -1;
        wxArrayString choices;
        for (int i = 0; i < choiceMeta->valueSize; i++) {
            choices.Add(wxString::FromUTF8(choiceMeta->values[i]));

            if (strcmp(choiceMeta->values[i], choiceMeta->defaultValue) == 0) {
                selectedItem = i;
            }
        }

        void *choice = nullptr;
        wxString separator = wxString::FromUTF8(choiceMeta->separator);
        if (choiceMeta->choiceType == ChoiceType::DROPDOWN) {
            choice = (void *)new wxChoice(parent, wxID_ANY, wxDefaultPosition,
                                          wxDefaultSize, choices);

            if (selectedItem >= 0) {
                ((wxChoice *)choice)->SetSelection(selectedItem);
            }

            ((wxChoice *)choice)
                ->Bind(wxEVT_SET_FOCUS, &FormFrame::HandleNormalFocus, this,
                       wxID_ANY);
            ((wxChoice *)choice)
                ->Bind(wxEVT_CHOICE, &FormFrame::OnFieldChanged, this,
                       wxID_ANY);

            std::unique_ptr<FieldWrapper> field(
                new ChoiceFieldWrapper((wxChoice *)choice));
            idMap[std::string(meta.id)] = std::move(field);
        } else {
            choice = (void *)new wxListBox(parent, wxID_ANY, wxDefaultPosition,
                                           wxDefaultSize, choices,
                                           wxLB_EXTENDED);

            if (selectedItem >= 0) {
                ((wxListBox *)choice)->SetSelection(selectedItem);
            }

            ((wxListBox *)choice)
                ->Bind(wxEVT_SET_FOCUS, &FormFrame::HandleNormalFocus, this,
                       wxID_ANY);
            ((wxListBox *)choice)
                ->Bind(wxEVT_LISTBOX, &FormFrame::OnFieldChanged, this,
                       wxID_ANY);
            // ListBoxes prevent the global CHAR_HOOK handler from handling the
            // Return key correctly, so we need to handle the double click event
            // too (which is triggered when the enter key is pressed). See:
            // https://github.com/espanso/espanso/issues/857
            ((wxListBox *)choice)
                ->Bind(wxEVT_LISTBOX_DCLICK, &FormFrame::OnListBoxEvent, this,
                       wxID_ANY);

            std::unique_ptr<FieldWrapper> field(
                new ListFieldWrapper((wxListBox *)choice, separator));
            idMap[std::string(meta.id)] = std::move(field);
        }

        control = choice;
        fields.push_back(choice);
        break;
    }
    case FieldType::ROW: {
        const RowMetadata *rowMeta =
            static_cast<const RowMetadata *>(meta.specific);

        auto innerPanel = new wxPanel(panel, wxID_ANY);
        wxBoxSizer *hbox = new wxBoxSizer(wxHORIZONTAL);
        innerPanel->SetSizer(hbox);
        sizer->Add(innerPanel, 0, wxEXPAND | wxALL, 0);
        fields.push_back(innerPanel);

        for (int field = 0; field < rowMeta->fieldSize; field++) {
            FieldMetadata innerMeta = rowMeta->fields[field];
            AddComponent(innerPanel, hbox, innerMeta);
        }

        break;
    }
    default:
        // TODO: handle unknown field type
        break;
    }

    if (control) {
        sizer->Add((wxWindow *)control, 0, wxEXPAND | wxALL, PADDING);
    }
}

wxString FormFrame::RenderField(FieldMetadata meta) {
    switch (meta.fieldType) {
    case FieldType::LABEL: {
        const LabelMetadata *labelMeta =
            static_cast<const LabelMetadata *>(meta.specific);
        return wxString::FromUTF8(labelMeta->text);
    }
    case FieldType::TEXT:
    case FieldType::CHOICE: {
        if (meta.id == nullptr) return "";
        auto field = idMap.find(std::string(meta.id));
        if (field == idMap.end()) return "";
        return field->second->getValue();
    }
    case FieldType::ROW: {
        const RowMetadata *rowMeta =
            static_cast<const RowMetadata *>(meta.specific);
        wxString row;
        for (int field = 0; field < rowMeta->fieldSize; field++) {
            row.Append(RenderField(rowMeta->fields[field]));
        }
        return row;
    }
    default:
        return "";
    }
}

wxString FormFrame::RenderPreview() {
    wxString preview;
    bool firstRow = true;

    for (int field = 0; field < formMetadata->fieldSize; field++) {
        FieldMetadata meta = formMetadata->fields[field];
        if (meta.id != nullptr && strcmp(meta.id, PREVIEW_SENTINEL_ID) == 0) {
            continue;
        }

        if (!firstRow) preview.Append("\n");
        preview.Append(RenderField(meta));
        firstRow = false;
    }

    return preview;
}

std::vector<ValuePair> FormFrame::CollectCurrentValues(
    std::vector<std::string> &ids,
    std::vector<std::string> &fieldValues) {
    ids.reserve(idMap.size());
    fieldValues.reserve(idMap.size());
    for (auto &field : idMap) {
        ids.push_back(field.first);
        wxCharBuffer buffer = field.second->getValue().ToUTF8();
        fieldValues.emplace_back(buffer.data() == nullptr ? "" : buffer.data());
    }

    std::vector<ValuePair> pairs;
    pairs.reserve(ids.size());
    for (size_t index = 0; index < ids.size(); index++) {
        pairs.push_back({ids[index].c_str(), fieldValues[index].c_str()});
    }
    return pairs;
}

bool FormFrame::RequestComputedPreview(int request) {
    if (!computedPreviewEnabled || previewCallback == nullptr) return true;

    std::vector<std::string> ids;
    std::vector<std::string> fieldValues;
    std::vector<ValuePair> pairs = CollectCurrentValues(ids, fieldValues);
    int status = 0;
    const char *response = previewCallback(
        pairs.data(), static_cast<int>(pairs.size()), request, &status, previewData);
    lastPreviewStatus = status;
    if (previewControl != nullptr) {
        previewControl->ChangeValue(
            response == nullptr ? wxString() : wxString::FromUTF8(response));
        previewControl->SetInsertionPoint(0);
    }
    return status == 0;
}

void FormFrame::UpdatePreview() {
    if (!previewEnabled || previewControl == nullptr) return;
    if (computedPreviewEnabled) {
        RequestComputedPreview(PREVIEW_REQUEST_LIVE);
        return;
    }
    previewControl->ChangeValue(RenderPreview());
    previewControl->SetInsertionPoint(0);
}

void FormFrame::Submit() {
    if (computedPreviewEnabled &&
        !RequestComputedPreview(PREVIEW_REQUEST_SUBMIT)) {
        return;
    }

    for (auto &field : idMap) {
        FieldWrapper *fieldWrapper = field.second.get();
        wxString value{fieldWrapper->getValue()};
        wxCharBuffer buffer{value.ToUTF8()};
        char *id = strdup(field.first.c_str());
        char *c_value = strdup(buffer.data() == nullptr ? "" : buffer.data());
        ValuePair valuePair = {id, c_value};
        values.push_back(valuePair);
    }

    Close(true);
}

void FormFrame::HandleNormalFocus(wxFocusEvent &event) {
    hasFocusedMultilineControl = false;
    UpdateHelpText();
    event.Skip();
}

void FormFrame::HandleMultilineFocus(wxFocusEvent &event) {
    hasFocusedMultilineControl = true;
    UpdateHelpText();
    event.Skip();
}

void FormFrame::UpdateHelpText() {
    if (hasFocusedMultilineControl) {
        helpText->SetLabel(wxString::FromUTF8(
            "Ctrl+Enter — вставить, Esc — отменить"));
    } else {
        helpText->SetLabel(wxString::FromUTF8(
            "Enter — вставить, Esc — отменить"));
    }
    this->SetClientSize(panel->GetBestSize());
}

void FormFrame::OnSubmitBtn(wxCommandEvent &event) { Submit(); }

void FormFrame::OnPreviewBtn(wxCommandEvent &event) {
    RequestComputedPreview(PREVIEW_REQUEST_MANUAL);
}

void FormFrame::OnPreviewTimer(wxTimerEvent &event) {
    RequestComputedPreview(PREVIEW_REQUEST_LIVE);
}

void FormFrame::OnFieldChanged(wxCommandEvent &event) {
    if (previewEnabled && computedPreviewEnabled) {
        if (previewMode == PREVIEW_MODE_LIVE) {
            previewControl->ChangeValue(wxString::FromUTF8("Расчёт…"));
            previewTimer.StartOnce(previewDebounceMs);
        } else if (previewMode == PREVIEW_MODE_MANUAL) {
            previewControl->ChangeValue(wxString::FromUTF8(
                "Данные изменены. Нажмите «Рассчитать»."));
        } else {
            previewControl->ChangeValue(wxString::FromUTF8(
                "Данные изменены. Результат будет рассчитан перед вставкой."));
        }
    } else {
        UpdatePreview();
    }
    event.Skip();
}

void FormFrame::OnCharHook(wxKeyEvent &event) {
    if (event.GetKeyCode() == WXK_ESCAPE) {
        Close(true);
    } else if (event.GetKeyCode() == WXK_RETURN) {
        if (!hasFocusedMultilineControl || wxGetKeyState(WXK_RAW_CONTROL)) {
            Submit();
        } else {
            event.Skip();
        }
    } else {
        event.Skip();
    }
}

void FormFrame::OnListBoxEvent(wxCommandEvent &event) { Submit(); }

extern "C" void interop_show_form(
    FormMetadata *_metadata,
    void (*callback)(ValuePair *values, int size, void *data),
    void *data,
    PreviewCallback _previewCallback,
    void *_previewData) {
// Setup high DPI support on Windows
#ifdef __WXMSW__
    SetProcessDPIAware();
#endif

    formMetadata = _metadata;
    previewCallback = _previewCallback;
    previewData = _previewData;
    values.clear();

    wxApp::SetInstance(new FormApp());
    int argc = 0;
    wxEntry(argc, (char **)nullptr);

    callback(values.data(), values.size(), data);

    for (auto pair : values) {
        free((void *)pair.id);
        free((void *)pair.value);
    }
    values.clear();
    previewCallback = nullptr;
    previewData = nullptr;
}
