use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

pub fn human_time(moment: OffsetDateTime) -> String {
    let here = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let shown = format_description!("[day] [month repr:short] [year], [hour]:[minute]");

    moment
        .to_offset(here)
        .format(shown)
        .unwrap_or_else(|_| "an unknown moment".to_owned())
}
