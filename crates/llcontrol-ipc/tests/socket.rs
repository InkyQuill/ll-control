use std::path::{Path, PathBuf};

#[test]
fn shared_socket_location() {
    assert_eq!(
        llcontrol_ipc::socket::socket_path_from_runtime(Path::new("/run/user/1000")),
        PathBuf::from("/run/user/1000/llcontrold.sock")
    );
}
