// Covers: ZDEP-006
// Requirement: `anyhow` is removed from the root crate. `crates/portable-pty-psmux`
// re-exports its error type as `portable_pty::Error`; `src/proxy_pane.rs` names
// that type in its `MasterPty` impl and builds errors from `std::io::Error`
// (converted with `into()`), so every error `ProxyMasterPty` returns downcasts
// to `std::io::Error`.

use crate::proxy_pane::ProxyMasterPty;
use portable_pty::{MasterPty, PtySize};

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
    let err = proxy.take_writer().expect_err("second take_writer must fail");
    assert!(
        format!("{}", err).contains("writer already taken"),
        "error must explain the writer was already taken, got: {}",
        err
    );
}

/// take_writer's second-call error downcasts to std::io::Error -- the proof
/// that ProxyMasterPty errors are io::Error behind portable_pty::Error, not
/// an anyhow-only string.
#[test]
fn take_writer_second_error_downcasts_to_io_error() {
    let proxy = make_proxy("not an addr");
    let _ = proxy.take_writer().expect("first take_writer must succeed");
    let err: portable_pty::Error = proxy.take_writer().expect_err("second take_writer must fail");
    assert!(
        err.downcast_ref::<std::io::Error>().is_some(),
        "take_writer error must downcast to std::io::Error, got: {:?}",
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

/// resize()'s bad-control-addr error downcasts to std::io::Error.
#[test]
fn resize_bad_addr_error_downcasts_to_io_error() {
    let proxy = make_proxy("not an addr");
    let size = PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 };
    let err: portable_pty::Error = proxy.resize(size).expect_err("resize must fail on an unparseable control addr");
    assert!(
        err.downcast_ref::<std::io::Error>().is_some(),
        "resize error must downcast to std::io::Error, got: {:?}",
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
