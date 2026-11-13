/// The most any limit may be, in bytes: what every backend counts.
pub const MOST_COUNTED: u64 = i64::MAX as u64;

/// What a limit bounds: one upload, every asset of the installation, all an account uploaded, or all a project holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    Upload,
    Instance,
    Account,
    Project,
}

/// The most each bound may hold, in bytes; none where it is not bounded, as an account is not without accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Limits {
    pub each_upload: Option<u64>,
    pub instance: Option<u64>,
    pub each_account: Option<u64>,
    pub each_project: Option<u64>,
}

/// What the bounds an upload counts against hold already: the installation, the account uploading, the project it goes into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub instance: u64,
    pub account: u64,
    pub project: u64,
}

/// One limit as it stands: the most its bound may hold, and what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub bound: Bound,
    pub most: u64,
    pub used: u64,
}

/// Which bounds an upload counts against beside the installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counted {
    pub by_an_account: bool,
    pub into_a_project: bool,
}

impl Limit {
    /// Whether the bound takes so many bytes more.
    pub fn takes(&self, adding: u64) -> bool {
        self.used.saturating_add(adding) <= self.most
    }
}

impl Limits {
    /// The limit on one upload, which takes it or not before anything is made of it.
    pub fn on_the_upload(&self) -> Option<Limit> {
        self.each_upload.map(|most| Limit {
            bound: Bound::Upload,
            most,
            used: 0,
        })
    }

    /// The limits what is kept for an upload counts against, as they stand, the installation's first.
    pub fn on_what_is_kept(&self, usage: Usage, counted: Counted) -> Vec<Limit> {
        [
            (Bound::Instance, self.instance, usage.instance, true),
            (
                Bound::Account,
                self.each_account,
                usage.account,
                counted.by_an_account,
            ),
            (
                Bound::Project,
                self.each_project,
                usage.project,
                counted.into_a_project,
            ),
        ]
        .into_iter()
        .filter_map(|(bound, most, used, counts)| {
            counts.then_some(())?;
            most.map(|most| Limit { bound, most, used })
        })
        .collect()
    }
}

/// The first of the limits that does not take so many bytes more.
pub fn first_passed(limits: impl IntoIterator<Item = Limit>, adding: u64) -> Option<Limit> {
    limits.into_iter().find(|limit| !limit.takes(adding))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERYTHING: Counted = Counted {
        by_an_account: true,
        into_a_project: true,
    };

    fn every_limit() -> Limits {
        Limits {
            each_upload: Some(10),
            instance: Some(1_000),
            each_account: Some(100),
            each_project: Some(50),
        }
    }

    #[test]
    fn a_limit_takes_up_to_its_most_and_nothing_past_it() {
        // given: a limit of a hundred holding ninety
        let limit = Limit {
            bound: Bound::Account,
            most: 100,
            used: 90,
        };

        // expect: ten more taken, eleven not
        assert!(limit.takes(10));
        assert!(!limit.takes(11));
        // and: however much is asked, nothing overflows
        assert!(!limit.takes(u64::MAX));
    }

    #[test]
    fn an_upload_is_bounded_alone_before_anything_is_made_of_it() {
        // expect: the upload's limit is its most, nothing used
        assert_eq!(
            every_limit().on_the_upload(),
            Some(Limit {
                bound: Bound::Upload,
                most: 10,
                used: 0,
            })
        );
        // and: none where none is set
        assert_eq!(Limits::default().on_the_upload(), None);
    }

    #[test]
    fn what_is_kept_counts_against_the_installation_the_account_and_the_project() {
        // given: what each holds
        let usage = Usage {
            instance: 500,
            account: 60,
            project: 45,
        };

        // when: an upload by an account into a project would add ten
        let passed = first_passed(every_limit().on_what_is_kept(usage, EVERYTHING), 10);

        // then: the project's limit is the one passed
        assert_eq!(
            passed,
            Some(Limit {
                bound: Bound::Project,
                most: 50,
                used: 45,
            })
        );
        // and: five would have been taken by every limit
        assert_eq!(
            first_passed(every_limit().on_what_is_kept(usage, EVERYTHING), 5),
            None
        );
    }

    #[test]
    fn the_installations_limit_is_told_first_when_several_are_passed() {
        // given: every bound full
        let usage = Usage {
            instance: 1_000,
            account: 100,
            project: 50,
        };

        // expect: the installation's limit told
        assert_eq!(
            first_passed(every_limit().on_what_is_kept(usage, EVERYTHING), 1)
                .map(|limit| limit.bound),
            Some(Bound::Instance)
        );
    }

    #[test]
    fn only_the_bounds_an_upload_counts_against_and_set_are_asked() {
        // given: an account and a project both full, the installation not
        let usage = Usage {
            instance: 0,
            account: 100,
            project: 50,
        };

        // expect: an upload by no account into no project is taken
        assert_eq!(
            first_passed(
                every_limit().on_what_is_kept(
                    usage,
                    Counted {
                        by_an_account: false,
                        into_a_project: false,
                    }
                ),
                1
            ),
            None,
            "without accounts nobody is held to an account's limit"
        );
        // and: no limit set takes anything
        assert!(
            Limits::default()
                .on_what_is_kept(usage, EVERYTHING)
                .is_empty()
        );
    }
}
