//! What the databases keep, each in the catalog's own form, apart from the API's,
//! so either can change without the other, and a form changes only together with a migration.
//! One file for each table that holds such a form; the blocks are shared by the scenes' texts and the copies' contents.

mod blocks;
pub(crate) mod books;
pub(crate) mod contents;
pub(crate) mod copies;
pub(crate) mod scene_texts;
