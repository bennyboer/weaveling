use assets_core::{CatalogError, Format, Owner, Usage};

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
