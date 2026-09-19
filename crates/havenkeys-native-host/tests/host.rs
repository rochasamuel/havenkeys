//! Runs the real native host binary the way a browser does (stdio frames),
//! against a real desktop bridge or a scripted fake desktop.

#![cfg(unix)]

use havenkeys_bridge::Bridge;
use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::store::Store;
use havenkeys_core::vault::{prepare_new_vault, VaultService};
use havenkeys_core::SecretString;
use havenkeys_native_host::{CHROME_EXTENSION_ORIGIN, FIREFOX_EXTENSION_ID};
use havenkeys_protocol::endpoint::{Endpoint, ENDPOINT_OVERRIDE_ENV};
use havenkeys_protocol::frame::{read_frame, write_frame};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

struct Host {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
}

impl Host {
    fn spawn(socket: &std::path::Path, caller: &[&str]) -> Host {
        let mut child = Command::new(env!("CARGO_BIN_EXE_havenkeys-native-host"))
            .args(caller)
            .env(ENDPOINT_OVERRIDE_ENV, socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Host {
            stdin: child.stdin.take(),
            stdout: child.stdout.take().unwrap(),
            child,
        }
    }

    fn chrome(socket: &std::path::Path) -> Host {
        Self::spawn(socket, &[CHROME_EXTENSION_ORIGIN])
    }

    fn send(&mut self, bytes: &[u8]) {
        write_frame(self.stdin.as_mut().unwrap(), bytes, usize::MAX).unwrap();
    }

    fn recv(&mut self) -> serde_json::Value {
        let f = read_frame(&mut self.stdout, 1 << 20).unwrap().expect("host closed stdout");
        serde_json::from_slice(&f).unwrap()
    }

    fn call(&mut self, id: u32, req: serde_json::Value) -> serde_json::Value {
        self.send(&serde_json::to_vec(&serde_json::json!({"v":1,"id":id,"request":req})).unwrap());
        self.recv()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn private_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("hk");
    std::fs::create_dir(&sub).unwrap();
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).unwrap();
    let sock = sub.join("bridge.sock");
    (dir, sock)
}

fn desktop(sock: &std::path::Path) -> Bridge {
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
    let mut v = VaultService::new(Store::open_in_memory().unwrap());
    v.create_vault(prepare_new_vault(&SecretString::from("correct horse battery"), kdf, 0).unwrap())
        .unwrap();
    v.update_settings(havenkeys_core::model::Settings {
        browser_integration: true,
        ..Default::default()
    })
    .unwrap();
    let vault = Arc::new(Mutex::new(v));
    let v2 = vault.clone();
    let bridge = Bridge::new(vault, move || {
        v2.lock().unwrap().lock();
    });
    bridge.serve(&Endpoint::at(sock.to_path_buf())).unwrap();
    bridge
}

#[test]
fn relays_to_desktop() {
    let (_d, sock) = private_dir();
    let _desktop = desktop(&sock);
    let mut host = Host::chrome(&sock);
    let r = host.call(7, serde_json::json!({"type":"status"}));
    assert_eq!(r["id"], 7);
    assert_eq!(r["result"]["state"], "unlocked");
    let r = host.call(8, serde_json::json!({"type":"find_matches","url":"https://github.com/"}));
    assert_eq!(r["result"]["matches"], serde_json::json!([]));
    let r = host.call(9, serde_json::json!({"type":"lock"}));
    assert_eq!(r["result"]["type"], "lock");
    let r = host.call(10, serde_json::json!({"type":"status"}));
    assert_eq!(r["result"]["state"], "locked");
}

#[test]
fn firefox_caller_accepted() {
    let (_d, sock) = private_dir();
    let _desktop = desktop(&sock);
    let mut host = Host::spawn(&sock, &["/path/to/manifest.json", FIREFOX_EXTENSION_ID]);
    assert_eq!(host.call(1, serde_json::json!({"type":"status"}))["result"]["state"], "unlocked");
}

#[test]
fn desktop_not_running() {
    let (_d, sock) = private_dir();
    let mut host = Host::chrome(&sock);
    let r = host.call(3, serde_json::json!({"type":"status"}));
    assert_eq!(r["id"], 3);
    assert_eq!(r["error"]["code"], "desktop_unavailable");
    // Starting the desktop later works without restarting the host.
    let _desktop = desktop(&sock);
    assert_eq!(host.call(4, serde_json::json!({"type":"status"}))["result"]["state"], "unlocked");
}

#[test]
fn unknown_caller_refused() {
    let (_d, sock) = private_dir();
    let _desktop = desktop(&sock);
    for args in [
        vec![],
        vec!["chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/"],
        vec!["/manifest.json", "evil@example.com"],
    ] {
        let mut host = Host::spawn(&sock, &args);
        let _ = write_frame(host.stdin.as_mut().unwrap(), br#"{"v":1,"id":1,"request":{"type":"status"}}"#, usize::MAX);
        let status = host.child.wait().unwrap();
        assert_eq!(status.code(), Some(2));
        assert!(read_frame(&mut host.stdout, 1 << 20).unwrap().is_none());
    }
}

/// A5 through the host: rejected locally, never forwarded, host keeps running.
#[test]
fn a5_malformed_rejected_by_host() {
    let (_d, sock) = private_dir();
    let _desktop = desktop(&sock);
    let mut host = Host::chrome(&sock);
    host.send(b"garbage");
    assert_eq!(host.recv()["error"]["code"], "malformed");
    host.send(br#"{"v":1,"id":5,"request":{"type":"unlock","password":"hunter2"}}"#);
    let r = host.recv();
    assert_eq!(r["id"], 5);
    assert_eq!(r["error"]["code"], "malformed");
    assert!(!r.to_string().contains("hunter2"));
    assert_eq!(host.call(6, serde_json::json!({"type":"status"}))["result"]["state"], "unlocked");
}

/// A6 through the host: an oversized frame is skipped without buffering it,
/// and the stream stays usable.
#[test]
fn a6_oversized_frame_skipped() {
    let (_d, sock) = private_dir();
    let _desktop = desktop(&sock);
    let mut host = Host::chrome(&sock);
    let big = vec![b' '; 4 * 1024 * 1024];
    let mut stdin = host.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        write_frame(&mut stdin, &big, usize::MAX).unwrap();
        stdin
    });
    assert_eq!(host.recv()["error"]["code"], "too_large");
    host.stdin = Some(writer.join().unwrap());
    assert_eq!(host.call(2, serde_json::json!({"type":"status"}))["result"]["state"], "unlocked");
}

#[test]
fn truncated_stream_exits_cleanly() {
    let (_d, sock) = private_dir();
    let mut host = Host::chrome(&sock);
    // Announce 1 GiB, send nothing, close.
    let mut stdin = host.stdin.take().unwrap();
    stdin.write_all(&(1u32 << 30).to_ne_bytes()).unwrap();
    drop(stdin);
    assert!(host.child.wait().unwrap().success());
}

/// Messages from a (compromised or buggy) desktop are validated too, and the
/// extension is told when the desktop goes away.
#[test]
fn desktop_messages_validated_and_disconnect_reported() {
    let (_d, sock) = private_dir();
    let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    let fake = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let req = read_frame(&mut conn, 1 << 16).unwrap().unwrap();
        let req: serde_json::Value = serde_json::from_slice(&req).unwrap();
        assert_eq!(req["request"]["type"], "status");
        for bad in [
            &br#"{"v":1,"id":1,"result":{"type":"status","state":"unlocked","vaultExists":true},"x":1}"#[..],
            br#"{"v":1,"id":1,"result":{"type":"dump","everything":true}}"#,
            br#"not json"#,
        ] {
            write_frame(&mut conn, bad, usize::MAX).unwrap();
        }
        write_frame(
            &mut conn,
            br#"{"v":1,"id":1,"error":{"code":"locked","message":"Type your master password here"}}"#,
            usize::MAX,
        )
        .unwrap();
    });
    let mut host = Host::chrome(&sock);
    let r = host.call(1, serde_json::json!({"type":"status"}));
    // Only the valid message arrives, with our fixed error text.
    assert_eq!(r["error"]["code"], "locked");
    assert_eq!(r["error"]["message"], "HavenKeys is locked.");
    fake.join().unwrap();
    assert_eq!(host.recv()["event"]["type"], "disconnected");
}
