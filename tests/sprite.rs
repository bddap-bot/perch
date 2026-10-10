use perch::sprite::{self, Frame};

#[test]
fn all_lists_every_frame_at_its_index() {
    let frames: Vec<Frame> = Frame::all().collect();
    assert_eq!(frames.len(), Frame::COUNT);
    for (i, frame) in frames.into_iter().enumerate() {
        assert_eq!(frame.index(), i);
    }
}

#[test]
fn every_image_is_full_size() {
    let size = |(w, h): (usize, usize)| w * h * 4;
    for frame in Frame::all() {
        assert_eq!(sprite::bird(frame).len(), size(sprite::BIRD));
    }
    for step in 0..2 {
        assert_eq!(sprite::baddie(step).len(), size(sprite::BADDIE));
    }
    assert_eq!(sprite::puff().len(), size(sprite::PUFF));
    assert_eq!(sprite::bubble().len(), size(sprite::BUBBLE));
}
