//! The elements for the clipboard history page
use iced::{
    ContentFit,
    border::Radius,
    widget::{
        Scrollable,
        image::{Handle, Viewer},
        scrollable::{Direction, Scrollbar},
        text::Wrapping,
        text_input,
    },
};

use crate::{
    app::{Editable, ToApp, pages::prelude::*},
    clipboard::ClipBoardContentType,
    styles::{delete_button_style, settings_text_input_item_style, clipboard_image_border_style},
};

/// The clipboard view
///
/// Takes:
/// - the clipboard content to render,
/// - the id of which element is focussed,
/// - and the [`Theme`]
///
/// Returns:
/// - the iced Element to render
pub fn clipboard_view(
    clipboard_content: Vec<ClipBoardContentType>,
    focussed_id: u32,
    theme: Theme,
    rankings: &std::collections::HashMap<String, i32>,
    search_query: &str,
) -> Element<'static, Message> {
    let theme_clone = theme.clone();
    let theme_clone_2 = theme.clone();
    if clipboard_content.is_empty() {
        return container(
            Text::new("Copy something to use the clipboard history")
                .font(theme.font())
                .size(30)
                .center()
                .wrapping(Wrapping::WordOrGlyph),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .style(move |_| result_row_container_style(&theme_clone, false))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into();
    }

    let mut apps: Vec<(crate::app::apps::App, ClipBoardContentType)> = clipboard_content
        .into_iter()
        .filter_map(|c| {
            let mut app = c.to_app();
            if !search_query.is_empty() 
                && !app.search_name.to_lowercase().contains(search_query) 
                && !app.display_name.to_lowercase().contains(search_query) {
                return None;
            }
            if let Some(r) = rankings.get(&app.search_name) {
                app.ranking = *r;
            }
            Some((app, c))
        })
        .collect();

    apps.sort_by(|a, b| {
        let rank_a = if a.0.ranking == -1 { 0 } else { 1 };
        let rank_b = if b.0.ranking == -1 { 0 } else { 1 };
        rank_a.cmp(&rank_b)
    });

    let mut elements: Vec<Element<'static, Message>> = Vec::new();
    let mut has_pinned = false;
    let mut has_copied = false;

    let apps_len = apps.len();
    for (i, (app, _)) in apps.iter().enumerate() {
        if app.ranking == -1 && !has_pinned {
            elements.push(
                container(
                    Text::new("Pinned")
                        .font(iced::Font { weight: iced::font::Weight::Bold, ..theme.font() })
                        .size(12)
                        .style(|_theme| iced::widget::text::Style { color: Some(iced::Color::from_rgb8(150, 150, 150)) })
                )
                .padding([5, 10])
                .into()
            );
            has_pinned = true;
        } else if app.ranking != -1 && !has_copied {
            if has_pinned {
                elements.push(Text::new("").size(10).into());
            }
            elements.push(
                container(
                    Text::new("Copied")
                        .font(iced::Font { weight: iced::font::Weight::Bold, ..theme.font() })
                        .size(12)
                        .style(|_theme| iced::widget::text::Style { color: Some(iced::Color::from_rgb8(150, 150, 150)) })
                )
                .padding([5, 10])
                .into()
            );
            has_copied = true;
        }
        elements.push(app.clone().render(theme.clone(), i as u32, focussed_id, None));
    }

    let viewport_content: Element<'static, Message> =
        if focussed_id < apps_len as u32 {
            let (_, content) = &apps[focussed_id as usize];
            viewport_content(content, &theme)
        } else {
            Text::new("").into()
        };

    container(Row::from_iter([
        container(
            Scrollable::with_direction(
                Column::from_iter(elements)
                .width(WINDOW_WIDTH / 3.),
                Direction::Vertical(Scrollbar::hidden()),
            )
            .id("results"),
        )
        .height(10000)
        .style(move |_| result_row_container_style(&theme_clone_2, false))
        .into(),
        container(viewport_content)
            .height(10000)
            .padding(10)
            .style(move |_| result_row_container_style(&theme_clone, false))
            .width((WINDOW_WIDTH / 3.) * 2.)
            .into(),
    ]))
    .height(280)
    .into()
}

fn viewport_content(content: &ClipBoardContentType, theme: &Theme) -> Element<'static, Message> {
    let viewer: Element<'static, Message> = match content {
        ClipBoardContentType::Text(txt) => Scrollable::with_direction(
            container(
                Text::new(txt.to_owned())
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .align_x(Alignment::Start)
                    .font(theme.font())
                    .size(16),
            )
            .width(Length::Fill)
            .height(Length::Fill),
            Direction::Both {
                vertical: Scrollbar::hidden(),
                horizontal: Scrollbar::hidden(),
            },
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .into(),

        ClipBoardContentType::Image(data) => {
            let bytes = data.bytes.to_vec();
            container(
                Viewer::new(
                    Handle::from_rgba(data.width as u32, data.height as u32, bytes)
                        .clone(),
                )
                .content_fit(ContentFit::ScaleDown)
                .scale_step(0.)
                .max_scale(1.)
                .min_scale(1.),
            )
            .padding(10)
            .style(|_| clipboard_image_border_style())
            .width(Length::Fill)
            .into()
        }
        ClipBoardContentType::Files(files, img_opt) => {
            let is_single_image = files.len() == 1 && {
                let p = std::path::Path::new(&files[0]);
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    matches!(
                        ext.to_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "tiff"
                    )
                } else {
                    false
                }
            };

            if is_single_image {
                container(
                    Viewer::new(Handle::from_path(&files[0]))
                        .content_fit(ContentFit::ScaleDown)
                        .scale_step(0.)
                        .max_scale(1.)
                        .min_scale(1.),
                )
                .padding(10)
                .style(|_| clipboard_image_border_style())
                .width(Length::Fill)
                .into()
            } else {
                let display_text = if files.len() > 1 {
                    let mut s = format!("{} Files Copied", files.len());
                    for f in files.iter().take(3) {
                        let fname = std::path::Path::new(f).file_name().unwrap_or_default().to_string_lossy();
                        s.push_str(&format!("\n• {}", fname));
                    }
                    if files.len() > 3 {
                        s.push_str(&format!("\n...and {} more", files.len() - 3));
                    }
                    s
                } else {
                    let fname = std::path::Path::new(&files[0]).file_name().unwrap_or_default().to_string_lossy();
                    format!("File: {}", fname)
                };

                let text_elem = Scrollable::with_direction(
                        container(
                            Text::new(display_text)
                                .height(Length::Fill)
                                .width(Length::Fill)
                                .align_x(Alignment::Start)
                                .font(theme.font())
                                .size(16),
                        )
                        .width(Length::Fill)
                        .height(Length::Fill),
                        Direction::Both {
                            vertical: Scrollbar::hidden(),
                            horizontal: Scrollbar::hidden(),
                        },
                    )
                    .height(Length::Fill)
                    .width(Length::Fill);

                if let Some(data) = img_opt {
                    let bytes = data.bytes.to_vec();
                    let image_elem = container(
                        Viewer::new(
                            Handle::from_rgba(data.width as u32, data.height as u32, bytes)
                                .clone(),
                        )
                        .content_fit(ContentFit::ScaleDown)
                        .scale_step(0.)
                        .max_scale(1.)
                        .min_scale(1.),
                    )
                    .padding(10)
                    .style(|_| clipboard_image_border_style())
                    .width(Length::Fill)
                    .height(Length::Fixed(220.0));

                    Column::new().push(image_elem).push(text_elem).into()
                } else {
                    text_elem.into()
                }
            }
        }
    };

    let theme_clone = theme.clone();
    let theme_clone_2 = theme.clone();
    Column::from_iter([
        viewer,
        container(
            Row::from_iter([
                Button::new("Delete")
                    .on_press(Message::EditClipboardHistory(Editable::Delete(
                        content.to_owned(),
                    )))
                    .style(move |_, _| delete_button_style(&theme_clone))
                    .into(),
                Button::new("Clear")
                    .on_press(Message::ClearClipboardHistory)
                    .style(move |_, _| delete_button_style(&theme_clone_2))
                    .into(),
            ])
            .spacing(10),
        )
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(10)
        .into(),
    ])
    .into()
}

#[allow(unused)]
fn editable_text(text: &str, theme: &Theme) -> Element<'static, Message> {
    let text_string = text.to_string();
    let theme_clone = theme.clone();
    container(
        text_input("Edit clipboard history text", text)
            .on_input(move |input| {
                Message::EditClipboardHistory(Editable::Update {
                    old: ClipBoardContentType::Text(text_string.clone()),
                    new: ClipBoardContentType::Text(input),
                })
            })
            .align_x(Alignment::Start)
            .size(16)
            .width(Length::Fill)
            .style(move |_, _| settings_text_input_item_style(&theme_clone))
            .font(theme.font()),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}
