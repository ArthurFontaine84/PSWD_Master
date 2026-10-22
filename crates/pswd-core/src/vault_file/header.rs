use super::FileError;
use crate::Secret;
use crate::Timestamp;
use crate::crypto::{
    KEY_LEN, KdfParams, Key, NONCE_LEN, SALT_LEN, Salt, Sealed, TAG_LEN, password_wrapping_key,
    recovery_wrapping_key, unwrap_key, wrap_key,
};

const MAGIC: [u8; 4] = *b"PSWD";
const FORMAT_VERSION: u16 = 1;
const PASSWORD_KIND: u8 = 1;
const RECOVERY_KIND: u8 = 2;
const WRAPPED_KEY_LEN: usize = KEY_LEN + TAG_LEN;

/// En-tête en clair du fichier. La version 1 contient exactement une enveloppe mot de passe et
/// une enveloppe de récupération.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Header {
    pub(super) keys_updated_at: Timestamp,
    pub(super) password: PasswordEnvelope,
    pub(super) recovery: RecoveryEnvelope,
}

impl Header {
    pub(super) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&self.keys_updated_at.as_millis().to_le_bytes());
        out.push(2);
        self.password.encode_into(&mut out);
        self.recovery.encode_into(&mut out);
        out
    }
}

/// Fichier découpé, sans aucun calcul cryptographique : un fichier piégé est rejeté avant la
/// dérivation de clé.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ParsedFile<'a> {
    pub(super) header: Header,
    /// Octets de l'en-tête, authentifiés avec le payload.
    pub(super) header_bytes: &'a [u8],
    pub(super) payload_nonce: [u8; NONCE_LEN],
    pub(super) payload: &'a [u8],
}

impl<'a> ParsedFile<'a> {
    pub(super) fn parse(file: &'a [u8]) -> Result<Self, FileError> {
        let mut reader = Reader(file);
        if reader.array()? != MAGIC {
            return Err(FileError::Malformed);
        }
        let version = reader.u16()?;
        if version != FORMAT_VERSION {
            return Err(FileError::UnsupportedVersion(version));
        }
        let keys_updated_at = Timestamp::from_millis(reader.i64()?);
        let (password, recovery) = parse_envelopes(&mut reader)?;
        let header_bytes = &file[..file.len() - reader.0.len()];
        Ok(Self {
            header: Header {
                keys_updated_at,
                password,
                recovery,
            },
            header_bytes,
            payload_nonce: reader.array()?,
            payload: reader.0,
        })
    }
}

fn parse_envelopes(
    reader: &mut Reader<'_>,
) -> Result<(PasswordEnvelope, RecoveryEnvelope), FileError> {
    let mut password = None;
    let mut recovery = None;
    for _ in 0..reader.u8()? {
        let duplicate = match reader.u8()? {
            PASSWORD_KIND => password.replace(PasswordEnvelope::parse(reader)?).is_some(),
            RECOVERY_KIND => recovery.replace(RecoveryEnvelope::parse(reader)?).is_some(),
            // EXTENSION(keyfile): enveloppe kind 3, mot de passe + fichier-clé.
            kind => return Err(FileError::UnknownEnvelope(kind)),
        };
        if duplicate {
            return Err(FileError::Malformed);
        }
    }
    password.zip(recovery).ok_or(FileError::Malformed)
}

/// Clé du coffre chiffrée par une clé dérivée du mot de passe maître.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PasswordEnvelope {
    params: KdfParams,
    salt: Salt,
    wrapped: Sealed,
}

impl PasswordEnvelope {
    pub(super) fn seal(
        vault_key: &Key,
        password: &Secret,
        params: KdfParams,
        salt: Salt,
    ) -> Result<Self, FileError> {
        let wrapping_key = password_wrapping_key(password, &salt, &params)?;
        let aad = Self::prefix(&params, &salt);
        Ok(Self {
            params,
            salt,
            wrapped: wrap_key(&wrapping_key, vault_key, &aad)?,
        })
    }

    pub(super) fn open(&self, password: &Secret) -> Result<Key, FileError> {
        let wrapping_key = password_wrapping_key(password, &self.salt, &self.params)?;
        unwrap_key(&wrapping_key, &self.wrapped, &self.aad()).map_err(|_| FileError::WrongPassword)
    }

    /// Type et paramètres de l'enveloppe : écrits en clair et authentifiés avec la clé chiffrée.
    fn prefix(params: &KdfParams, salt: &Salt) -> Vec<u8> {
        let mut prefix = vec![PASSWORD_KIND];
        prefix.extend_from_slice(&params.memory_kib().to_le_bytes());
        prefix.extend_from_slice(&params.iterations().to_le_bytes());
        prefix.extend_from_slice(&params.parallelism().to_le_bytes());
        prefix.extend_from_slice(salt.as_bytes());
        prefix
    }

    fn aad(&self) -> Vec<u8> {
        Self::prefix(&self.params, &self.salt)
    }

    fn parse(reader: &mut Reader<'_>) -> Result<Self, FileError> {
        let memory_kib = reader.u32()?;
        let iterations = reader.u32()?;
        let parallelism = reader.u32()?;
        let params = KdfParams::new(memory_kib, iterations, parallelism)?;
        let salt = Salt::from_bytes(reader.array::<SALT_LEN>()?);
        Ok(Self {
            params,
            salt,
            wrapped: parse_wrapped_key(reader)?,
        })
    }

    fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.aad());
        encode_wrapped_key(&self.wrapped, out);
    }
}

/// Clé du coffre chiffrée par une clé dérivée de la clé de récupération.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RecoveryEnvelope {
    wrapped: Sealed,
}

impl RecoveryEnvelope {
    const AAD: [u8; 1] = [RECOVERY_KIND];

    pub(super) fn seal(vault_key: &Key, recovery_key: &Key) -> Result<Self, FileError> {
        let wrapping_key = recovery_wrapping_key(recovery_key)?;
        Ok(Self {
            wrapped: wrap_key(&wrapping_key, vault_key, &Self::AAD)?,
        })
    }

    pub(super) fn open(&self, recovery_key: &Key) -> Result<Key, FileError> {
        let wrapping_key = recovery_wrapping_key(recovery_key)?;
        unwrap_key(&wrapping_key, &self.wrapped, &Self::AAD)
            .map_err(|_| FileError::WrongRecoveryKey)
    }

    fn parse(reader: &mut Reader<'_>) -> Result<Self, FileError> {
        Ok(Self {
            wrapped: parse_wrapped_key(reader)?,
        })
    }

    fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&Self::AAD);
        encode_wrapped_key(&self.wrapped, out);
    }
}

fn parse_wrapped_key(reader: &mut Reader<'_>) -> Result<Sealed, FileError> {
    Ok(Sealed {
        nonce: reader.array()?,
        ciphertext: reader.array::<WRAPPED_KEY_LEN>()?.to_vec(),
    })
}

fn encode_wrapped_key(wrapped: &Sealed, out: &mut Vec<u8>) {
    out.extend_from_slice(&wrapped.nonce);
    out.extend_from_slice(&wrapped.ciphertext);
}

/// Lecture séquentielle d'octets ; toute lecture au-delà de la fin est une erreur.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn array<const N: usize>(&mut self) -> Result<[u8; N], FileError> {
        let (head, tail) = self.0.split_first_chunk().ok_or(FileError::Malformed)?;
        self.0 = tail;
        Ok(*head)
    }

    fn u8(&mut self) -> Result<u8, FileError> {
        self.array().map(u8::from_le_bytes)
    }

    fn u16(&mut self) -> Result<u16, FileError> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, FileError> {
        self.array().map(u32::from_le_bytes)
    }

    fn i64(&mut self) -> Result<i64, FileError> {
        self.array().map(i64::from_le_bytes)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::crypto::CryptoError;

    const HEADER_LEN: usize = 4
        + 2
        + 8
        + 1
        + (1 + 12 + SALT_LEN + NONCE_LEN + WRAPPED_KEY_LEN)
        + (1 + NONCE_LEN + WRAPPED_KEY_LEN);

    fn wrapped(fill: u8) -> Sealed {
        Sealed {
            nonce: [fill; NONCE_LEN],
            ciphertext: vec![fill; WRAPPED_KEY_LEN],
        }
    }

    fn sample_header() -> Header {
        Header {
            keys_updated_at: Timestamp::from_millis(1_700_000_000_000),
            password: PasswordEnvelope {
                params: KdfParams::DEFAULT,
                salt: Salt::from_bytes([0x5A; SALT_LEN]),
                wrapped: wrapped(0xAA),
            },
            recovery: RecoveryEnvelope {
                wrapped: wrapped(0xBB),
            },
        }
    }

    fn sample_file() -> Vec<u8> {
        let mut file = sample_header().encode();
        file.extend_from_slice(&[0xCC; NONCE_LEN]);
        file.extend_from_slice(b"payload");
        file
    }

    /// En-tête avec une liste d'enveloppes choisie : (type, octets qui suivent le type).
    fn file_with_envelopes(envelopes: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let header = sample_header().encode();
        let mut file = header[..14].to_vec();
        file.push(u8::try_from(envelopes.len()).unwrap());
        for (kind, body) in envelopes {
            file.push(*kind);
            file.extend_from_slice(body);
        }
        file.extend_from_slice(&[0xCC; NONCE_LEN]);
        file
    }

    fn password_body() -> Vec<u8> {
        let mut out = Vec::new();
        sample_header().password.encode_into(&mut out);
        out.split_off(1)
    }

    fn recovery_body() -> Vec<u8> {
        let mut out = Vec::new();
        sample_header().recovery.encode_into(&mut out);
        out.split_off(1)
    }

    #[test]
    fn header_layout_follows_the_spec() {
        let header = sample_header().encode();
        assert_eq!(header.len(), HEADER_LEN);
        assert_eq!(header[..4], *b"PSWD");
        assert_eq!(header[4..6], 1_u16.to_le_bytes());
        assert_eq!(header[6..14], 1_700_000_000_000_i64.to_le_bytes());
        assert_eq!(header[14], 2);
        assert_eq!(header[15], PASSWORD_KIND);
        assert_eq!(header[16..20], (64 * 1024_u32).to_le_bytes());
        assert_eq!(header[20..24], 3_u32.to_le_bytes());
        assert_eq!(header[24..28], 1_u32.to_le_bytes());
        assert_eq!(header[28..44], [0x5A; SALT_LEN]);
        assert_eq!(header[44..68], [0xAA; NONCE_LEN]);
        assert_eq!(header[68..116], [0xAA; WRAPPED_KEY_LEN]);
        assert_eq!(header[116], RECOVERY_KIND);
        assert_eq!(header[117..141], [0xBB; NONCE_LEN]);
        assert_eq!(header[141..], [0xBB; WRAPPED_KEY_LEN]);
    }

    #[test]
    fn parse_splits_header_nonce_and_payload() {
        let file = sample_file();
        let parsed = ParsedFile::parse(&file).unwrap();
        assert_eq!(parsed.header, sample_header());
        assert_eq!(parsed.header_bytes, &file[..HEADER_LEN]);
        assert_eq!(parsed.payload_nonce, [0xCC; NONCE_LEN]);
        assert_eq!(parsed.payload, b"payload");
    }

    #[test]
    fn envelopes_may_come_in_any_order() {
        let file = file_with_envelopes(&[
            (RECOVERY_KIND, recovery_body()),
            (PASSWORD_KIND, password_body()),
        ]);
        assert_eq!(ParsedFile::parse(&file).unwrap().header, sample_header());
    }

    #[test]
    fn truncated_file_is_malformed() {
        let file = sample_file();
        for len in 0..HEADER_LEN + NONCE_LEN {
            assert_eq!(
                ParsedFile::parse(&file[..len]),
                Err(FileError::Malformed),
                "longueur {len}"
            );
        }
    }

    #[test]
    fn wrong_magic_is_malformed() {
        let mut file = sample_file();
        file[0] = b'X';
        assert_eq!(ParsedFile::parse(&file), Err(FileError::Malformed));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let mut file = sample_file();
        file[4..6].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            ParsedFile::parse(&file),
            Err(FileError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn unknown_envelope_is_rejected() {
        let file = file_with_envelopes(&[(3, Vec::new())]);
        assert_eq!(ParsedFile::parse(&file), Err(FileError::UnknownEnvelope(3)));
    }

    #[test]
    fn missing_or_duplicated_envelopes_are_malformed() {
        let cases = [
            vec![],
            vec![(PASSWORD_KIND, password_body())],
            vec![(RECOVERY_KIND, recovery_body())],
            vec![
                (PASSWORD_KIND, password_body()),
                (PASSWORD_KIND, password_body()),
                (RECOVERY_KIND, recovery_body()),
            ],
            vec![
                (PASSWORD_KIND, password_body()),
                (RECOVERY_KIND, recovery_body()),
                (RECOVERY_KIND, recovery_body()),
            ],
        ];
        for envelopes in cases {
            assert_eq!(
                ParsedFile::parse(&file_with_envelopes(&envelopes)),
                Err(FileError::Malformed),
                "{} enveloppe(s)",
                envelopes.len()
            );
        }
    }

    #[test]
    fn out_of_bounds_kdf_params_are_rejected() {
        let mut file = sample_file();
        file[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            ParsedFile::parse(&file),
            Err(FileError::Crypto(CryptoError::InvalidKdfParams))
        );
    }

    #[test]
    fn envelopes_open_only_with_the_right_secret() {
        let vault_key = Key::generate().unwrap();
        let recovery_key = Key::generate().unwrap();
        let password = Secret::new("maître".to_owned());
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let envelope =
            PasswordEnvelope::seal(&vault_key, &password, params, Salt::generate().unwrap())
                .unwrap();
        let recovery = RecoveryEnvelope::seal(&vault_key, &recovery_key).unwrap();

        assert_eq!(
            envelope.open(&password).unwrap().as_bytes(),
            vault_key.as_bytes()
        );
        assert_eq!(
            recovery.open(&recovery_key).unwrap().as_bytes(),
            vault_key.as_bytes()
        );
        assert_eq!(
            envelope.open(&Secret::new("autre".to_owned())).err(),
            Some(FileError::WrongPassword)
        );
        assert_eq!(
            recovery.open(&vault_key).err(),
            Some(FileError::WrongRecoveryKey)
        );
    }

    #[test]
    fn tampered_kdf_params_do_not_open_the_envelope() {
        let vault_key = Key::generate().unwrap();
        let password = Secret::new("maître".to_owned());
        let salt = Salt::generate().unwrap();
        let mut envelope = PasswordEnvelope::seal(
            &vault_key,
            &password,
            KdfParams::new(8 * 1024, 1, 1).unwrap(),
            salt,
        )
        .unwrap();
        envelope.params = KdfParams::new(8 * 1024, 2, 1).unwrap();
        assert_eq!(
            envelope.open(&password).err(),
            Some(FileError::WrongPassword)
        );
    }

    fn arb_wrapped() -> impl Strategy<Value = Sealed> {
        (
            prop::array::uniform24(any::<u8>()),
            prop::collection::vec(any::<u8>(), WRAPPED_KEY_LEN),
        )
            .prop_map(|(nonce, ciphertext)| Sealed { nonce, ciphertext })
    }

    fn arb_header() -> impl Strategy<Value = Header> {
        (
            any::<i64>(),
            (8 * 1024..=1024 * 1024_u32, 1..=10_u32, 1..=8_u32),
            prop::array::uniform16(any::<u8>()),
            arb_wrapped(),
            arb_wrapped(),
        )
            .prop_map(
                |(millis, (memory, iterations, parallelism), salt, password, recovery)| Header {
                    keys_updated_at: Timestamp::from_millis(millis),
                    password: PasswordEnvelope {
                        params: KdfParams::new(memory, iterations, parallelism).unwrap(),
                        salt: Salt::from_bytes(salt),
                        wrapped: password,
                    },
                    recovery: RecoveryEnvelope { wrapped: recovery },
                },
            )
    }

    proptest! {
        #[test]
        fn parse_reverses_encode(
            header in arb_header(),
            nonce in prop::array::uniform24(any::<u8>()),
            payload in prop::collection::vec(any::<u8>(), 0..64),
        ) {
            let mut file = header.encode();
            file.extend_from_slice(&nonce);
            file.extend_from_slice(&payload);
            let parsed = ParsedFile::parse(&file).unwrap();
            let encoded = header.encode();
            prop_assert_eq!(parsed.header_bytes, encoded.as_slice());
            prop_assert_eq!(parsed.header, header);
            prop_assert_eq!(parsed.payload_nonce, nonce);
            prop_assert_eq!(parsed.payload, payload.as_slice());
        }

        /// Un fichier altéré est soit rejeté, soit relu de façon cohérente : l'en-tête relu se
        /// réencode à l'identique, donc rien n'échappe à l'authentification.
        #[test]
        fn altered_file_is_rejected_or_parsed_consistently(
            position in any::<prop::sample::Index>(),
            value in any::<u8>(),
        ) {
            let mut file = sample_file();
            let index = position.index(file.len());
            file[index] = value;
            if let Ok(parsed) = ParsedFile::parse(&file) {
                let encoded = parsed.header.encode();
                prop_assert_eq!(encoded.as_slice(), parsed.header_bytes);
            }
        }
    }
}
