use std::time::Duration;

use serde_json::{Value, json};

use super::{GoalReading, goal_reading};
use crate::harness::hook::{
    Dialect, Event, Input, background_work_outstanding, run_as, stop_block,
};
use crate::harness::loop_text::{Invocation, continuation};
use crate::harness::{GateContext, session};

const UNCONFIRMED_ATTEMPTS: u32 = 3;
const UNCONFIRMED_RETRY_PAUSE: Duration = Duration::from_millis(500);

fn retry_pause() {
    std::thread::sleep(UNCONFIRMED_RETRY_PAUSE);
}

/// What the shared loop decision reached.
pub(super) enum Decision {
    /// Keep going; the value is the `Stop` answer that blocks.
    Continue(Value),
    /// End the loop; `reason` says why, `terminal` whether a later turn should
    /// stay stopped or the loop may continue, and `requested` whether a pending
    /// stop-after request is what asked for it.
    Stop {
        value: Value,
        reason: String,
        terminal: bool,
        requested: bool,
    },
}

impl Decision {
    /// The `Stop` answer, whichever way the decision went.
    pub(super) fn into_value(self) -> Value {
        match self {
            Self::Continue(value) => value,
            Self::Stop { value, .. } => value,
        }
    }

    /// Whether a pending stop-after request asked for this stop, so the caller
    /// may consume the request once the stop is durable.
    pub(super) fn consumes_stop_request(&self) -> bool {
        matches!(
            self,
            Self::Stop {
                requested: true,
                ..
            }
        )
    }
}

pub(super) fn answer(
    dialect: Dialect,
    agent: Option<&str>,
    event: Event,
    input: &Input,
    context: Option<&GateContext>,
) -> Value {
    if event == Event::Stop
        && !input.unreadable
        && !input.absent
        && let Some(context) = context
        && let Some(agent) = agent
    {
        let run_id = session::run_id(session::runtime_of_agent(Some(agent)), &input.session_id);
        if !session::is_nameless(&run_id) {
            if agent == "codex" {
                let decision = decide(&run_id, Invocation::Mention, None, retry_pause, || {
                    goal_reading(context, &run_id, false)
                });
                if decision.consumes_stop_request() {
                    session::clear_stop_after(&context.state_root, &run_id);
                }
                return decision.into_value();
            }
            if agent == "claude-code" {
                return claude(context, input, &run_id);
            }
        }
    }
    run_as(dialect, agent, event, input, context)
}

/// Claude Code's `Stop`, when the `Loop plugin` row registered the loop entry.
///
/// With no loop stored the answer is the ordinary claim-holding one, byte for
/// byte; with one running it continues, stops or waits on the shared decision.
fn claude(context: &GateContext, input: &Input, run_id: &str) -> Value {
    let Some(mut goal) = session::loop_goal(&context.state_root, run_id) else {
        return run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::Stop,
            input,
            Some(context),
        );
    };
    // A stopped loop is terminal for the loop, not for the gate: the next turn
    // gets the ordinary claim-holding answer, which an empty answer would skip.
    if goal.stopped.is_some() {
        return run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::Stop,
            input,
            Some(context),
        );
    }
    if background_work_outstanding(&input.transcript_path) {
        return json!({});
    }
    let decision = decide(run_id, Invocation::Slash, goal.max, retry_pause, || {
        goal_reading(context, run_id, goal.max.is_some())
    });
    match decision {
        Decision::Continue(value) => {
            session::record_loop_answer(
                &context.state_root,
                run_id,
                &mut goal,
                "the turn continues",
                false,
            );
            value
        }
        Decision::Stop {
            value,
            reason,
            terminal,
            requested,
        } => {
            let recorded = session::record_loop_answer(
                &context.state_root,
                run_id,
                &mut goal,
                &reason,
                terminal,
            );
            // Consumed only once the stop is durable: a record that could not
            // be written leaves the request for the next turn to honour again.
            if requested && recorded {
                session::clear_stop_after(&context.state_root, run_id);
            }
            value
        }
    }
}

fn unconfirmed(reading: &GoalReading) -> bool {
    matches!(reading.standing, "unread" | "unreadable")
}

fn ended(why: &str) -> Value {
    json!({ "systemMessage": format!("Estigia loop ended: {why}") })
}

fn waiting_on(reading: &GoalReading) -> String {
    [
        (
            reading.held,
            "in progress or in review that this run cannot claim at the last read",
        ),
        (
            reading.pending,
            "blocked on a condition that has not come true",
        ),
    ]
    .into_iter()
    .filter_map(|(count, what)| {
        count
            .filter(|count| *count > 0)
            .map(|count| format!("{count} {what}"))
    })
    .collect::<Vec<_>>()
    .join(", ")
}

/// The one loop decision, shared by Codex's `Stop` and Claude Code's.
///
/// `max` is the completion cap a stored loop carries; `pause` is how the caller
/// waits between attempts at a read that has not answered yet, and `read` is
/// asked with the completion count wanted only when a cap is set.
pub(super) fn decide(
    run_id: &str,
    invocation: Invocation,
    max: Option<u64>,
    mut pause: impl FnMut(),
    mut read: impl FnMut() -> GoalReading,
) -> Decision {
    let mut reading = read();
    let mut attempts = 1;
    while unconfirmed(&reading) && attempts < UNCONFIRMED_ATTEMPTS {
        pause();
        reading = read();
        attempts += 1;
    }
    if unconfirmed(&reading) {
        let retries = attempts - 1;
        let reason = format!(
            "Estigia could not confirm this run's claim ({}) after {retries} retries",
            reading.standing
        );
        return Decision::Stop {
            value: ended(&reason),
            reason,
            terminal: false,
            requested: false,
        };
    }
    if let Some(cap) = max {
        // Ordering is the OpenCode plugin's: the cap is decided before the
        // claim, so a run already at its cap stops even holding a live one.
        match reading.completed {
            None => {
                let reason =
                    "Estigia did not report how many issues this run has completed".to_owned();
                return Decision::Stop {
                    value: ended(&reason),
                    reason,
                    terminal: true,
                    requested: false,
                };
            }
            Some(completed) if completed >= cap => {
                let reason = format!("the completion cap ({cap}) was reached");
                return Decision::Stop {
                    value: ended(&reason),
                    reason,
                    terminal: true,
                    requested: false,
                };
            }
            Some(_) => {}
        }
    }
    if reading.live
        && let Some(issue) = &reading.issue
    {
        return Decision::Continue(stop_block(issue, run_id));
    }
    if let Some(reason) = &reading.stop_after {
        let why = format!("the agent asked to stop after the current task ({reason})");
        return Decision::Stop {
            value: ended(&why),
            reason: why,
            terminal: true,
            requested: true,
        };
    }
    let (Some(claimable), Some(dischargeable)) = (reading.claimable, reading.dischargeable) else {
        let reason = "Estigia did not report how much work this run's queues hold".to_owned();
        return Decision::Stop {
            value: ended(&reason),
            reason,
            terminal: true,
            requested: false,
        };
    };
    if claimable + dischargeable > 0 {
        return Decision::Continue(json!({
            "decision": "block",
            "reason": continuation(invocation, run_id, dischargeable),
        }));
    }
    let waiting = waiting_on(&reading);
    if !waiting.is_empty() {
        let reason =
            format!("no claimable or dischargeable work, and the queue waits on {waiting}");
        return Decision::Stop {
            value: ended(&format!(
                "{reason}; a Stop hook cannot wait, so the turn ends here"
            )),
            reason,
            terminal: false,
            requested: false,
        };
    }
    if let Some(count) = reading.unadmitted.filter(|count| *count > 0) {
        let reason = format!(
            "no claimable or dischargeable work, and {count} open issue(s) the store has never \
             admitted: admit each with an explicit `transition`, and the loop can claim it"
        );
        return Decision::Stop {
            value: ended(&format!(
                "{reason}; a Stop hook cannot wait, so the turn ends here"
            )),
            reason,
            terminal: false,
            requested: false,
        };
    }
    let unread = if reading.held.is_none() || reading.pending.is_none() {
        " (what is held or pending could not be read)"
    } else {
        ""
    };
    let left = match reading.unconditioned {
        Some(count) if count > 0 => format!(
            "; {count} blocked with no recorded condition, or one that is not well formed, are \
             left for a person"
        ),
        _ => String::new(),
    };
    let reason = format!(
        "Estigia reports no live claim and no claimable or dischargeable work{unread}{left}"
    );
    Decision::Stop {
        value: ended(&reason),
        reason,
        terminal: true,
        requested: false,
    }
}
