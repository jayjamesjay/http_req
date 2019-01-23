use std::io;

use crate::error::Error as HttpError;

pub struct Config {
    #[cfg(feature = "rust-tls")]
    client_config: std::sync::Arc<rustls::ClientConfig>,
}

pub struct Conn<I> {
    stream: I,
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
    pub fn connect<H, S>(
        &self,
        hostname: H,
        stream: S,
    ) -> Result<Conn<native_tls::TlsStream<S>>, HttpError>
    where
        H: AsRef<str>,
        S: io::Read + io::Write,
    {
        let connector = native_tls::TlsConnector::new()?;
        let stream = connector.connect(hostname.as_ref(), stream)?;
        Ok(Conn { stream })
    }

    #[cfg(feature = "rust-tls")]
    pub fn connect<H, S>(
        &self,
        hostname: H,
        stream: S,
    ) -> Result<Conn<rustls::StreamOwned<rustls::ClientSession, S>>, HttpError>
    where
        H: AsRef<str>,
        S: io::Read + io::Write,
    {
        let session = rustls::ClientSession::new(
            &self.client_config,
            webpki::DNSNameRef::try_from_ascii_str(hostname.as_ref())
                .map_err(|()| HttpError::Tls)?,
        );
        let stream = rustls::StreamOwned::new(session, stream);
        Ok(Conn { stream })
    }
}

impl<I: io::Read> io::Read for Conn<I> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.stream.read(buf)
    }
}

impl<I: io::Write> io::Write for Conn<I> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.stream.flush()
    }
}
