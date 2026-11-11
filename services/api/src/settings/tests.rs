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
    let _gone = fs::remove_dir_all(&folder);
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

    // then: the work is kept in memory, without accounts, anyone may sign up, mails go nowhere
    assert_eq!(settings.backend().expect("built in"), Backend::InMemory);
    assert_eq!(settings.accounts().expect("built in"), Accounts::Off);
    assert_eq!(settings.sign_up().expect("built in"), SignUp::Open);
    assert_eq!(settings.mailing(), Ok(Mailing::Nowhere));
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
    assert_eq!(settings.mailing(), Ok(Mailing::Nowhere));
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
        Ok(Mailing::Dropped {
            into: "/mails".into()
        })
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

const A_KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

#[test]
fn without_a_totp_key_there_is_none() {
    // given: no settings
    let folder = a_folder();

    // when: the settings are read
    let settings = Settings::read(no_arguments(), variables(&[]), &folder)
        .expect("nothing to read is no failure");

    // then: there is no key
    assert!(settings.totp_key().expect("none is fine").is_none());
}

#[test]
fn a_totp_key_is_given_itself_or_in_a_file_of_its_own() {
    // given: a file holding a key, with a line break after it
    let folder = a_folder();
    let key_file = a_file(&folder, "totp.key", &format!("{A_KEY}\n"));

    // when: the key is given by a variable
    let given = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_TOTP_KEY, A_KEY)]),
        &folder,
    )
    .expect("a key is read");
    // then: there is a key
    assert!(given.totp_key().expect("a key").is_some());
    // when: it is given as the file by a variable
    let in_file = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_TOTP_KEY_FILE, &key_file.display().to_string())]),
        &folder,
    )
    .expect("the file is read");
    // then: there is a key
    assert!(in_file.totp_key().expect("a key").is_some());
    // when: it is given as the file by the settings file
    a_file(
        &folder,
        BESIDE,
        &format!("[accounts]\ntotp_key_file = '{}'\n", key_file.display()),
    );
    let filed = Settings::read(no_arguments(), variables(&[]), &folder).expect("both are read");
    // then: there is a key
    assert!(filed.totp_key().expect("a key").is_some());
}

#[test]
fn a_totp_key_is_shown_nowhere() {
    // given: settings with a key
    let folder = a_folder();
    let settings = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_TOTP_KEY, A_KEY)]),
        &folder,
    )
    .expect("a key is read");

    // expect: debugging the settings does not show it
    assert!(!format!("{settings:?}").contains(&A_KEY[..16]));
    // when: a key that cannot be read is given
    let refused = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_TOTP_KEY, "0123456789abcdef-not-a-key")]),
        &folder,
    )
    .expect("the variable is read")
    .totp_key()
    .expect_err("not 64 hexadecimal digits");
    // then: the refusal names where it was set, and not what it is
    let refusal = refused.to_string();
    assert!(refusal.contains(WEAVELING_TOTP_KEY), "{refusal}");
    assert!(!refusal.contains("0123456789abcdef"), "{refusal}");
}

#[test]
fn a_totp_key_given_both_itself_and_in_a_file_stops_the_start() {
    // given: a key and a file of one, both by variables
    let folder = a_folder();
    let key_file = a_file(&folder, "totp.key", A_KEY);

    // when: the settings are read
    let refused = Settings::read(
        no_arguments(),
        variables(&[
            (WEAVELING_TOTP_KEY, A_KEY),
            (WEAVELING_TOTP_KEY_FILE, &key_file.display().to_string()),
        ]),
        &folder,
    );

    // then: it is refused, naming both
    assert!(
        matches!(&refused, Err(Unsettled::Doubled(one, other)) if one == WEAVELING_TOTP_KEY && other == WEAVELING_TOTP_KEY_FILE),
        "which of two keys was meant cannot be guessed: {refused:?}"
    );
}

#[test]
fn a_totp_key_file_that_cannot_be_read_stops_the_start() {
    // given: a variable naming a file that is not there
    let folder = a_folder();

    // when: the settings are read
    let refused = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_TOTP_KEY_FILE, "missing.key")]),
        &folder,
    );

    // then: it is refused, naming the variable and the file
    assert!(
        matches!(&refused, Err(Unsettled::SecretFile(source, path, _)) if source == WEAVELING_TOTP_KEY_FILE && path.ends_with("missing.key")),
        "{refused:?}"
    );
}

#[test]
fn a_totp_key_by_variable_wins_over_the_files() {
    // given: a settings file giving a key
    let folder = a_folder();
    a_file(
        &folder,
        BESIDE,
        "[accounts]\ntotp_key = '0000000000000000000000000000000000000000000000000000000000000000'\n",
    );

    // when: the settings are read with the key by variable as well
    let settings = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_TOTP_KEY, "not a key")]),
        &folder,
    )
    .expect("both are read");

    // then: the variable's holds
    let refusal = settings
        .totp_key()
        .expect_err("the variable's is no key")
        .to_string();
    assert!(refusal.contains(WEAVELING_TOTP_KEY), "{refusal}");
}

#[test]
fn the_browser_tests_settings_hold_a_totp_key() {
    // given: the settings file the browser tests start the server with
    let folder = a_folder();
    let e2e = a_file(
        &folder,
        "e2e.toml",
        include_str!("../../../../clients/web/e2e/weaveling.toml"),
    );

    // when: the settings are read from it
    let settings = Settings::read(
        ["--config".to_owned(), e2e.display().to_string()],
        variables(&[]),
        &folder,
    )
    .expect("the file is read");

    // then: it holds a key, so two-step sign-in can be set up in every browser test
    assert!(settings.totp_key().expect("a key").is_some());
}

#[test]
fn without_a_public_url_no_key_can_be_added() {
    // given: no settings
    let folder = a_folder();

    // when: the settings are read
    let settings = Settings::read(no_arguments(), variables(&[]), &folder)
        .expect("nothing to read is no failure");

    // then: no key ceremonies are held
    assert!(settings.key_ceremonies().expect("none is fine").is_none());
}

#[test]
fn a_public_url_is_read_from_the_file_and_a_variable_wins_over_it() {
    // given: a settings file naming where the instance is served
    let folder = a_folder();
    a_file(
        &folder,
        BESIDE,
        "[server]\npublic_url = \"https://weaveling.example.com\"\n",
    );

    // when: the settings are read
    let filed = Settings::read(no_arguments(), variables(&[]), &folder).expect("the file is read");

    // then: keys can be made for it
    assert!(filed.key_ceremonies().expect("an address").is_some());
    // when: a variable names an address keys cannot be bound to
    let refused = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_PUBLIC_URL, "http://weaveling.example.com")]),
        &folder,
    )
    .expect("both are read")
    .key_ceremonies();
    // then: the variable's is the one refused, saying why
    let refusal = refused.expect_err("http away from localhost").to_string();
    assert!(
        refusal.contains(WEAVELING_PUBLIC_URL) && refusal.contains("https"),
        "{refusal}"
    );
}

#[test]
fn a_public_url_keys_cannot_be_bound_to_says_where_it_was_written() {
    // given: a settings file naming the instance by its IP address
    let folder = a_folder();
    a_file(
        &folder,
        BESIDE,
        "[server]\npublic_url = \"https://192.0.2.7\"\n",
    );

    // when: the key ceremonies are asked for
    let refused = Settings::read(no_arguments(), variables(&[]), &folder)
        .expect("the file itself is read")
        .key_ceremonies();

    // then: it is refused, naming the key, the value and the reason
    let refusal = refused.expect_err("an IP address").to_string();
    assert!(
        refusal.contains("server.public_url in")
            && refusal.contains("https://192.0.2.7")
            && refusal.contains("domain name"),
        "{refusal}"
    );
}

#[test]
fn no_client_is_handed_out_unless_one_is_named() {
    // given: no settings, and not started by cargo-leptos
    let folder = a_folder();

    // when: the settings are read
    let settings = Settings::read(no_arguments(), variables(&[]), &folder)
        .expect("nothing to read is no failure");

    // then: the server hands out no client
    assert_eq!(settings.client(), None);
}

#[test]
fn the_client_is_named_by_the_file_a_variable_or_else_the_site_cargo_leptos_built() {
    // given: a settings file naming the client's folder
    let folder = a_folder();
    a_file(&folder, BESIDE, "[server]\nclient_dir = \"filed/site\"\n");

    // expect: the file's folder wins over the site cargo-leptos names
    let filed = Settings::read(
        no_arguments(),
        variables(&[("LEPTOS_SITE_ROOT", "target/site")]),
        &folder,
    )
    .expect("the file is read");
    assert_eq!(
        filed.client().map(|client| client.dir),
        Some(PathBuf::from("filed/site"))
    );
    // and: a variable wins over the file
    let told = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_CLIENT_DIR, "told/site")]),
        &folder,
    )
    .expect("both are read");
    assert_eq!(
        told.client().map(|client| client.dir),
        Some(PathBuf::from("told/site"))
    );
    // and: with nothing else, the site cargo-leptos built is handed out
    let built = Settings::read(
        no_arguments(),
        variables(&[("LEPTOS_SITE_ROOT", "target/site")]),
        &a_folder(),
    )
    .expect("a variable is read");
    assert_eq!(
        built.client().map(|client| client.dir),
        Some(PathBuf::from("target/site"))
    );
}

#[test]
fn the_page_reloads_on_changes_only_while_cargo_leptos_watches() {
    // given: a client named, and cargo-leptos telling its reload port
    let folder = a_folder();
    let reading = |watching: &str| {
        Settings::read(
            no_arguments(),
            variables(&[
                ("LEPTOS_SITE_ROOT", "target/site"),
                ("LEPTOS_RELOAD_PORT", "3001"),
                ("LEPTOS_WATCH", watching),
            ]),
            &folder,
        )
        .expect("variables are read")
        .client()
        .and_then(|client| client.reloading)
    };

    // expect: while it watches, the port is told to the page
    assert_eq!(reading("true"), Some(3001));
    // and: otherwise nothing reloads
    assert_eq!(reading(""), None);
}

#[test]
fn mail_goes_to_the_log_only_when_the_file_or_a_variable_says_so() {
    // given: a file writing mail to the log
    let folder = a_folder();
    a_file(
        &folder,
        "weaveling.toml",
        "[mail]
log = true
",
    );

    // expect: mail goes to the log
    let settings =
        Settings::read(no_arguments(), variables(&[]), &folder).expect("the file beside is read");
    assert_eq!(settings.mailing(), Ok(Mailing::Logged));
    // and: the variable turns it off again
    let settings = Settings::read(
        no_arguments(),
        variables(&[(WEAVELING_MAIL_LOG, "off")]),
        &folder,
    )
    .expect("the file beside is read");
    assert_eq!(settings.mailing(), Ok(Mailing::Nowhere));
}
