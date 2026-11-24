use super::is_text_edit;

pub(crate) const ADMITTED: &[&str] = &[
    "gh issue edit 3 --title x",
    "gh issue edit 3 --body-file /tmp/b.md",
    "gh issue edit 3 -t 'a; b' -F b.md",
    "gh issue edit https://github.com/o/r/issues/3 --body 'several\nlines'",
    "gh pr edit 3 --body-file b.md --title \"it's one\"",
    "gh pr edit --body=x",
    "gh pr edit '#3' --title=x --repo o/r",
    "gh pr edit https://github.com/o/r/pull/3 -b x -R o/r",
    "gh api -X PATCH repos/o/r/issues/3 -F body=@/tmp/b.md",
    "gh api --method PATCH /repos/o/r/pulls/3 -f title=x -f 'body=a b' --silent",
    "gh api repos/o/r/pulls/3 -XPATCH --raw-field body=x --jq .number",
];

pub(crate) const EXCLUDED: &[&str] = &[
    "gh issue edit 3",
    "gh issue edit --title x",
    "gh issue edit 3 4 --title x",
    "gh issue edit 3 --title x --add-label bug",
    "gh issue edit 3 --title x --add-assignee @me",
    "gh issue edit 3 --milestone x",
    "gh pr edit 3 --base main --title x",
    "gh pr edit 3 --add-reviewer someone",
    "gh pr edit branch --title x",
    "gh pr edit 3 --title",
    "gh pr edit https://github.com/o/r/issues/3 --title x",
    "gh pr edit 3 --title x; gh pr review 3 --approve",
    "gh pr edit 3 --title x && git push",
    "gh pr edit 3 --title x | cat",
    "gh pr edit 3 --title x > out",
    "gh pr edit 3 --title x\ngh pr review 3 --approve",
    "gh pr edit 3 --title \"$(gh pr review 3 --approve)\"",
    "gh pr edit 3 --title \"`gh pr review 3 --approve`\"",
    "gh pr edit 3 --title $X",
    "gh pr edit 3 --title 'x",
    "FOO=1 gh pr edit 3 --title x",
    "sudo gh pr edit 3 --title x",
    "gh --repo o/r pr edit 3 --title x",
    "gh pr comment 3 --body x",
    "gh pr review 3 --approve",
    "gh pr close 3",
    "gh pr ready 3",
    "gh issue close 3",
    "gh api repos/o/r/issues/3 -f body=x",
    "gh api -X POST repos/o/r/issues/3 -f body=x",
    "gh api -X PATCH -X DELETE repos/o/r/issues/3 -f body=x",
    "gh api -X PATCH repos/o/r/issues/3",
    "gh api -X PATCH repos/o/r/issues/3 -f state=closed",
    "gh api -X PATCH repos/o/r/issues/3 -f body=x -f state=closed",
    "gh api -X PATCH repos/o/r/pulls/3 -f base=main -f title=x",
    "gh api -X PATCH repos/o/r/issues/3 --input b.json",
    "gh api -X PATCH repos/o/r/issues/comments/3 -f body=x",
    "gh api -X PATCH repos/o/r/pulls/3/reviews/1 -f body=x",
    "gh api -X PATCH repos/o/r/releases/3 -f body=x",
    "gh api -X PATCH repos/o/r -f title=x",
    "gh api -X PATCH repos/o/r/issues/3 repos/o/r/issues/4 -f body=x",
    "gh api graphql -f query='mutation { updateIssue(input: {}) { x } }'",
];

#[test]
fn a_plain_title_or_body_edit_is_one() {
    for line in ADMITTED {
        assert!(is_text_edit(line), "{line}");
    }
}

#[test]
fn anything_more_than_a_title_or_a_body_is_not() {
    for line in EXCLUDED {
        assert!(!is_text_edit(line), "{line}");
    }
}
