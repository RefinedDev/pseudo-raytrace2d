use std::sync::atomic::{AtomicUsize, Ordering};

use bevy::{
    color::palettes::tailwind::YELLOW_100,
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin},
    input::mouse::{AccumulatedMouseMotion, MouseButtonInput},
    math::ops::{cos, sin},
    prelude::*,
};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
const SUN_RADIUS: f32 = 75.0;
const TARGET_RADIUS: f32 = 100.0;
const R_SQ: f32 = TARGET_RADIUS * TARGET_RADIUS;
const R_SUN_SQ: f32 = SUN_RADIUS * SUN_RADIUS;

#[derive(Resource)]
struct M1Held(bool);

#[derive(Resource)]
struct NumberOfRays(f32);

#[derive(Resource)]
struct Reflection(bool);

#[derive(Component)]
struct Target {
    radius: f32,
    id: usize,
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
                    font_size: 20.0,
                    ..default()
                },
                text_color: Color::linear_rgb(0.0, 255.0, 0.0),
                enabled: true,
            },
        },
    ))
    .add_systems(Startup, (setup_window, spawn).chain())
    .add_systems(Update, (draw_rays, input_handle))
    .add_systems(FixedUpdate, (oscillate_target, move_objects))
    .insert_resource(M1Held(false))
    .insert_resource(Reflection(false))
    .insert_resource(NumberOfRays(100.0))
    .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
    .run();
}

fn input_handle(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,

    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,

    targets: Query<(&mut Transform, &mut Target, Entity), With<Target>>,

    mut rays: ResMut<NumberOfRays>,
    mut ray_text: Single<&mut Text, With<RayText>>,

    mut reflections: ResMut<Reflection>,
) {
    // REFLECTIONS
    if keyboard_input.just_pressed(KeyCode::Space) {
        reflections.0 = !reflections.0;
    }

    // RAYS
    if keyboard_input.pressed(KeyCode::ArrowRight) {
        rays.0 += 1.0;
    } else if keyboard_input.pressed(KeyCode::ArrowLeft) {
        rays.0 -= 1.0;
    }

    ray_text.0 = format!("Number of rays: {}", rays.0);

    let Some(pix_pos) = window.cursor_position() else {
        return;
    };

    // TARGETS
    if keyboard_input.just_pressed(KeyCode::ArrowUp) {
        let position = camera.0.viewport_to_world(camera.1, pix_pos).unwrap();
        commands.spawn((
            Mesh2d(meshes.add(Circle::new(TARGET_RADIUS))),
            Target {
                radius: TARGET_RADIUS,
                id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            },
            MeshMaterial2d(materials.add(Color::linear_rgb(255.0, 255.0, 255.0))),
            Transform::from_translation(position.origin),
        ));
    } else if keyboard_input.just_pressed(KeyCode::ArrowDown) {
        let position = camera.0.viewport_to_world_2d(camera.1, pix_pos).unwrap();
        for target in targets.iter() {
            let r = target.1.radius;
            let pos = (position.x - target.0.translation.x).powi(2)
                + (position.y - target.0.translation.y).powi(2);
            if pos < r * r {
                commands.entity(target.2).despawn();
                break;
            }
        }
    }
}

fn draw_rays(
    n_rays: Res<NumberOfRays>,
    sun: Single<(&Transform, &Sun), With<Sun>>,
    targets: Query<(&Transform, &Target), With<Target>>,

    window: Single<&Window>,
    mut gizmos: Gizmos,

    is_reflecting: Res<Reflection>,
) {
    // cannot manually draw pixels, so i have to check for intersections with math
    let viewport_size = 0.5 * window.size();

    let increment = (360.0 / n_rays.0).to_radians();
    let mut angle = 0.0;

    for _ in 0..(n_rays.0 as u32) {
        let x_sun = sun.0.translation.x;
        let y_sun = sun.0.translation.y;

        let mut start = Vec2::new(x_sun + sun.1.radius * sin(angle), y_sun + sun.1.radius * cos(angle));
        let mut end = Vec2::new(1.5 * viewport_size.x * sin(angle), 1.5 * viewport_size.y * cos(angle));

        let mut reflecting_off_of: Option<&Target> = None;
        let mut reflecting_off_sun = false;
        let mut being_reflected = false;
        let mut no_of_reflections = 0; // too many reflections lead to crashes so i gotta limit :(
        loop {
            let slope = (end.y - start.y) / (end.x - start.x); // (slope)
            let intercept = -slope * start.x + start.y; // y = mx + (-mx1 + y1) (intercept)

            let mut nearest_target_x = 0.0; // x center of nearest target
            let mut nearest_target_y = 0.0; // y center of nearest target
            let mut n_target: Option<&Target> = None; // nearest target object
            let mut nearest_target_distance = 10e10; // nearest distance from (sun/target), 10e10 as placeholder

            for target in targets.iter() {
                if let Some(v) = reflecting_off_of {
                    if v.id == target.1.id {
                        // dont want it reflecting off of itself
                        continue;
                    }
                }

                let x_target = target.0.translation.x;
                let y_target = target.0.translation.y;

                let d_start_target = (x_target - start.x).powi(2) + (y_target - start.y).powi(2);
                let d_sq = (slope * x_target - y_target + intercept).powi(2) / (slope * slope + 1.0); // perpendicular distance from center of target^2

                if d_sq < R_SQ {
                    if being_reflected {
                        if d_start_target < nearest_target_distance {
                            nearest_target_distance = d_start_target;
                            nearest_target_x = x_target;
                            nearest_target_y = y_target;
                            n_target = Some(target.1);
                        }
                    } else {
                        let d_sun_target = (x_target - x_sun).powi(2) + (y_target - y_sun).powi(2);
                        if d_start_target <= d_sun_target {
                            if d_sun_target < nearest_target_distance {
                                nearest_target_distance = d_sun_target;
                                nearest_target_x = x_target;
                                nearest_target_y = y_target;
                                n_target = Some(target.1);
                            }
                        }
                    }
                }
            }

            // check reflections with the sun
            if being_reflected {
                let d_start_sun = (x_sun - start.x).powi(2) + (y_sun - start.y).powi(2);
                let d_sq = (slope * x_sun - y_sun + intercept).powi(2) / (slope * slope + 1.0);
                if d_start_sun < nearest_target_distance && !reflecting_off_sun && d_sq < R_SUN_SQ {
                    // only wanna do it if ray closest to sun
                    nearest_target_x = x_sun;
                    nearest_target_y = y_sun;
                    n_target = None;
                    reflecting_off_sun = true;
                } else {
                    reflecting_off_sun = false;
                }
            }

            let mut foot_x = 0.0; // intersection x
            let mut foot_y = 0.0; // intersection y
            if n_target.is_some() || reflecting_off_sun {
                let mut r = R_SQ;
                if reflecting_off_sun {
                    r = R_SUN_SQ;
                }

                // intersection of a line and a circle
                // you can put the formula (x-h)^2 + (mx + c - k)^2 = r^2 into wolfram or something to get soln in terms of x
                let sqrt = (r * slope * slope + r - intercept * intercept - 2.0 * intercept * nearest_target_x * slope + 2.0 * intercept * nearest_target_y
                    - nearest_target_x * nearest_target_x * slope * slope
                    + 2.0 * nearest_target_x * nearest_target_y * slope
                    - nearest_target_y * nearest_target_y)
                    .sqrt();

                let exp1 = -intercept * slope + nearest_target_x + nearest_target_y * slope;
                let exp2 = slope * slope + 1.0;

                foot_x = (-sqrt + exp1) / exp2;

                if foot_x < start.x {
                    foot_x = (sqrt + exp1) / exp2;
                }

                foot_y = foot_x * slope + intercept;

                end.x = foot_x;
                end.y = foot_y;
            }

            if is_reflecting.0 {
                if n_target.is_some() || reflecting_off_sun {
                    gizmos.line_2d(start, end, YELLOW_100);

                    no_of_reflections += 1;
                    if no_of_reflections == 10 {
                        break;
                    }
                    
                    // https://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf
                    let dirn_vector = (end - start).normalize();
                    let normal_vector = (Vec2::new(foot_x, foot_y) - Vec2::new(nearest_target_x, nearest_target_y)).normalize();
                    let perp_component = dirn_vector.dot(normal_vector) * normal_vector;
                    let parallel_component = dirn_vector - perp_component;
                    let resultant = parallel_component - perp_component;

                    start = end;
                    end = resultant * 10e2;

                    reflecting_off_of = n_target;
                    being_reflected = true;
                } else {
                    if being_reflected {
                        gizmos.line_2d(start, end, YELLOW_100);
                    }
                    break;
                }
            } else {
                gizmos.line_2d(start, end, YELLOW_100);
                break;
            }
        }
        angle += increment;
    }
}

fn oscillate_target(
    mut targets: Query<&mut Transform, With<Target>>,
    time: Res<Time<Fixed>>,
    is_reflecting: Res<Reflection>,
) {
    if is_reflecting.0 == true {
        return;
    }
    for mut target in targets.iter_mut() {
        target.translation += Vec3::new(
            30.0 * 0.5 * cos(time.elapsed_secs()) * time.delta_secs(),
            30.0 * 1.1 * sin(time.elapsed_secs()) * time.delta_secs(),
            0.0,
        );
    }
}

fn move_objects(
    mut button_events: EventReader<MouseButtonInput>,
    mut m1held: ResMut<M1Held>,
    delta: Res<AccumulatedMouseMotion>,

    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,

    sun: Single<(&mut Transform, &Sun), With<Sun>>,
    mut targets: Query<(&mut Transform, &Target), (With<Target>, Without<Sun>)>,
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
                for target in targets.iter_mut() {
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
            intensity: 0.05,
            ..default()
        },
    ));

    // sun
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(SUN_RADIUS))),
        Sun { radius: SUN_RADIUS },
        MeshMaterial2d(materials.add(Color::linear_rgb(255.0, 255.0, 0.0))),
        Transform::from_xyz(-200.0, 0.0, 0.0),
    ));

    // target
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(TARGET_RADIUS))),
        Target {
            radius: TARGET_RADIUS,
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
        },
        MeshMaterial2d(materials.add(Color::linear_rgb(255.0, 255.0, 255.0))),
        Transform::from_xyz(200.0, 0.0, 1.0),
    ));

    commands.spawn((
        Text::new("Number of rays: _"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::linear_rgb(0.0, 255.0, 0.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            ..default()
        },
        RayText,
    ));

    commands.spawn((
        Text::new(
            "RIGHT/LEFT arrow for rays\nUP/DOWN arrow for targets\nSPACE to toggle reflections",
        ),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::linear_rgb(0.0, 255.0, 0.0)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            ..default()
        },
    ));
}

fn setup_window(mut window: Single<&mut Window>) {
    window.title = String::from("Raytrace 2D");
    window.position = WindowPosition::Centered(MonitorSelection::Current);
}
