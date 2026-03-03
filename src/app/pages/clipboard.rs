use iced::widget::{
    Scrollable, scrollable,
    scrollable::{Direction, Scrollbar},
};

use crate::{app::pages::prelude::*, clipboard::ClipBoardContentType};

pub fn clipboard_view(
    clipboard_content: Vec<ClipBoardContentType>,
    focussed_id: u32,
    theme: Theme,
    focus_id: u32,
) -> Element<'static, Message> {
    let theme_clone = theme.clone();
    let theme_clone_2 = theme.clone();
    container(Row::from_vec(vec![
        container(
            scrollable(
                Column::from_iter(clipboard_content.iter().enumerate().map(|(i, content)| {
                    content.to_app().render(theme.clone(), i as u32, focus_id)
                }))
                .width(WINDOW_WIDTH / 3.),
            )
            .id("results"),
        )
        .height(10000)
        .style(move |_| result_row_container_style(&theme_clone_2, false))
        .into(),
        container(Scrollable::with_direction(
            Text::new(
                clipboard_content
                    .get(focussed_id as usize)
                    .map(|x| x.to_app().search_name)
                    .unwrap_or("".to_string()),
            )
            .height(Length::Fill)
            .width(Length::Fill)
            .align_x(Alignment::Start)
            .font(theme.font())
            .size(16),
            Direction::Both {
                vertical: Scrollbar::new().scroller_width(0.).width(0.),
                horizontal: Scrollbar::new().scroller_width(0.).width(0.),
            },
        ))
        .padding(10)
        .style(move |_| result_row_container_style(&theme_clone, false))
        .width((WINDOW_WIDTH / 3.) * 2.)
        .into(),
    ]))
    .height(280)
    .into()
}
