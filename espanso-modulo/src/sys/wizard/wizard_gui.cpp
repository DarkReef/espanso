///////////////////////////////////////////////////////////////////////////
// C++ code generated with wxFormBuilder (version Oct 26 2018)
// http://www.wxformbuilder.org/
//
// PLEASE DO *NOT* EDIT THIS FILE!
///////////////////////////////////////////////////////////////////////////

#define _UNICODE

#ifdef _MSC_VER
#pragma execution_character_set("utf-8")
#endif

#include "wizard_gui.h"

namespace {
wxString ui_text(const char *text) { return wxString::FromUTF8(text); }
} // namespace

///////////////////////////////////////////////////////////////////////////

WizardFrame::WizardFrame(wxWindow *parent, wxWindowID id, const wxString &title,
                         const wxPoint &pos, const wxSize &size, long style)
    : wxFrame(parent, id, title, pos, size, style) {
    this->SetSizeHints(wxDefaultSize, wxDefaultSize);
    this->SetBackgroundColour(wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    check_timer.SetOwner(this, wxID_ANY);
    check_timer.Start(500);

    wxBoxSizer *bSizer1;
    bSizer1 = new wxBoxSizer(wxVERTICAL);

    m_simplebook =
        new wxSimplebook(this, wxID_ANY, wxDefaultPosition, wxDefaultSize, 0);
    welcome_panel = new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition,
                                wxDefaultSize, wxTAB_TRAVERSAL);
    welcome_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer13;
    bSizer13 = new wxBoxSizer(wxVERTICAL);

    m_scrolledWindow2 =
        new wxScrolledWindow(welcome_panel, wxID_ANY, wxDefaultPosition,
                             wxDefaultSize, wxTAB_TRAVERSAL);
    m_scrolledWindow2->SetScrollRate(0, 0);
    wxBoxSizer *bSizer2;
    bSizer2 = new wxBoxSizer(wxVERTICAL);

    welcome_image =
        new wxStaticBitmap(m_scrolledWindow2, wxID_ANY, wxNullBitmap,
                           wxDefaultPosition, wxSize(160, 160), 0);
    welcome_image->SetMinSize(wxSize(160, 160));

    bSizer2->Add(welcome_image, 0, wxALIGN_CENTER | wxALL, 0);

    welcome_title_text = new wxStaticText(m_scrolledWindow2, wxID_ANY,
                                          ui_text("Добро пожаловать в rEspanso"),
                                          wxDefaultPosition, wxDefaultSize, 0);
    welcome_title_text->Wrap(-1);
    welcome_title_text->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT,
                                       wxFONTSTYLE_NORMAL, wxFONTWEIGHT_BOLD,
                                       false, wxEmptyString));

    bSizer2->Add(welcome_title_text, 0, wxALIGN_CENTER_HORIZONTAL | wxTOP, 12);

    welcome_version_text =
        new wxStaticText(m_scrolledWindow2, wxID_ANY, ui_text("Версия 1.2.3"),
                         wxDefaultPosition, wxDefaultSize, 0);
    welcome_version_text->Wrap(-1);
    bSizer2->Add(welcome_version_text, 0, wxALIGN_CENTER_HORIZONTAL | wxALL, 5);

    bSizer2->Add(0, 20, 0, 0, 5);

    welcome_description_text = new wxStaticText(
        m_scrolledWindow2, wxID_ANY,
        ui_text("rEspanso ускоряет ввод текста с помощью сокращений, шаблонов и локальной автоматизации.\n\nНажмите «Начать», чтобы выполнить первоначальную настройку."),
        wxDefaultPosition, wxDefaultSize, 0);
    welcome_description_text->Wrap(500);
    bSizer2->Add(welcome_description_text, 0, wxALL, 10);

    bSizer2->Add(0, 0, 1, wxEXPAND, 5);

    m_scrolledWindow2->SetSizer(bSizer2);
    m_scrolledWindow2->Layout();
    bSizer13->Add(m_scrolledWindow2, 1, wxEXPAND | wxALL, 5);

    welcome_start_button = new wxButton(welcome_panel, wxID_ANY, ui_text("Начать"),
                                        wxDefaultPosition, wxDefaultSize, 0);

    welcome_start_button->SetDefault();
    bSizer13->Add(welcome_start_button, 0, wxALIGN_RIGHT | wxALL, 10);

    welcome_panel->SetSizer(bSizer13);
    welcome_panel->Layout();
    bSizer13->Fit(welcome_panel);
    m_simplebook->AddPage(welcome_panel, ui_text("Страница"), false);
    move_bundle_panel = new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition,
                                    wxDefaultSize, wxTAB_TRAVERSAL);
    move_bundle_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer22;
    bSizer22 = new wxBoxSizer(wxVERTICAL);

    move_bundle_title = new wxStaticText(move_bundle_panel, wxID_ANY,
                                         ui_text("Переместите rEspanso в папку «Программы»"),
                                         wxDefaultPosition, wxDefaultSize, 0);
    move_bundle_title->Wrap(-1);
    move_bundle_title->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT,
                                      wxFONTSTYLE_NORMAL, wxFONTWEIGHT_BOLD,
                                      false, wxEmptyString));

    bSizer22->Add(move_bundle_title, 0, wxALIGN_CENTER_HORIZONTAL | wxTOP, 20);

    bSizer22->Add(0, 20, 0, 0, 5);

    move_bundle_description = new wxStaticText(
        move_bundle_panel, wxID_ANY,
        ui_text("rEspanso запущен не из системной папки «Программы», поэтому macOS может блокировать его корректную работу.\n\nПереместите rEspanso.app в папку /Applications и запустите приложение снова."),
        wxDefaultPosition, wxDefaultSize, 0);
    move_bundle_description->Wrap(500);
    bSizer22->Add(move_bundle_description, 0, wxALL, 10);

    bSizer22->Add(0, 20, 1, wxEXPAND, 5);

    move_bundle_quit_button =
        new wxButton(move_bundle_panel, wxID_ANY, ui_text("Закрыть"),
                     wxDefaultPosition, wxDefaultSize, 0);

    move_bundle_quit_button->SetDefault();
    bSizer22->Add(move_bundle_quit_button, 0, wxALIGN_RIGHT | wxALL, 10);

    move_bundle_panel->SetSizer(bSizer22);
    move_bundle_panel->Layout();
    bSizer22->Fit(move_bundle_panel);
    m_simplebook->AddPage(move_bundle_panel, ui_text("Страница"), false);
    legacy_version_panel =
        new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition, wxDefaultSize,
                    wxTAB_TRAVERSAL);
    legacy_version_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer21;
    bSizer21 = new wxBoxSizer(wxVERTICAL);

    legacy_version_title = new wxStaticText(
        legacy_version_panel, wxID_ANY, ui_text("Обнаружена старая версия rEspanso"),
        wxDefaultPosition, wxDefaultSize, 0);
    legacy_version_title->Wrap(-1);
    legacy_version_title->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT,
                                         wxFONTSTYLE_NORMAL, wxFONTWEIGHT_BOLD,
                                         false, wxEmptyString));

    bSizer21->Add(legacy_version_title, 0,
                  wxALIGN_CENTER_HORIZONTAL | wxALIGN_LEFT | wxTOP, 20);

    bSizer21->Add(0, 20, 0, 0, 5);

    legacy_version_description = new wxStaticText(
        legacy_version_panel, wxID_ANY,
        ui_text("Запущен процесс старой версии Espanso, который мешает работе rEspanso.\n\nЗавершите старый процесс и удалите прежнюю версию приложения. Если она уже удалена, перезагрузите компьютер, чтобы система обновила состояние.\n\nДополнительная информация:"),
        wxDefaultPosition, wxDefaultSize, 0);
    legacy_version_description->Wrap(500);
    bSizer21->Add(legacy_version_description, 0, wxLEFT | wxRIGHT | wxTOP, 10);

    legacy_version_docs_link = new wxHyperlinkCtrl(
        legacy_version_panel, wxID_ANY,
        wxT("https://espanso.org/legacy/uninstall"),
        wxT("https://espanso.org/legacy/uninstall"), wxDefaultPosition,
        wxDefaultSize, wxHL_DEFAULT_STYLE);
    bSizer21->Add(legacy_version_docs_link, 0, wxLEFT | wxRIGHT, 10);

    bSizer21->Add(0, 0, 1, wxEXPAND, 5);

    legacy_version_continue_button =
        new wxButton(legacy_version_panel, wxID_ANY, ui_text("Продолжить"),
                     wxDefaultPosition, wxDefaultSize, 0);

    legacy_version_continue_button->SetDefault();
    legacy_version_continue_button->Enable(false);

    bSizer21->Add(legacy_version_continue_button, 0, wxALIGN_RIGHT | wxALL, 10);

    legacy_version_panel->SetSizer(bSizer21);
    legacy_version_panel->Layout();
    bSizer21->Fit(legacy_version_panel);
    m_simplebook->AddPage(legacy_version_panel, ui_text("Страница"), false);
    wrong_edition_panel = new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition,
                                      wxDefaultSize, wxTAB_TRAVERSAL);
    wrong_edition_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer213;
    bSizer213 = new wxBoxSizer(wxVERTICAL);

    wrong_edition_title = new wxStaticText(wrong_edition_panel, wxID_ANY,
                                           ui_text("Обнаружена несовместимая сборка"),
                                           wxDefaultPosition, wxDefaultSize, 0);
    wrong_edition_title->Wrap(-1);
    wrong_edition_title->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT,
                                        wxFONTSTYLE_NORMAL, wxFONTWEIGHT_BOLD,
                                        false, wxEmptyString));

    bSizer213->Add(wrong_edition_title, 0,
                   wxALIGN_CENTER_HORIZONTAL | wxALIGN_LEFT | wxTOP, 20);

    bSizer213->Add(0, 20, 0, 0, 5);

    wrong_edition_description_x11 = new wxStaticText(
        wrong_edition_panel, wxID_ANY,
        ui_text("Установленная сборка rEspanso предназначена для X11, но текущий сеанс работает в Wayland.\n\nЭти варианты несовместимы. Переключитесь на сеанс X11 или установите сборку для Wayland.\n\nДополнительная информация:"),
        wxDefaultPosition, wxDefaultSize, 0);
    wrong_edition_description_x11->Wrap(500);
    bSizer213->Add(wrong_edition_description_x11, 0,
                   wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 10);

    wrong_edition_description_wayland = new wxStaticText(
        wrong_edition_panel, wxID_ANY,
        ui_text("Установленная сборка rEspanso предназначена для Wayland, но текущий сеанс работает в X11.\n\nЭти варианты несовместимы. Переключитесь на сеанс Wayland или установите сборку для X11.\n\nДополнительная информация:"),
        wxDefaultPosition, wxDefaultSize, 0);
    wrong_edition_description_wayland->Wrap(500);
    bSizer213->Add(wrong_edition_description_wayland, 0,
                   wxEXPAND | wxLEFT | wxTOP, 10);

    wrong_edition_link = new wxHyperlinkCtrl(
        wrong_edition_panel, wxID_ANY, wxT("https://espanso.org/install"),
        wxT("https://espanso.org/install"), wxDefaultPosition, wxDefaultSize,
        wxHL_DEFAULT_STYLE);
    bSizer213->Add(wrong_edition_link, 0, wxLEFT | wxRIGHT, 10);

    bSizer213->Add(0, 0, 1, wxEXPAND, 5);

    wrong_edition_button =
        new wxButton(wrong_edition_panel, wxID_ANY, ui_text("Закрыть rEspanso"),
                     wxDefaultPosition, wxDefaultSize, 0);

    wrong_edition_button->SetDefault();
    bSizer213->Add(wrong_edition_button, 0, wxALIGN_RIGHT | wxALL, 10);

    wrong_edition_panel->SetSizer(bSizer213);
    wrong_edition_panel->Layout();
    bSizer213->Fit(wrong_edition_panel);
    m_simplebook->AddPage(wrong_edition_panel, ui_text("Страница"), false);
    migrate_panel = new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition,
                                wxDefaultSize, wxTAB_TRAVERSAL);
    migrate_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer16;
    bSizer16 = new wxBoxSizer(wxVERTICAL);

    migrate_title =
        new wxStaticText(migrate_panel, wxID_ANY, ui_text("Перенос конфигурации"),
                         wxDefaultPosition, wxDefaultSize, 0);
    migrate_title->Wrap(-1);
    migrate_title->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT, wxFONTSTYLE_NORMAL,
                                  wxFONTWEIGHT_BOLD, false, wxEmptyString));

    bSizer16->Add(migrate_title, 0,
                  wxALIGN_CENTER_HORIZONTAL | wxALIGN_LEFT | wxTOP, 20);

    m_scrolledWindow4 = new wxScrolledWindow(
        migrate_panel, wxID_ANY, wxDefaultPosition, wxDefaultSize, wxVSCROLL);
    m_scrolledWindow4->SetScrollRate(5, 5);
    wxBoxSizer *bSizer211;
    bSizer211 = new wxBoxSizer(wxVERTICAL);

    bSizer211->Add(0, 20, 0, 0, 5);

    migrate_description = new wxStaticText(
        m_scrolledWindow4, wxID_ANY,
        ui_text("Новая версия использует обновлённый формат конфигурации, необходимый для новых возможностей rEspanso.\n\nВыберите вариант:\n\n• создать резервную копию старой конфигурации в папке «Документы» и автоматически перенести настройки — рекомендуется;\n• продолжить в режиме совместимости без изменения файлов.\n\nРежим совместимости поддерживает не все новые возможности. Перенос можно выполнить позже.\n\nДополнительная информация:"),
        wxDefaultPosition, wxDefaultSize, 0);
    migrate_description->Wrap(500);
    bSizer211->Add(migrate_description, 0, wxLEFT | wxRIGHT | wxTOP, 10);

    migrate_link = new wxHyperlinkCtrl(
        m_scrolledWindow4, wxID_ANY, wxT("https://espanso.org/migration"),
        wxT("https://espanso.org/migration"), wxDefaultPosition, wxDefaultSize,
        wxHL_DEFAULT_STYLE);
    bSizer211->Add(migrate_link, 0, wxLEFT | wxRIGHT, 10);

    m_scrolledWindow4->SetSizer(bSizer211);
    m_scrolledWindow4->Layout();
    bSizer211->Fit(m_scrolledWindow4);
    bSizer16->Add(m_scrolledWindow4, 1, wxEXPAND | wxALL, 5);

    wxBoxSizer *bSizer8;
    bSizer8 = new wxBoxSizer(wxHORIZONTAL);

    migrate_compatibility_mode_button =
        new wxButton(migrate_panel, wxID_ANY, ui_text("Режим совместимости"),
                     wxDefaultPosition, wxDefaultSize, 0);
    bSizer8->Add(migrate_compatibility_mode_button, 0, wxALL, 10);

    bSizer8->Add(0, 0, 1, wxEXPAND, 5);

    migrate_backup_and_migrate_button =
        new wxButton(migrate_panel, wxID_ANY, ui_text("Создать копию и перенести"),
                     wxDefaultPosition, wxDefaultSize, 0);

    migrate_backup_and_migrate_button->SetDefault();
    bSizer8->Add(migrate_backup_and_migrate_button, 0, wxALL, 10);

    bSizer16->Add(bSizer8, 0, wxEXPAND, 5);

    migrate_panel->SetSizer(bSizer16);
    migrate_panel->Layout();
    bSizer16->Fit(migrate_panel);
    m_simplebook->AddPage(migrate_panel, ui_text("Страница"), false);
    auto_start_panel = new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition,
                                   wxDefaultSize, wxTAB_TRAVERSAL);
    auto_start_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer18;
    bSizer18 = new wxBoxSizer(wxVERTICAL);

    m_scrolledWindow6 =
        new wxScrolledWindow(auto_start_panel, wxID_ANY, wxDefaultPosition,
                             wxDefaultSize, wxHSCROLL | wxVSCROLL);
    m_scrolledWindow6->SetScrollRate(5, 5);
    wxBoxSizer *bSizer2122;
    bSizer2122 = new wxBoxSizer(wxVERTICAL);

    auto_start_title = new wxStaticText(m_scrolledWindow6, wxID_ANY,
                                        ui_text("Автозапуск rEspanso"),
                                        wxDefaultPosition, wxDefaultSize, 0);
    auto_start_title->Wrap(-1);
    auto_start_title->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT,
                                     wxFONTSTYLE_NORMAL, wxFONTWEIGHT_BOLD,
                                     false, wxEmptyString));

    bSizer2122->Add(auto_start_title, 0,
                    wxALIGN_CENTER_HORIZONTAL | wxALIGN_LEFT | wxTOP, 20);

    bSizer2122->Add(0, 20, 0, 0, 5);

    auto_start_description =
        new wxStaticText(m_scrolledWindow6, wxID_ANY,
                         ui_text("rEspanso может автоматически запускаться после входа в систему.\n\nВключить автозапуск?"),
                         wxDefaultPosition, wxDefaultSize, 0);
    auto_start_description->Wrap(500);
    bSizer2122->Add(auto_start_description, 0, wxLEFT | wxRIGHT | wxTOP, 10);

    auto_start_checkbox = new wxCheckBox(
        m_scrolledWindow6, wxID_ANY,
        ui_text("Запускать rEspanso при входе в систему (рекомендуется)"),
        wxDefaultPosition, wxDefaultSize, 0);
    auto_start_checkbox->SetValue(true);
    bSizer2122->Add(auto_start_checkbox, 0, wxALL, 20);

    auto_start_note =
        new wxStaticText(m_scrolledWindow6, wxID_ANY,
                         ui_text("Автозапуск можно отключить позже в настройках."),
                         wxDefaultPosition, wxDefaultSize, 0);
    auto_start_note->Wrap(500);
    bSizer2122->Add(auto_start_note, 0, wxALL, 10);

    bSizer2122->Add(0, 0, 1, wxEXPAND, 5);

    m_scrolledWindow6->SetSizer(bSizer2122);
    m_scrolledWindow6->Layout();
    bSizer2122->Fit(m_scrolledWindow6);
    bSizer18->Add(m_scrolledWindow6, 1, wxEXPAND | wxALL, 5);

    auto_start_continue =
        new wxButton(auto_start_panel, wxID_ANY, ui_text("Продолжить"),
                     wxDefaultPosition, wxDefaultSize, 0);

    auto_start_continue->SetDefault();
    bSizer18->Add(auto_start_continue, 0, wxALIGN_RIGHT | wxALL, 10);

    auto_start_panel->SetSizer(bSizer18);
    auto_start_panel->Layout();
    bSizer18->Fit(auto_start_panel);
    m_simplebook->AddPage(auto_start_panel, ui_text("Страница"), false);
    add_path_panel = new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition,
                                 wxDefaultSize, wxTAB_TRAVERSAL);
    add_path_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer20;
    bSizer20 = new wxBoxSizer(wxVERTICAL);

    m_scrolledWindow8 =
        new wxScrolledWindow(add_path_panel, wxID_ANY, wxDefaultPosition,
                             wxDefaultSize, wxHSCROLL | wxVSCROLL);
    m_scrolledWindow8->SetScrollRate(5, 5);
    wxBoxSizer *bSizer212;
    bSizer212 = new wxBoxSizer(wxVERTICAL);

    add_path_title =
        new wxStaticText(m_scrolledWindow8, wxID_ANY, ui_text("Команда rEspanso в PATH"),
                         wxDefaultPosition, wxDefaultSize, 0);
    add_path_title->Wrap(-1);
    add_path_title->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT, wxFONTSTYLE_NORMAL,
                                   wxFONTWEIGHT_BOLD, false, wxEmptyString));

    bSizer212->Add(add_path_title, 0,
                   wxALIGN_CENTER_HORIZONTAL | wxALIGN_LEFT | wxTOP, 20);

    bSizer212->Add(0, 20, 0, 0, 5);

    add_path_description = new wxStaticText(
        m_scrolledWindow8, wxID_ANY,
        ui_text("rEspanso предоставляет командную строку для управления приложением и диагностики конфигурации.\n\nДобавление программы в переменную PATH позволит запускать команду espanso из терминала. Добавить rEspanso в PATH?"),
        wxDefaultPosition, wxDefaultSize, 0);
    add_path_description->Wrap(500);
    bSizer212->Add(add_path_description, 0, wxLEFT | wxRIGHT | wxTOP, 10);

    add_path_checkbox = new wxCheckBox(m_scrolledWindow8, wxID_ANY,
                                       ui_text("Добавить команду espanso в PATH"),
                                       wxDefaultPosition, wxDefaultSize, 0);
    add_path_checkbox->SetValue(true);
    bSizer212->Add(add_path_checkbox, 0, wxALL, 20);

    add_path_note = new wxStaticText(
        m_scrolledWindow8, wxID_ANY,
        ui_text("Если вы не используете терминал, этот параметр можно оставить включённым."),
        wxDefaultPosition, wxDefaultSize, 0);
    add_path_note->Wrap(500);
    bSizer212->Add(add_path_note, 0, wxALL, 10);

    bSizer212->Add(0, 0, 1, wxEXPAND, 5);

    m_scrolledWindow8->SetSizer(bSizer212);
    m_scrolledWindow8->Layout();
    bSizer212->Fit(m_scrolledWindow8);
    bSizer20->Add(m_scrolledWindow8, 1, wxEXPAND | wxALL, 5);

    add_path_continue_button =
        new wxButton(add_path_panel, wxID_ANY, ui_text("Продолжить"),
                     wxDefaultPosition, wxDefaultSize, 0);

    add_path_continue_button->SetDefault();
    bSizer20->Add(add_path_continue_button, 0, wxALIGN_RIGHT | wxALL, 10);

    add_path_panel->SetSizer(bSizer20);
    add_path_panel->Layout();
    bSizer20->Fit(add_path_panel);
    m_simplebook->AddPage(add_path_panel, ui_text("Страница"), false);
    accessibility_panel = new wxPanel(m_simplebook, wxID_ANY, wxDefaultPosition,
                                      wxDefaultSize, wxTAB_TRAVERSAL);
    accessibility_panel->SetBackgroundColour(
        wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW));

    wxBoxSizer *bSizer2121;
    bSizer2121 = new wxBoxSizer(wxVERTICAL);

    accessibility_title = new wxStaticText(accessibility_panel, wxID_ANY,
                                           ui_text("Разрешение универсального доступа"),
                                           wxDefaultPosition, wxDefaultSize, 0);
    accessibility_title->Wrap(-1);
    accessibility_title->SetFont(wxFont(18, wxFONTFAMILY_DEFAULT,
                                        wxFONTSTYLE_NORMAL, wxFONTWEIGHT_BOLD,
                                        false, wxEmptyString));

    bSizer2121->Add(accessibility_title, 0,
                    wxALIGN_CENTER_HORIZONTAL | wxALIGN_LEFT | wxTOP, 20);

    bSizer2121->Add(0, 20, 0, 0, 5);

    m_scrolledWindow1 =
        new wxScrolledWindow(accessibility_panel, wxID_ANY, wxDefaultPosition,
                             wxDefaultSize, wxVSCROLL);
    m_scrolledWindow1->SetScrollRate(5, 5);
    wxBoxSizer *bSizer81;
    bSizer81 = new wxBoxSizer(wxVERTICAL);

    accessibility_description = new wxStaticText(
        m_scrolledWindow1, wxID_ANY,
        ui_text("Для распознавания сокращений и вставки текста rEspanso требуется разрешение «Универсальный доступ» в macOS.\n\n1. Нажмите «Открыть настройки».\n2. В системном диалоге выберите «Открыть Системные настройки»."),
        wxDefaultPosition, wxDefaultSize, 0);
    accessibility_description->Wrap(500);
    bSizer81->Add(accessibility_description, 0, wxLEFT | wxRIGHT | wxTOP, 10);

    accessibility_image1 =
        new wxStaticBitmap(m_scrolledWindow1, wxID_ANY, wxNullBitmap,
                           wxDefaultPosition, wxDefaultSize, 0);
    bSizer81->Add(accessibility_image1, 0, wxALIGN_CENTER_HORIZONTAL | wxALL,
                  5);

    accessibility_description2 =
        new wxStaticText(m_scrolledWindow1, wxID_ANY,
                         ui_text("3. Откройте раздел «Конфиденциальность и безопасность» → «Универсальный доступ».\n4. Разрешите доступ для rEspanso, как показано на изображении."),
                         wxDefaultPosition, wxDefaultSize, 0);
    accessibility_description2->Wrap(500);
    bSizer81->Add(accessibility_description2, 0, wxALL, 10);

    accessibility_image2 =
        new wxStaticBitmap(m_scrolledWindow1, wxID_ANY, wxNullBitmap,
                           wxDefaultPosition, wxDefaultSize, 0);
    bSizer81->Add(accessibility_image2, 0, wxALIGN_CENTER_HORIZONTAL | wxALL,
                  5);

    m_scrolledWindow1->SetSizer(bSizer81);
    m_scrolledWindow1->Layout();
    bSizer81->Fit(m_scrolledWindow1);
    bSizer2121->Add(m_scrolledWindow1, 1, wxEXPAND | wxALL, 0);

    accessibility_enable_button =
        new wxButton(accessibility_panel, wxID_ANY, ui_text("Открыть настройки"),
                     wxDefaultPosition, wxDefaultSize, 0);

    accessibility_enable_button->SetDefault();
    bSizer2121->Add(accessibility_enable_button, 0, wxALIGN_RIGHT | wxALL, 10);

    accessibility_panel->SetSizer(bSizer2121);
    accessibility_panel->Layout();
    bSizer2121->Fit(accessibility_panel);
    m_simplebook->AddPage(accessibility_panel, ui_text("Страница"), false);

    bSizer1->Add(m_simplebook, 1, wxEXPAND | wxALL, 5);

    this->SetSizer(bSizer1);
    this->Layout();

    this->Centre(wxBOTH);

    // Connect Events
    this->Connect(wxID_ANY, wxEVT_TIMER,
                  wxTimerEventHandler(WizardFrame::check_timer_tick));
    m_simplebook->Connect(wxEVT_COMMAND_BOOKCTRL_PAGE_CHANGED,
                          wxBookCtrlEventHandler(WizardFrame::on_page_changed),
                          NULL, this);
    welcome_start_button->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::welcome_start_clicked), NULL, this);
    move_bundle_quit_button->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::move_bundle_quit_clicked), NULL,
        this);
    wrong_edition_button->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::quit_espanso_clicked), NULL, this);
    migrate_compatibility_mode_button->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::migrate_compatibility_mode_clicked),
        NULL, this);
    migrate_backup_and_migrate_button->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::migrate_button_clicked), NULL, this);
    auto_start_continue->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::auto_start_continue_clicked), NULL,
        this);
    add_path_continue_button->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::add_path_continue_clicked), NULL,
        this);
    accessibility_enable_button->Connect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::accessibility_enable_clicked), NULL,
        this);
}

WizardFrame::~WizardFrame() {
    // Disconnect Events
    this->Disconnect(wxID_ANY, wxEVT_TIMER,
                     wxTimerEventHandler(WizardFrame::check_timer_tick));
    m_simplebook->Disconnect(
        wxEVT_COMMAND_BOOKCTRL_PAGE_CHANGED,
        wxBookCtrlEventHandler(WizardFrame::on_page_changed), NULL, this);
    welcome_start_button->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::welcome_start_clicked), NULL, this);
    move_bundle_quit_button->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::move_bundle_quit_clicked), NULL,
        this);
    wrong_edition_button->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::quit_espanso_clicked), NULL, this);
    migrate_compatibility_mode_button->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::migrate_compatibility_mode_clicked),
        NULL, this);
    migrate_backup_and_migrate_button->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::migrate_button_clicked), NULL, this);
    auto_start_continue->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::auto_start_continue_clicked), NULL,
        this);
    add_path_continue_button->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::add_path_continue_clicked), NULL,
        this);
    accessibility_enable_button->Disconnect(
        wxEVT_COMMAND_BUTTON_CLICKED,
        wxCommandEventHandler(WizardFrame::accessibility_enable_clicked), NULL,
        this);
}
