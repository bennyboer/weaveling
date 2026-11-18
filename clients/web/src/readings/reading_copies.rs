use std::fmt::{self, Display, Formatter};

use readings_contract::{ExtentDTO, ListedReadingCopyDTO};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http::ApiError;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadingCopyId(String);

impl From<String> for ReadingCopyId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl Display for ReadingCopyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// What a reading copy holds of the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extent {
    Book,
    Sections(u32),
    Scenes(u32),
}

/// A reading copy as the project's members find it listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedReadingCopy {
    pub id: ReadingCopyId,
    pub name: String,
    pub maker: String,
    pub made_by_you: bool,
    pub made_at: OffsetDateTime,
    pub extent: Extent,
    pub open_until: Option<OffsetDateTime>,
    pub deleted_on: Option<OffsetDateTime>,
    pub open: bool,
}

impl TryFrom<ListedReadingCopyDTO> for ListedReadingCopy {
    type Error = ApiError;

    fn try_from(dto: ListedReadingCopyDTO) -> Result<Self, ApiError> {
        Ok(Self {
            id: ReadingCopyId::from(dto.id),
            name: dto.name,
            maker: dto.maker,
            made_by_you: dto.made_by_you,
            made_at: moment(&dto.made_at)?,
            extent: match dto.extent {
                ExtentDTO::Book => Extent::Book,
                ExtentDTO::Sections { count } => Extent::Sections(count),
                ExtentDTO::Scenes { count } => Extent::Scenes(count),
            },
            open_until: dto.open_until.as_deref().map(moment).transpose()?,
            deleted_on: dto.deleted_on.as_deref().map(moment).transpose()?,
            open: dto.open,
        })
    }
}

fn moment(written: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(written, &Rfc3339).map_err(|_| ApiError::Unexpected)
}
