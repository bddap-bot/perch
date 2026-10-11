use std::cell::RefCell;
use std::rc::Rc;

use perch::seg::Mask;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlVideoElement;

#[wasm_bindgen(module = "/web/camera.js")]
extern "C" {
    #[wasm_bindgen(js_name = openCamera)]
    fn open_camera(video: &HtmlVideoElement) -> js_sys::Promise;
    #[wasm_bindgen(js_name = lockCamera)]
    fn lock_camera(video: &HtmlVideoElement);
    #[wasm_bindgen(js_name = releaseCamera)]
    fn release_camera(video: &HtmlVideoElement);
}

#[wasm_bindgen(module = "/web/models.js")]
extern "C" {
    fn eyes(video: &HtmlVideoElement) -> Vec<f64>;
}

#[wasm_bindgen(module = "/web/vision.js")]
extern "C" {
    #[wasm_bindgen(js_name = visionSize)]
    fn vision_size(width: u32, height: u32);
    #[wasm_bindgen(js_name = visionReset)]
    fn vision_reset();
    #[wasm_bindgen(js_name = visionCapture)]
    fn vision_capture();
    #[wasm_bindgen(js_name = visionFeed)]
    fn vision_feed(video: &HtmlVideoElement, crop: &[f64]);
    #[wasm_bindgen(js_name = visionResult)]
    fn vision_result() -> JsValue;
}

pub enum Status {
    Waiting,
    Live,
    Failed(String),
}

pub struct Webcam {
    video: HtmlVideoElement,
    status: Rc<RefCell<Status>>,
}

pub struct Vision {
    pub width: u32,
    pub height: u32,
}

pub struct Seen {
    pub mask: Mask,
    pub captured: bool,
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
        Self { video, status }
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

    pub fn feed(&self, vision: &Vision) {
        if matches!(*self.status.borrow(), Status::Live) && self.video.ready_state() >= 2 {
            let crop = self.crop(vision.width as f64, vision.height as f64);
            vision_feed(&self.video, &crop);
        }
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

    pub fn eyes(&self, vision: &Vision) -> Option<(f32, f32)> {
        let &[x, y] = eyes(&self.video).as_slice() else {
            return None;
        };
        let (w, h) = (vision.width as f64, vision.height as f64);
        let [sx, sy, sw, sh] = self.crop(w, h);
        Some(((w - (x - sx) / sw * w) as f32, ((y - sy) / sh * h) as f32))
    }
}

impl Vision {
    pub fn new(width: u32, height: u32) -> Self {
        vision_size(width, height);
        Self { width, height }
    }

    pub fn reset(&self) {
        vision_reset();
    }

    pub fn capture(&self) {
        vision_capture();
    }

    pub fn seen(&self) -> Option<Seen> {
        let reply = vision_result();
        if reply.is_null() {
            return None;
        }
        let field = |name: &str| js_sys::Reflect::get(&reply, &name.into()).unwrap();
        let cells = js_sys::Uint8Array::new(&field("mask")).to_vec();
        Some(Seen {
            mask: Mask {
                width: self.width as usize,
                height: self.height as usize,
                cells: cells.into_iter().map(|c| c != 0).collect(),
            },
            captured: field("captured").is_truthy(),
        })
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
