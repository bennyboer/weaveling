use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static FOLDERS: AtomicUsize = AtomicUsize::new(0);

fn a_folder() -> PathBuf {
    let folder = env::temp_dir().join(format!(
        "weaveling-settings-{}-{}",
        std::process::id(),
        FOLDERS.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&folder).expect("a folder can be made");

    folder
}

fn a_file(folder: &Path, named: &str, written: &str) -> PathBuf {
    let path = folder.join(named);
    fs::write(&path, written).expect("a file can be written");

    path
}

fn variables(set: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let set: HashMap<String, String> = set
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();

    move |name| set.get(name).cloned()
}

fn no_arguments() -> Vec<String> {
    Vec::new()
}

#[test]
fn with_neither_a_file_nor_variables_everything_is_as_built_in() {
    // given: no settings file and no variables
    let folder = a_folder();

    // when: the settings are read
    let settings = Settings::read(no_arguments(), variables(&[]), &folder)
        .expect("nothing to read is no failure");

    // then: the work is kept in memory, without accounts, anyone may sign up, mails go to the log
    assert_eq!(settings.backend().expect("built in"), Backend::InMemory);
    assert_eq!(settings.accounts().expect("built in"), Accounts::Off);
    assert_eq!(settings.sign_up().expect("built in"), SignUp::Open);
    assert_eq!(settings.mailing(), Mailing::Logged);
    // and: nothing is refused on purpose
    assert!(settings.flakiness().expect("built in").is_none());
}

#[test]
fn a_file_beside_the_server_is_read_by_itself() {
    // given: a settings file beside the server
    let folder = a_folder();
    a_file(
        &folder,
        BESIDE,
        "[storage]\ndata = \"/srv/weaveling\"\n\n[accounts]\nenabled = true\nsign_up = \"invitation\"\n",
    );

    // when: the settings are read
    let settings =
        Settings::read(no_arguments(), variables(&[]), &folder).expect("a file beside is read");

    // then: what it says holds
    assert_eq!(
        settings.backend().expect("one place"),
        Backend::Sqlite {
            directory: "/srv/weaveling".into()
        }
    );
    assert_eq!(settings.accounts().expect("on"), Accounts::On);
    assert_eq!(
        settings.sign_up().expect("by invitation"),
        SignUp::ByInvitation
    );
}

#[test]
fn a_variable_wins_over_the_file() {
    // given: a settings file keeping the work in a folder
    let folder = a_folder();
    a_file(&folder, BESIDE, "[storage]\ndata = \"/srv/weaveling\"\n");

    // when: the settings are read with a variable naming another
    let settings = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_DATA, "/elsewhere")]),
        &folder,
    )
    .expect("both are read");

    // then: the variable's holds
    assert_eq!(
        settings.backend().expect("one place"),
        Backend::Sqlite {
            directory: "/elsewhere".into()
        }
    );
}

#[test]
fn a_blank_variable_leaves_the_file_s_value() {
    // given: a settings file turning accounts on
    let folder = a_folder();
    a_file(&folder, BESIDE, "[accounts]\nenabled = \"on\"\n");

    // when: the settings are read with the variable set but blank
    let settings = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_ACCOUNTS, "  ")]),
        &folder,
    )
    .expect("both are read");

    // then: the file's holds
    assert_eq!(settings.accounts().expect("on"), Accounts::On);
}

#[test]
fn a_file_named_by_flag_or_variable_is_read_in_place_of_the_one_beside() {
    // given: a file beside the server, and two others
    let folder = a_folder();
    a_file(&folder, BESIDE, "[accounts]\nsign_up = \"open\"\n");
    let flagged = a_file(
        &folder,
        "flagged.toml",
        "[accounts]\nsign_up = \"invitation\"\n",
    );
    let named = a_file(&folder, "named.toml", "[mail]\ndrop = \"/mails\"\n");

    // when: one is named by the flag and the other by the variable
    let settings = Settings::read(
        ["--config".to_owned(), flagged.display().to_string()],
        variables(&[(WEAVELING_CONFIG, &named.display().to_string())]),
        &folder,
    )
    .expect("the flagged file is read");

    // then: the flag's file holds
    assert_eq!(
        settings.sign_up().expect("by invitation"),
        SignUp::ByInvitation
    );
    // and: the variable's file is not read
    assert_eq!(settings.mailing(), Mailing::Logged);
    // when: only the variable names one
    let settings = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_CONFIG, &named.display().to_string())]),
        &folder,
    )
    .expect("the named file is read");
    // then: its holds, and not the one beside
    assert_eq!(
        settings.mailing(),
        Mailing::Dropped {
            into: "/mails".into()
        }
    );
}

#[test]
fn a_file_named_that_is_not_there_stops_the_start() {
    // given: no file where the flag points
    let folder = a_folder();

    // when: the settings are read
    let refused = Settings::read(
        ["--config=missing.toml".to_owned()],
        variables(&[]),
        &folder,
    );

    // then: it is refused, naming who named it
    assert!(
        matches!(&refused, Err(Unsettled::NoFile(by, _)) if by == "--config"),
        "a file asked for and missing is a mistake to be told, not a reason to start on defaults: {refused:?}"
    );
}

#[test]
fn a_key_the_file_does_not_know_stops_the_start() {
    // given: a settings file with a misspelt key
    let folder = a_folder();
    a_file(&folder, BESIDE, "[accounts]\nenabeld = true\n");

    // when: the settings are read
    let refused = Settings::read(no_arguments(), variables(&[]), &folder);

    // then: it is refused, naming the file
    assert!(
        matches!(&refused, Err(Unsettled::Unreadable(path, why)) if path.ends_with(BESIDE) && why.contains("enabeld")),
        "a misspelt key read as missing would leave the setting at its default without a word: {refused:?}"
    );
}

#[test]
fn a_value_that_cannot_be_read_says_where_it_was_written() {
    // given: a settings file with a word accounts do not know
    let folder = a_folder();
    a_file(&folder, BESIDE, "[accounts]\nenabled = \"maybe\"\n");

    // when: the settings are read and accounts asked for
    let settings =
        Settings::read(no_arguments(), variables(&[]), &folder).expect("the file itself is read");
    let refused = settings.accounts();

    // then: the refusal names the key and the file
    let refusal = refused
        .expect_err("maybe is neither on nor off")
        .to_string();
    assert!(
        refusal.contains("accounts.enabled in") && refusal.contains(BESIDE),
        "{refusal}"
    );
}

#[test]
fn the_server_listens_where_it_is_set_and_at_its_usual_place_otherwise() {
    // given: no settings
    let folder = a_folder();

    // when: the settings are read
    let settings = Settings::read(no_arguments(), variables(&[]), &folder)
        .expect("nothing to read is no failure");

    // then: it listens on this machine only, at port 3000
    assert_eq!(
        settings.listen().expect("built in"),
        "127.0.0.1:3000".parse().expect("an address")
    );
    // when: a file and a variable each name a place
    a_file(
        &folder,
        BESIDE,
        "[server]
listen = \"0.0.0.0:8000\"
",
    );
    let settings = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_LISTEN, "[::1]:9000")]),
        &folder,
    )
    .expect("both are read");
    // then: the variable's holds
    assert_eq!(
        settings.listen().expect("an address"),
        "[::1]:9000".parse().expect("an address")
    );
}

#[test]
fn a_place_to_listen_that_is_no_address_and_port_says_where_it_was_written() {
    // given: a settings file naming a port without an address
    let folder = a_folder();
    a_file(
        &folder,
        BESIDE,
        "[server]
listen = \"3000\"
",
    );

    // when: where to listen is asked
    let refused = Settings::read(no_arguments(), variables(&[]), &folder)
        .expect("the file itself is read")
        .listen();

    // then: it is refused, naming the key and what would do
    let refusal = refused.expect_err("a port alone is no place").to_string();
    assert!(
        refusal.contains("server.listen in") && refusal.contains("127.0.0.1:3000"),
        "{refusal}"
    );
}

#[test]
fn the_example_names_only_keys_the_server_knows() {
    // given: the example settings file with every key it shows written in
    let folder = a_folder();
    let example = include_str!("../../../../weaveling.example.toml");
    let written: String = example
        .lines()
        .map(|line| match line.strip_prefix("# ") {
            Some(key)
                if key
                    .split_once(" = ")
                    .is_some_and(|(name, _)| !name.contains(' ')) =>
            {
                key
            }
            _ => line,
        })
        .map(|line| {
            format!(
                "{line}
"
            )
        })
        .collect();
    a_file(&folder, BESIDE, &written);

    // when: the settings are read
    let read = Settings::read(no_arguments(), variables(&[]), &folder);

    // then: every key is one the server reads
    assert!(
        read.is_ok(),
        "the example is where an operator learns the keys, so a key it names and the server refuses would stop their first start: {read:?}"
    );
}

#[test]
fn numbers_and_truths_in_the_file_are_read_as_their_settings_are() {
    // given: a settings file writing a share as a number and accounts as a truth
    let folder = a_folder();
    a_file(
        &folder,
        BESIDE,
        "[accounts]\nenabled = false\n\n[testing]\nflaky = 0.25\n",
    );

    // when: the settings are read
    let settings =
        Settings::read(no_arguments(), variables(&[]), &folder).expect("the file is read");

    // then: they read as on/off and a share
    assert_eq!(settings.accounts().expect("off"), Accounts::Off);
    assert!(settings.flakiness().expect("a share").is_some());
}
