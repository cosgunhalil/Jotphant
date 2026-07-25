//! Strongly-typed identifiers for domain entities.
//!
//! Each identifier is an [`i64`] newtype (matching the SQLite row id) so that ids of
//! different entities cannot be interchanged (see `CODING_STANDARDS.md` §6). The wrapper
//! is erased at compile time — zero runtime cost.

/// Defines an `i64`-backed identifier newtype with a constructor and accessor.
macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(i64);

        impl $name {
            /// Wraps a raw identifier value.
            #[must_use]
            pub fn new(value: i64) -> Self {
                Self(value)
            }

            /// Returns the underlying raw identifier value.
            #[must_use]
            pub fn value(self) -> i64 {
                self.0
            }
        }
    };
}

id_type!(
    /// Identifies a task.
    TaskId
);
id_type!(
    /// Identifies a Pomodoro session.
    PomodoroSessionId
);
id_type!(
    /// Identifies a bank ledger transaction.
    BankTransactionId
);
id_type!(
    /// Identifies a note.
    NoteId
);
