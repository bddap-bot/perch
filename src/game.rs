// The derive macros name bevy_ecs directly when bevy is only a target-specific dependency.
use bevy::asset::RenderAssetUsages;
use bevy::camera::ScalingMode;
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use perch::body::{self, Body, Input, Pose};
use perch::seg::Segmenter;
use perch::sprite;

use crate::webcam::{Surface, Webcam};

const MASK: (u32, u32) = (240, 180);
const DISPLAY: (u32, u32) = (640, 480);
const COUNTDOWN: f32 = 3.0;
const TINT: [u8; 4] = [70, 230, 255, 70];
const EDGE: [u8; 4] = [200, 250, 255, 220];

pub fn run() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some("#perch".into()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.09)))
        .insert_non_send(Feed {
            webcam: Webcam::start(),
            display: Surface::new(DISPLAY.0, DISPLAY.1),
            small: Surface::new(MASK.0, MASK.1),
            buf: Vec::new(),
        })
        .insert_resource(Play {
            seg: Segmenter::new(MASK.0 as usize, MASK.1 as usize),
            body: None,
            phase: Phase::WaitingCamera,
            run_clock: 0.0,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (feed, control, hint).chain())
        .run();
}

struct Feed {
    webcam: Webcam,
    display: Surface,
    small: Surface,
    buf: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    WaitingCamera,
    Countdown { until: f32 },
    Capturing,
    Playing,
}

#[derive(Resource)]
struct Play {
    seg: Segmenter,
    body: Option<Body>,
    phase: Phase,
    run_clock: f32,
}

#[derive(Resource)]
struct Art {
    display: Handle<Image>,
    mask: Handle<Image>,
    poses: [Handle<Image>; 5],
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
    let blank = |w: u32, h: u32| image(w, h, vec![0; (w * h * 4) as usize]);
    let display = images.add(blank(DISPLAY.0, DISPLAY.1));
    let mask = images.add(blank(MASK.0, MASK.1));
    let (sw, sh) = (sprite::SIZE.0 as u32, sprite::SIZE.1 as u32);
    let poses = [
        (Pose::Idle, false),
        (Pose::Run, true),
        (Pose::Run, false),
        (Pose::Jump, false),
        (Pose::Fall, false),
    ]
    .map(|(p, phase)| images.add(image(sw, sh, sprite::frame(p, phase))));
    commands.spawn((
        Sprite {
            image: display.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    commands.spawn((
        Sprite {
            image: mask.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));
    commands.spawn((
        Player,
        Sprite {
            image: poses[0].clone(),
            custom_size: Some(Vec2::new(body::WIDTH as f32, body::HEIGHT as f32)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 2.0),
        Visibility::Hidden,
    ));
    commands.insert_resource(Art {
        display,
        mask,
        poses,
    });
}

fn feed(
    mut feed: NonSendMut<Feed>,
    mut play: ResMut<Play>,
    art: Res<Art>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    if !feed.webcam.new_frame() {
        return;
    }
    if play.phase == Phase::WaitingCamera {
        play.phase = Phase::Countdown {
            until: now + COUNTDOWN,
        };
    }
    let Feed {
        webcam,
        display,
        small,
        buf,
    } = &mut *feed;

    webcam.read(display, buf);
    if let Some(mut img) = images.get_mut(&art.display) {
        img.data = Some(buf.clone());
    }

    webcam.read(small, buf);
    match play.phase {
        Phase::Countdown { until } if now >= until => {
            play.seg.begin_capture();
            play.seg.add_capture_frame(buf);
            play.phase = Phase::Capturing;
        }
        Phase::Capturing => {
            if play.seg.add_capture_frame(buf) {
                play.phase = Phase::Playing;
                play.body = Some(Body::spawn(&play.seg.mask));
            }
        }
        Phase::Playing => play.seg.segment(buf),
        _ => {}
    }

    if let Some(mut img) = images.get_mut(&art.mask) {
        let mask = &play.seg.mask;
        let tinted = (0..mask.height as i32)
            .flat_map(|y| (0..mask.width as i32).map(move |x| (x, y)))
            .flat_map(|(x, y)| match (mask.solid(x, y), mask.solid(x, y - 1)) {
                (true, false) => EDGE,
                (true, true) => TINT,
                _ => [0; 4],
            })
            .collect();
        img.data = Some(tinted);
    }
}

fn control(
    mut play: ResMut<Play>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    art: Res<Art>,
    time: Res<Time>,
    mut player: Single<(&mut Transform, &mut Sprite, &mut Visibility), With<Player>>,
) {
    let any_key = |k: &[KeyCode]| keys.any_pressed(k.iter().copied());
    let any_new = |k: &[KeyCode]| keys.any_just_pressed(k.iter().copied());
    let mut input = Input {
        run: any_key(&[KeyCode::ArrowRight, KeyCode::KeyD]) as i8 as f32
            - any_key(&[KeyCode::ArrowLeft, KeyCode::KeyA]) as i8 as f32,
        jump_pressed: any_new(&[
            KeyCode::Space,
            KeyCode::ArrowUp,
            KeyCode::KeyW,
            KeyCode::KeyZ,
        ]),
        jump_held: any_key(&[
            KeyCode::Space,
            KeyCode::ArrowUp,
            KeyCode::KeyW,
            KeyCode::KeyZ,
        ]),
    };
    let mut recapture = keys.just_pressed(KeyCode::KeyB);
    for pad in &pads {
        let stick = pad.left_stick().x;
        let dpad = pad.pressed(GamepadButton::DPadRight) as i8 as f32
            - pad.pressed(GamepadButton::DPadLeft) as i8 as f32;
        if stick.abs() > 0.25 || dpad != 0.0 {
            input.run = if dpad != 0.0 { dpad } else { stick };
        }
        input.jump_pressed |= pad.just_pressed(GamepadButton::South);
        input.jump_held |= pad.pressed(GamepadButton::South);
        recapture |=
            pad.just_pressed(GamepadButton::East) || pad.just_pressed(GamepadButton::Select);
    }

    let now = time.elapsed_secs();
    if recapture && matches!(play.phase, Phase::Playing | Phase::Capturing) {
        play.phase = Phase::Countdown {
            until: now + COUNTDOWN,
        };
        play.body = None;
    }

    let (transform, sprite, visibility) = &mut *player;
    let Play {
        seg,
        body,
        run_clock,
        ..
    } = &mut *play;
    let Some(body) = body else {
        **visibility = Visibility::Hidden;
        return;
    };
    body.step(&seg.mask, input, time.delta_secs());
    *run_clock += time.delta_secs();
    let pose = body.pose();
    let stride = ((*run_clock * 8.0) as u32).is_multiple_of(2);
    sprite.image = art.poses[match pose {
        Pose::Idle => 0,
        Pose::Run if stride => 1,
        Pose::Run => 2,
        Pose::Jump => 3,
        Pose::Fall => 4,
    }]
    .clone();
    sprite.flip_x = body.facing_left;
    **visibility = Visibility::Visible;
    transform.translation.x = body.x.round() + body::WIDTH as f32 / 2.0 - MASK.0 as f32 / 2.0;
    transform.translation.y = MASK.1 as f32 / 2.0 - body.y.round() - body::HEIGHT as f32 / 2.0;
}

fn hint(play: Res<Play>, feed: NonSend<Feed>, time: Res<Time>, mut shown: Local<String>) {
    let text = match (feed.webcam.failure(), play.phase) {
        (Some(why), _) => why,
        (None, Phase::WaitingCamera) => String::new(),
        (None, Phase::Countdown { until }) => {
            format!(
                "Step out of view · {}",
                (until - time.elapsed_secs()).ceil().max(1.0)
            )
        }
        (None, Phase::Capturing) => "Hold still".into(),
        (None, Phase::Playing) => "B: recapture the empty scene".into(),
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
