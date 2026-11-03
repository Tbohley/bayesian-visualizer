//! Raster icons and their geometric primitives.
use super::*;

// Small antialiased line icons, kept independent of button behavior for easy replacement.
fn icon_segments(action: Action) -> Vec<([f32; 2], [f32; 2])> {
    match action {
        Action::Edit => vec![
            ([5., 16.], [15., 6.]),
            ([15., 6.], [19., 10.]),
            ([19., 10.], [9., 20.]),
            ([9., 20.], [4., 21.]),
            ([4., 21.], [5., 16.]),
            ([13., 8.], [17., 12.]),
            ([17., 4.], [21., 8.]),
        ],
        Action::Grip => vec![
            ([8., 6.], [8., 6.5]),
            ([16., 6.], [16., 6.5]),
            ([8., 12.], [8., 12.5]),
            ([16., 12.], [16., 12.5]),
            ([8., 18.], [8., 18.5]),
            ([16., 18.], [16., 18.5]),
        ],
        Action::NodeType => vec![([7., 10.], [12., 15.]), ([12., 15.], [17., 10.])],
        Action::Tool(Tool::Select) => vec![
            ([5., 2.], [5., 20.]),
            ([5., 20.], [9., 16.]),
            ([9., 16.], [12., 22.]),
            ([12., 22.], [15., 20.5]),
            ([15., 20.5], [12., 14.5]),
            ([12., 14.5], [18., 14.5]),
            ([18., 14.5], [5., 2.]),
        ],
        Action::Tool(Tool::Node) => vec![
            ([12., 3.], [21., 12.]),
            ([21., 12.], [12., 21.]),
            ([12., 21.], [3., 12.]),
            ([3., 12.], [12., 3.]),
            ([8., 12.], [16., 12.]),
            ([12., 8.], [12., 16.]),
        ],
        Action::Tool(Tool::Edge) => vec![
            ([4., 20.], [20., 4.]),
            ([12., 4.], [20., 4.]),
            ([20., 4.], [20., 12.]),
        ],
        // The filled corner badge is rasterized by plate_pixel.
        Action::Tool(Tool::Plate) => vec![],
        Action::Undo => vec![
            ([9., 5.], [4., 10.]),
            ([4., 10.], [9., 15.]),
            ([4., 10.], [15., 10.]),
            ([15., 10.], [19., 13.]),
            ([19., 13.], [19., 18.]),
        ],
        Action::Redo => icon_segments(Action::Undo)
            .into_iter()
            .map(|(a, b)| ([24. - a[0], a[1]], [24. - b[0], b[1]]))
            .collect(),
        Action::ZoomOut => vec![([5., 12.], [19., 12.])],
        Action::ZoomIn => vec![([5., 12.], [19., 12.]), ([12., 5.], [12., 19.])],
        Action::Fit => vec![
            ([4., 9.], [4., 4.]),
            ([4., 4.], [9., 4.]),
            ([15., 4.], [20., 4.]),
            ([20., 4.], [20., 9.]),
            ([20., 15.], [20., 20.]),
            ([20., 20.], [15., 20.]),
            ([9., 20.], [4., 20.]),
            ([4., 20.], [4., 15.]),
            ([9., 9.], [15., 9.]),
            ([15., 9.], [15., 15.]),
            ([15., 15.], [9., 15.]),
            ([9., 15.], [9., 9.]),
        ],
    }
}

pub(super) fn icon_image(action: Action) -> Image {
    let segments = icon_segments(action);
    let size = 48u32;
    let mut pixels = vec![0; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let point = Vec2::new((x as f32 + 0.5) / 2., (y as f32 + 0.5) / 2.);
            let i = ((y * size + x) * 4) as usize;
            if action == Action::Tool(Tool::Plate) {
                pixels[i..i + 4].copy_from_slice(&plate_pixel(point));
                continue;
            }
            let d = segments
                .iter()
                .map(|(a, b)| {
                    let a = Vec2::from_array(*a);
                    let ab = Vec2::from_array(*b) - a;
                    point.distance(
                        a + ab * ((point - a).dot(ab) / ab.length_squared()).clamp(0., 1.),
                    )
                })
                .fold(f32::INFINITY, f32::min);
            if action == Action::Tool(Tool::Select) {
                let inside = segments.iter().fold(false, |inside, (a, b)| {
                    let crosses = (a[1] > point.y) != (b[1] > point.y)
                        && point.x < (b[0] - a[0]) * (point.y - a[1]) / (b[1] - a[1]) + a[0];
                    inside ^ crosses
                });
                let pixel = if inside {
                    [24, 30, 39, 255]
                } else {
                    [255, 255, 255, ((1. - d).clamp(0., 1.) * 255.) as u8]
                };
                pixels[i..i + 4].copy_from_slice(&pixel);
                continue;
            }
            let color = match action {
                Action::Tool(Tool::Plate) => [10, 51, 122],
                Action::Tool(Tool::Edge) => [55, 113, 169],
                Action::Tool(Tool::Select) => [24, 30, 39],
                Action::ZoomIn | Action::ZoomOut | Action::Fit => [43, 99, 163],
                Action::Undo | Action::Redo => [112, 84, 145],
                _ => [50, 65, 85],
            };
            pixels[i..i + 4].copy_from_slice(&[
                color[0],
                color[1],
                color[2],
                ((1.25 - d).clamp(0., 1.) * 255.) as u8,
            ]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        pixels,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

// Signed box distance keeps the reference icon's square corners sharp at any scale.
fn box_distance(point: Vec2, center: Vec2, half_size: Vec2) -> f32 {
    let d = (point - center).abs() - half_size;
    d.max(Vec2::ZERO).length() + d.max_element().min(0.)
}

fn plate_pixel(point: Vec2) -> [u8; 4] {
    let outer = box_distance(point, Vec2::splat(12.), Vec2::splat(11.));
    let inner = box_distance(point, Vec2::splat(12.), Vec2::splat(9.));
    let border = outer.max(-inner);
    let badge = box_distance(point, Vec2::splat(17.5), Vec2::splat(5.5));
    let coverage = (0.5 - border.min(badge)).clamp(0., 1.);
    let glyph = [
        (Vec2::new(15., 20.), Vec2::new(17., 14.)),
        (Vec2::new(17., 14.), Vec2::new(19., 20.)),
        (Vec2::new(16., 18.), Vec2::new(18., 18.)),
    ];
    let distance = glyph
        .iter()
        .map(|(a, b)| {
            let ab = *b - *a;
            point.distance(*a + ab * ((point - *a).dot(ab) / ab.length_squared()).clamp(0., 1.))
        })
        .fold(f32::INFINITY, f32::min);
    let white = ((0.9 - distance) * 2.).clamp(0., 1.);
    let blue = [13., 46., 112.];
    [
        (blue[0] + (255. - blue[0]) * white) as u8,
        (blue[1] + (255. - blue[1]) * white) as u8,
        (blue[2] + (255. - blue[2]) * white) as u8,
        (coverage * 255.) as u8,
    ]
}
