use assets_core::{
    CatalogError, Format, Held, Holders, Owner, ProjectLink, Sorted, UploadsSorted, Usage,
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

/// The owner read back from its kind and id; a kind never written is a failure.
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

/// The totals an asset counts in, each a bound and whose: the installation's, its uploader's where there is one, its project's where it is a project's.
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

/// The usage read back from the totals asked, each by its bound; a total never kept is nothing.
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

/// What a person is found by: their name and address, lowered, so a term of any case finds them on every backend alike.
pub(crate) fn person_searched(name: &str, email: &str) -> String {
    format!("{name} {email}").to_lowercase()
}

pub(crate) fn project_searched(name: &str) -> String {
    name.to_lowercase()
}

/// The pattern a term is looked for by: anywhere in what is searched, lowered, its own `%`, `_` and `\` taken as they are.
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

/// How the holdings are asked for, written with the backend's placeholder: the bound and how many first, then the last listed's total, where sorted by it, and id, and last the pattern. Where a term is given the names are searched first and their totals joined, so a term found by few is not weighed against every holding.
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

/// How an account's uploads are asked for, written with the backend's placeholder: the account and how many first, then the last listed's size or moment, as sorted, and its id; each project's name joined where the assets heard it.
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
