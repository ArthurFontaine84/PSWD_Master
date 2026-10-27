use std::fmt;

use csv::{ByteRecord, ReaderBuilder};
#[cfg(any(test, fuzzing))]
use csv::{Terminator, WriterBuilder};
#[cfg(any(test, fuzzing))]
use zeroize::Zeroizing;

#[cfg(any(test, fuzzing))]
use crate::buffer::write_exact;
use crate::{EntryContent, EntryKind, Secret, Tags};

const HEADER: [&str; 7] = [
    "title", "kind", "username", "password", "urls", "notes", "tags",
];
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
const LIST_SEPARATOR: char = ';';
const ESCAPE: char = '\\';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvError {
    InvalidHeader,
    InvalidLine { line: u64, problem: LineProblem },
    Internal,
}

/// Raison du rejet d'une ligne. Le contenu de la ligne n'est jamais repris : il peut contenir
/// un mot de passe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineProblem {
    FieldCount,
    UnknownKind,
    InvalidUtf8,
}

impl fmt::Display for CsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "en-tête CSV attendu : {}", HEADER.join(",")),
            Self::InvalidLine { line, problem } => write!(f, "ligne {line} : {problem}"),
            Self::Internal => f.write_str("erreur interne de lecture CSV"),
        }
    }
}

impl fmt::Display for LineProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FieldCount => "7 colonnes attendues",
            Self::UnknownKind => "type inconnu (login ou note attendu)",
            Self::InvalidUtf8 => "texte qui n'est pas en UTF-8",
        })
    }
}

impl std::error::Error for CsvError {}

/// Erreur d'E/S de la crate `csv` qui ne peut pas survenir : la lecture se fait en mémoire.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
fn internal_error<E>(_: E) -> CsvError {
    CsvError::Internal
}

/// Écrit des entrées au format d'import : l'application n'exporte jamais en clair, cette écriture
/// ne sert qu'à vérifier que l'import relit fidèlement chaque champ (tests, fuzzing).
#[cfg(any(test, fuzzing))]
fn write_contents(contents: &[&EntryContent]) -> Result<Zeroizing<Vec<u8>>, csv::Error> {
    write_exact(|out| {
        let mut writer = WriterBuilder::new()
            .terminator(Terminator::CRLF)
            .from_writer(out);
        writer.write_record(HEADER)?;
        for content in contents {
            let kind = match content.kind {
                EntryKind::Login => "login",
                EntryKind::Note => "note",
            };
            let username = content.username.as_deref().unwrap_or_default();
            let password = content.password.as_ref().map_or("", Secret::expose);
            let urls = encode_list(&content.urls);
            let notes = content.notes.as_deref().unwrap_or_default();
            let tags = encode_list(content.tags.as_slice());
            writer.write_record([
                content.title.as_str(),
                kind,
                username,
                password,
                &urls,
                notes,
                &tags,
            ])?;
        }
        Ok(writer.flush()?)
    })
}

/// Lit un CSV complet. Rien n'est renvoyé si une seule ligne est invalide : l'import est tout
/// ou rien.
///
/// # Errors
///
/// [`CsvError::InvalidHeader`] si la première ligne n'est pas l'en-tête attendu,
/// [`CsvError::InvalidLine`] avec le numéro de la première ligne invalide.
pub fn import_csv(data: &[u8]) -> Result<Vec<EntryContent>, CsvError> {
    let data = data.strip_prefix(UTF8_BOM).unwrap_or(data);
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(data);
    let mut record = ByteRecord::new();
    if !reader
        .read_byte_record(&mut record)
        .map_err(internal_error)?
        || record
            .iter()
            .ne(HEADER.iter().map(|column| column.as_bytes()))
    {
        return Err(CsvError::InvalidHeader);
    }
    let mut contents = Vec::new();
    while reader
        .read_byte_record(&mut record)
        .map_err(internal_error)?
    {
        let content = parse_record(&record).map_err(|problem| CsvError::InvalidLine {
            line: line_of(data, &record),
            problem,
        })?;
        contents.push(content);
    }
    Ok(contents)
}

/// Numéro de la ligne où commence l'enregistrement. Celui de la crate `csv` est faux avec `\r\n`
/// ou des lignes vides, et sa position en octets peut tomber sur une fin de ligne qui précède.
fn line_of(data: &[u8], record: &ByteRecord) -> u64 {
    let reported = record.position().map_or(0, csv::Position::byte);
    let reported = usize::try_from(reported).map_or(data.len(), |byte| byte.min(data.len()));
    let start = reported
        + data[reported..]
            .iter()
            .take_while(|&&byte| byte == b'\r' || byte == b'\n')
            .count();
    let before = &data[..start];
    // Fins de ligne reconnues par la crate `csv` : `\n`, `\r\n` et `\r` seul.
    let line_breaks = before
        .iter()
        .enumerate()
        .filter(|&(index, &byte)| {
            byte == b'\n' || (byte == b'\r' && before.get(index + 1) != Some(&b'\n'))
        })
        .count();
    u64::try_from(line_breaks).map_or(u64::MAX, |line_breaks| line_breaks + 1)
}

fn parse_record(record: &ByteRecord) -> Result<EntryContent, LineProblem> {
    let fields = record
        .iter()
        .map(|field| std::str::from_utf8(field).map_err(|_| LineProblem::InvalidUtf8))
        .collect::<Result<Vec<&str>, _>>()?;
    let [title, kind, username, password, urls, notes, tags] = fields[..] else {
        return Err(LineProblem::FieldCount);
    };
    let kind = if kind.eq_ignore_ascii_case("login") {
        EntryKind::Login
    } else if kind.eq_ignore_ascii_case("note") {
        EntryKind::Note
    } else {
        return Err(LineProblem::UnknownKind);
    };
    Ok(EntryContent {
        kind,
        title: title.to_owned(),
        username: non_empty(username),
        password: non_empty(password).map(Secret::new),
        urls: decode_list(urls),
        notes: non_empty(notes),
        tags: Tags::new(decode_list(tags)),
    })
}

/// Une cellule vide correspond à un champ absent.
fn non_empty(field: &str) -> Option<String> {
    (!field.is_empty()).then(|| field.to_owned())
}

/// Éléments séparés par `;` ; `\;` et `\\` désignent un `;` et un `\` littéraux.
#[cfg(any(test, fuzzing))]
fn encode_list(items: &[String]) -> String {
    let mut out = String::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push(LIST_SEPARATOR);
        }
        for c in item.chars() {
            if c == LIST_SEPARATOR || c == ESCAPE {
                out.push(ESCAPE);
            }
            out.push(c);
        }
    }
    out
}

/// Inverse de [`encode_list`]. Un `\` qui ne précède ni `;` ni `\` est gardé tel quel ; les
/// éléments vides sont ignorés.
fn decode_list(field: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        match c {
            LIST_SEPARATOR => items.push(std::mem::take(&mut current)),
            ESCAPE => match chars.clone().next() {
                Some(next @ (LIST_SEPARATOR | ESCAPE)) => {
                    current.push(next);
                    chars.next();
                }
                _ => current.push(ESCAPE),
            },
            _ => current.push(c),
        }
    }
    items.push(current);
    items.retain(|item| !item.is_empty());
    items
}

/// Point d'entrée de `cargo fuzz`, compilé seulement avec `--cfg fuzzing`.
#[cfg(fuzzing)]
pub mod fuzz {
    /// # Panics
    ///
    /// Si un CSV accepté ne se relit pas à l'identique après réécriture : le fuzzer signale
    /// ainsi un bug.
    pub fn import_csv(data: &[u8]) {
        if let Ok(contents) = super::import_csv(data) {
            let refs: Vec<_> = contents.iter().collect();
            match super::write_contents(&refs) {
                Ok(csv) => assert_eq!(super::import_csv(&csv).as_ref(), Ok(&contents)),
                Err(error) => panic!("réécriture impossible : {error}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::collection::vec;
    use proptest::prelude::*;

    use super::*;

    const SPEC_EXAMPLE: &str = "title,kind,username,password,urls,notes,tags\r\n\
        GitHub,login,arthur,xxxx,https://github.com,,dev;perso\r\n\
        Code Wi-Fi,note,,,,\"Box: ABCD-1234\",maison\r\n";

    fn github() -> EntryContent {
        EntryContent {
            username: Some("arthur".to_owned()),
            password: Some(Secret::new("xxxx".to_owned())),
            urls: vec!["https://github.com".to_owned()],
            tags: Tags::new(["dev", "perso"]),
            ..EntryContent::new(EntryKind::Login, "GitHub".to_owned())
        }
    }

    fn wifi() -> EntryContent {
        EntryContent {
            notes: Some("Box: ABCD-1234".to_owned()),
            tags: Tags::new(["maison"]),
            ..EntryContent::new(EntryKind::Note, "Code Wi-Fi".to_owned())
        }
    }

    fn import_str(text: &str) -> Result<Vec<EntryContent>, CsvError> {
        import_csv(text.as_bytes())
    }

    fn header_then(lines: &str) -> String {
        format!("{}\r\n{lines}", HEADER.join(","))
    }

    #[test]
    fn spec_example_is_imported() {
        assert_eq!(import_str(SPEC_EXAMPLE), Ok(vec![github(), wifi()]));
    }

    #[test]
    fn test_writer_follows_the_import_format() {
        let csv = write_contents(&[&github(), &wifi()]).unwrap();
        let expected = "title,kind,username,password,urls,notes,tags\r\n\
            GitHub,login,arthur,xxxx,https://github.com,,dev;perso\r\n\
            Code Wi-Fi,note,,,,Box: ABCD-1234,maison\r\n";
        assert_eq!(std::str::from_utf8(&csv).unwrap(), expected);
    }

    #[test]
    fn quotes_commas_and_line_breaks_survive() {
        let content = EntryContent {
            notes: Some("ligne 1\r\nligne 2, \"citée\"".to_owned()),
            ..EntryContent::new(EntryKind::Note, "a,b".to_owned())
        };
        let csv = write_contents(&[&content]).unwrap();
        assert_eq!(import_csv(&csv), Ok(vec![content]));
    }

    #[test]
    fn utf8_bom_is_ignored() {
        let with_bom = [UTF8_BOM, SPEC_EXAMPLE.as_bytes()].concat();
        assert_eq!(import_csv(&with_bom), Ok(vec![github(), wifi()]));
    }

    #[test]
    fn header_only_gives_no_entry() {
        assert_eq!(import_str(&header_then("")), Ok(Vec::new()));
    }

    #[test]
    fn missing_or_wrong_header_is_rejected() {
        for text in [
            "",
            "title,kind\r\n",
            "Title,kind,username,password,urls,notes,tags\r\n",
        ] {
            assert_eq!(import_str(text), Err(CsvError::InvalidHeader), "{text:?}");
        }
    }

    #[test]
    fn kind_is_case_insensitive() {
        let entries = import_str(&header_then("a,LOGIN,,,,,\r\nb,Note,,,,,\r\n")).unwrap();
        assert_eq!(entries[0].kind, EntryKind::Login);
        assert_eq!(entries[1].kind, EntryKind::Note);
    }

    #[test]
    fn invalid_line_is_reported_with_its_number() {
        let cases = [
            ("a,login,,,,\r\n", LineProblem::FieldCount),
            ("a,login,,,,,,\r\n", LineProblem::FieldCount),
            ("a,card,,,,,\r\n", LineProblem::UnknownKind),
        ];
        for (line, problem) in cases {
            let text = header_then(&format!(
                "ok,note,,,,,\r\n\"multi\r\nligne\",note,,,,,\r\n{line}"
            ));
            assert_eq!(
                import_str(&text),
                Err(CsvError::InvalidLine { line: 5, problem }),
                "{line:?}"
            );
        }
    }

    #[test]
    fn line_numbers_handle_lf_and_blank_lines() {
        let text = "title,kind,username,password,urls,notes,tags\nok,note,,,,,\n\n\nbad,note\n";
        assert_eq!(
            import_str(text),
            Err(CsvError::InvalidLine {
                line: 5,
                problem: LineProblem::FieldCount
            })
        );
    }

    #[test]
    fn line_numbers_handle_old_mac_line_endings() {
        let text = "title,kind,username,password,urls,notes,tags\rok,note,,,,,\rbad,note\r";
        assert_eq!(
            import_str(text),
            Err(CsvError::InvalidLine {
                line: 3,
                problem: LineProblem::FieldCount
            })
        );
    }

    #[test]
    fn line_numbers_handle_blank_crlf_lines() {
        let text =
            "title,kind,username,password,urls,notes,tags\r\nok,note,,,,,\r\n\r\n\r\nbad,note\r\n";
        assert_eq!(
            import_str(text),
            Err(CsvError::InvalidLine {
                line: 5,
                problem: LineProblem::FieldCount
            })
        );
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let mut data = header_then("").into_bytes();
        data.extend_from_slice(b"a\xFF,note,,,,,\r\n");
        assert_eq!(
            import_csv(&data),
            Err(CsvError::InvalidLine {
                line: 2,
                problem: LineProblem::InvalidUtf8
            })
        );
    }

    #[test]
    fn lists_escape_separators_and_backslashes() {
        let items = vec!["a;b".to_owned(), "c\\d".to_owned(), "e".to_owned()];
        let encoded = encode_list(&items);
        assert_eq!(encoded, "a\\;b;c\\\\d;e");
        assert_eq!(decode_list(&encoded), items);
    }

    #[test]
    fn lone_backslash_is_kept_and_empty_items_dropped() {
        assert_eq!(decode_list("a\\b;;c\\"), ["a\\b", "c\\"]);
        assert_eq!(decode_list(""), Vec::<String>::new());
    }

    #[test]
    fn empty_cells_are_absent_fields() {
        let entries = import_str(&header_then("a,login,,,,,\r\n")).unwrap();
        assert_eq!(
            entries,
            [EntryContent::new(EntryKind::Login, "a".to_owned())]
        );
    }

    #[test]
    fn errors_never_quote_the_content() {
        let text = header_then("a,login,,hunter2,,\r\n");
        let message = import_str(&text).unwrap_err().to_string();
        assert_eq!(message, "ligne 2 : 7 colonnes attendues");
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                CsvError::InvalidHeader,
                "en-tête CSV attendu : title,kind,username,password,urls,notes,tags",
            ),
            (
                CsvError::InvalidLine {
                    line: 3,
                    problem: LineProblem::UnknownKind,
                },
                "ligne 3 : type inconnu (login ou note attendu)",
            ),
            (
                CsvError::InvalidLine {
                    line: 4,
                    problem: LineProblem::InvalidUtf8,
                },
                "ligne 4 : texte qui n'est pas en UTF-8",
            ),
            (CsvError::Internal, "erreur interne de lecture CSV"),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }

    fn arb_text() -> impl Strategy<Value = String> {
        "(?s).{0,8}"
    }

    fn arb_non_empty() -> impl Strategy<Value = String> {
        "(?s).{1,8}"
    }

    /// Contenu déjà normalisé : champs vides absents, listes sans élément vide.
    fn arb_content() -> impl Strategy<Value = EntryContent> {
        (
            prop_oneof![Just(EntryKind::Login), Just(EntryKind::Note)],
            arb_text(),
            proptest::option::of(arb_non_empty()),
            proptest::option::of(arb_non_empty()),
            vec(arb_non_empty(), 0..3),
            proptest::option::of(arb_non_empty()),
            vec(arb_text(), 0..3),
        )
            .prop_map(|(kind, title, username, password, urls, notes, tags)| {
                EntryContent {
                    kind,
                    title,
                    username,
                    password: password.map(Secret::new),
                    urls,
                    notes,
                    tags: Tags::new(tags),
                }
            })
    }

    proptest! {
        #[test]
        fn import_reverses_export(contents in vec(arb_content(), 0..5)) {
            let refs: Vec<&EntryContent> = contents.iter().collect();
            let csv = write_contents(&refs).unwrap();
            prop_assert_eq!(import_csv(&csv), Ok(contents));
        }
    }
}
