use pswd_core::{PasswordOptions, Secret, generate_password};
use rand::SeedableRng;
use rand::rngs::{StdRng, SysRng};

use crate::AppError;

/// Mot de passe aléatoire, tiré avec l'aléa du système.
///
/// # Errors
///
/// Si les options sont invalides ou si l'aléa du système manque.
pub fn generate(options: PasswordOptions) -> Result<Secret, AppError> {
    let mut rng = StdRng::try_from_rng(&mut SysRng).map_err(random_unavailable)?;
    Ok(generate_password(options, &mut rng)?)
}

/// L'aléa du système ne manque que sur une machine défaillante.
// Exclue de la couverture : impossible à provoquer en test.
#[cfg_attr(coverage_nightly, coverage(off))]
fn random_unavailable<E>(_: E) -> AppError {
    AppError::RandomUnavailable
}

#[cfg(test)]
mod tests {
    use pswd_core::GeneratorError;

    use super::*;

    #[test]
    fn generated_passwords_follow_the_options_and_differ() {
        let options = PasswordOptions {
            length: 40,
            symbols: false,
        };
        let first = generate(options).unwrap();
        assert_eq!(first.expose().len(), 40);
        assert!(first.expose().chars().all(char::is_alphanumeric));
        assert_ne!(generate(options).unwrap(), first);
    }

    #[test]
    fn invalid_options_are_refused() {
        let options = PasswordOptions {
            length: 3,
            ..PasswordOptions::default()
        };
        assert_eq!(
            generate(options),
            Err(AppError::Generator(GeneratorError::InvalidLength))
        );
    }
}
