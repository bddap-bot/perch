use wasm_bindgen::prelude::wasm_bindgen;

use crate::seg::{DepthEstimate, Segmenter};

#[wasm_bindgen]
pub struct WorkerSegmenter(Segmenter);

#[wasm_bindgen]
impl WorkerSegmenter {
    #[wasm_bindgen(constructor)]
    pub fn new(width: usize, height: usize) -> WorkerSegmenter {
        WorkerSegmenter(Segmenter::new(width, height))
    }

    pub fn forget(&mut self) {
        self.0.forget();
    }

    pub fn capture(&mut self) {
        self.0.begin_capture();
    }

    pub fn has_background(&self) -> bool {
        self.0.has_background()
    }

    pub fn depth_background(&mut self) -> Option<Vec<u8>> {
        self.0.depth_background()
    }

    pub fn observe_depth(&mut self, frame: Vec<f32>, background: Vec<f32>, pixels: Vec<u8>) {
        self.0.observe_depth(DepthEstimate {
            frame,
            background,
            pixels,
        });
    }

    /// True on the frame that completes a capture.
    pub fn frame(&mut self, rgba: &[u8], person: Option<Vec<u8>>) -> bool {
        if self.0.has_background() {
            self.0.segment(rgba, person.as_deref());
            return false;
        }
        self.0.add_capture_frame(rgba)
    }

    pub fn mask(&self) -> Vec<u8> {
        self.0.mask.cells.iter().map(|&c| c as u8).collect()
    }
}
