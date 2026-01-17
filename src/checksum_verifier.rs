use digest::{Digest, DynDigest};
use std::io::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChecksumVerifierError {
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
}

pub struct ChecksumVerifier<T, W> {
    checksum: Box<[u8]>,
    checksummer: Box<T>,
    delegate: W,
}

impl<T: DynDigest, W: Write> ChecksumVerifier<T, W> {
    pub fn new(checksum: &str, checksummer: Box<T>, delegate: W) -> Self {
        let checksum = hex::decode(checksum)
            .unwrap_or_else(|_| {
                panic!("Failed to decode checksum: {}", checksum);
            })
            .into_boxed_slice();
        if checksum.len() != checksummer.output_size() {
            panic!(
                "Checksum has incorrect length: expected {}, got {}",
                checksummer.output_size(),
                checksum.len()
            );
        }
        Self {
            checksum,
            checksummer,
            delegate,
        }
    }

    pub fn verify(self) -> Result<(), ChecksumVerifierError> {
        let actual = self.checksummer.finalize();
        let expected = self.checksum;
        if actual == expected {
            Ok(())
        } else {
            Err(ChecksumVerifierError::ChecksumMismatch {
                expected: hex::encode(expected),
                actual: hex::encode(actual),
            })
        }
    }
}

impl<T: Digest, W: Write> Write for ChecksumVerifier<T, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.checksummer.update(buf);
        self.delegate.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.delegate.flush()
    }
}
