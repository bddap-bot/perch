use bevy::asset::RenderAssetUsages;
use bevy::camera::ScalingMode;
// The derive macros name bevy_ecs directly when bevy is only a target-specific dependency.
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;
use perch::bird::LOW_AIR;
use perch::body::{HEIGHT, MAX_STEP};
use perch::play::{Loss, MAX_BADDIES, MAX_POOFS, POOF_SECONDS, Round};
use perch::seg::{Mask, Segmenter};
use perch::sprite::{self, Frame};

use crate::webcam::{Surface, Webcam};

const MASK: (u32, u32) = (240, 180);
const COUNTDOWN: f32 = 3.0;
const STALL: f32 = 3.0;
const RESTART: f32 = 3.0;
const EYE_FOLLOW: f32 = 8.0;
const EYES_LOST: f32 = 0.5;
const TINT: [u8; 4] = [70, 230, 255, 26];
const EDGE: [u8; 4] = [200, 250, 255, 160];
const STALE_AIR: [u8; 3] = [255, 96, 72];
const BUBBLES: usize = 5;

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
            mask_dirty: false,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (feed, recapture, play, draw, hint).chain())
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
    Capturing,
    Playing { round: Box<Round>, lost_at: f32 },
}

#[derive(Resource)]
struct Game {
    seg: Segmenter,
    phase: Phase,
    mask_dirty: bool,
}

#[derive(Resource)]
struct Art {
    mask: Handle<Image>,
    bird: Vec<Handle<Image>>,
    baddie: [Handle<Image>; 2],
}

#[derive(Component, Clone, Copy)]
enum Role {
    Bird,
    Baddie(usize),
    Poof(usize),
    Bubble(usize),
}

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

fn sized(images: &mut Assets<Image>, (w, h): (usize, usize), data: Vec<u8>) -> Handle<Image> {
    images.add(image(w as u32, h as u32, data))
}

fn seed() -> u32 {
    (js_sys::Math::random() * u32::MAX as f64) as u32
}

fn world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x - MASK.0 as f32 / 2.0, MASK.1 as f32 / 2.0 - y)
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
    let mask = images.add(image(
        MASK.0,
        MASK.1,
        vec![0; (MASK.0 * MASK.1 * 4) as usize],
    ));
    let bird: Vec<_> = Frame::all()
        .map(|f| sized(&mut images, sprite::BIRD, sprite::bird(f)))
        .collect();
    let baddie = [0, 1].map(|s| sized(&mut images, sprite::BADDIE, sprite::baddie(s)));
    let puff = sized(&mut images, sprite::PUFF, sprite::puff());
    let bubble = sized(&mut images, sprite::BUBBLE, sprite::bubble());
    commands.spawn(Sprite {
        image: mask.clone(),
        custom_size: Some(Vec2::new(MASK.0 as f32, MASK.1 as f32)),
        ..default()
    });
    let mut slot = |role: Role, image: &Handle<Image>, size: Vec2, z: f32, anchor: Anchor| {
        commands.spawn((
            role,
            Sprite {
                image: image.clone(),
                custom_size: Some(size),
                ..default()
            },
            anchor,
            Transform::from_xyz(0.0, 0.0, z),
            Visibility::Hidden,
        ));
    };
    let half = |(w, h): (usize, usize)| Vec2::new(w as f32 / 2.0, h as f32 / 2.0);
    slot(
        Role::Bird,
        &bird[0],
        half(sprite::BIRD),
        3.0,
        Anchor::BOTTOM_CENTER,
    );
    for i in 0..MAX_BADDIES {
        slot(
            Role::Baddie(i),
            &baddie[0],
            half(sprite::BADDIE),
            2.0,
            Anchor::CENTER,
        );
    }
    for i in 0..MAX_POOFS {
        slot(
            Role::Poof(i),
            &puff,
            half(sprite::PUFF),
            4.0,
            Anchor::CENTER,
        );
    }
    for i in 0..BUBBLES {
        slot(
            Role::Bubble(i),
            &bubble,
            half(sprite::BUBBLE),
            4.0,
            Anchor::CENTER,
        );
    }
    commands.insert_resource(Art { mask, bird, baddie });
}

fn feed(mut camera: NonSendMut<Camera>, mut game: ResMut<Game>, time: Res<Time>) {
    let now = time.elapsed_secs();
    if !camera.webcam.new_frame() {
        return;
    }
    camera.last_frame = now;
    let frame = camera.webcam.read(&camera.surface);

    let Game {
        seg,
        phase,
        mask_dirty,
    } = &mut *game;
    *mask_dirty = true;
    match phase {
        Phase::WaitingCamera => {
            *phase = Phase::Countdown {
                until: now + COUNTDOWN,
            }
        }
        Phase::Countdown { until } if now >= *until => {
            camera.webcam.lock();
            seg.begin_capture();
            seg.add_capture_frame(&frame);
            *phase = Phase::Capturing;
        }
        Phase::Countdown { .. } => {}
        Phase::Capturing => {
            if seg.add_capture_frame(&frame) {
                *phase = Phase::Playing {
                    round: Box::new(Round::new(&seg.mask, seed())),
                    lost_at: 0.0,
                };
            }
        }
        Phase::Playing { round, .. } => {
            seg.segment(&frame, camera.surface.person().as_deref());
            round.observe(&seg.mask);
        }
    }
}

fn recapture(
    mut game: ResMut<Game>,
    camera: NonSend<Camera>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    time: Res<Time>,
) {
    if !(mouse.just_pressed(MouseButton::Left) || touches.any_just_pressed()) {
        return;
    }
    let Game { seg, phase, .. } = &mut *game;
    if matches!(phase, Phase::Playing { .. } | Phase::Capturing) {
        seg.begin_capture();
        camera.webcam.release();
        *phase = Phase::Countdown {
            until: time.elapsed_secs() + COUNTDOWN,
        };
    }
}

#[derive(Default)]
struct EyeTrack {
    at: Option<(f32, f32)>,
    seen: f32,
}

fn play(
    mut game: ResMut<Game>,
    camera: NonSend<Camera>,
    time: Res<Time>,
    mut eyes: Local<EyeTrack>,
) {
    let now = time.elapsed_secs();
    match (camera.webcam.eyes(&camera.surface), eyes.at) {
        (Some(raw), Some(at)) => {
            let k = 1.0 - (-time.delta_secs() * EYE_FOLLOW).exp();
            eyes.at = Some((at.0 + (raw.0 - at.0) * k, at.1 + (raw.1 - at.1) * k));
            eyes.seen = now;
        }
        (Some(raw), None) => {
            eyes.at = Some(raw);
            eyes.seen = now;
        }
        (None, _) if now - eyes.seen > EYES_LOST => eyes.at = None,
        (None, _) => {}
    }
    let Game { seg, phase, .. } = &mut *game;
    let Phase::Playing { round, lost_at } = phase else {
        return;
    };
    let was_lost = round.lost.is_some();
    let mut left = time.delta_secs().min(0.25);
    while left > 0.0 {
        let dt = left.min(MAX_STEP);
        round.step(&seg.mask, eyes.at, dt);
        left -= dt;
    }
    match (was_lost, round.lost.is_some()) {
        (false, true) => *lost_at = now,
        (true, _) if now - *lost_at > RESTART => **round = Round::new(&seg.mask, seed()),
        _ => {}
    }
}

fn paint(data: &mut [u8], mask: &Mask, round: Option<&Round>) {
    for (i, px) in data.chunks_exact_mut(4).enumerate() {
        let (x, y) = ((i % mask.width) as i32, (i / mask.width) as i32);
        px.copy_from_slice(&match (mask.solid(x, y), mask.solid(x, y - 1)) {
            (true, false) => EDGE,
            (true, true) => TINT,
            _ => [0; 4],
        });
    }
    if let Some(round) = round
        && !round.air.breathes()
    {
        let a = 0.1 + 0.25 * (1.0 - round.breath);
        let px = STALE_AIR.map(|c| (c as f32 * a) as u8);
        for i in round.air.pocket() {
            data[i * 4..i * 4 + 4].copy_from_slice(&[px[0], px[1], px[2], (a * 255.0) as u8]);
        }
    }
}

fn draw(
    mut game: ResMut<Game>,
    art: Res<Art>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
    mut tinted: Local<bool>,
    mut slots: Query<(&Role, &mut Transform, &mut Sprite, &mut Visibility)>,
) {
    let t = time.elapsed_secs();
    let Game {
        seg,
        phase,
        mask_dirty,
    } = &mut *game;
    let round = match phase {
        Phase::Playing { round, .. } => Some(&**round),
        _ => None,
    };
    let sealed = round.is_some_and(|r| !r.air.breathes());
    if (*mask_dirty || sealed || *tinted)
        && let Some(mut img) = images.get_mut(&art.mask)
        && let Some(data) = img.data.as_mut()
    {
        paint(data, &seg.mask, round);
        *mask_dirty = false;
        *tinted = sealed;
    }

    for (role, mut transform, mut sprite, mut visibility) in &mut slots {
        *visibility = Visibility::Hidden;
        let Some(round) = round else {
            continue;
        };
        match *role {
            Role::Bird => {
                let look = round.look;
                let puffed = if look.frame.puffed { 1.1 } else { 1.0 };
                let shake = Vec2::new((t * 53.0).sin(), (t * 47.0).cos() * 0.5) * look.shake;
                let feet = world(round.body.centre().0, round.body.y + HEIGHT as f32);
                sprite.image = art.bird[look.frame.index()].clone();
                sprite.flip_x = look.face_left;
                transform.translation = (feet.round() + shake).extend(transform.translation.z);
                transform.rotation = Quat::from_rotation_z(if look.face_left {
                    look.tilt
                } else {
                    -look.tilt
                });
                transform.scale = Vec3::new(
                    (1.0 + look.squash * 0.6) * puffed,
                    (1.0 - look.squash) * puffed,
                    1.0,
                );
            }
            Role::Baddie(i) => {
                let Some(b) = round.baddies.get(i) else {
                    continue;
                };
                let step = ((t * 7.0 + i as f32 * 0.37) as usize) % 2;
                let bob = (t * 9.0 + i as f32).sin() * 0.6;
                sprite.image = art.baddie[step].clone();
                sprite.flip_x = b.facing_left;
                transform.translation = world(b.x, b.y + bob).extend(transform.translation.z);
            }
            Role::Poof(i) => {
                let Some(p) = round.poofs.get(i) else {
                    continue;
                };
                let k = p.age / POOF_SECONDS;
                sprite.color = Color::srgba(1.0, 1.0, 1.0, 1.0 - k);
                transform.translation = world(p.x, p.y - k * 4.0).extend(transform.translation.z);
                transform.scale = Vec3::splat(1.0 + k * 1.5);
            }
            Role::Bubble(i) => {
                if round.breath >= 1.0 || round.lost.is_some() {
                    continue;
                }
                let left = (round.breath * BUBBLES as f32).ceil() as usize;
                if i >= left {
                    continue;
                }
                let x = round.body.centre().0 + (i as f32 - (BUBBLES - 1) as f32 / 2.0) * 2.5;
                let wobble = (t * 6.0 + i as f32 * 1.3).sin() * 0.4;
                transform.translation =
                    world(x, round.body.y - 3.0 + wobble).extend(transform.translation.z);
                sprite.color = if round.breath < LOW_AIR {
                    Color::srgb(1.0, 0.55, 0.5)
                } else {
                    Color::WHITE
                };
            }
        }
        *visibility = Visibility::Visible;
    }
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
        match &game.phase {
            Phase::WaitingCamera => String::new(),
            Phase::Countdown { until } => {
                format!("Step out of view · {}", (until - now).ceil().max(1.0))
            }
            Phase::Capturing => "Hold still".into(),
            Phase::Playing { round, .. } => {
                let secs = round.time as u32;
                match round.lost {
                    None => format!("{secs} s"),
                    Some(Loss::Caught) => format!("Caught · {secs} s"),
                    Some(Loss::Suffocated) => format!("Out of air · {secs} s"),
                }
            }
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
