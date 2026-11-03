use super::*;
use crate::bayesian_core::ScalarType;

#[derive(Component)]
pub struct VectorValuesTextbox {
    pub node: Entity,
    pub element: ScalarType,
}

#[derive(Event)]
pub struct ApplyVectorValues {
    pub node: Entity,
    pub element: ScalarType,
    pub text: String,
}

pub fn build_vector_sidebar(
    commands: &mut Commands,
    sidebar: Entity,
    entity: Entity,
    vector: &VectorNode,
) {
    add_node_name_field(commands, sidebar, vector.name.as_deref());
    commands.entity(sidebar).with_child(divider());
    commands.entity(sidebar).with_child((
        Text::new(format!(
            "{} vector, n = {}",
            match vector.elements.element_type() {
                ScalarType::Real => "Real",
                ScalarType::Integer => "Integer",
            },
            vector.elements.len()
        )),
        text_font(),
        TextColor(NODE_NAME_COLOR),
    ));
    commands.entity(sidebar).with_child((
        Text::new("Elements, separated by commas"),
        text_font(),
        TextColor(NODE_NAME_COLOR),
        Node {
            margin: px(8).top(),
            ..default()
        },
    ));
    commands.entity(sidebar).with_child((
        VectorValuesTextbox {
            node: entity,
            element: vector.elements.element_type(),
        },
        Node {
            width: percent(100),
            min_height: px(30),
            padding: px(4).all(),
            border: px(2).all(),
            margin: px(8).bottom(),
            ..default()
        },
        BorderColor::all(Color::from(SLATE_300)),
        BackgroundColor(DARK_GREY.into()),
        EditableText {
            visible_width: Some(22.0),
            ..EditableText::new(vector.elements.text())
        },
        text_font(),
        TextColor(Color::WHITE),
        TextLayout::no_wrap(),
        TextCursorStyle::default(),
        TabIndex(1),
        Name::new("vector_values_textbox"),
    ));
    let (label, next_element) = match vector.elements.element_type() {
        ScalarType::Real => ("Real", ScalarType::Integer),
        ScalarType::Integer => ("Integer", ScalarType::Real),
    };
    let toggle = plate_menu::selector_button(commands, "vector_element_type_toggle", label)
        .observe(
            move |mut event: On<Pointer<Click>>,
                  fields: Query<(&VectorValuesTextbox, &EditableText)>,
                  mut commands: Commands| {
                event.propagate(false);
                if let Some((_, text)) = fields.iter().find(|(field, _)| field.node == entity) {
                    commands.trigger(ApplyVectorValues {
                        node: entity,
                        element: next_element,
                        text: text.value().to_string(),
                    });
                }
            },
        )
        .id();
    commands.entity(sidebar).add_child(toggle);
    commands.entity(sidebar).with_child(divider());
}

pub fn on_vector_enter(
    keyboard: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    fields: Query<(&VectorValuesTextbox, &EditableText)>,
    mut commands: Commands,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        if let Some((field, text)) = focus.get().and_then(|entity| fields.get(entity).ok()) {
            commands.trigger(ApplyVectorValues {
                node: field.node,
                element: field.element,
                text: text.value().to_string(),
            });
        }
    }
}

pub fn apply_vector_values(
    event: On<ApplyVectorValues>,
    mut nodes: Query<&mut VectorNode>,
    mut labels: Query<(&ChildOf, &mut Text2d), With<NodeLabel>>,
    mut commands: Commands,
    inference: Option<Res<crate::bevy_to_fugue::InferenceResultResource>>,
) {
    let Ok(mut vector) = nodes.get_mut(event.node) else {
        return;
    };
    let elements = match VectorElements::parse(&event.text, event.element) {
        Ok(elements) => elements,
        Err(text) => {
            commands.trigger(crate::ui::ErrorToast {
                text,
                color: ERR_COLOR,
            });
            return;
        }
    };
    if vector.elements == elements {
        return;
    }
    if global::graph_change_requires_confirmation(&mut commands, inference.as_deref(), None) {
        return;
    }
    vector.elements = elements;
    for (parent, mut text) in &mut labels {
        if parent.parent() == event.node {
            text.0 = vector.label();
        }
    }
    commands.trigger(ReloadSidebar);
}

#[cfg(test)]
#[path = "tests/vector_menu.rs"]
mod tests;
