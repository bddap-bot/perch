use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement};

#[wasm_bindgen(module = "/web/camera.js")]
extern "C" {
    #[wasm_bindgen(js_name = openCamera)]
    fn open_camera(video: &HtmlVideoElement) -> js_sys::Promise;
    #[wasm_bindgen(js_name = lockCamera)]
    fn lock_camera(video: &HtmlVideoElement);
    #[wasm_bindgen(js_name = releaseCamera)]
    fn release_camera(video: &HtmlVideoElement);
}

#[wasm_bindgen(module = "/web/face.js")]
extern "C" {
    fn eyes(video: &HtmlVideoElement) -> Vec<f64>;
}

pub enum Status {
    Waiting,
    Live,
    Failed(String),
}

pub struct Webcam {
    video: HtmlVideoElement,
    status: Rc<RefCell<Status>>,
    frames_seen: u32,
}

pub struct Surface {
    ctx: CanvasRenderingContext2d,
    width: u32,
    height: u32,
}

impl Webcam {
    pub fn start() -> Self {
        let document = web_sys::window().unwrap().document().unwrap();
        let video: HtmlVideoElement = document
            .get_element_by_id("camera")
            .unwrap()
            .unchecked_into();
        let status = Rc::new(RefCell::new(Status::Waiting));
        let (v, s) = (video.clone(), status.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let next = match wasm_bindgen_futures::JsFuture::from(open_camera(&v)).await {
                Ok(_) => Status::Live,
                Err(e) => Status::Failed(describe(&e)),
            };
            *s.borrow_mut() = next;
        });
        Self {
            video,
            status,
            frames_seen: 0,
        }
    }

    pub fn waiting(&self) -> bool {
        matches!(*self.status.borrow(), Status::Waiting)
    }

    pub fn failure(&self) -> Option<String> {
        match &*self.status.borrow() {
            Status::Failed(why) => Some(why.clone()),
            _ => None,
        }
    }

    pub fn lock(&self) {
        lock_camera(&self.video);
    }

    pub fn release(&self) {
        release_camera(&self.video);
    }

    pub fn new_frame(&mut self) -> bool {
        if !matches!(*self.status.borrow(), Status::Live) || self.video.ready_state() < 2 {
            return false;
        }
        let frames = self.video.get_video_playback_quality().total_video_frames();
        if frames == self.frames_seen {
            return false;
        }
        self.frames_seen = frames;
        true
    }

    fn crop(&self, w: f64, h: f64) -> [f64; 4] {
        let (vw, vh) = (
            self.video.video_width() as f64,
            self.video.video_height() as f64,
        );
        let (sw, sh) = if vw / vh > w / h {
            (vh * w / h, vh)
        } else {
            (vw, vw * h / w)
        };
        [(vw - sw) / 2.0, (vh - sh) / 2.0, sw, sh]
    }

    pub fn read(&self, surface: &Surface) -> Vec<u8> {
        let (w, h) = (surface.width as f64, surface.height as f64);
        let [sx, sy, sw, sh] = self.crop(w, h);
        let ctx = &surface.ctx;
        ctx.set_transform(-1.0, 0.0, 0.0, 1.0, w, 0.0).unwrap();
        ctx.draw_image_with_html_video_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &self.video,
            sx,
            sy,
            sw,
            sh,
            0.0,
            0.0,
            w,
            h,
        )
        .unwrap();
        ctx.get_image_data(0.0, 0.0, w, h).unwrap().data().0
    }

    pub fn eyes(&self, surface: &Surface) -> Option<(f32, f32)> {
        let &[x, y] = eyes(&self.video).as_slice() else {
            return None;
        };
        let (w, h) = (surface.width as f64, surface.height as f64);
        let [sx, sy, sw, sh] = self.crop(w, h);
        Some(((w - (x - sx) / sw * w) as f32, ((y - sy) / sh * h) as f32))
    }
}

impl Surface {
    pub fn new(width: u32, height: u32) -> Self {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas: HtmlCanvasElement = document.create_element("canvas").unwrap().unchecked_into();
        canvas.set_width(width);
        canvas.set_height(height);
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"willReadFrequently".into(), &true.into()).unwrap();
        let ctx = canvas
            .get_context_with_context_options("2d", &options)
            .unwrap()
            .unwrap()
            .unchecked_into();
        Self { ctx, width, height }
    }
}

fn describe(e: &JsValue) -> String {
    let name = js_sys::Reflect::get(e, &"name".into())
        .ok()
        .and_then(|n| n.as_string())
        .unwrap_or_default();
    match name.as_str() {
        "NotAllowedError" | "SecurityError" => {
            "Camera access was blocked. Allow it for this page and reload.".into()
        }
        "NotFoundError" => "No camera found.".into(),
        "NotReadableError" => "The camera is busy in another app.".into(),
        _ => format!("Camera unavailable: {}", e.as_string().unwrap_or(name)),
    }
}
