//! MCP-12: client discovery never changes the advertised name or candidates.
use super::*;
use std::{
    fs,
    os::unix::{
        fs::{MetadataExt, PermissionsExt, symlink},
        net::UnixListener,
    },
    path::Path,
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("mcp12-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn socket(&self, endpoint: &Endpoint) -> PathBuf {
        let parent = self.0.join(endpoint.name());
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        parent.join("socket")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn uid(path: &Path) -> u32 {
    fs::metadata(path).unwrap().uid()
}
fn private(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn mcp12_candidate_order_is_bounded_absolute_and_deduplicated() {
    let endpoint = Endpoint::random();
    let paths = client_socket_paths(
        &endpoint,
        123,
        Some(PathBuf::from("/xdg")),
        PathBuf::from("/temp"),
    );
    let expected: Vec<_> = ["/xdg", "/run/user/123", "/temp", "/tmp"]
        .map(|base| Path::new(base).join(endpoint.name()).join("socket"))
        .into();
    assert_eq!(paths, expected);
    assert_eq!(
        client_socket_paths(
            &endpoint,
            123,
            Some(PathBuf::from("/tmp")),
            PathBuf::from("/tmp")
        ),
        vec![expected[3].clone(), expected[1].clone()]
    );
    for bad in [
        PathBuf::from("relative"),
        PathBuf::from(format!("/{}", "x".repeat(120))),
    ] {
        assert_eq!(
            client_socket_paths(&endpoint, 123, Some(bad), PathBuf::from("/temp")),
            expected[1..]
        );
    }
    assert_eq!(
        client_socket_paths(&endpoint, 123, None, PathBuf::from("/temp")),
        expected[1..]
    );
}

#[test]
fn mcp12_only_private_owned_socket_candidates_are_selected_and_never_changed() {
    let first = Directory::new();
    let second = Directory::new();
    let endpoint = Endpoint::random();
    let a = first.socket(&endpoint);
    let b = second.socket(&endpoint);
    let _listener = UnixListener::bind(&b).unwrap();
    private(&b);
    let owner = uid(&b);
    let candidates = vec![a.clone(), b.clone()];
    assert_eq!(select_socket(&candidates, Some(owner)).unwrap(), b);
    assert!(select_socket(&candidates, None).is_err());
    assert!(select_socket(&candidates, Some(owner.wrapping_add(1))).is_err());
    fs::write(&a, b"not a socket").unwrap();
    private(&a);
    assert_eq!(select_socket(&candidates, Some(owner)).unwrap(), b);
    fs::remove_file(&a).unwrap();
    symlink(&b, &a).unwrap();
    assert_eq!(select_socket(&candidates, Some(owner)).unwrap(), b);
    fs::remove_file(&a).unwrap();
    fs::remove_dir(a.parent().unwrap()).unwrap();
    symlink(b.parent().unwrap(), a.parent().unwrap()).unwrap();
    assert_eq!(select_socket(&candidates, Some(owner)).unwrap(), b);
    fs::remove_file(a.parent().unwrap()).unwrap();
    fs::create_dir(a.parent().unwrap()).unwrap();
    let _first = UnixListener::bind(&a).unwrap();
    private(&a);
    for mode in [0o701, 0o710, 0o777] {
        fs::set_permissions(a.parent().unwrap(), fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(select_socket(&candidates, Some(owner)).unwrap(), b);
        assert_eq!(
            fs::metadata(a.parent().unwrap()).unwrap().mode() & 0o777,
            mode
        );
    }
    fs::set_permissions(a.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
    for mode in [0o601, 0o610, 0o666] {
        fs::set_permissions(&a, fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(select_socket(&candidates, Some(owner)).unwrap(), b);
        assert_eq!(fs::metadata(&a).unwrap().mode() & 0o777, mode);
    }
    private(&a);
    assert_eq!(
        select_socket(&candidates, Some(owner)).unwrap(),
        a,
        "first safe existing socket wins"
    );
}

#[test]
fn mcp12_a_dead_first_valid_socket_does_not_retry_the_next() {
    let first = Directory::new();
    let second = Directory::new();
    let endpoint = Endpoint::random();
    let a = first.socket(&endpoint);
    let b = second.socket(&endpoint);
    drop(UnixListener::bind(&a).unwrap());
    private(&a);
    let listener = UnixListener::bind(&b).unwrap();
    private(&b);
    listener.set_nonblocking(true).unwrap();
    let selected = select_socket(&[a.clone(), b], Some(uid(&a))).unwrap();
    assert_eq!(selected, a);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert!(
        runtime
            .block_on(tokio::net::UnixStream::connect(selected))
            .is_err()
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
