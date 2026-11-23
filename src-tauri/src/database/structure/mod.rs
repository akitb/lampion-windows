//! The Structure tab and its sheets (database part B1). Pure logic, unit-tested without a server:
//! `sql` builds every statement and checks names, `load` turns the server's answers into what the tab shows.

pub mod load;
pub mod sql;
