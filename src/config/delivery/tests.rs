use super::super::{Config, Group, SETTINGS, Setting, Tracker, Workflow};
use super::{Corrections, DeliveryPreset, Party, ReviewStage, StageKind};

fn with(rows: &[(Setting, &str)]) -> Config {
    let mut config = Config::default();
    for (setting, value) in rows {
        setting
            .apply(&mut config, value)
            .unwrap_or_else(|refusal| panic!("{} {value:?}: {refusal:?}", setting.label()));
    }
    config
}

fn linear() -> Config {
    Config {
        tracker: Tracker::Linear {
            team: "ENG".to_owned(),
        },
        ..Config::default()
    }
}

#[test]
fn the_delivery_group_is_drawn_in_the_order_the_steps_happen_with_the_preset_first() {
    let drawn: Vec<Setting> = SETTINGS
        .iter()
        .copied()
        .filter(|setting| setting.group() == Group::Delivery)
        .collect();
    assert_eq!(
        drawn,
        [
            Setting::DeliveryPreset,
            Setting::Workflow,
            Setting::StateNames,
            Setting::SelfReview,
            Setting::ReadyBy,
            Setting::ExternalReviewers,
            Setting::CorrectionsAfterReady,
            Setting::MergeBy,
        ]
    );
    let at = |group: Group| Group::ALL.iter().position(|each| *each == group);
    assert_eq!(at(Group::Delivery), at(Group::Repository).map(|at| at + 1));
}

#[test]
fn choosing_a_preset_sets_every_row_it_stands_for() {
    for preset in DeliveryPreset::ALL {
        let config = with(&[(Setting::DeliveryPreset, preset.as_value())]);
        let rows = preset.rows();
        assert_eq!(config.workflow, rows.workflow);
        assert_eq!(config.self_review(), rows.self_review);
        assert_eq!(config.ready_by(), rows.ready_by);
        assert_eq!(config.corrections_after_ready(), rows.corrections);
        assert_eq!(config.merge_by(), rows.merge_by);
        assert_eq!(Setting::DeliveryPreset.value_of(&config), preset.as_value());
    }
}

#[test]
fn changing_a_row_after_a_preset_reads_custom_and_keeps_the_others() {
    let config = with(&[
        (Setting::DeliveryPreset, "team"),
        (Setting::SelfReview, "off"),
    ]);
    assert_eq!(Setting::DeliveryPreset.value_of(&config), "custom");
    assert_eq!(config.workflow, Workflow::Assisted);
    assert_eq!(config.ready_by(), Party::Agent);
    assert_eq!(config.corrections_after_ready(), Corrections::PushReady);
    assert_eq!(config.merge_by(), Party::Person);
}

#[test]
fn a_custom_preset_writes_nothing() {
    let config = with(&[(Setting::DeliveryPreset, "team")]);
    let mut again = config.clone();
    Setting::DeliveryPreset
        .apply(&mut again, "custom")
        .expect("custom is accepted");
    assert_eq!(again, config);
}

#[test]
fn rows_left_unwritten_follow_what_claim_workflow_stands_for() {
    let assisted = with(&[(Setting::Workflow, "assisted")]);
    assert_eq!(
        Setting::DeliveryPreset.value_of(&assisted),
        "assisted",
        "an installed `Claim workflow assisted` with no other row is the assisted preset"
    );
    assert!(!assisted.self_review());
    assert_eq!(assisted.ready_by(), Party::Person);
    assert_eq!(assisted.merge_by(), Party::Person);
    assert_eq!(assisted.delivery_conflict(), None);
    assert_eq!(
        Setting::DeliveryPreset.value_of(&Config::default()),
        "autonomous"
    );
    assert_eq!(Config::default().delivery_conflict(), None);
}

#[test]
fn autonomous_refuses_a_delivery_row_it_fixes_naming_both_rows() {
    for (setting, value) in [
        (Setting::SelfReview, "off"),
        (Setting::ReadyBy, "person"),
        (Setting::CorrectionsAfterReady, "push-ready"),
        (Setting::MergeBy, "person"),
        (Setting::ExternalReviewers, "review:any"),
    ] {
        let refusal = with(&[(setting, value)])
            .delivery_conflict()
            .unwrap_or_else(|| panic!("{} {value} under autonomous", setting.label()));
        assert_eq!(refusal.code, "autonomous-fixes-delivery-rows");
        assert!(
            refusal.message.contains("Claim workflow"),
            "{}",
            refusal.message
        );
        assert!(
            refusal.message.contains(setting.label()),
            "{}",
            refusal.message
        );
    }
}

#[test]
fn assisted_refuses_an_agent_merge() {
    let refusal = with(&[(Setting::Workflow, "assisted"), (Setting::MergeBy, "agent")])
        .delivery_conflict()
        .expect("an assisted run holds no claim to deliver under");
    assert_eq!(refusal.code, "assisted-merge-is-the-persons");
    assert!(refusal.message.contains("Merge by") && refusal.message.contains("Claim workflow"));
}

#[test]
fn state_names_need_a_linear_tracker() {
    let rows = [
        (Setting::DeliveryPreset, "team"),
        (Setting::StateNames, "in-progress=In Progress"),
    ];
    let refusal = with(&rows)
        .delivery_conflict()
        .expect("GitHub states are labels Estigia writes already");
    assert_eq!(refusal.code, "state-names-need-linear");
    assert!(refusal.message.contains("State names") && refusal.message.contains("Tracker"));

    let mut on_linear = linear();
    for (setting, value) in rows {
        setting.apply(&mut on_linear, value).expect("a row");
    }
    assert_eq!(on_linear.delivery_conflict(), None);
}

#[test]
fn the_team_flow_with_cubic_and_a_teammate_is_one_coherent_configuration() {
    let mut config = linear();
    for (setting, value) in [
        (Setting::DeliveryPreset, "team"),
        (
            Setting::StateNames,
            "in-progress=In Progress, review=In Review",
        ),
        (
            Setting::ExternalReviewers,
            "check:cubic@cubic-dev-ai[bot], review:any",
        ),
    ] {
        setting.apply(&mut config, value).expect("a row");
    }
    assert_eq!(config.delivery_conflict(), None);
    assert_eq!(
        config.external_reviewers,
        vec![
            ReviewStage {
                kind: StageKind::Check {
                    name: "cubic".to_owned(),
                    author: Some("cubic-dev-ai[bot]".to_owned()),
                },
                blocking: true,
            },
            ReviewStage {
                kind: StageKind::Review {
                    reviewer: "any".to_owned(),
                },
                blocking: true,
            },
        ]
    );
}

#[test]
fn external_reviewers_read_back_as_written() {
    for written in [
        "none",
        "check:cubic",
        "check:cubic · AI code reviewer@cubic-dev-ai[bot], review:acme/backend:advisory",
        "review:octocat, review:any:advisory",
    ] {
        let config = with(&[(Setting::ExternalReviewers, written)]);
        assert_eq!(Setting::ExternalReviewers.value_of(&config), written);
    }
    let blocking = with(&[(Setting::ExternalReviewers, "review:any:blocking")]);
    assert_eq!(Setting::ExternalReviewers.value_of(&blocking), "review:any");
}

#[test]
fn a_stage_that_names_nothing_is_refused() {
    for written in [
        "check:",
        "check:cubic@",
        "review:",
        "review:two words",
        "person:octocat",
        "cubic",
        "review:any,",
    ] {
        let refusal = Setting::ExternalReviewers
            .apply(&mut Config::default(), written)
            .expect_err(written);
        assert_eq!(refusal.code, "config-value-unrecognised", "{written}");
    }
}

#[test]
fn state_names_take_only_the_states_estigia_moves_and_each_once() {
    let config = with(&[(Setting::StateNames, "In-Progress=Doing , done=Shipped")]);
    assert_eq!(
        Setting::StateNames.value_of(&config),
        "in-progress=Doing, done=Shipped"
    );
    for written in [
        "blocked=Waiting",
        "review=",
        "review=In Review, review=QA",
        "In Review",
    ] {
        let refusal = Setting::StateNames
            .apply(&mut Config::default(), written)
            .expect_err(written);
        assert_eq!(refusal.code, "config-value-unrecognised", "{written}");
    }
}

#[test]
fn only_the_rows_a_preset_does_not_name_are_fixed_under_autonomous() {
    let autonomous = Config::default();
    let assisted = with(&[(Setting::Workflow, "assisted")]);
    for setting in SETTINGS.iter().copied() {
        let fixed = matches!(
            setting,
            Setting::StateNames
                | Setting::SelfReview
                | Setting::ReadyBy
                | Setting::ExternalReviewers
                | Setting::CorrectionsAfterReady
                | Setting::MergeBy
        );
        assert_eq!(
            setting.fixed_by(&autonomous).is_some(),
            fixed,
            "{}",
            setting.label()
        );
        assert_eq!(setting.fixed_by(&assisted), None, "{}", setting.label());
    }
}
