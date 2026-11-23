//! The data/query half of the Database Manager (part A). Everything here except `run` is pure logic,
//! unit-tested without a server: statement splitting, SQL building, completion, formatting, history.

pub mod complete;
pub mod format;
pub mod history;
pub mod marks;
pub mod run;
pub mod split;
pub mod sql;
