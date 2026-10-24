use pswd_app::Vaults;

/// État partagé par toutes les commandes.
pub struct AppState {
    pub vaults: Vaults,
}

impl AppState {
    pub fn new(vaults: Vaults) -> Self {
        Self { vaults }
    }
}
