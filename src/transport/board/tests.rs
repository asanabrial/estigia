use super::*;

fn meta_with(columns: &[(&str, &str)]) -> Meta {
    Meta {
        project_id: "P".to_owned(),
        field_id: "F".to_owned(),
        columns: columns
            .iter()
            .enumerate()
            .map(|(at, (name, description))| Column {
                id: format!("opt{at}"),
                name: (*name).to_owned(),
                description: (*description).to_owned(),
            })
            .collect(),
    }
}

#[test]
fn a_state_matches_a_column_by_name_or_description() {
    let meta = meta_with(&[
        ("Analysis", "status:analysis"),
        ("In progress", "status:in-progress"),
        ("Blocked", "blocked"),
        // The last column tracks the tracker's own closed flag, so it is
        // described as `closed` rather than `status:done`.
        ("Done", "closed"),
    ]);
    assert_eq!(
        meta.column_for("analysis").map(|column| column.name),
        Some("Analysis".to_owned()),
        "the description `status:analysis` is the label a board carries"
    );
    assert_eq!(
        meta.column_for("in-progress").map(|column| column.name),
        Some("In progress".to_owned()),
        "a space in a column's own name matches a dashed state"
    );
    assert_eq!(
        meta.column_for("blocked").map(|column| column.name),
        Some("Blocked".to_owned()),
        "a bare description equal to the state also matches"
    );
    assert_eq!(
        meta.column_for("done").map(|column| column.name),
        Some("Done".to_owned()),
        "the state's own name matches even where its description is `closed`"
    );
    assert_eq!(meta.column_for("review"), None, "no column mirrors review");
}

#[test]
fn a_project_item_carries_its_card_the_current_column_and_the_field() {
    let node = serde_json::json!({
        "id": "PVTI_1",
        "project": {
            "id": "PVT_1",
            "number": 12,
            "title": "Sprint",
            "fields": {
                "nodes": [
                    { "id": "F1", "name": "Priority", "options": [] },
                    {
                        "id": "F2",
                        "name": "Status",
                        "options": [
                            { "id": "o1", "name": "Ready", "description": "status:ready" },
                            { "id": "o2", "name": "Done", "description": "closed" },
                        ],
                    },
                ],
            },
        },
        "fieldValueByName": { "name": "Ready" },
    });
    let item = read_item(&node);
    assert_eq!(item.item_id, "PVTI_1");
    assert_eq!(item.project_id, "PVT_1");
    assert_eq!(item.number, 12);
    assert_eq!(item.title, "Sprint");
    assert_eq!(item.column.as_deref(), Some("Ready"));
    assert_eq!(
        item.meta
            .and_then(|meta| meta.column_for("done"))
            .map(|column| column.id),
        Some("o2".to_owned()),
        "the Status field and its options are read off the project the item names"
    );
}

#[test]
fn a_project_without_a_status_field_has_no_meta() {
    let node = serde_json::json!({
        "id": "PVTI_2",
        "project": {
            "id": "PVT_2",
            "number": 1,
            "title": "Elsewhere",
            "fields": { "nodes": [ { "id": "F1", "name": "Priority" } ] },
        },
    });
    let item = read_item(&node);
    assert!(item.meta.is_none(), "there is no Status field to address");
    assert!(item.column.is_none(), "an unset status reads as no column");
}
