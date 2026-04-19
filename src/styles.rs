//! This handles most of the styling for the rustcast elements
use crate::config::Theme as ConfigTheme;
use iced::Shadow;
use iced::border::Radius;
use iced::widget::{button, checkbox, container, radio, scrollable, slider};
use iced::{Background, Border, Color, widget::text_input};

/// Helper: mix base color with white (simple “tint”)
pub fn tint(mut c: Color, amount: f32) -> Color {
    c.r = c.r + (1.0 - c.r) * amount;
    c.g = c.g + (1.0 - c.g) * amount;
    c.b = c.b + (1.0 - c.b) * amount;
    c
}

/// Helper: apply alpha
pub fn with_alpha(mut c: Color, a: f32) -> Color {
    c.a = a;
    c
}

/// Styling for the main text box
pub fn rustcast_text_input_style(theme: &ConfigTheme) -> text_input::Style {
    let base = theme.bg_color();
    let focused = false; // if you have state, pass it in and use it
    let surface = glass_surface(base, focused);
    text_input::Style {
        background: Background::Color(surface),
        border: Border {
            color: glass_border(theme.text_color(0.), focused),
            width: 0.,
            radius: Radius::new(10.).bottom(0.),
        },
        icon: theme.text_color(0.),
        placeholder: theme.text_color(0.2),
        value: theme.text_color(0.9),
        selection: theme.text_color(0.2),
    }
}

/// Container styling for all the elements in the rustcast window
pub fn contents_style(theme: &ConfigTheme) -> container::Style {
    container::Style {
        background: None,
        text_color: None,
        border: iced::Border {
            color: theme.text_color(0.9),
            width: 0.4,
            radius: Radius::new(14.0),
        },
        ..Default::default()
    }
}

pub fn delete_button_style(theme: &ConfigTheme) -> button::Style {
    let red_clr = Color::from_rgb(1.0, 0.2, 0.2);
    button::Style {
        text_color: red_clr,
        background: Some(Background::Color(theme.bg_color())),
        border: Border {
            color: with_alpha(red_clr, 0.3),
            width: 0.5,
            radius: Radius::new(15),
        },
        ..Default::default()
    }
}

/// Styling for each of the buttons that are what the "results" of rustcast are
pub fn result_button_style(theme: &ConfigTheme) -> button::Style {
    button::Style {
        text_color: theme.text_color(1.),
        background: Some(Background::Color(theme.bg_color())),
        ..Default::default()
    }
}

pub fn favourite_button_style(
    theme: &ConfigTheme,
    status: button::Status,
    is_favourite: bool,
) -> button::Style {
    let (base, pressed, hovered) = if is_favourite {
        (1.0, 0.8, 0.9)
    } else {
        (0.1, 1.0, 0.5)
    };

    let text_color = match status {
        button::Status::Pressed => theme.text_color(pressed),
        button::Status::Hovered => theme.text_color(hovered),
        _ => theme.text_color(base),
    };

    button::Style {
        text_color,
        background: Some(Background::Color(theme.bg_color())),
        ..Default::default()
    }
}

pub fn results_scrollbar_style(tile: &ConfigTheme) -> scrollable::Style {
    let clr = with_alpha(tile.bg_color(), 0.7);

    scrollable::Style {
        container: container::Style {
            text_color: None,
            background: None,
            border: Border::default(),
            shadow: Shadow::default(),
            snap: false,
        },
        vertical_rail: scrollable::Rail {
            background: None,
            border: Border {
                color: clr,
                width: 1.,
                radius: Radius::new(10),
            },
            scroller: scrollable::Scroller {
                background: Background::Color(tile.text_color(0.7)),
                border: Border {
                    color: tile.text_color(1.),
                    width: 0.1,
                    radius: Radius::new(0),
                },
            },
        },
        horizontal_rail: scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
            },
        },
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

pub fn settings_radio_button_style(theme: &ConfigTheme) -> radio::Style {
    radio::Style {
        background: Background::Color(Color::TRANSPARENT),
        dot_color: theme.text_color(0.4),
        border_width: 1.,
        border_color: theme.text_color(0.7),
        text_color: Some(theme.text_color(1.)),
    }
}

/// Each rustcast results rows style
pub fn result_row_container_style(tile: &ConfigTheme, focused: bool) -> container::Style {
    container::Style {
        background: Some(Background::Color(glass_surface(tile.bg_color(), focused))),
        border: Border {
            color: glass_border(tile.text_color(1.), focused),
            width: 0.,
            radius: Radius::new(0.0),
        },
        text_color: Some(tile.text_color(1.0)),
        ..Default::default()
    }
}

/// The emoji results container style
///
/// Takes a focused boolean, to know if this specific button is focused or not
pub fn emoji_button_container_style(tile_theme: &ConfigTheme, focused: bool) -> container::Style {
    container::Style {
        background: Some(Background::Color(glass_surface(
            tile_theme.bg_color(),
            focused,
        ))),
        text_color: Some(tile_theme.text_color(1.0)),
        border: Border {
            color: glass_border(tile_theme.text_color(1.0), focused),
            width: 1.0,
            radius: Radius::new(10.0),
        },
        ..Default::default()
    }
}

/// Emoji buttons styling
pub fn emoji_button_style(tile_theme: &ConfigTheme) -> button::Style {
    let base = tile_theme.bg_color();
    let bg = with_alpha(tint(base, 0.10), 0.28);
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: tile_theme.text_color(1.0),
        border: Border {
            color: glass_border(tile_theme.text_color(1.0), false),
            width: 1.0,
            radius: Radius::new(10.0),
        },
        ..Default::default()
    }
}

pub fn settings_text_input_item_style(theme: &ConfigTheme) -> text_input::Style {
    let base = theme.bg_color();
    let surface = glass_surface(base, false);
    text_input::Style {
        background: Background::Color(surface),
        border: Border {
            color: glass_border(theme.text_color(1.0), false),
            width: 0.2,
            radius: Radius::new(10.),
        },
        icon: theme.text_color(0.75),
        placeholder: theme.text_color(0.50),
        value: theme.text_color(1.0),
        selection: with_alpha(theme.text_color(1.0), 0.20),
    }
}

pub fn settings_save_button_style(theme: &ConfigTheme) -> button::Style {
    button::Style {
        text_color: theme.text_color(1.),
        background: Some(Background::Color(with_alpha(theme.bg_color(), 0.3))),
        border: Border {
            color: theme.text_color(0.7),
            width: 0.1,
            radius: Radius::new(5),
        },
        ..Default::default()
    }
}

pub fn settings_add_button_style(theme: &ConfigTheme) -> button::Style {
    button::Style {
        background: None,
        text_color: theme.text_color(1.),
        border: Border {
            color: theme.text_color(0.7),
            width: 0.7,
            radius: Radius::new(10),
        },
        ..Default::default()
    }
}

pub fn settings_checkbox_style(theme: &ConfigTheme) -> checkbox::Style {
    checkbox::Style {
        background: Background::Color(Color::TRANSPARENT),
        icon_color: theme.text_color(1.),
        border: iced::Border {
            color: theme.text_color(1.),
            width: 1.,
            radius: Radius::new(2.),
        },
        text_color: None,
    }
}

pub fn settings_slider_style(theme: &ConfigTheme) -> slider::Style {
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(theme.text_color(1.)),
                Background::Color(theme.bg_color()),
            ),
            width: 1.5,
            border: Border {
                color: theme.text_color(1.),
                width: 0.3,
                radius: Radius::new(0),
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 10. },
            background: Background::Color(theme.text_color(1.)),
            border_width: 0.1,
            border_color: Color::WHITE,
        },
    }
}

/// Helper fn for making a color look like its glassy
pub fn glass_surface(base: Color, focused: bool) -> Color {
    let t = if focused { 0.2 } else { 0.06 };
    let a = if focused { 0.9 } else { 0.58 };
    with_alpha(tint(base, t), a)
}

/// Helper fn for making a borders color look like its glassy
pub fn glass_border(base_text: Color, focused: bool) -> Color {
    let a = if focused { 0.35 } else { 0.22 };
    with_alpha(base_text, a)
}

pub fn clipboard_image_border_style() -> container::Style {
    container::Style {
        border: Border {
            color: Color::WHITE,
            width: 1.,
            radius: Radius::new(0.),
        },
        ..Default::default()
    }
}
