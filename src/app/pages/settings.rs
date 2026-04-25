//! The settings page UI

use std::collections::HashMap;

use iced::widget::Slider;
use iced::widget::Space;
use iced::widget::TextInput;
use iced::widget::checkbox;
use iced::widget::radio;
use iced::widget::text_input;

use crate::app::Editable;
use crate::app::SetConfigBufferFields;
use crate::app::SetConfigThemeFields;
use crate::commands::Function;
use crate::config::MainPage;
use crate::config::Shelly;
use crate::styles::delete_button_style;
use crate::styles::settings_add_button_style;
use crate::styles::settings_checkbox_style;
use crate::styles::settings_radio_button_style;
use crate::styles::settings_save_button_style;
use crate::styles::settings_slider_style;
use crate::styles::settings_text_input_item_style;
use crate::{
    app::{SetConfigFields, pages::prelude::*},
    config::Config,
};

const SETTINGS_ITEM_PADDING: u16 = 5;
const SETTINGS_ITEM_HEIGHT: u32 = 80;
const SETTINGS_ITEM_COL_SPACING: u32 = 5;

pub fn settings_page(config: Config) -> Element<'static, Message> {
    let config = Box::new(config.clone());
    let theme = config.theme.clone();

    let hotkey_theme = theme.clone();
    let hotkey = settings_item_column([
        settings_hint_text(theme.clone(), "Toggle hotkey"),
        text_input("Toggle Hotkey", &config.toggle_hotkey)
            .on_input(|input| Message::SetConfig(SetConfigFields::ToggleHotkey(input.clone())))
            .on_submit(Message::WriteConfig(false))
            .width(Length::Fill)
            .style(move |_, _| settings_text_input_item_style(&hotkey_theme))
            .into(),
        notice_item(theme.clone(), "Use \"+\" as a seperator"),
    ]);

    let cb_theme = theme.clone();
    let cb_hotkey = settings_item_column([
        settings_hint_text(theme.clone(), "Clipboard hotkey"),
        text_input("Clipboard Hotkey", &config.clipboard_hotkey)
            .on_input(|input| Message::SetConfig(SetConfigFields::ClipboardHotkey(input.clone())))
            .on_submit(Message::WriteConfig(false))
            .width(Length::Fill)
            .style(move |_, _| settings_text_input_item_style(&cb_theme))
            .into(),
        notice_item(theme.clone(), "Use \"+\" as a seperator"),
    ]);

    let placeholder_theme = theme.clone();
    let placeholder_setting = settings_item_column([
        settings_hint_text(theme.clone(), "Set the rustcast placeholder"),
        text_input("Set Placeholder", &config.placeholder)
            .on_input(|input| Message::SetConfig(SetConfigFields::PlaceHolder(input.clone())))
            .on_submit(Message::WriteConfig(false))
            .width(Length::Fill)
            .style(move |_, _| settings_text_input_item_style(&placeholder_theme))
            .into(),
        notice_item(theme.clone(), "What the text box shows when its empty"),
    ]);

    let theme_clone = theme.clone();
    let search = settings_item_column([
        settings_hint_text(theme.clone(), "Set the search URL"),
        text_input("Set Search URL", &config.search_url)
            .on_input(|input| Message::SetConfig(SetConfigFields::SearchUrl(input.clone())))
            .on_submit(Message::WriteConfig(false))
            .width(Length::Fill)
            .style(move |_, _| settings_text_input_item_style(&theme_clone))
            .into(),
        notice_item(theme.clone(), "Which search engine to use (%s = query)"),
    ]);

    let theme_clone = theme.clone();
    let clipboard_history = Row::from_iter([
        settings_hint_text(theme.clone(), "Enable Clipboard history"),
        checkbox(config.clone().cbhist)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(|input| Message::SetConfig(SetConfigFields::ClipboardHistory(input)))
            .into(),
        notice_item(
            theme.clone(),
            "If you want your clipboard history to be stored",
        ),
    ])
    .align_y(Alignment::Center)
    .spacing(SETTINGS_ITEM_COL_SPACING * 2)
    .padding(SETTINGS_ITEM_PADDING)
    .height(SETTINGS_ITEM_HEIGHT);

    let theme_clone = theme.clone();
    let current_delay = config.debounce_delay;
    let debounce = settings_item_column([
        settings_hint_text(theme.clone(), "Set the debounce time"),
        text_input("Set Debounce time (ms)", &config.debounce_delay.to_string())
            .on_input(move |input: String| {
                let delay = input.parse::<u64>().unwrap_or(current_delay);
                Message::SetConfig(SetConfigFields::DebounceDelay(delay))
            })
            .on_submit(Message::WriteConfig(false))
            .width(Length::Fill)
            .style(move |_, _| settings_text_input_item_style(&theme_clone))
            .into(),
        notice_item(
            theme.clone(),
            "How quickly you want file searching to return a value",
        ),
    ]);

    let theme_clone = theme.clone();
    let start_at_login = settings_item_row([
        settings_hint_text(theme.clone(), "Start at login"),
        checkbox(config.clone().start_at_login)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(Message::ToggleAutoStartup)
            .into(),
        notice_item(theme.clone(), "If you want rustcast to start on login"),
    ]);

    let theme_clone = theme.clone();
    let haptic = Row::from_iter([
        settings_hint_text(theme.clone(), "Haptic feedback"),
        checkbox(config.clone().haptic_feedback)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(|input| Message::SetConfig(SetConfigFields::HapticFeedback(input)))
            .into(),
        notice_item(
            theme.clone(),
            "If there should be haptic feedback when you type",
        ),
    ])
    .align_y(Alignment::Center)
    .spacing(SETTINGS_ITEM_COL_SPACING * 2)
    .padding(SETTINGS_ITEM_PADDING)
    .height(SETTINGS_ITEM_HEIGHT);

    let theme_clone = theme.clone();
    let tray_icon = settings_item_row([
        settings_hint_text(theme.clone(), "Show menubar icon"),
        checkbox(config.clone().show_trayicon)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(|input| Message::SetConfig(SetConfigFields::ShowMenubarIcon(input)))
            .into(),
        notice_item(
            theme.clone(),
            "If the menubar icon should be shown in rustcast",
        ),
    ]);

    let theme_clone = theme.clone();
    let auto_suggest = settings_item_column([
        settings_hint_text(theme.clone(), "Suggestions on open"),
        settings_item_row([
            radio(
                "Favourites",
                MainPage::Favourites,
                Some(config.main_page),
                |page| Message::SetConfig(SetConfigFields::SetPage(page)),
            )
            .style({
                let theme_clone = theme_clone.clone();
                move |_, _| settings_radio_button_style(&theme_clone.clone())
            })
            .into(),
            radio(
                "Frequents",
                MainPage::FrequentlyUsed,
                Some(config.main_page),
                |page| Message::SetConfig(SetConfigFields::SetPage(page)),
            )
            .style({
                let theme_clone = theme_clone.clone();
                move |_, _| settings_radio_button_style(&theme_clone.clone())
            })
            .into(),
            radio("Events", MainPage::Events, Some(config.main_page), |page| {
                Message::SetConfig(SetConfigFields::SetPage(page))
            })
            .style({
                let theme_clone = theme_clone.clone();
                move |_, _| settings_radio_button_style(&theme_clone.clone())
            })
            .into(),
            radio("Nothing", MainPage::Blank, Some(config.main_page), |page| {
                Message::SetConfig(SetConfigFields::SetPage(page))
            })
            .style(move |_, _| settings_radio_button_style(&theme_clone.clone()))
            .into(),
        ])
        .spacing(30)
        .into(),
        notice_item(theme.clone(), "What an empty query should show"),
    ]);

    let theme_clone = theme.clone();
    let show_scrollbar = settings_item_row([
        settings_hint_text(theme.clone(), "Show scrollbar"),
        checkbox(config.theme.show_scroll_bar)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(|input| {
                Message::SetConfig(SetConfigFields::SetThemeFields(
                    SetConfigThemeFields::ShowScrollBar(input),
                ))
            })
            .into(),
        notice_item(theme.clone(), "If there should be a scrollbar"),
    ]);

    let theme_clone = theme.clone();
    let clear_on_hide = settings_item_row([
        settings_hint_text(theme.clone(), "Clear on hide"),
        checkbox(config.clone().buffer_rules.clear_on_hide)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(move |input| {
                Message::SetConfig(SetConfigFields::SetBufferFields(
                    SetConfigBufferFields::ClearOnHide(input),
                ))
            })
            .into(),
        notice_item(
            theme.clone(),
            "If the query should be cleared when rustcast is hidden",
        ),
    ]);

    let theme_clone = theme.clone();
    let clear_on_enter = settings_item_row([
        settings_hint_text(theme.clone(), "Clear on enter"),
        checkbox(config.clone().buffer_rules.clear_on_enter)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(move |input| {
                Message::SetConfig(SetConfigFields::SetBufferFields(
                    SetConfigBufferFields::ClearOnEnter(input),
                ))
            })
            .into(),
        notice_item(
            theme.clone(),
            "If the query should be cleared when an app is opened",
        ),
    ]);

    let theme_clone = theme.clone();
    let show_icons = settings_item_row([
        settings_hint_text(theme.clone(), "Show icons"),
        checkbox(config.clone().theme.show_icons)
            .style(move |_, _| settings_checkbox_style(&theme_clone))
            .on_toggle(move |input| {
                Message::SetConfig(SetConfigFields::SetThemeFields(
                    SetConfigThemeFields::ShowIcons(input),
                ))
            })
            .into(),
        notice_item(theme.clone(), "If you want app icons to be visible"),
    ]);

    let theme_clone = theme.clone();
    let font_family = settings_item_column([
        settings_hint_text(theme.clone(), "Set Font family"),
        text_input(
            "Font family",
            &config.theme.font.clone().unwrap_or("".to_string()),
        )
        .on_input(move |input: String| {
            Message::SetConfig(SetConfigFields::SetThemeFields(SetConfigThemeFields::Font(
                input,
            )))
        })
        .on_submit(Message::WriteConfig(false))
        .width(Length::Fill)
        .style(move |_, _| settings_text_input_item_style(&theme_clone))
        .into(),
        notice_item(theme.clone(), "What font rustcast should use"),
    ]);

    let theme_clone = theme.clone();
    let event_duration = settings_item_column([
        settings_hint_text(theme.clone(), "Set Event duration"),
        text_input("Event duration", &config.event_duration.to_string())
            .on_input(move |input: String| {
                Message::SetConfig(SetConfigFields::SetEventDuration(input))
            })
            .on_submit(Message::WriteConfig(false))
            .width(Length::Fill)
            .style(move |_, _| settings_text_input_item_style(&theme_clone))
            .into(),
        notice_item(
            theme.clone(),
            "How many minutes from now the events should be displayed",
        ),
    ]);

    let theme_clone = theme.clone();
    let theme_clone_1 = theme.clone();
    let theme_clone_2 = theme.clone();
    let theme_clone_3 = theme.clone();
    let text_clr = Column::from_iter([
        settings_hint_text(theme.clone(), "Set text colour"),
        Column::from_iter([
            settings_hint_text(
                theme.clone(),
                format!("R value: {}", theme_clone.text_color.0),
            ),
            Slider::new(
                0..=100,
                (theme_clone.text_color.0 * 100.) as i32,
                move |change| {
                    let txt_clr = theme_clone.text_color;
                    let change = change as f32 / 100.;
                    Message::SetConfig(SetConfigFields::SetThemeFields(
                        SetConfigThemeFields::TextColor(change, txt_clr.1, txt_clr.2),
                    ))
                },
            )
            .style(move |_, _| settings_slider_style(&theme_clone_1))
            .width((WINDOW_WIDTH / 5.) * 4.)
            .into(),
            settings_hint_text(
                theme.clone(),
                format!("G value: {}", theme_clone.text_color.1),
            ),
            Slider::new(
                0..=100,
                (theme_clone.text_color.1 * 100.) as i32,
                move |change| {
                    let txt_clr = theme_clone.text_color;
                    let change = change as f32 / 100.;
                    Message::SetConfig(SetConfigFields::SetThemeFields(
                        SetConfigThemeFields::TextColor(txt_clr.0, change, txt_clr.2),
                    ))
                },
            )
            .style(move |_, _| settings_slider_style(&theme_clone_2))
            .width((WINDOW_WIDTH / 5.) * 4.)
            .into(),
            settings_hint_text(
                theme.clone(),
                format!("B value: {}", theme_clone.text_color.2),
            ),
            Slider::new(
                0..=100,
                (theme_clone.text_color.2 * 100.) as i32,
                move |change| {
                    let txt_clr = theme_clone.text_color;
                    let change = change as f32 / 100.;
                    Message::SetConfig(SetConfigFields::SetThemeFields(
                        SetConfigThemeFields::TextColor(txt_clr.0, txt_clr.1, change),
                    ))
                },
            )
            .style(move |_, _| settings_slider_style(&theme_clone_3))
            .width((WINDOW_WIDTH / 5.) * 4.)
            .into(),
            notice_item(theme.clone(), "Text colour in RGB format"),
        ])
        .spacing(7)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into(),
    ]);

    let theme_clone = theme.clone();
    let theme_clone_1 = theme.clone();
    let theme_clone_2 = theme.clone();
    let theme_clone_3 = theme.clone();
    let bg_clr = Column::from_iter([
        settings_hint_text(theme.clone(), "Set background colour"),
        Column::from_iter([
            settings_hint_text(
                theme.clone(),
                format!("R value: {}", theme_clone.background_color.0),
            ),
            Slider::new(
                0..=100,
                (theme_clone.background_color.0 * 100.) as i32,
                move |change| {
                    let txt_clr = theme_clone.background_color;
                    let change = change as f32 / 100.;
                    Message::SetConfig(SetConfigFields::SetThemeFields(
                        SetConfigThemeFields::BackgroundColor(change, txt_clr.1, txt_clr.2),
                    ))
                },
            )
            .style(move |_, _| settings_slider_style(&theme_clone_1))
            .width((WINDOW_WIDTH / 5.) * 4.)
            .into(),
            settings_hint_text(
                theme.clone(),
                format!("G value: {}", theme_clone.background_color.1),
            ),
            Slider::new(
                0..=100,
                (theme_clone.background_color.1 * 100.) as i32,
                move |change| {
                    let txt_clr = theme_clone.background_color;
                    let change = change as f32 / 100.;
                    Message::SetConfig(SetConfigFields::SetThemeFields(
                        SetConfigThemeFields::BackgroundColor(txt_clr.0, change, txt_clr.2),
                    ))
                },
            )
            .style(move |_, _| settings_slider_style(&theme_clone_2))
            .width((WINDOW_WIDTH / 5.) * 4.)
            .into(),
            settings_hint_text(
                theme.clone(),
                format!("B value: {}", theme_clone.background_color.2),
            ),
            Slider::new(
                0..=100,
                (theme_clone.background_color.2 * 100.) as i32,
                move |change| {
                    let txt_clr = theme_clone.background_color;
                    let change = change as f32 / 100.;
                    Message::SetConfig(SetConfigFields::SetThemeFields(
                        SetConfigThemeFields::BackgroundColor(txt_clr.0, txt_clr.1, change),
                    ))
                },
            )
            .style(move |_, _| settings_slider_style(&theme_clone_3))
            .width((WINDOW_WIDTH / 5.) * 4.)
            .into(),
            notice_item(theme.clone(), "Background colour in RGB format"),
        ])
        .spacing(7)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into(),
    ]);

    let items = Column::from_iter([
        hotkey.into(),
        cb_hotkey.into(),
        placeholder_setting.into(),
        search.into(),
        debounce.into(),
        start_at_login.into(),
        haptic.into(),
        tray_icon.into(),
        clipboard_history.into(),
        auto_suggest.into(),
        show_scrollbar.into(),
        clear_on_hide.into(),
        clear_on_enter.into(),
        show_icons.into(),
        font_family.into(),
        event_duration.into(),
        text_clr.into(),
        bg_clr.into(),
        settings_hint_text(theme.clone(), "Aliases"),
        aliases_item(config.aliases.clone(), &theme),
        settings_hint_text(theme.clone(), "Modes"),
        modes_item(config.modes.clone(), &theme),
        settings_hint_text(theme.clone(), "Search Directories"),
        search_dirs_item(&theme, config.search_dirs.clone()),
        Space::new().height(30).into(),
        settings_hint_text(theme.clone(), "Shell commands"),
        shell_commands_item(config.shells.clone(), theme.clone()),
        Row::from_iter([
            savebutton(theme.clone()),
            default_button(theme.clone()),
            copy_config_button(config),
            wiki_button(theme.clone()),
        ])
        .spacing(5)
        .width(Length::Fill)
        .into(),
    ])
    .spacing(10);

    container(items)
        .style(move |_| result_row_container_style(&theme, false))
        .height(Length::Fill)
        .width(Length::Fill)
        .padding(10)
        .align_x(Alignment::Center)
        .into()
}

fn savebutton(theme: Theme) -> Element<'static, Message> {
    Button::new(
        Text::new("Save")
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .font(theme.font()),
    )
    .style(move |_, _| settings_save_button_style(&theme))
    .width(Length::Fill)
    .on_press(Message::WriteConfig(true))
    .into()
}

fn default_button(theme: Theme) -> Element<'static, Message> {
    Button::new(
        Text::new("To default")
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .font(theme.font()),
    )
    .style(move |_, _| settings_save_button_style(&theme))
    .width(Length::Fill)
    .on_press(Message::SetConfig(SetConfigFields::ToDefault))
    .into()
}

fn wiki_button(theme: Theme) -> Element<'static, Message> {
    Button::new(
        Text::new("Open file")
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .font(theme.font()),
    )
    .style(move |_, _| settings_save_button_style(&theme))
    .width(Length::Fill)
    .on_press(Message::RunFunction(crate::commands::Function::OpenApp(
        std::env::var("HOME").unwrap_or("".to_string()) + "/.config/rustcast/config.toml",
    )))
    .into()
}

fn copy_config_button(config: Box<Config>) -> Element<'static, Message> {
    let theme = config.theme.clone();
    Button::new(
        Text::new("Copy config")
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .font(theme.font()),
    )
    .style(move |_, _| settings_save_button_style(&theme))
    .width(Length::Fill)
    .on_press(Message::RunFunction(Function::CopyToClipboard(
        crate::clipboard::ClipBoardContentType::Text(
            toml::to_string(&config).unwrap_or("".to_string()),
        ),
    )))
    .into()
}

fn settings_hint_text(theme: Theme, text: impl ToString) -> Element<'static, Message> {
    let text = text.to_string();

    Text::new(text)
        .font(theme.font())
        .color(theme.text_color(0.7))
        .into()
}

fn settings_item_column(
    elems: impl IntoIterator<Item = Element<'static, Message>>,
) -> Column<'static, Message> {
    Column::from_iter(elems)
        .spacing(SETTINGS_ITEM_COL_SPACING)
        .padding(SETTINGS_ITEM_PADDING)
        .height(SETTINGS_ITEM_HEIGHT)
}

fn settings_item_row(
    elems: impl IntoIterator<Item = Element<'static, Message>>,
) -> Row<'static, Message> {
    Row::from_iter(elems)
        .align_y(Alignment::Center)
        .spacing(SETTINGS_ITEM_COL_SPACING)
        .padding(SETTINGS_ITEM_PADDING)
        .height(SETTINGS_ITEM_HEIGHT)
}

fn notice_item(theme: Theme, notice: impl ToString) -> Element<'static, Message> {
    Text::new(notice.to_string())
        .font(theme.font())
        .color(theme.text_color(0.7))
        .size(10)
        .width(Length::Fill)
        .align_x(Alignment::End)
        .into()
}

fn aliases_item(aliases: HashMap<String, String>, theme: &Theme) -> Element<'static, Message> {
    let theme_clone = theme.clone();
    let mut aliases = aliases
        .iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect::<Vec<(String, String)>>();
    aliases.sort_by_key(|x| x.0.len());
    Column::from_iter([
        container(
            Column::from_iter(aliases.iter().map(|(key, value)| {
                let key_clone = key.clone();
                let val_clone = value.clone();
                let key_clone_2 = key.clone();
                let val_clone_2 = value.clone();
                let theme_clone_2 = theme.clone();
                Row::from_iter([
                    text_input_cell(key.to_owned(), &theme_clone, "Shorthand")
                        .on_input(move |input| {
                            Message::SetConfig(SetConfigFields::Aliases(Editable::Update {
                                old: (key_clone.clone(), val_clone.clone()),
                                new: (input.clone(), val_clone.clone()),
                            }))
                        })
                        .into(),
                    text_input_cell(value.to_owned(), &theme_clone, "Term")
                        .on_input(move |input| {
                            Message::SetConfig(SetConfigFields::Aliases(Editable::Update {
                                old: (key_clone_2.clone(), val_clone_2.clone()),
                                new: (key_clone_2.clone(), input.clone()),
                            }))
                        })
                        .into(),
                    Button::new("Delete")
                        .on_press(Message::SetConfig(SetConfigFields::Aliases(
                            Editable::Delete((key.clone(), value.clone())),
                        )))
                        .style(move |_, _| delete_button_style(&theme_clone_2))
                        .into(),
                ])
                .spacing(10)
                .into()
            }))
            .spacing(10),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .into(),
        Button::new(
            Text::new("+")
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .style(move |_, _| settings_add_button_style(&theme_clone.clone()))
        .on_press(Message::SetConfig(SetConfigFields::Aliases(
            Editable::Create((String::new(), String::new())),
        )))
        .into(),
    ])
    .spacing(10)
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}

fn search_dirs_item(theme: &Theme, search_dirs: Vec<String>) -> Element<'static, Message> {
    let theme_clone = theme.clone();
    let search_dirs = search_dirs.clone();
    Column::from_iter([
        container(
            Column::from_iter(search_dirs.iter().map(|dir| {
                let theme_clone_2 = theme.clone();
                let directory = dir.clone();
                container(
                    Row::from_iter([
                        dir_picker_button(directory, dir, theme_clone.clone()).into(),
                        Button::new("Delete")
                            .on_press(Message::SetConfig(SetConfigFields::SearchDirs(
                                Editable::Delete(dir.clone()),
                            )))
                            .style(move |_, _| delete_button_style(&theme_clone_2))
                            .into(),
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .into()
            }))
            .spacing(10),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into(),
        dir_adder_button("+", theme.to_owned()).into(),
    ])
    .spacing(10)
    .height(Length::Fill)
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}

fn text_input_cell(text: String, theme: &Theme, placeholder: &str) -> TextInput<'static, Message> {
    text_input(placeholder, &text)
        .font(theme.font())
        .padding(5)
        .on_submit(Message::WriteConfig(false))
}

fn modes_item(modes: HashMap<String, String>, theme: &Theme) -> Element<'static, Message> {
    let theme_clone = theme.clone();
    let mut modes = modes
        .iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect::<Vec<(String, String)>>();
    modes.sort_by_key(|x| x.0.len());
    Column::from_iter([
        container(
            Column::from_iter(modes.iter().map(|(key, value)| {
                let theme_clone_1 = theme_clone.clone();
                let display_val = if value.is_empty() {
                    "Pick a file".to_string()
                } else {
                    value.replace(&std::env::var("HOME").unwrap_or("".to_string()), "~")
                };
                let key_clone = key.clone();
                let val_clone = value.clone();
                let theme_clone_2 = theme.clone();
                Row::from_iter([
                    text_input_cell(key.to_owned(), &theme_clone, "Mode name")
                        .on_input(move |input| {
                            Message::SetConfig(SetConfigFields::Modes(Editable::Update {
                                old: (key_clone.clone(), val_clone.clone()),
                                new: (input.clone(), val_clone.clone()),
                            }))
                        })
                        .into(),
                    Button::new(Text::new(display_val))
                        .on_press(Message::OpenFileDialogue(key.to_owned()))
                        .style(move |_, _| settings_add_button_style(&theme_clone_1.clone()))
                        .into(),
                    Button::new("Delete")
                        .on_press(Message::SetConfig(SetConfigFields::Modes(
                            Editable::Delete((key.clone(), value.clone())),
                        )))
                        .style(move |_, _| delete_button_style(&theme_clone_2))
                        .into(),
                ])
                .spacing(10)
                .into()
            }))
            .spacing(10),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .into(),
        Button::new(
            Text::new("+")
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .on_press(Message::SetConfig(SetConfigFields::Modes(
            Editable::Create((String::new(), String::new())),
        )))
        .style(move |_, _| settings_add_button_style(&theme_clone.clone()))
        .into(),
    ])
    .spacing(10)
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}

fn dir_picker_button(directory: String, dir: &str, theme: Theme) -> Button<'static, Message> {
    let home = std::env::var("HOME").unwrap_or("/".to_string());
    Button::new(Text::new(dir.to_owned().replace(&home, "~")))
        .on_press_with(move || {
            rfd::FileDialog::new()
                .set_directory(home.clone())
                .set_can_create_directories(false)
                .pick_folder()
                .map(|path| {
                    let new = path.to_str().unwrap_or("").to_string();
                    Message::SetConfig(SetConfigFields::SearchDirs(Editable::Update {
                        old: directory.clone(),
                        new,
                    }))
                })
                .unwrap_or(Message::SetConfig(SetConfigFields::SearchDirs(
                    Editable::Update {
                        old: directory.clone(),
                        new: directory.clone(),
                    },
                )))
        })
        .style(move |_, _| settings_add_button_style(&theme.clone()))
}

fn dir_adder_button(dir: &str, theme: Theme) -> Button<'static, Message> {
    Button::new(Text::new(dir.to_owned()))
        .on_press_with(move || {
            rfd::FileDialog::new()
                .set_directory(std::env::var("HOME").unwrap_or("/".to_string()))
                .set_can_create_directories(false)
                .pick_folder()
                .map(|path| {
                    let new = path.to_str().unwrap_or("").to_string();
                    Message::SetConfig(SetConfigFields::SearchDirs(Editable::Create(new)))
                })
                .unwrap_or(Message::SetConfig(SetConfigFields::SearchDirs(
                    Editable::Create(String::new()),
                )))
        })
        .style(move |_, _| settings_add_button_style(&theme.clone()))
}

fn shell_commands_item(shells: Vec<Shelly>, theme: Theme) -> Element<'static, Message> {
    let mut col =
        Column::from_iter(shells.iter().map(|x| x.editable_render(theme.clone()))).spacing(30);

    let theme_clone = theme.clone();

    col = col
        .push(
            Button::new(
                Text::new("+")
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
            .style(move |_, _| settings_add_button_style(&theme_clone.clone()))
            .on_press(Message::SetConfig(SetConfigFields::ShellCommands(
                Editable::Create(Shelly::default()),
            ))),
        )
        .width(Length::Fill)
        .align_x(Alignment::Center);

    col.into()
}

impl Shelly {
    pub fn editable_render(&self, theme: Theme) -> Element<'static, Message> {
        let shell = self.to_owned();
        Column::from_iter([
            tuple_row(
                shellcommand_hint_text(theme.clone(), "Display name"),
                text_input_cell(self.alias.clone(), &theme, "Display Name")
                    .on_input({
                        let shell = shell.clone();
                        move |input| {
                            let old = shell.clone();
                            let mut new = old.clone();
                            new.alias = input;
                            Message::SetConfig(SetConfigFields::ShellCommands(Editable::Update {
                                old,
                                new,
                            }))
                        }
                    })
                    .into(),
            )
            .into(),
            tuple_row(
                shellcommand_hint_text(theme.clone(), "Search name"),
                text_input_cell(self.alias_lc.clone(), &theme, "Search Name")
                    .on_input({
                        let shell = shell.clone();
                        move |input| {
                            let old = shell.clone();
                            let mut new = old.clone();
                            new.alias_lc = input;
                            Message::SetConfig(SetConfigFields::ShellCommands(Editable::Update {
                                old,
                                new,
                            }))
                        }
                    })
                    .into(),
            )
            .into(),
            tuple_row(
                shellcommand_hint_text(theme.clone(), "Command"),
                text_input_cell(self.command.clone(), &theme, "Command")
                    .on_input({
                        let shell = shell.clone();
                        move |input| {
                            let old = shell.clone();
                            let mut new = old.clone();
                            new.command = input;
                            Message::SetConfig(SetConfigFields::ShellCommands(Editable::Update {
                                old,
                                new,
                            }))
                        }
                    })
                    .into(),
            )
            .into(),
            tuple_row(
                shellcommand_hint_text(theme.clone(), "Icon File"),
                text_input_cell(
                    self.icon_path.clone().unwrap_or("".to_string()),
                    &theme,
                    "Icon path",
                )
                .on_input({
                    let shell = shell.clone();
                    move |input| {
                        let old = shell.clone();
                        let mut new = old.clone();
                        new.icon_path = if input.is_empty() { None } else { Some(input) };
                        Message::SetConfig(SetConfigFields::ShellCommands(Editable::Update {
                            old,
                            new,
                        }))
                    }
                })
                .into(),
            )
            .into(),
            tuple_row(
                shellcommand_hint_text(theme.clone(), "Hotkey"),
                text_input_cell(
                    self.hotkey.clone().unwrap_or("".to_string()),
                    &theme,
                    "Hotkey",
                )
                .on_input({
                    let shell = shell.clone();
                    move |input| {
                        let old = shell.clone();
                        let mut new = old.clone();
                        new.hotkey = Some(input);
                        Message::SetConfig(SetConfigFields::ShellCommands(Editable::Update {
                            old,
                            new,
                        }))
                    }
                })
                .into(),
            )
            .into(),
            tuple_row(
                Button::new("Delete")
                    .on_press(Message::SetConfig(SetConfigFields::ShellCommands(
                        Editable::Delete(self.clone()),
                    )))
                    .style({
                        let theme = theme.clone();
                        move |_, _| delete_button_style(&theme)
                    })
                    .into(),
                notice_item(theme.clone(), "Icon path and hotkey are optional"),
            )
            .into(),
        ])
        .spacing(10)
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
    }
}

fn tuple_row(
    left: Element<'static, Message>,
    right: Element<'static, Message>,
) -> Row<'static, Message> {
    Row::from_iter([left, right])
        .spacing(10)
        .width(Length::Fill)
}

fn shellcommand_hint_text(theme: Theme, text: impl ToString) -> Element<'static, Message> {
    let text = text.to_string();

    Text::new(text)
        .font(theme.font())
        .color(theme.text_color(0.7))
        .width(WINDOW_WIDTH * 0.3)
        .into()
}
