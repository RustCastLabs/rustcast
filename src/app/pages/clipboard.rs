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
    styles::{delete_button_style, settings_text_input_item_style},
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

    let viewport_content: Element<'static, Message> =
        match clipboard_content.get(focussed_id as usize) {
            Some(content) => viewport_content(content, &theme),
            None => Text::new("").into(),
        };
    container(Row::from_iter([
        container(
            Scrollable::with_direction(
                Column::from_iter(clipboard_content.iter().enumerate().map(|(i, content)| {
                    content
                        .to_app()
                        .render(theme.clone(), i as u32, focussed_id, None)
                }))
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
            let bytes = data.to_owned_img().into_owned_bytes();
            container(
                Viewer::new(
                    Handle::from_rgba(data.width as u32, data.height as u32, bytes.to_vec())
                        .clone(),
                )
                .content_fit(ContentFit::ScaleDown)
                .scale_step(0.)
                .max_scale(1.)
                .min_scale(1.),
            )
            .padding(10)
            .style(|_| container::Style {
                border: iced::Border {
                    color: iced::Color::WHITE,
                    width: 1.,
                    radius: Radius::new(0.),
                },
                ..Default::default()
            })
            .width(Length::Fill)
            .into()
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
