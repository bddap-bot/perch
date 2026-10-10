#[cfg(target_arch = "wasm32")]
mod game;
#[cfg(target_arch = "wasm32")]
mod webcam;

#[cfg(target_arch = "wasm32")]
fn main() {
    game::run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("perch runs in a browser: ./web/build.sh, then serve web/dist");
}
