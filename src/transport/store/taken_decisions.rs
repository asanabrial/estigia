use super::{Store, int, unreadable};
use crate::transport::Failure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TakenDecision {
    pub(crate) question: String,
    pub(crate) decision: String,
}

impl Store {
    pub(crate) fn decision_taken(
        &self,
        run_id: &str,
        taken: &TakenDecision,
        now: u64,
    ) -> Result<(), Failure> {
        self.connection()
            .execute(
                "INSERT INTO taken_decisions (run_id, question, decision, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![run_id, taken.question, taken.decision, int(now)],
            )
            .map_err(unreadable("record a decision taken"))?;
        Ok(())
    }

    pub(crate) fn decisions_reported(
        &self,
        run_id: &str,
        now: u64,
    ) -> Result<Vec<TakenDecision>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "UPDATE taken_decisions SET reported_at = ?2
                 WHERE run_id = ?1 AND reported_at IS NULL
                 RETURNING id, question, decision",
            )
            .map_err(unreadable("take the unreported decisions"))?;
        let mut taken = statement
            .query_map(rusqlite::params![run_id, int(now)], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    TakenDecision {
                        question: row.get(1)?,
                        decision: row.get(2)?,
                    },
                ))
            })
            .map_err(unreadable("take the unreported decisions"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("take the unreported decisions"))?;
        taken.sort_by_key(|(id, _)| *id);
        Ok(taken.into_iter().map(|(_, decision)| decision).collect())
    }
}
