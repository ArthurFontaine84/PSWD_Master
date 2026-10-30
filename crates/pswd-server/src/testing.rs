//! Vrai serveur HTTPS lancé dans un thread, pour tester les clients (`pswd-infra`, `pswd-app`,
//! `pswd-cli`). Réservé aux tests : une erreur y est une panique.
#![allow(clippy::expect_used)]

use std::net::TcpListener;

use pswd_core::Secret;
use pswd_core::crypto::FINGERPRINT_LEN;

use crate::{Identity, KeyPurpose, ShutdownHandle, Storage, issue_connection_key, router};
use crate::{serve, server_config};

pub struct TestServer {
    /// `127.0.0.1:<port>`.
    pub address: String,
    pub fingerprint: [u8; FINGERPRINT_LEN],
    /// Clé administrateur, pas encore utilisée.
    pub admin_key: Secret,
}

impl TestServer {
    /// Serveur neuf, à base en mémoire ; il tourne jusqu'à la fin du processus de test.
    ///
    /// # Panics
    ///
    /// Si le serveur ne peut pas être lancé.
    #[must_use]
    pub fn start() -> Self {
        let identity = Identity::generate().expect("certificat");
        let fingerprint = identity.fingerprint();
        let mut storage = Storage::open_in_memory().expect("base");
        let admin_key = issue_connection_key(&mut storage, &fingerprint, &KeyPurpose::Admin)
            .expect("clé administrateur");
        let config = server_config(&identity).expect("configuration TLS");
        let listener = TcpListener::bind("127.0.0.1:0").expect("port");
        let address = listener.local_addr().expect("adresse").to_string();
        let server = serve(
            listener,
            router(storage, fingerprint),
            config,
            ShutdownHandle::new(),
        );
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        std::thread::spawn(move || runtime.block_on(server));
        Self {
            address,
            fingerprint,
            admin_key,
        }
    }
}
