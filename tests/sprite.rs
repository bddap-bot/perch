use perch::sprite::{Frame, SIZE, pixels};

#[test]
fn every_frame_is_full_size() {
    for frame in Frame::ALL {
        assert_eq!(pixels(frame).len(), SIZE.0 * SIZE.1 * 4);
    }
}
