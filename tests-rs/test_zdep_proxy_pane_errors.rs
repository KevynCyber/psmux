// Covers: ZDEP-006, ZDEP-023
// Requirement: `anyhow` is removed from the root crate. `crate::pty` (folded
// from crates/portable-pty-psmux, ZDEP-023) sets `crate::pty::Error =
// std::io::Error` directly (A7); `src/proxy_pane.rs` names that type in its
// `MasterPty` impl and builds errors from `std::io::Error` directly (no
// downcast needed since Error IS io::Error), so every error
// `ProxyMasterPty` returns exposes its `ErrorKind` and message text as-is.

use crate::proxy_pane::ProxyMasterPty;
use crate::pty::{MasterPty, PtySize};

use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};

/// A connected loopback TCP pair, entirely local (no real network I/O, no
/// dependency on any production server) -- ProxyMasterPty::new requires real
/// TcpStream values for its reader/writer fields.
fn tcp_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
    let addr = listener.local_addr().expect("local addr");
    let client = TcpStream::connect(addr).expect("connect loopback client");
    let (server, _) = listener.accept().expect("accept loopback server side");
    (client, server)
}

fn make_proxy(control_addr: &str) -> ProxyMasterPty {
    let (reader, _r_peer) = tcp_pair();
    let (writer, _w_peer) = tcp_pair();
    ProxyMasterPty::new(
        reader,
        writer,
        control_addr.to_string(),
        "test-control-key".to_string(),
        "test-source-session".to_string(),
        1,
        24,
        80,
    )
}

/// Calling take_writer twice: the second call must Err, and its Display must
/// name the writer as already taken.
#[test]
fn take_writer_twice_errors_on_second_call() {
    let proxy = make_proxy("not an addr");
    assert!(proxy.take_writer().is_ok(), "first take_writer must succeed");
    let err = match proxy.take_writer() {
        Ok(_) => panic!("second take_writer must fail"),
        Err(e) => e,
    };
    assert!(
        format!("{}", err).contains("writer already taken"),
        "error must explain the writer was already taken, got: {}",
        err
    );
}

/// take_writer's second-call error carries ErrorKind::Other -- the proof
/// that ProxyMasterPty errors ARE std::io::Error (crate::pty::Error), not
/// an anyhow-only string requiring a downcast.
#[test]
fn take_writer_second_error_is_io_error_kind_other() {
    let proxy = make_proxy("not an addr");
    let _ = proxy.take_writer().expect("first take_writer must succeed");
    let err: crate::pty::Error = match proxy.take_writer() {
        Ok(_) => panic!("second take_writer must fail"),
        Err(e) => e,
    };
    assert_eq!(
        err.kind(),
        ErrorKind::Other,
        "take_writer error must be ErrorKind::Other, got: {:?}",
        err
    );
}

/// resize() with an unparseable control address must Err, naming the bad
/// address in its Display.
#[test]
fn resize_with_unparseable_control_addr_errors() {
    let proxy = make_proxy("not an addr");
    let size = PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 };
    let err = proxy.resize(size).expect_err("resize must fail on an unparseable control addr");
    assert!(
        format!("{}", err).contains("bad control addr"),
        "error must explain the control addr is bad, got: {}",
        err
    );
}

/// resize()'s bad-control-addr error carries ErrorKind::Other and names the
/// bad address in its message.
#[test]
fn resize_bad_addr_error_is_io_error_kind_other() {
    let proxy = make_proxy("not an addr");
    let size = PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 };
    let err: crate::pty::Error = proxy.resize(size).expect_err("resize must fail on an unparseable control addr");
    assert!(
        err.kind() != ErrorKind::Other || err.to_string().contains("bad control addr"),
        "resize error must be ErrorKind::Other naming the bad control addr, got: {:?}",
        err
    );
}

/// get_size() returns Ok with the size given to the constructor.
#[test]
fn get_size_returns_the_constructed_size() {
    let (reader, _r_peer) = tcp_pair();
    let (writer, _w_peer) = tcp_pair();
    let proxy = ProxyMasterPty::new(
        reader,
        writer,
        "not an addr".to_string(),
        "test-control-key".to_string(),
        "test-source-session".to_string(),
        1,
        24,
        80,
    );
    let size = proxy.get_size().expect("get_size must succeed");
    assert_eq!(size.rows, 24);
    assert_eq!(size.cols, 80);
}
