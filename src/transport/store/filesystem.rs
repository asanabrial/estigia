use std::path::Path;

use super::Failure;

/// The filesystem kinds that cannot keep the locking and WAL the store needs
/// when they are reached over a network.
const NETWORK_KINDS: &[&str] = &[
    "nfs", "nfs4", "smbfs", "cifs", "smb2", "afpfs", "webdav", "9p", "afs", "coda",
];

/// Whether a filesystem kind is one a store must not be opened on.
///
/// Pure and fed, so the classification is testable without a network mount.
pub(super) fn is_network_filesystem(kind: &str) -> bool {
    NETWORK_KINDS
        .iter()
        .any(|known| kind.eq_ignore_ascii_case(known))
}

/// The refusal a store on a network filesystem answers with.
pub(super) fn store_unwritable(kind: &str, at: &Path) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "store-unwritable",
        "detail": format!(
            "the local store's directory {} sits on a {kind} filesystem, which cannot keep the \
             locking and WAL the store needs",
            at.display()
        ),
        "action": "point HOME at a local filesystem, or move ~/.estigia off the network mount",
    }))
}

#[cfg(target_os = "macos")]
pub(super) fn filesystem_kind(path: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut buffer = unsafe { std::mem::zeroed::<libc::statfs>() };
    if unsafe { libc::statfs(path.as_ptr(), &mut buffer) } != 0 {
        return None;
    }
    let name = unsafe { std::ffi::CStr::from_ptr(buffer.f_fstypename.as_ptr()) };
    name.to_str().ok().map(ToOwned::to_owned)
}

#[cfg(target_os = "linux")]
pub(super) fn filesystem_kind(path: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut buffer = unsafe { std::mem::zeroed::<libc::statfs>() };
    if unsafe { libc::statfs(path.as_ptr(), &mut buffer) } != 0 {
        return None;
    }
    kind_of(buffer.f_type as u64).map(ToOwned::to_owned)
}

#[cfg(target_os = "linux")]
pub(super) fn kind_of(magic: u64) -> Option<&'static str> {
    Some(match magic {
        0x6969 => "nfs",
        0xFF534D42 => "cifs",
        0x517B => "smbfs",
        0xFE534D42 => "smb2",
        0x5346414F => "afs",
        0x73757245 => "coda",
        0x01021997 => "9p",
        _ => return None,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn filesystem_kind(_path: &Path) -> Option<String> {
    None
}
