use std::f32::consts::PI;

use bevy::{
    color::palettes::css::WHITE,
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin},
    input::mouse::{AccumulatedMouseMotion, MouseButtonInput},
    prelude::*,
};

fn inv_sqrt(x: f32) -> f32 {
    let i = x.to_bits();
    let i = 0x5f3759df - (i >> 1);
    let y = f32::from_bits(i);

    y * (1.5 - 0.5 * x * y * y)
}

#[derive(Resource)]
struct M1Held(bool);

#[derive(Resource)]
struct NumberOfRays(f32);

#[derive(Component)]
struct Oscillate {
    radius: f32,
}

#[derive(Component)]
struct Sun {
    radius: f32,
}

#[derive(Component)]
struct RayText;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins,
        FpsOverlayPlugin {
            config: FpsOverlayConfig {
                text_config: TextFont {
                    font_size: 30.0,
                    ..default()
                },
                text_color: Color::linear_rgb(0.0, 255.0, 0.0),
                enabled: true,
            },
        },
    ))
    .add_systems(Startup, (setup_window, spawn).chain())
    .add_systems(
        Update,
        (
            oscillate_target,
            (move_sun, draw_rays).chain(),
            control_rays_amount,
        ),
    )
    .insert_resource(M1Held(false))
    .insert_resource(NumberOfRays(100.0))
    .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
    .run();
}

fn control_rays_amount(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut rays: ResMut<NumberOfRays>,
    mut text: Single<&mut Text, With<RayText>>,
) {
    if keyboard_input.pressed(KeyCode::ArrowRight) {
        rays.0 += 1.0;
    } else if keyboard_input.pressed(KeyCode::ArrowLeft) {
        rays.0 -= 1.0;
    }

    text.0 = format!("Number of rays: {}", rays.0);
}

fn draw_rays(
    n_rays: Res<NumberOfRays>,
    sun: Single<(&Transform, &Sun), With<Sun>>,
    target: Single<(&Transform, &Oscillate), With<Oscillate>>,
    window: Single<&Window>,
    mut gizmos: Gizmos,
) {
    // cannot manually draw pixels, so i have to check for intersections with math
    let sun = sun.into_inner();
    let target = target.into_inner();
    let viewport_size = 0.5 * window.size();
    let increment = (PI / 180.0) * (360.0 / n_rays.0);

    let mut angle = 0.0;
    for _ in 0..(n_rays.0 as u32) {
        let cosx = ops::cos(angle);
        let sinx = ops::sin(angle);

        let x_target = target.0.translation.x;
        let y_target = target.0.translation.y;

        let x_sun = sun.0.translation.x;
        let y_sun = sun.0.translation.y;

        let start = Vec2::new(x_sun + sun.1.radius * sinx, y_sun + sun.1.radius * cosx);
        let mut end = Vec2::new(1.5 * viewport_size.x * sinx, 1.5 * viewport_size.y * cosx);

        // need to find a better soln for this
        // two coodinates which are on equal magnitude but opposite angles they both satisfy d<target_radius for some reason
        // so for now i just check if distance of the point away from the target dont check in for now...
        let d_sun_target = (x_target - x_sun).powi(2) + (y_target - y_sun).powi(2);
        let d_start_target = (x_target - start.x).powi(2) + (y_target - start.y).powi(2);

        if d_start_target <= d_sun_target {
            // double point formula
            let m = (end.y - start.y) / (end.x - start.x);
            let c = -m * start.x + start.y; // y = mx + (-mx1 + y1)
            let d = (m * x_target - y_target + c).abs() * inv_sqrt(m * m + 1.0); // perpendicular distance from center of target

            if d < target.1.radius {
                // foot of perpendicular formula
                let foot_x = -m * (m * x_target - y_target + c) / (m * m + 1.0) + x_target;
                let foot_y = (m * x_target - y_target + c) / (m * m + 1.0) + y_target;
                end.x = foot_x;
                end.y = foot_y;
            }
        }

        gizmos.line_2d(start, end, WHITE);

        angle += increment;
    }
}

fn oscillate_target(target_q: Single<&mut Transform, With<Oscillate>>, time: Res<Time>) {
    let mut object = target_q.into_inner();
    object.translation = Vec3::new(
        350.0 + 50.0 * ops::cos(time.elapsed_secs()),
        100.0 * ops::sin(time.elapsed_secs()),
        1.0,
    );
}

fn move_sun(
    mut button_events: EventReader<MouseButtonInput>,
    mut m1held: ResMut<M1Held>,
    window: Single<&Window>,
    m_d: Res<AccumulatedMouseMotion>,
    sun: Single<(&mut Transform, &Sun), With<Sun>>,
) {
    for button_event in button_events.read() {
        if button_event.button != MouseButton::Left {
            continue;
        }
        *m1held = M1Held(button_event.state.is_pressed());
    }
    if m1held.0 && m_d.delta != Vec2::ZERO {
        let obj = sun.into_inner();
        let mut shape = obj.0;
        let data = obj.1;

        let viewport_size = window.size() * 0.5;
        shape.translation += Vec3::new(m_d.delta.x, -m_d.delta.y, 0.0);
        shape.translation = shape.translation.clamp(
            Vec3::new(
                -viewport_size.x + data.radius,
                -viewport_size.y + data.radius,
                0.0,
            ),
            Vec3::new(
                viewport_size.x - data.radius,
                viewport_size.y - data.radius,
                0.0,
            ),
        );
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Camera2d,
        Camera {
            hdr: true,
            ..default()
        },
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.10,
            ..default()
        },
    ));

    // sun
    const SUN_RADIUS: f32 = 75.0;
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(SUN_RADIUS))),
        Sun { radius: SUN_RADIUS },
        MeshMaterial2d(materials.add(Color::linear_rgb(255.0, 255.0, 0.0))),
        Transform::from_xyz(-200.0, 0.0, 0.0),
    ));

    // target
    const TARGET_RADIUS: f32 = 150.0;
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(TARGET_RADIUS))),
        Oscillate {
            radius: TARGET_RADIUS,
        },
        MeshMaterial2d(materials.add(Color::linear_rgb(255.0, 255.0, 255.0))),
    ));

    commands.spawn((
        Text::new("Number of rays: _"),
        TextFont {
            font_size: 30.0,
            ..default()
        },
        TextColor(Color::linear_rgb(0.0, 255.0, 0.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(30.0),
            ..default()
        },
        RayText
    ));
}

fn setup_window(mut window: Single<&mut Window>) {
    window.title = String::from("Raytrace 2D");
    window.position = WindowPosition::Centered(MonitorSelection::Current);
}
