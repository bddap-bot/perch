use perch::body::Pose;
use perch::sprite::{SIZE, frame};

#[test]
fn every_pose_is_a_full_frame() {
    for pose in [Pose::Idle, Pose::Run, Pose::Jump, Pose::Fall] {
        for phase in [false, true] {
            assert_eq!(frame(pose, phase).len(), SIZE.0 * SIZE.1 * 4);
        }
    }
}
