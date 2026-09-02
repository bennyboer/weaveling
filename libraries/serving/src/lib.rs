mod conditional;
mod refusal;

#[cfg(test)]
mod tests;

pub use conditional::{Unreadable, demanded, tag};
pub use refusal::refusal;
