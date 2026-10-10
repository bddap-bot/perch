use perch::sprite::{Frame, SIZE, pixels};

#[test]
fn all_lists_every_frame_at_its_index() {
    for (i, frame) in Frame::ALL.into_iter().enumerate() {
        assert_eq!(frame as usize, i);
    }
}

#[test]
fn every_frame_is_full_size() {
    for frame in Frame::ALL {
        assert_eq!(pixels(frame).len(), SIZE.0 * SIZE.1 * 4);
    }
}
