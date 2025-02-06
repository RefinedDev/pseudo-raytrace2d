use bevy::{
    color::palettes::css::WHITE,
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin},
    input::mouse::{AccumulatedMouseMotion, MouseButtonInput},
    prelude::*,
};

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
    .add_systems(Update, (draw_rays, control_rays_amount))
    .add_systems(FixedUpdate, (oscillate_target, move_objects))
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
    let increment = (360.0 / n_rays.0).to_radians();

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

        let d_sun_target = (x_target - x_sun).powi(2) + (y_target - y_sun).powi(2);
        let d_start_target = (x_target - start.x).powi(2) + (y_target - start.y).powi(2);

        if d_start_target <= d_sun_target {
            // double point formula
            let m = (end.y - start.y) / (end.x - start.x);
            let c = -m * start.x + start.y; // y = mx + (-mx1 + y1)
            let d = (m * x_target - y_target + c).powi(2) / (m * m + 1.0); // perpendicular distance from center of target^2

            if d < target.1.radius.powi(2) {
                // foot of perpendicular formula for line, its not accurate for circle
                // due to its curvature but i can conceal the extended part by ZIndex :P
                // i should rather intersect for the exact coordinate
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

fn oscillate_target(target_q: Single<&mut Transform, With<Oscillate>>, time: Res<Time<Fixed>>) {
    let mut object = target_q.into_inner();
    object.translation += Vec3::new(
        30.0 * 0.5 * ops::cos(time.elapsed_secs()) * time.delta_secs(),
        30.0 * 1.1 * ops::sin(time.elapsed_secs()) * time.delta_secs(),
        0.0,
    );
}

fn move_objects(
    mut button_events: EventReader<MouseButtonInput>,
    mut m1held: ResMut<M1Held>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    delta: Res<AccumulatedMouseMotion>,
    sun: Single<(&mut Transform, &Sun), With<Sun>>,
    target: Single<(&mut Transform, &Oscillate), (With<Oscillate>, Without<Sun>)>,
) {
    for button_event in button_events.read() {
        if button_event.button == MouseButton::Left {
            *m1held = M1Held(button_event.state.is_pressed());
            break;
        }
    }

    if m1held.0 && delta.delta != Vec2::ZERO {
        if let Some(mouse_pos) = window
            .cursor_position()
            .and_then(|cursor| camera.0.viewport_to_world_2d(camera.1, cursor).ok())
        {
            let viewport_size = window.size() * 0.5;
            let v_x = viewport_size.x;
            let v_y = viewport_size.y;

            let sun = sun.into_inner();
            let mut sun_object = sun.0;
            let r = sun.1.radius;

            let pos = (mouse_pos.x - sun_object.translation.x).powi(2)
                + (mouse_pos.y - sun_object.translation.y).powi(2);

            if pos < r * r {
                sun_object.translation += Vec3::new(delta.delta.x, -delta.delta.y, 0.0);
                sun_object.translation = sun_object.translation.clamp(
                    Vec3::new(-v_x + r, -v_y + r, 0.0),
                    Vec3::new(v_x - r, v_y - r, 0.0),
                );
            } else {
                let target = target.into_inner();
                let mut target_object = target.0;
                let r = target.1.radius;

                let pos = (mouse_pos.x - target_object.translation.x).powi(2)
                    + (mouse_pos.y - target_object.translation.y).powi(2);

                if pos < r * r {
                    target_object.translation += Vec3::new(delta.delta.x, -delta.delta.y, 1.0);
                    target_object.translation = target_object.translation.clamp(
                        Vec3::new(-v_x + r, -v_y + r, 1.0),
                        Vec3::new(v_x - r, v_y - r, 1.0),
                    );
                }
            }
        }
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
            intensity: 0.07,
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
        Transform::from_xyz(200.0, 0.0, 1.0),
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
        RayText,
    ));
}

fn setup_window(mut window: Single<&mut Window>) {
    window.title = String::from("Raytrace 2D");
    window.position = WindowPosition::Centered(MonitorSelection::Current);
}
