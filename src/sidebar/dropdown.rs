//! Shared controls: middle grey marks editable fields.
use super::*;

const INK: Color = Color::WHITE;
const OUTLINE: Color = Color::srgb(0.32, 0.32, 0.32);
pub(crate) const FIELD_SURFACE: Color = Color::srgb(0.46, 0.46, 0.46);

#[derive(Component)]
pub(crate) struct DropdownOption;

pub(crate) fn panel(pos: Vec2) -> impl Bundle {
    (
        ContextMenu,
        GlobalZIndex(1000),
        ScrollPosition::default(),
        Node {
            position_type: PositionType::Absolute,
            left: px(pos.x),
            top: px(pos.y),
            min_width: px(220),
            max_width: px(360),
            max_height: px(360),
            padding: px(6).all(),
            row_gap: px(3),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::scroll_y(),
            border: px(1).all(),
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
        BackgroundColor(FIELD_SURFACE),
        BorderColor::all(OUTLINE),
    )
}

pub(crate) fn item(text: &str) -> impl Bundle + use<> {
    (
        DropdownOption,
        Button,
        Node {
            width: percent(100),
            min_height: px(34),
            flex_shrink: 0.,
            padding: UiRect::axes(px(10), px(7)),
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        children![(
            Pickable::IGNORE,
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(14.),
                ..text_font()
            },
            TextColor(INK),
        )],
    )
}

pub(crate) fn section(text: &str) -> impl Bundle + use<> {
    (
        Node {
            padding: UiRect::new(px(10), px(10), px(10), px(5)),
            margin: px(3).top(),
            border: px(1).top(),
            flex_shrink: 0.,
            ..default()
        },
        BorderColor::all(OUTLINE),
        children![(
            Pickable::IGNORE,
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(12.),
                ..text_font()
            },
            TextColor(Color::srgb(0.75, 0.75, 0.75)),
        )],
    )
}

pub(crate) fn selector<T: Into<String>>(name: T, value: &str) -> impl Bundle + use<T> {
    control(name, value, true)
}

pub(crate) fn button<T: Into<String>>(name: T, value: &str) -> impl Bundle + use<T> {
    control(name, value, false)
}

fn control<T: Into<String>>(name: T, value: &str, dropdown: bool) -> impl Bundle + use<T> {
    (
        Name::new(name.into()),
        Button,
        Node {
            width: percent(100),
            min_height: px(36),
            flex_shrink: 0.,
            padding: UiRect::axes(px(10), px(7)),
            border: px(1).all(),
            border_radius: BorderRadius::MAX,

            margin: px(4).bottom(),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(8),
            ..default()
        },
        BorderColor::all(OUTLINE),
        BackgroundColor(FIELD_SURFACE),
        children![
            (
                Pickable::IGNORE,
                Text::new(value),
                TextFont {
                    font_size: FontSize::Px(14.),
                    ..text_font()
                },
                TextColor(INK),
                Node {
                    flex_shrink: 1.,
                    ..default()
                },
            ),
            (
                Pickable::IGNORE,
                Text::new(if dropdown { "v" } else { "" }),
                TextFont {
                    font_size: FontSize::Px(14.),
                    ..text_font()
                },
                TextColor(INK),
            )
        ],
    )
}

pub(crate) fn style_items(
    mut items: Query<
        (&Interaction, &ChildOf, &mut BackgroundColor),
        (Changed<Interaction>, With<DropdownOption>),
    >,
    menus: Query<(), With<ContextMenu>>,
) {
    for (interaction, parent, mut bg) in &mut items {
        if menus.contains(parent.parent()) {
            bg.0 = if *interaction != Interaction::None {
                Color::srgb(0.3, 0.3, 0.3)
            } else {
                Color::NONE
            };
        }
    }
}

/// Labels remain plain; only editable controls have a grey surface.
pub(crate) fn label(text: impl Into<String>) -> impl Bundle {
    label_with_font(text, text_font())
}

pub(crate) fn bold_label(text: impl Into<String>) -> impl Bundle {
    label_with_font(
        text,
        text_font().with_font_weight(bevy::text::FontWeight::BOLD),
    )
}

fn label_with_font(text: impl Into<String>, font: TextFont) -> impl Bundle {
    (
        Node {
            width: percent(100),
            padding: UiRect::axes(px(8), px(0)),
            margin: px(1).bottom(),
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(Color::NONE),
        children![(
            Text::new(text.into()),
            font,
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
        )],
    )
}
