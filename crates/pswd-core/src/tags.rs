/// Liste de tags normalisée : sans espaces autour, en minuscules, sans vide, sans doublon, triée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags(Vec<String>);

impl Tags {
    pub fn new<I, S>(tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut tags: Vec<String> = tags
            .into_iter()
            .map(|tag| tag.as_ref().trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        tags.sort_unstable();
        tags.dedup();
        Self(tags)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_lowercases() {
        assert_eq!(Tags::new(["  Perso "]).as_slice(), ["perso"]);
    }

    #[test]
    fn drops_empty_tags() {
        assert_eq!(Tags::new(["", "   ", "dev"]).as_slice(), ["dev"]);
    }

    #[test]
    fn sorts_tags() {
        assert_eq!(
            Tags::new(["web", "dev", "perso"]).as_slice(),
            ["dev", "perso", "web"]
        );
    }

    #[test]
    fn removes_duplicates_after_normalization() {
        assert_eq!(Tags::new(["dev", "DEV", " dev "]).as_slice(), ["dev"]);
    }

    #[test]
    fn default_is_empty() {
        assert_eq!(Tags::default().as_slice(), Vec::<String>::new());
    }
}
