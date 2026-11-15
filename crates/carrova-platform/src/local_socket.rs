//! Same-user local sockets: [`LocalListener`] and [`LocalStream`], the
//! types of [`crate::Platform::bind_local_socket`].

use std::io::{self, Read, Write};
use std::path::Path;

use crate::PlatformError;

#[cfg(target_os = "linux")]
use std::os::unix::net::{UnixListener as RawListener, UnixStream as RawStream};
// Other OSes have no local sockets yet: these are never constructed.
#[cfg(not(target_os = "linux"))]
use std::fs::File as RawStream;
#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
struct RawListener;

/// A listening local socket that only serves its owner's uid.
#[derive(Debug)]
pub struct LocalListener {
    inner: RawListener,
    /// The uid every peer must have.
    owner: u32,
}

/// A connection over a local socket; frame it like any byte stream.
#[derive(Debug)]
pub struct LocalStream(RawStream);

impl LocalListener {
    /// Listen at `path` (mode `0600`) for peers running as `owner`.
    #[cfg(target_os = "linux")]
    pub(crate) fn bind(path: &Path, owner: u32) -> Result<Self, PlatformError> {
        use std::os::unix::fs::PermissionsExt;

        let inner = RawListener::bind(path).map_err(|e| PlatformError::io(path, e))?;
        // Other users can connect in the instant before this, but `accept`
        // refuses them by uid anyway.
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| PlatformError::io(path, e))?;
        Ok(Self { inner, owner })
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn bind(_: &Path, _: u32) -> Result<Self, PlatformError> {
        Err(PlatformError::Unsupported("bind_local_socket"))
    }

    /// Wait for a peer. A peer of another uid is refused with
    /// [`io::ErrorKind::PermissionDenied`]: call again for the next one.
    #[cfg(target_os = "linux")]
    pub fn accept(&self) -> io::Result<LocalStream> {
        let (stream, _) = self.inner.accept()?;
        let peer = rustix::net::sockopt::socket_peercred(&stream)?.uid.as_raw();
        if peer == self.owner {
            Ok(LocalStream(stream))
        } else {
            Err(io::ErrorKind::PermissionDenied.into())
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn accept(&self) -> io::Result<LocalStream> {
        let _ = (&self.inner, self.owner);
        Err(io::ErrorKind::Unsupported.into())
    }
}

impl LocalStream {
    /// Connect to the socket at `path`.
    #[cfg(target_os = "linux")]
    pub(crate) fn connect(path: &Path) -> Result<Self, PlatformError> {
        RawStream::connect(path)
            .map(Self)
            .map_err(|e| PlatformError::io(path, e))
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn connect(_: &Path) -> Result<Self, PlatformError> {
        Err(PlatformError::Unsupported("connect_local_socket"))
    }
}

impl Read for LocalStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for LocalStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
