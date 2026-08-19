//! The pomo bank ledger.
//!
//! The bank stores **pomos** as its authoritative unit (1 completed focus pomo = 1
//! credit). It is a ledger of signed transactions: task rewards credit it, and the
//! user spends from it on whatever they like, with an optional note (see `SCOPE.md`).

use chrono::{DateTime, Utc};

use crate::domain::ids::{BankTransactionId, TaskId};

/// The kind of a bank ledger entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BankTransactionType {
    /// Pomos credited for completing a task.
    TaskReward,
    /// Pomos the user spent from the bank.
    Spend,
}

/// A signed entry in the pomo bank ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankTransaction {
    id: BankTransactionId,
    task_id: Option<TaskId>,
    amount_pomos: i32,
    transaction_type: BankTransactionType,
    note: Option<String>,
    created_at: DateTime<Utc>,
}

impl BankTransaction {
    /// Creates a ledger entry crediting or debiting `amount_pomos`.
    #[must_use]
    pub fn new(
        id: BankTransactionId,
        task_id: Option<TaskId>,
        amount_pomos: i32,
        transaction_type: BankTransactionType,
        note: Option<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            task_id,
            amount_pomos,
            transaction_type,
            note,
            created_at,
        }
    }

    /// What the transaction was for, when the user left a note.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// The transaction's identifier.
    #[must_use]
    pub fn id(&self) -> BankTransactionId {
        self.id
    }

    /// The task this transaction relates to, if any.
    #[must_use]
    pub fn task_id(&self) -> Option<TaskId> {
        self.task_id
    }

    /// The signed pomo amount (positive credits, negative debits).
    #[must_use]
    pub fn amount_pomos(&self) -> i32 {
        self.amount_pomos
    }

    /// The kind of transaction.
    #[must_use]
    pub fn transaction_type(&self) -> BankTransactionType {
        self.transaction_type
    }

    /// When the transaction was recorded.
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

/// Sums a ledger into the current pomo balance.
///
/// The sum widens each `i32` amount to `i64` so a long ledger cannot overflow.
#[must_use]
pub fn balance(transactions: &[BankTransaction]) -> i64 {
    transactions
        .iter()
        .map(|txn| i64::from(txn.amount_pomos()))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(0, 0).expect("epoch is a valid timestamp")
    }

    fn reward(id: i64, amount: i32) -> BankTransaction {
        BankTransaction::new(
            BankTransactionId::new(id),
            Some(TaskId::new(id)),
            amount,
            BankTransactionType::TaskReward,
            None,
            ts(),
        )
    }

    #[test]
    fn empty_ledger_has_zero_balance() {
        assert_eq!(balance(&[]), 0);
    }

    #[test]
    fn balance_sums_signed_amounts() {
        let ledger = [reward(1, 3), reward(2, 5), reward(3, -2)];
        assert_eq!(balance(&ledger), 6);
    }
}
