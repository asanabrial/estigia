//! A fresh empty directory for one reviewer run, removed when it is dropped.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    pub(super) fn new() -> std::io::Result<Self> {
        let unique = format!(
            "estigia-reviewer-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        );
        let dir = std::env::temp_dir().join(unique);
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&dir)?;
        Ok(Self { dir })
    }

    pub(super) fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
