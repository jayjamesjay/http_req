//!secure connection over TLS

use crate::error::Error as HttpError;
use std::io;

#[cfg(not(any(feature = "native-tls", feature = "rust-tls")))]
compile_error!("one of the `native-tls` or `rust-tls` features must be enabled");

///wrapper around TLS Stream,
///depends on selected TLS library
pub struct Conn<S: io::Read + io::Write> {
    #[cfg(feature = "native-tls")]
    stream: native_tls::TlsStream<S>,

    #[cfg(feature = "rust-tls")]
    stream: rustls::StreamOwned<rustls::ClientSession, S>,
}

impl<S: io::Read + io::Write> io::Read for Conn<S> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let len = self.stream.read(buf);

        #[cfg(feature = "rust-tls")]
        {
            // TODO: this api returns ConnectionAborted with a "..CloseNotify.." string.
            // TODO: we should work out if self.stream.sess exposes enough information
            // TODO: to not read in this situation, and return EOF directly.
            // TODO: c.f. the checks in the implementation. connection_at_eof() doesn't
            // TODO: seem to be exposed. The implementation:
            // TODO: https://github.com/ctz/rustls/blob/f93c325ce58f2f1e02f09bcae6c48ad3f7bde542/src/session.rs#L789-L792
            if let Err(ref e) = len {
                if io::ErrorKind::ConnectionAborted == e.kind() {
                    return Ok(0);
                }
            }
        }

        len
    }
}

impl<S: io::Read + io::Write> io::Write for Conn<S> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        self.stream.flush()
    }
}

///client configuration
pub struct Config {
    #[cfg(feature = "rust-tls")]
    client_config: std::sync::Arc<rustls::ClientConfig>,
}

impl Default for Config {
    #[cfg(feature = "native-tls")]
    fn default() -> Self {
        Config {}
    }

    #[cfg(feature = "rust-tls")]
    fn default() -> Self {
        let mut config = rustls::ClientConfig::new();
        config
            .root_store
            .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

        Config {
            client_config: std::sync::Arc::new(config),
        }
    }
}

impl Config {
    #[cfg(feature = "native-tls")]
    pub fn connect<H, S>(&self, hostname: H, stream: S) -> Result<Conn<S>, HttpError>
    where
        H: AsRef<str>,
        S: io::Read + io::Write,
    {
        let connector = native_tls::TlsConnector::new()?;
        let stream = connector.connect(hostname.as_ref(), stream)?;

        Ok(Conn { stream })
    }

    #[cfg(feature = "rust-tls")]
    pub fn connect<H, S>(&self, hostname: H, stream: S) -> Result<Conn<S>, HttpError>
    where
        H: AsRef<str>,
        S: io::Read + io::Write,
    {
        use rustls::{ClientSession, StreamOwned};

        let session = ClientSession::new(
            &self.client_config,
            webpki::DNSNameRef::try_from_ascii_str(hostname.as_ref())
                .map_err(|()| HttpError::Tls)?,
        );
        let stream = StreamOwned::new(session, stream);

        Ok(Conn { stream })
    }
}
