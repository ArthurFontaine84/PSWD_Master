use std::io;

use zeroize::Zeroizing;

/// Appelle `write` deux fois : d'abord pour mesurer, puis dans un buffer de la taille exacte.
/// Le buffer n'est jamais réalloué, donc aucune copie du contenu n'est abandonnée en mémoire
/// sans être effacée.
pub(crate) fn write_exact<E>(
    mut write: impl FnMut(&mut dyn io::Write) -> Result<(), E>,
) -> Result<Zeroizing<Vec<u8>>, E> {
    let mut counter = ByteCounter(0);
    write(&mut counter)?;
    let mut buffer = Zeroizing::new(Vec::with_capacity(counter.0));
    write(&mut *buffer)?;
    Ok(buffer)
}

struct ByteCounter(usize);

impl io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn buffer_has_exactly_the_written_size() {
        let buffer = write_exact(|out| {
            out.write_all(b"hello ")?;
            out.write_all(b"vault")
        })
        .unwrap();
        assert_eq!(buffer.as_slice(), b"hello vault");
        assert_eq!(buffer.capacity(), buffer.len());
    }

    #[test]
    fn first_error_is_returned() {
        let result = write_exact(|_| Err::<(), _>("échec"));
        assert_eq!(result.err(), Some("échec"));
    }

    #[test]
    fn counter_counts_written_bytes() {
        let mut counter = ByteCounter(0);
        counter.write_all(b"abc").unwrap();
        counter.flush().unwrap();
        assert_eq!(counter.0, 3);
    }
}
