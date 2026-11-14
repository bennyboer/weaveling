use assets_core::{
    CatalogError, Format, Held, Holder, Holders, Owner, ProjectLink, Purpose, Sorted,
    UploadsSorted, Usage,
};
use eventsourcing::AccountId;

pub(crate) const PROJECT: &str = "project";
pub(crate) const ACCOUNT: &str = "account";
pub(crate) const INSTANCE: &str = "instance";

pub(crate) fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

/// How an owner is written: its kind and its id.
pub(crate) fn owner_written(owner: &Owner) -> (&'static str, String) {
    match owner {
        Owner::Project(project) => (PROJECT, project.to_string()),
        Owner::Account(account) => (ACCOUNT, account.to_string()),
    }
}

/// Reads an owner back from its stored kind and id.
/// An unknown kind is an error.
pub(crate) fn owner_read(kind: &str, owner: &str) -> Result<Owner, CatalogError> {
    match kind {
        PROJECT => Ok(Owner::Project(ProjectLink::from(owner))),
        ACCOUNT => Ok(Owner::Account(AccountId::from(owner))),
        unknown => Err(unreadable(format!("no owner is of the kind {unknown}"))),
    }
}

pub(crate) fn format_read(format: &str) -> Result<Format, CatalogError> {
    match format {
        "jpeg" => Ok(Format::Jpeg),
        "png" => Ok(Format::Png),
        "gif" => Ok(Format::Gif),
        "webp" => Ok(Format::Webp),
        unknown => Err(unreadable(format!("no format is {unknown}"))),
    }
}

fn unreadable(why: String) -> CatalogError {
    CatalogError::Backend(why.into())
}

/// How a holder is written: its kind and its id.
pub(crate) fn holder_written(holder: &Holder) -> (&'static str, String) {
    match holder {
        Holder::Account(account) => (ACCOUNT, account.to_string()),
    }
}

/// Reads a holder back from its stored kind and id. An unknown kind is an error.
pub(crate) fn holder_read(kind: &str, holder: &str) -> Result<Holder, CatalogError> {
    match kind {
        ACCOUNT => Ok(Holder::Account(AccountId::from(holder))),
        unknown => Err(unreadable(format!("no holder is of the kind {unknown}"))),
    }
}

pub(crate) fn purpose_written(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::AccountPicture => "account-picture",
    }
}

/// Reads a purpose back. An unknown purpose is an error.
pub(crate) fn purpose_read(purpose: &str) -> Result<Purpose, CatalogError> {
    match purpose {
        "account-picture" => Ok(Purpose::AccountPicture),
        unknown => Err(unreadable(format!("no asset is used for {unknown}"))),
    }
}

pub(crate) fn format_written(format: Format) -> &'static str {
    match format {
        Format::Jpeg => "jpeg",
        Format::Png => "png",
        Format::Gif => "gif",
        Format::Webp => "webp",
    }
}

pub(crate) fn size_written(size: u64) -> Result<i64, CatalogError> {
    i64::try_from(size).map_err(unreachable)
}

/// Returns the totals an asset counts in, each as a bound and whose total it is.
/// The installation's total always counts.
/// The uploader's total counts if there is an uploader.
/// The project's total counts if the asset belongs to a project.
pub(crate) fn bounds_of(
    uploader: Option<&str>,
    owner_kind: &str,
    owner: &str,
) -> Vec<(&'static str, String)> {
    let mut bounds = vec![(INSTANCE, String::new())];
    if let Some(uploader) = uploader {
        bounds.push((ACCOUNT, uploader.to_owned()));
    }
    if owner_kind == PROJECT {
        bounds.push((PROJECT, owner.to_owned()));
    }

    bounds
}

/// Builds the usage from the totals that were read, matching each total by its bound.
/// A total that was never stored counts as zero.
pub(crate) fn usage_of(
    totals: impl IntoIterator<Item = (String, i64)>,
) -> Result<Usage, CatalogError> {
    let mut usage = Usage::default();
    for (bound, total) in totals {
        let total = u64::try_from(total).map_err(unreachable)?;
        match bound.as_str() {
            INSTANCE => usage.instance = total,
            ACCOUNT => usage.account = total,
            PROJECT => usage.project = total,
            _ => {}
        }
    }

    Ok(usage)
}

/// The text a person is searched by: name and email, in lowercase.
/// Storing it in lowercase makes search ignore case the same way on every backend.
pub(crate) fn person_searched(name: &str, email: &str) -> String {
    format!("{name} {email}").to_lowercase()
}

pub(crate) fn project_searched(name: &str) -> String {
    name.to_lowercase()
}

/// Turns a search term into a `LIKE` pattern that matches it anywhere, in lowercase.
/// The characters `%`, `_` and `\` in the term are escaped, so they match literally.
pub(crate) fn pattern_of(term: &str) -> String {
    let mut pattern = String::from("%");
    for sign in term.to_lowercase().chars() {
        if matches!(sign, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(sign);
    }
    pattern.push('%');

    pattern
}

/// Builds the SQL for the storage overview, using the backend's placeholder style.
///
/// The parameters are bound in this order: the bound and the page size,
/// then the cursor's total (only when sorted by use) and id, and last the search pattern.
///
/// With a search term the query starts from the names and joins their totals.
/// That way a rare term does not make the database walk every total.
pub(crate) fn holdings_asked(
    placeholder: fn(usize) -> String,
    holders: Holders,
    sorted: Sorted,
    after: Option<&Held>,
    matching: bool,
) -> String {
    let mut next = 2;
    let mut bound_next = || {
        next += 1;
        placeholder(next)
    };
    let (names, key, email) = match holders {
        Holders::Accounts => ("asset_people", "account", "named.email"),
        Holders::Projects => ("asset_project_names", "project", "CAST(NULL AS TEXT)"),
    };
    let (joined, on) = if matching {
        (
            format!("{names} named CROSS JOIN asset_usage usage"),
            format!(" AND usage.id = named.{key}"),
        )
    } else {
        (
            format!("asset_usage usage LEFT JOIN {names} named ON named.{key} = usage.id"),
            String::new(),
        )
    };
    let mut asked = format!(
        "SELECT usage.id AS id, usage.total AS total, named.name AS name, {email} AS email \
         FROM {joined} \
         WHERE usage.bound = {} AND usage.total > 0{on}",
        placeholder(1)
    );
    if after.is_some() {
        match sorted {
            Sorted::MostUsed => {
                let total = bound_next();
                let id = bound_next();
                asked.push_str(&format!(
                    " AND (usage.total < {total} OR (usage.total = {total} AND usage.id > {id}))"
                ));
            }
            Sorted::ById => asked.push_str(&format!(" AND usage.id > {}", bound_next())),
        }
    }
    if matching {
        asked.push_str(&format!(
            " AND named.searched LIKE {} ESCAPE '\\'",
            bound_next()
        ));
    }
    asked.push_str(match sorted {
        Sorted::MostUsed => " ORDER BY usage.total DESC, usage.id",
        Sorted::ById => " ORDER BY usage.id",
    });
    asked.push_str(&format!(" LIMIT {}", placeholder(2)));

    asked
}

/// Builds the SQL for an account's uploads, using the backend's placeholder style.
///
/// The parameters are bound in this order: the account and the page size,
/// then the cursor's size or time (depending on the order) and its id.
/// The project's name is joined for uploads into projects.
pub(crate) fn uploads_asked(
    placeholder: fn(usize) -> String,
    sorted: UploadsSorted,
    after: bool,
) -> String {
    let key = match sorted {
        UploadsSorted::Largest => "summary.total_size",
        UploadsSorted::Newest => "summary.uploaded_at",
    };
    let mut asked = format!(
        "SELECT summary.id AS id, summary.owner_kind AS owner_kind, summary.owner AS owner, \
         summary.format AS format, summary.width AS width, summary.height AS height, \
         summary.total_size AS total_size, summary.uploaded_at AS uploaded_at, \
         named.name AS project_name \
         FROM asset_summaries summary \
         LEFT JOIN asset_project_names named \
         ON summary.owner_kind = '{PROJECT}' AND named.project = summary.owner \
         WHERE summary.uploader = {}",
        placeholder(1)
    );
    if after {
        let (mark, id) = (placeholder(3), placeholder(4));
        asked.push_str(&format!(
            " AND ({key} < {mark} OR ({key} = {mark} AND summary.id > {id}))"
        ));
    }
    asked.push_str(&format!(
        " ORDER BY {key} DESC, summary.id LIMIT {}",
        placeholder(2)
    ));

    asked
}
