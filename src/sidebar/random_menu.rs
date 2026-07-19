use bevy::color::palettes::css::BLACK;
use bevy::prelude::*;
use bevy::text::EditableText;
use crate::constants::*;
use crate::nodes::*;
use super::*;
use super::link_params::*;

impl SidebarContent for RandomNode{
    fn build(
        &self, 
        mut commands: &mut Commands, 
        sidebar_entity: Entity, 
        node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
        finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        node: Entity
    ){
        commands.entity(sidebar_entity).with_child(
            (
                Text::new(if let Some(label) = &self.name {
                    format!("Label: {}", label)
                } else {
                    "Type to name".to_string()
                }),
                Node {
                    margin: px(8).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            ));
        commands.entity(sidebar_entity).with_child(divider());
        
        available_links(&mut commands, &node_data, &finished_links, sidebar_entity, node);
        commands.entity(sidebar_entity).with_child(divider());

        commands.entity(sidebar_entity).with_child((
                Text::new("Distribution:"),
                Node {
                    margin: px(4).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            ));

        //spawn context menu
        let context_menu = commands.spawn((
            Name::new("distribution_context_menu"),
            Button,
            Node {
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30),
                border: UiRect::all(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::BLACK),
            children![(
                Pickable::IGNORE,
                Text::new(self.dist_type.clone()),
                TextColor(Color::WHITE),
                TextShadow::default(),
            )],
        )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
            // by default this event would bubble up further leading to the `CloseContextMenus`
            // event being triggered and undoing the opening of one here right away.
            event.propagate(false);
            println!("Clicked context menu");
            debug!("click: {}", event.pointer_location.position);

            commands.trigger(OpenDistributionMenu {
                pos: event.pointer_location.position,
            });
        }).id();

        commands.entity(sidebar_entity).add_child(context_menu);

        commands.entity(sidebar_entity).with_child(divider());

        //HERE is where you need to add context menus per parameter.

        let link_labels = get_ents_and_labels(commands, node_data, &finished_links, node);
        
        for (i, _param) in self.params.iter().enumerate() {        
            build_link_param_selector(commands, link_labels.clone(), self.params.clone(), i, &sidebar_entity);
        }

        commands.entity(sidebar_entity).with_child(divider());

        let sample_button = commands.spawn((
        Name::new("sample_button"),
        Button,
        Node {
            width: px(SIDEBAR_WIDTH * 0.75),
            height: px(30),
            border: UiRect::all(px(5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::MAX,
            margin: px(8.).bottom(),
            ..default()
        },
        BorderColor::all(Color::BLACK),
        BackgroundColor(Color::BLACK),
        children![(
            Pickable::IGNORE,
            Text::new("Sample"),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
        )).observe(sample_node_toast).id();
        commands.entity(sidebar_entity).add_child(sample_button);
        commands.entity(sidebar_entity).with_child(divider());
        }

}


/*fn build_param_textbox(     //TODO: rip out param textbox, replace with context menu of graphlink options
    commands: &mut Commands,
    random_var: &RandomNode,
    param_num: usize
) -> Entity {
    let param = random_var
        .params
        .get(param_num)
        .expect("invalid param_num");

    let param_name = param.0;
    let param_value = param.1;

    commands
        .spawn((
            Node {
                width: percent(100.),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.),
                margin: px(8.).bottom(),
                ..default()
            },
            Name::new(format!("param_row_{}", param_name)),
            children![
                (
                    Text::new(param_name),
                    TextColor(NODE_NAME_COLOR),
                ),
                (
                    Node {
                        width: px(120.),
                        min_height: px(25.),
                        border: px(2).all(),
                        padding: px(4).all(),
                        ..default()
                    },
                    BorderColor::from(Color::from(SLATE_300)),
                    BackgroundColor(DARK_GREY.into()),
                    EditableText::new(param_value.to_string()),
                    TextLayout::no_wrap(),
                    TextCursorStyle::default(),
                    TabIndex(param_num.try_into().unwrap()),
                    Name::new(format!("{}_textbox", param_name)),
                    ParamTextbox(param_num)
                ),
            ],
        ))
        .id()
}*/

fn on_select_distribution(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &Selected)>>,
    mut random_vars: Query<(&mut RandomNode, &Selected)>
){
    let target = event.original_event_target();
    if let Some(single) = selected{
        let (entity, _selected_comp) = single.into_inner();

        if let Ok(item) = menu_items.get(target) {
            //set distribution of node to new dist... or maybe on apply?
            println!("Selected distribution {}", item.0);
            let (mut random_var, _selected_comp) = random_vars.get_mut(entity).unwrap();
            random_var.dist_type = item.0.clone();
            //buggy line, can panic, need to set to default params on dist change.
            refresh_var_dist(&mut random_var, &mut commands);
            commands.trigger(CloseContextMenus);
            commands.trigger(ReloadSidebar);
            
        }
    }
}

pub fn on_open_distribution_menu(
    event: On<OpenDistributionMenu>, 
    mut commands: Commands,
) {
    commands.trigger(CloseContextMenus);
    let pos = event.pos;
    debug!("open context menu at: {pos}");

    commands
        .spawn((
            Name::new("distribution selector"),
            ContextMenu,
            ZIndex(999),
            Node {
                position_type: PositionType::Absolute,
                left: px(pos.x),
                top: px(pos.y),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::linear_rgb(0.1, 0.1, 0.1)),
            children![
                context_item("Normal"),
                context_item("LogNormal"),
                context_item("Beta"),
                context_item("Gamma"),
                context_item("Exponential"),
                context_item("Uniform")
            ],
        ))
        .observe(on_select_distribution);
}