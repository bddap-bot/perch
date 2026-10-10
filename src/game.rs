use bevy::asset::RenderAssetUsages;
use bevy::camera::ScalingMode;
// The derive macros name bevy_ecs directly when bevy is only a target-specific dependency.
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use perch::body::{self, Body, Input};
use perch::seg::Segmenter;
use perch::sprite::{self, Frame};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::webcam::{Surface, Webcam};

const MASK: (u32, u32) = (240, 180);
const COUNTDOWN: f32 = 3.0;
const STALL: f32 = 3.0;
const TINT: [u8; 4] = [70, 230, 255, 26];
const EDGE: [u8; 4] = [200, 250, 255, 160];
const JUMP_KEYS: [KeyCode; 4] = [
    KeyCode::Space,
    KeyCode::ArrowUp,
    KeyCode::KeyW,
    KeyCode::KeyZ,
];

pub fn run() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some("#perch".into()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        transparent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(Color::NONE))
        .insert_non_send(Camera {
            webcam: Webcam::start(),
            surface: Surface::new(MASK.0, MASK.1),
            last_frame: 0.0,
        })
        .insert_resource(Game {
            seg: Segmenter::new(MASK.0 as usize, MASK.1 as usize),
            phase: Phase::WaitingCamera,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (feed, control, hint).chain())
        .run();
}

struct Camera {
    webcam: Webcam,
    surface: Surface,
    last_frame: f32,
}

enum Phase {
    WaitingCamera,
    Countdown { until: f32 },
    Holding(Arc<AtomicBool>),
    Capturing,
    Playing { body: Body, anim_clock: f32 },
}

#[derive(Resource)]
struct Game {
    seg: Segmenter,
    phase: Phase,
}

#[derive(Resource)]
struct Art {
    mask: Handle<Image>,
    frames: [Handle<Image>; Frame::ALL.len()],
}

#[derive(Component)]
struct Player;

fn image(width: u32, height: u32, data: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: MASK.0 as f32,
                min_height: MASK.1 as f32,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
    let size = Vec2::new(MASK.0 as f32, MASK.1 as f32);
    let mask = images.add(image(
        MASK.0,
        MASK.1,
        vec![0; (MASK.0 * MASK.1 * 4) as usize],
    ));
    let (sw, sh) = (sprite::SIZE.0 as u32, sprite::SIZE.1 as u32);
    let frames = Frame::ALL.map(|f| images.add(image(sw, sh, sprite::pixels(f))));
    commands.spawn(Sprite {
        image: mask.clone(),
        custom_size: Some(size),
        ..default()
    });
    commands.spawn((
        Player,
        Sprite {
            image: frames[0].clone(),
            custom_size: Some(Vec2::new(body::WIDTH as f32, body::HEIGHT as f32)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 2.0),
        Visibility::Hidden,
    ));
    commands.insert_resource(Art { mask, frames });
}

fn feed(
    mut camera: NonSendMut<Camera>,
    mut game: ResMut<Game>,
    art: Res<Art>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    if !camera.webcam.new_frame() {
        return;
    }
    camera.last_frame = now;
    let frame = camera.webcam.read(&camera.surface);

    let Game { seg, phase } = &mut *game;
    match phase {
        Phase::WaitingCamera => {
            *phase = Phase::Countdown {
                until: now + COUNTDOWN,
            }
        }
        Phase::Countdown { until } if now >= *until => {
            *phase = Phase::Holding(camera.webcam.hold(true));
        }
        Phase::Holding(settled) if settled.load(Ordering::Relaxed) => {
            seg.begin_capture();
            seg.add_capture_frame(&frame);
            *phase = Phase::Capturing;
        }
        Phase::Countdown { .. } | Phase::Holding(_) => {}
        Phase::Capturing => {
            if seg.add_capture_frame(&frame) {
                *phase = Phase::Playing {
                    body: Body::spawn(&seg.mask),
                    anim_clock: 0.0,
                };
            }
        }
        Phase::Playing { .. } => seg.segment(&frame),
    }

    if let Some(mut img) = images.get_mut(&art.mask)
        && let Some(data) = img.data.as_mut()
    {
        let mask = &seg.mask;
        for (i, px) in data.chunks_exact_mut(4).enumerate() {
            let (x, y) = ((i % mask.width) as i32, (i / mask.width) as i32);
            px.copy_from_slice(&match (mask.solid(x, y), mask.solid(x, y - 1)) {
                (true, false) => EDGE,
                (true, true) => TINT,
                _ => [0; 4],
            });
        }
    }
}

fn control(
    mut game: ResMut<Game>,
    camera: NonSend<Camera>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    art: Res<Art>,
    time: Res<Time>,
    mut player: Single<(&mut Transform, &mut Sprite, &mut Visibility), With<Player>>,
) {
    let held = |k: &[KeyCode]| keys.any_pressed(k.iter().copied());
    let mut input = Input {
        run: held(&[KeyCode::ArrowRight, KeyCode::KeyD]) as i8 as f32
            - held(&[KeyCode::ArrowLeft, KeyCode::KeyA]) as i8 as f32,
        jump_pressed: keys.any_just_pressed(JUMP_KEYS),
        jump_held: held(&JUMP_KEYS),
    };
    let mut recapture = keys.just_pressed(KeyCode::KeyB);
    for pad in &pads {
        let stick = pad.left_stick().x;
        let dpad = pad.pressed(GamepadButton::DPadRight) as i8 as f32
            - pad.pressed(GamepadButton::DPadLeft) as i8 as f32;
        if dpad != 0.0 {
            input.run = dpad;
        } else if stick.abs() > 0.25 {
            input.run = stick;
        }
        input.jump_pressed |= pad.just_pressed(GamepadButton::South);
        input.jump_held |= pad.pressed(GamepadButton::South);
        recapture |=
            pad.just_pressed(GamepadButton::East) || pad.just_pressed(GamepadButton::Select);
    }

    let Game { seg, phase } = &mut *game;
    if recapture && matches!(phase, Phase::Playing { .. } | Phase::Capturing) {
        seg.begin_capture();
        camera.webcam.hold(false);
        *phase = Phase::Countdown {
            until: time.elapsed_secs() + COUNTDOWN,
        };
    }

    let (transform, sprite, visibility) = &mut *player;
    let Phase::Playing { body, anim_clock } = phase else {
        **visibility = Visibility::Hidden;
        return;
    };
    body.step(&seg.mask, input, time.delta_secs());
    *anim_clock += time.delta_secs();
    let stride = ((*anim_clock * 8.0) as u32).is_multiple_of(2);
    let frame = body.pose().frame(stride);
    sprite.image = art.frames[frame as usize].clone();
    sprite.flip_x = body.facing_left;
    **visibility = Visibility::Visible;
    transform.translation.x = body.x.round() + body::WIDTH as f32 / 2.0 - MASK.0 as f32 / 2.0;
    transform.translation.y = MASK.1 as f32 / 2.0 - body.y.round() - body::HEIGHT as f32 / 2.0;
}

fn hint(game: Res<Game>, camera: NonSend<Camera>, time: Res<Time>, mut shown: Local<String>) {
    let now = time.elapsed_secs();
    let text = if let Some(why) = camera.webcam.failure() {
        why
    } else if camera.webcam.waiting() {
        "Allow the camera to play".into()
    } else if !matches!(game.phase, Phase::WaitingCamera) && now - camera.last_frame > STALL {
        "The camera stopped sending video. Reload to retry.".into()
    } else {
        match game.phase {
            Phase::WaitingCamera => String::new(),
            Phase::Countdown { until } => {
                format!("Step out of view · {}", (until - now).ceil().max(1.0))
            }
            Phase::Holding(_) | Phase::Capturing => "Hold still".into(),
            Phase::Playing { .. } => "B: recapture the empty scene".into(),
        }
    };
    if *shown != text {
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("hint"))
        {
            el.set_text_content(Some(&text));
        }
        *shown = text;
    }
}
