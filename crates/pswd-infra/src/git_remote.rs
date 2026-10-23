use std::cell::Cell;
use std::path::Path;
use std::sync::Once;

use git2::{
    Commit, Cred, ErrorCode, FetchOptions, Oid, PushOptions, RemoteCallbacks, Repository, Signature,
};
use pswd_core::Secret;
use pswd_core::ports::{PortError, PushError, RemoteFile, RemoteStore, RemoteVersion};

const BRANCH: &str = "refs/heads/main";
const TRACKING: &str = "refs/remotes/origin/main";
const FETCH_REFSPEC: &str = "+refs/heads/main:refs/remotes/origin/main";
const PUSH_REFSPEC: &str = "refs/heads/main:refs/heads/main";
const VAULT_FILE: &str = "vault.enc";
const FILE_MODE: i32 = 0o100_644;
/// Délai maximal de connexion au serveur, puis d'attente de chacune de ses réponses.
const NETWORK_TIMEOUT_MS: i32 = 5_000;

/// Un serveur injoignable ne bloque pas l'ouverture du coffre : il est alors utilisé hors ligne.
fn configure_network_timeouts() {
    static CONFIGURED: Once = Once::new();
    CONFIGURED.call_once(|| {
        // SAFETY: ces options sont des variables globales de libgit2, lues sans synchronisation.
        // Ce bloc ne s'exécute qu'une fois, avant que le premier `GitRemote` existe ; or toute
        // opération réseau de libgit2 passe par un `GitRemote` : aucune ne peut tourner en même
        // temps. Ces fonctions ne peuvent pas échouer (documentation de `git2`).
        unsafe {
            let _ = git2::opts::set_server_connect_timeout_in_milliseconds(NETWORK_TIMEOUT_MS);
            let _ = git2::opts::set_server_timeout_in_milliseconds(NETWORK_TIMEOUT_MS);
        }
    });
}

/// Dépôt distant, avec un dépôt nu local comme cache. Le serveur refuse tout push qui n'est pas
/// une avance rapide : c'est lui qui garantit qu'aucune version n'est écrasée.
pub struct GitRemote {
    repo: Repository,
    url: String,
    token: Option<Secret>,
    /// Dernier commit distant sans `vault.enc` (dépôt créé avec un README) : il servira de
    /// parent au premier commit, pour que le push soit accepté.
    base_without_vault: Option<Oid>,
}

impl GitRemote {
    /// Ouvre le dépôt nu `repo_dir`, ou le crée.
    ///
    /// # Errors
    ///
    /// Si le dépôt local ne peut être ni ouvert ni créé.
    pub fn open(repo_dir: &Path, url: &str, token: Option<Secret>) -> Result<Self, PortError> {
        configure_network_timeouts();
        let repo = Repository::open_bare(repo_dir)
            .or_else(|_| Repository::init_bare(repo_dir))
            .map_err(git_error("dépôt local"))?;
        Ok(Self {
            repo,
            url: url.to_owned(),
            token,
            base_without_vault: None,
        })
    }

    fn callbacks(&self) -> RemoteCallbacks<'_> {
        let mut callbacks = RemoteCallbacks::new();
        if let Some(token) = &self.token {
            authenticate(&mut callbacks, token);
        }
        callbacks
    }

    fn read_tracking(&mut self) -> Result<Option<RemoteFile>, git2::Error> {
        let tip = match self.repo.find_reference(TRACKING) {
            Ok(reference) => reference.peel_to_commit()?,
            Err(error) if error.code() == ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let Some(entry) = tip.tree()?.get_name(VAULT_FILE).map(|entry| entry.id()) else {
            self.base_without_vault = Some(tip.id());
            return Ok(None);
        };
        self.base_without_vault = None;
        Ok(Some(RemoteFile {
            file: self.repo.find_blob(entry)?.content().to_vec(),
            version: RemoteVersion(tip.id().to_string()),
        }))
    }

    /// Commit contenant `file`, sur la version distante attendue.
    fn commit(&self, file: &[u8], expected: Option<&RemoteVersion>) -> Result<Oid, git2::Error> {
        let parent_id = match expected {
            Some(version) => Some(Oid::from_str(&version.0)?),
            None => self.base_without_vault,
        };
        let parent = parent_id.map(|id| self.repo.find_commit(id)).transpose()?;
        let base_tree = parent.as_ref().map(Commit::tree).transpose()?;
        let mut tree = self.repo.treebuilder(base_tree.as_ref())?;
        tree.insert(VAULT_FILE, self.repo.blob(file)?, FILE_MODE)?;
        let tree = self.repo.find_tree(tree.write()?)?;
        let signature = Signature::now("pswd", "pswd@localhost")?;
        let parents: Vec<&Commit> = parent.iter().collect();
        self.repo.commit(
            None,
            &signature,
            &signature,
            "Mise à jour du coffre",
            &tree,
            &parents,
        )
    }

    /// `Ok(false)` si le serveur a refusé la mise à jour de la branche.
    fn push_branch(&self) -> Result<bool, git2::Error> {
        let rejected = Cell::new(false);
        let mut callbacks = self.callbacks();
        callbacks.push_update_reference(|_, status| {
            rejected.set(status.is_some());
            Ok(())
        });
        let mut options = PushOptions::new();
        options.remote_callbacks(callbacks);
        self.repo
            .remote_anonymous(&self.url)?
            .push(&[PUSH_REFSPEC], Some(&mut options))?;
        Ok(!rejected.get())
    }

    fn download(&self) -> Result<(), git2::Error> {
        let mut options = FetchOptions::new();
        options.remote_callbacks(self.callbacks());
        self.repo
            .remote_anonymous(&self.url)?
            .fetch(&[FETCH_REFSPEC], Some(&mut options), None)
    }
}

impl RemoteStore for GitRemote {
    fn fetch(&mut self) -> Result<Option<RemoteFile>, PortError> {
        self.download()
            .map_err(git_error("récupération du dépôt distant"))?;
        self.read_tracking()
            .map_err(git_error("lecture du dépôt distant"))
    }

    fn push(&mut self, file: &[u8], expected: Option<&RemoteVersion>) -> Result<(), PushError> {
        let commit = self.commit(file, expected).map_err(push_failed)?;
        self.repo
            .reference(BRANCH, commit, true, "pswd")
            .map_err(push_failed)?;
        push_outcome(self.push_branch())
    }
}

/// Un push refusé arrive comme une erreur (transport local) ou comme un statut renvoyé par le
/// serveur (HTTPS).
fn push_outcome(result: Result<bool, git2::Error>) -> Result<(), PushError> {
    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(PushError::Conflict),
        Err(error) if error.code() == ErrorCode::NotFastForward => Err(PushError::Conflict),
        Err(error) => Err(push_failed(error)),
    }
}

fn push_failed(error: git2::Error) -> PushError {
    PushError::Failed(git_error("publication")(error))
}

/// Identifiants HTTPS : le jeton est présenté une seule fois, pour ne pas boucler s'il est
/// refusé.
// Exclue de la couverture : n'est appelée que par un serveur HTTPS qui demande une
// authentification, ce que le transport local des tests ne fait jamais.
#[cfg_attr(coverage_nightly, coverage(off))]
fn authenticate<'a>(callbacks: &mut RemoteCallbacks<'a>, token: &'a Secret) {
    let mut attempted = false;
    callbacks.credentials(move |_, _, _| {
        if attempted {
            return Err(git2::Error::from_str("jeton d'accès refusé"));
        }
        attempted = true;
        Cred::userpass_plaintext("x-access-token", token.expose())
    });
}

fn git_error(action: &'static str) -> impl Fn(git2::Error) -> PortError {
    move |error| PortError::new(format!("{action} : {}", error.message()))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// Un dépôt nu « distant » et deux appareils qui le partagent.
    struct Setup {
        dir: TempDir,
        url: String,
    }

    impl Setup {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let remote = dir.path().join("remote.git");
            Repository::init_bare(&remote).unwrap();
            let url = remote.to_str().unwrap().to_owned();
            Self { dir, url }
        }

        fn device(&self, name: &str) -> GitRemote {
            GitRemote::open(&self.dir.path().join(name), &self.url, None).unwrap()
        }

        /// Un premier commit avec seulement un README, comme GitHub peut le créer.
        fn add_readme(&self) {
            let repo = Repository::open_bare(&self.url).unwrap();
            let mut tree = repo.treebuilder(None).unwrap();
            tree.insert("README.md", repo.blob(b"# Coffre").unwrap(), FILE_MODE)
                .unwrap();
            let tree = repo.find_tree(tree.write().unwrap()).unwrap();
            let signature = Signature::now("test", "test@localhost").unwrap();
            repo.commit(Some(BRANCH), &signature, &signature, "README", &tree, &[])
                .unwrap();
        }
    }

    #[test]
    fn empty_remote_has_no_vault() {
        let setup = Setup::new();
        assert_eq!(setup.device("a").fetch(), Ok(None));
    }

    #[test]
    fn pushed_file_is_fetched_by_another_device() {
        let setup = Setup::new();
        let mut a = setup.device("a");
        assert_eq!(a.fetch(), Ok(None));
        a.push(b"version 1", None).unwrap();
        let fetched = setup.device("b").fetch().unwrap().unwrap();
        assert_eq!(fetched.file, b"version 1");
    }

    #[test]
    fn push_on_the_fetched_version_succeeds() {
        let setup = Setup::new();
        let mut a = setup.device("a");
        a.push(b"version 1", None).unwrap();
        let mut b = setup.device("b");
        let fetched = b.fetch().unwrap().unwrap();
        b.push(b"version 2", Some(&fetched.version)).unwrap();
        let latest = a.fetch().unwrap().unwrap();
        assert_eq!(latest.file, b"version 2");
        assert_ne!(latest.version, fetched.version);
    }

    #[test]
    fn push_on_an_outdated_version_is_a_conflict() {
        let setup = Setup::new();
        let mut a = setup.device("a");
        a.push(b"version 1", None).unwrap();
        let mut b = setup.device("b");
        let outdated = b.fetch().unwrap().unwrap().version;
        let current = a.fetch().unwrap().unwrap().version;
        a.push(b"version 2", Some(&current)).unwrap();
        assert_eq!(
            b.push(b"version 2 bis", Some(&outdated)),
            Err(PushError::Conflict)
        );
        assert_eq!(a.fetch().unwrap().unwrap().file, b"version 2");
    }

    #[test]
    fn creating_over_an_existing_vault_is_a_conflict() {
        let setup = Setup::new();
        setup.device("a").push(b"existing", None).unwrap();
        assert_eq!(
            setup.device("b").push(b"other", None),
            Err(PushError::Conflict)
        );
    }

    #[test]
    fn repository_with_a_readme_accepts_the_first_vault() {
        let setup = Setup::new();
        setup.add_readme();
        let mut a = setup.device("a");
        assert_eq!(a.fetch(), Ok(None));
        a.push(b"vault", None).unwrap();
        assert_eq!(a.fetch().unwrap().unwrap().file, b"vault");
        let repo = Repository::open_bare(&setup.url).unwrap();
        let tree = repo.find_reference(BRANCH).unwrap().peel_to_tree().unwrap();
        assert!(tree.get_name("README.md").is_some());
    }

    #[test]
    fn opening_a_remote_bounds_network_waits() {
        let setup = Setup::new();
        setup.device("a");
        // SAFETY: simple lecture, une fois les options écrites par `configure_network_timeouts`.
        let (connect, server) = unsafe {
            (
                git2::opts::get_server_connect_timeout_in_milliseconds().unwrap(),
                git2::opts::get_server_timeout_in_milliseconds().unwrap(),
            )
        };
        assert_eq!((connect, server), (NETWORK_TIMEOUT_MS, NETWORK_TIMEOUT_MS));
    }

    #[test]
    fn existing_local_repository_is_reopened() {
        let setup = Setup::new();
        setup.device("a").push(b"vault", None).unwrap();
        let mut reopened = setup.device("a");
        assert_eq!(reopened.fetch().unwrap().unwrap().file, b"vault");
    }

    #[test]
    fn unreachable_remote_is_an_error() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("missing.git");
        let mut remote = GitRemote::open(
            &dir.path().join("local"),
            missing.to_str().unwrap(),
            Some(Secret::new("jeton".to_owned())),
        )
        .unwrap();
        let error = remote.fetch().unwrap_err().to_string();
        assert!(
            error.starts_with("récupération du dépôt distant : "),
            "{error}"
        );
        assert!(matches!(
            remote.push(b"vault", None),
            Err(PushError::Failed(_))
        ));
    }

    #[test]
    fn unknown_version_cannot_be_pushed() {
        let setup = Setup::new();
        let unknown = RemoteVersion("0".repeat(40));
        let invalid = RemoteVersion("pas un commit".to_owned());
        for version in [unknown, invalid] {
            assert!(matches!(
                setup.device("a").push(b"vault", Some(&version)),
                Err(PushError::Failed(_))
            ));
        }
    }

    #[test]
    fn server_refusal_by_status_or_by_error_is_a_conflict() {
        let not_fast_forward = git2::Error::new(
            ErrorCode::NotFastForward,
            git2::ErrorClass::Reference,
            "non fast-forward",
        );
        let other = git2::Error::from_str("connexion perdue");
        assert_eq!(push_outcome(Ok(true)), Ok(()));
        assert_eq!(push_outcome(Ok(false)), Err(PushError::Conflict));
        assert_eq!(
            push_outcome(Err(not_fast_forward)),
            Err(PushError::Conflict)
        );
        assert_eq!(
            push_outcome(Err(other)),
            Err(PushError::Failed(PortError::new(
                "publication : connexion perdue"
            )))
        );
    }

    #[test]
    fn corrupted_local_reference_is_an_error() {
        let setup = Setup::new();
        let mut a = setup.device("a");
        let tracking = setup.dir.path().join("a").join(TRACKING);
        std::fs::create_dir_all(tracking.parent().unwrap()).unwrap();
        std::fs::write(&tracking, b"pas un identifiant de commit\n").unwrap();
        let error = a.fetch().unwrap_err().to_string();
        assert!(error.starts_with("lecture du dépôt distant : "), "{error}");
    }

    #[test]
    fn local_repository_that_cannot_be_created_is_an_error() {
        let dir = TempDir::new().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"").unwrap();
        let error = GitRemote::open(&blocker.join("repo.git"), "url", None)
            .err()
            .unwrap();
        assert!(error.to_string().starts_with("dépôt local : "), "{error}");
    }
}
