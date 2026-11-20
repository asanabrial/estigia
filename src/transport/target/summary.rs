//! Verbatim moves and whole-file deletions in the review payload.
//!
//! The payload is what every judge of an epoch reads, and the ceiling on it is
//! a ceiling on bytes a reader is handed. Content the change removes is already
//! in the base, and content moved between paths appears twice in a plain diff,
//! so both can be replaced by a line that names exactly what is not shown and a
//! digest over the bytes it stands for.

use std::collections::HashMap;

use crate::transport::ownership::sha256_hex;

pub(crate) const MOVED_PREFIX: &str = "# estigia-moved ";
pub(crate) const DELETED_PREFIX: &str = "# estigia-deleted ";
pub(crate) const BLOCK_LINES: usize = 8;
const CANDIDATES_PER_LINE: usize = 64;

pub(crate) struct Summary {
    pub(crate) text: String,
    pub(crate) moves: usize,
    pub(crate) deletions: usize,
    pub(crate) saved: usize,
}

struct Parsed<'a> {
    lines: Vec<&'a str>,
    sections: Vec<Section>,
    runs: Vec<Run>,
}

struct Section {
    start: usize,
    end: usize,
    readable: bool,
    whole_deletion: bool,
    whole_addition: bool,
    hunks: Vec<Hunk>,
}

struct Hunk {
    header: usize,
    end: usize,
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    tail: String,
    runs: Vec<usize>,
}

struct Run {
    first: usize,
    last: usize,
    bytes: Vec<u8>,
    path: String,
    start: usize,
    end: usize,
    addition: bool,
}

impl Run {
    fn count(&self) -> usize {
        self.end + 1 - self.start
    }
}

pub(crate) fn summarise(raw: &str) -> Summary {
    let Some(mut parsed) = parse(raw) else {
        return Summary {
            text: raw.to_owned(),
            moves: 0,
            deletions: 0,
            saved: 0,
        };
    };
    let shifts = split_at_blocks(&mut parsed);
    let matches = matched_runs(&parsed, &shifts);
    let text = render(&parsed, &matches).concat();
    let saved = raw.len().saturating_sub(text.len());
    Summary {
        text,
        moves: matches.moves,
        deletions: matches.deletions,
        saved,
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Indent {
    Removed(String),
    Added(String),
}

impl Indent {
    fn between(from: &str, to: &str) -> Option<Self> {
        let prefix = |long: &str, short: &str| {
            long.strip_suffix(short)
                .filter(|prefix| {
                    !prefix.is_empty() && prefix.bytes().all(|byte| byte == b' ' || byte == b'\t')
                })
                .map(str::to_owned)
        };
        if from.trim().is_empty() || to.trim().is_empty() {
            return None;
        }
        prefix(from, to)
            .map(Self::Removed)
            .or_else(|| prefix(to, from).map(Self::Added))
    }

    fn carries(&self, from: &str, to: &str) -> bool {
        if from.trim().is_empty() {
            return from == to;
        }
        match self {
            Self::Removed(prefix) => from.strip_prefix(prefix.as_str()) == Some(to),
            Self::Added(prefix) => to.strip_prefix(prefix.as_str()) == Some(from),
        }
    }

    fn field(&self) -> String {
        let (key, prefix) = match self {
            Self::Removed(prefix) => ("indent-removed", prefix),
            Self::Added(prefix) => ("indent-added", prefix),
        };
        format!("\"{key}\":{},", serde_json::json!(prefix))
    }
}

struct Shift {
    from: usize,
    to: usize,
    indent: Indent,
}

fn split_at_blocks(parsed: &mut Parsed<'_>) -> Vec<Shift> {
    let content = |line: usize| &parsed.lines[line][1..];
    let mut destinations: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (index, run) in parsed
        .runs
        .iter()
        .enumerate()
        .filter(|(_, run)| run.addition)
    {
        for offset in 0..run.count() {
            destinations
                .entry(content(run.first + offset))
                .or_default()
                .push((index, offset));
        }
    }
    let mut claimed = vec![false; parsed.lines.len()];
    let mut cuts: Vec<Vec<usize>> = vec![Vec::new(); parsed.runs.len()];
    for (index, run) in parsed
        .runs
        .iter()
        .enumerate()
        .filter(|(_, run)| !run.addition)
    {
        let mut at = 0;
        while at < run.count() {
            let mut best = (0, 0, 0);
            let candidates = destinations
                .get(content(run.first + at))
                .map_or(&[][..], Vec::as_slice);
            for &(other, offset) in candidates.iter().take(CANDIDATES_PER_LINE) {
                let to = &parsed.runs[other];
                if to.path == run.path {
                    continue;
                }
                let mut length = 0;
                while at + length < run.count()
                    && offset + length < to.count()
                    && !claimed[to.first + offset + length]
                    && content(run.first + at + length) == content(to.first + offset + length)
                {
                    length += 1;
                }
                if length > best.0 {
                    best = (length, other, offset);
                }
            }
            let (length, other, offset) = best;
            if length < BLOCK_LINES {
                at += 1;
                continue;
            }
            let to_first = parsed.runs[other].first;
            for line in &mut claimed[to_first + offset..to_first + offset + length] {
                *line = true;
            }
            for line in &mut claimed[run.first + at..run.first + at + length] {
                *line = true;
            }
            cuts[index].extend([at, at + length]);
            cuts[other].extend([offset, offset + length]);
            at += length;
        }
    }
    let mut shifted: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (index, run) in parsed
        .runs
        .iter()
        .enumerate()
        .filter(|(_, run)| run.addition)
    {
        for offset in 0..run.count() {
            let line = content(run.first + offset);
            if !claimed[run.first + offset] && !line.trim().is_empty() {
                shifted
                    .entry(line.trim_start())
                    .or_default()
                    .push((index, offset));
            }
        }
    }
    let mut found: Vec<(usize, usize, usize, usize, Indent)> = Vec::new();
    for (index, run) in parsed
        .runs
        .iter()
        .enumerate()
        .filter(|(_, run)| !run.addition)
    {
        let mut at = 0;
        while at < run.count() {
            let line = content(run.first + at);
            if claimed[run.first + at] || line.trim().is_empty() {
                at += 1;
                continue;
            }
            let mut best: Option<(usize, usize, usize, Indent)> = None;
            let candidates = shifted
                .get(line.trim_start())
                .map_or(&[][..], Vec::as_slice);
            for &(other, offset) in candidates.iter().take(CANDIDATES_PER_LINE) {
                let to = &parsed.runs[other];
                if to.path == run.path || claimed[to.first + offset] {
                    continue;
                }
                let Some(indent) = Indent::between(line, content(to.first + offset)) else {
                    continue;
                };
                let mut length = 0;
                while at + length < run.count()
                    && offset + length < to.count()
                    && !claimed[run.first + at + length]
                    && !claimed[to.first + offset + length]
                    && indent.carries(
                        content(run.first + at + length),
                        content(to.first + offset + length),
                    )
                {
                    length += 1;
                }
                if best.as_ref().is_none_or(|(longest, ..)| length > *longest) {
                    best = Some((length, other, offset, indent));
                }
            }
            let Some((length, other, offset, indent)) =
                best.filter(|(length, ..)| *length >= BLOCK_LINES)
            else {
                at += 1;
                continue;
            };
            let to_first = parsed.runs[other].first;
            for line in &mut claimed[to_first + offset..to_first + offset + length] {
                *line = true;
            }
            for line in &mut claimed[run.first + at..run.first + at + length] {
                *line = true;
            }
            cuts[index].extend([at, at + length]);
            cuts[other].extend([offset, offset + length]);
            found.push((index, at, other, offset, indent));
            at += length;
        }
    }
    if cuts.iter().all(Vec::is_empty) {
        return Vec::new();
    }
    let mut pieces: Vec<Run> = Vec::with_capacity(parsed.runs.len());
    let mut replaced: Vec<Vec<usize>> = Vec::with_capacity(parsed.runs.len());
    let mut starts: Vec<Vec<(usize, usize)>> = Vec::with_capacity(parsed.runs.len());
    for (run, mut cut) in parsed.runs.drain(..).zip(cuts) {
        cut.push(0);
        cut.push(run.count());
        cut.sort_unstable();
        cut.dedup();
        let mut indices = Vec::with_capacity(cut.len() - 1);
        let mut offsets = Vec::with_capacity(cut.len() - 1);
        for bounds in cut.windows(2) {
            offsets.push((bounds[0], pieces.len()));
            indices.push(pieces.len());
            pieces.push(piece(&parsed.lines, &run, bounds[0], bounds[1]));
        }
        replaced.push(indices);
        starts.push(offsets);
    }
    let piece_at = |run: usize, offset: usize| {
        starts[run]
            .iter()
            .find(|(start, _)| *start == offset)
            .map(|(_, piece)| *piece)
    };
    let shifts: Vec<Shift> = found
        .into_iter()
        .filter_map(|(index, at, other, offset, indent)| {
            Some(Shift {
                from: piece_at(index, at)?,
                to: piece_at(other, offset)?,
                indent,
            })
        })
        .collect();
    parsed.runs = pieces;
    for hunk in parsed
        .sections
        .iter_mut()
        .flat_map(|section| section.hunks.iter_mut())
    {
        hunk.runs = hunk
            .runs
            .iter()
            .flat_map(|index| replaced[*index].iter().copied())
            .collect();
    }
    shifts
}

fn piece(lines: &[&str], run: &Run, from: usize, to: usize) -> Run {
    let mut bytes: Vec<u8> = lines[run.first + from..run.first + to]
        .iter()
        .flat_map(|line| line.as_bytes()[1..].iter().copied())
        .collect();
    let last = if to == run.count() {
        run.last
    } else {
        run.first + to
    };
    if to == run.count() && run.last > run.first + run.count() && bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Run {
        first: run.first + from,
        last,
        bytes,
        path: run.path.clone(),
        start: run.start + from,
        end: run.start + to - 1,
        addition: run.addition,
    }
}

struct Matches {
    pair: Vec<Option<usize>>,
    indent: Vec<Option<Indent>>,
    deletion: Vec<bool>,
    moves: usize,
    deletions: usize,
}

impl Matches {
    fn indent(&self, one: usize, other: usize) -> Option<&Indent> {
        self.indent[one].as_ref().or(self.indent[other].as_ref())
    }
}

fn matched_runs(parsed: &Parsed<'_>, shifts: &[Shift]) -> Matches {
    let mut pair: Vec<Option<usize>> = vec![None; parsed.runs.len()];
    let mut indent: Vec<Option<Indent>> = vec![None; parsed.runs.len()];
    let mut taken = vec![false; parsed.runs.len()];
    let mut deletion = vec![false; parsed.runs.len()];
    for shift in shifts {
        let line = moved_line(
            &parsed.runs[shift.from],
            &parsed.runs[shift.to],
            Some(&shift.indent),
        );
        if line.len() < line_bytes(parsed, shift.from) + line_bytes(parsed, shift.to) {
            pair[shift.from] = Some(shift.to);
            pair[shift.to] = Some(shift.from);
            taken[shift.from] = true;
            taken[shift.to] = true;
            indent[shift.from] = Some(shift.indent.clone());
        }
    }
    let mut additions: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, run) in parsed
        .runs
        .iter()
        .enumerate()
        .filter(|(_, run)| run.addition)
    {
        additions
            .entry(sha256_hex(&run.bytes))
            .or_default()
            .push(index);
    }
    for (index, run) in parsed
        .runs
        .iter()
        .enumerate()
        .filter(|(_, run)| !run.addition)
    {
        if taken[index] {
            continue;
        }
        let Some(candidates) = additions.get(&sha256_hex(&run.bytes)) else {
            continue;
        };
        for &other in candidates {
            if taken[other] {
                continue;
            }
            let to = &parsed.runs[other];
            if to.path == run.path || to.bytes != run.bytes {
                continue;
            }
            let line = moved_line(run, to, None);
            if line.len() < line_bytes(parsed, index) + line_bytes(parsed, other) {
                pair[index] = Some(other);
                pair[other] = Some(index);
                taken[index] = true;
                taken[other] = true;
                break;
            }
        }
    }
    for (index, run) in parsed
        .runs
        .iter()
        .enumerate()
        .filter(|(index, run)| !run.addition && pair[*index].is_none())
    {
        let Some(section) = parsed
            .sections
            .iter()
            .find(|section| section.whole_deletion && owns(section, index))
        else {
            continue;
        };
        if deleted_line(run).len() < line_bytes(parsed, index) + header_bytes(parsed, section) {
            deletion[index] = true;
        }
    }
    let moves = pair.iter().filter(|one| one.is_some()).count() / 2;
    let deletions = deletion.iter().filter(|one| **one).count();
    Matches {
        pair,
        indent,
        deletion,
        moves,
        deletions,
    }
}

fn owns(section: &Section, run: usize) -> bool {
    section.hunks.iter().any(|hunk| hunk.runs.contains(&run))
}

fn line_bytes(parsed: &Parsed<'_>, run: usize) -> usize {
    let run = &parsed.runs[run];
    parsed.lines[run.first..run.last]
        .iter()
        .map(|line| line.len())
        .sum()
}

fn header_bytes(parsed: &Parsed<'_>, section: &Section) -> usize {
    section
        .hunks
        .iter()
        .map(|hunk| parsed.lines[hunk.header].len())
        .sum()
}

fn render(parsed: &Parsed<'_>, matches: &Matches) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(parsed.lines.len());
    let mut cursor = 0;
    for section in &parsed.sections {
        copy(parsed, &mut out, cursor, section.start);
        cursor = section.end;
        if !section.readable || !section_changed(section, matches) {
            copy(parsed, &mut out, section.start, section.end);
            continue;
        }
        let first_hunk = section
            .hunks
            .first()
            .map_or(section.end, |hunk| hunk.header);
        copy(parsed, &mut out, section.start, first_hunk);
        if (section.whole_deletion || section.whole_addition) && section.hunks[0].runs.len() == 1 {
            let run = section.hunks[0].runs[0];
            match matches.pair[run] {
                Some(other) => out.push(moved_line(
                    &parsed.runs[run],
                    &parsed.runs[other],
                    matches.indent(run, other),
                )),
                None => out.push(deleted_line(&parsed.runs[run])),
            }
            continue;
        }
        let mut position = first_hunk;
        for hunk in &section.hunks {
            copy(parsed, &mut out, position, hunk.header);
            position = hunk.end;
            if !hunk_changed(hunk, matches) {
                copy(parsed, &mut out, hunk.header, hunk.end);
                continue;
            }
            out.push(rewritten_header(hunk, parsed, matches));
            let mut at = hunk.header + 1;
            while at < hunk.end {
                let found = hunk
                    .runs
                    .iter()
                    .copied()
                    .find(|index| parsed.runs[*index].first == at);
                let Some(index) = found else {
                    out.push(parsed.lines[at].to_owned());
                    at += 1;
                    continue;
                };
                let one = &parsed.runs[index];
                if let Some(other) = matches.pair[index] {
                    out.push(moved_line(
                        one,
                        &parsed.runs[other],
                        matches.indent(index, other),
                    ));
                } else if matches.deletion[index] {
                    out.push(deleted_line(one));
                } else {
                    copy(parsed, &mut out, one.first, one.last);
                }
                at = one.last;
            }
        }
        copy(parsed, &mut out, position, section.end);
    }
    copy(parsed, &mut out, cursor, parsed.lines.len());
    out
}

fn copy(parsed: &Parsed<'_>, out: &mut Vec<String>, start: usize, end: usize) {
    out.extend(
        parsed.lines[start..end]
            .iter()
            .map(|line| (*line).to_owned()),
    );
}

fn section_changed(section: &Section, matches: &Matches) -> bool {
    section.hunks.iter().any(|hunk| hunk_changed(hunk, matches))
}

fn hunk_changed(hunk: &Hunk, matches: &Matches) -> bool {
    hunk.runs
        .iter()
        .any(|index| matches.pair[*index].is_some() || matches.deletion[*index])
}

fn rewritten_header(hunk: &Hunk, parsed: &Parsed<'_>, matches: &Matches) -> String {
    let mut old_count = hunk.old_count;
    let mut new_count = hunk.new_count;
    for index in &hunk.runs {
        if matches.pair[*index].is_some() || matches.deletion[*index] {
            let run = &parsed.runs[*index];
            if run.addition {
                new_count = new_count.saturating_sub(run.count());
            } else {
                old_count = old_count.saturating_sub(run.count());
            }
        }
    }
    format!(
        "@@ -{} +{} @@{}\n",
        side(hunk.old_start, old_count),
        side(hunk.new_start, new_count),
        hunk.tail
    )
}

fn side(start: usize, count: usize) -> String {
    match count {
        1 => start.to_string(),
        _ => format!("{start},{count}"),
    }
}

fn moved_line(one: &Run, other: &Run, indent: Option<&Indent>) -> String {
    let (from, to) = if one.addition {
        (other, one)
    } else {
        (one, other)
    };
    format!(
        "{MOVED_PREFIX}{{\"from\":{from_path},\"from-lines\":{from_lines},\"to\":{to_path},\"to-lines\":{to_lines},{indent}\"bytes\":{bytes},\"sha256\":{digest}}}\n",
        from_path = serde_json::json!(from.path),
        from_lines = serde_json::json!(format!("{}-{}", from.start, from.end)),
        to_path = serde_json::json!(to.path),
        to_lines = serde_json::json!(format!("{}-{}", to.start, to.end)),
        indent = indent.map(Indent::field).unwrap_or_default(),
        bytes = from.bytes.len(),
        digest = serde_json::json!(sha256_hex(&from.bytes)),
    )
}

fn deleted_line(run: &Run) -> String {
    format!(
        "{DELETED_PREFIX}{{\"path\":{path},\"lines\":{lines},\"bytes\":{bytes},\"sha256\":{digest}}}\n",
        path = serde_json::json!(run.path),
        lines = serde_json::json!(format!("{}-{}", run.start, run.end)),
        bytes = run.bytes.len(),
        digest = serde_json::json!(sha256_hex(&run.bytes)),
    )
}

fn parse(raw: &str) -> Option<Parsed<'_>> {
    if raw.is_empty() || !raw.ends_with('\n') {
        return None;
    }
    let lines: Vec<&str> = raw.split_inclusive('\n').collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("diff --git "))
        .map(|(index, _)| index)
        .collect();
    if starts.is_empty() {
        return None;
    }
    let mut sections = Vec::with_capacity(starts.len());
    let mut runs: Vec<Run> = Vec::new();
    for (position, start) in starts.iter().enumerate() {
        let end = starts.get(position + 1).copied().unwrap_or(lines.len());
        let (mut section, local) = parse_section(&lines, *start, end);
        if section.readable {
            let base = runs.len();
            for hunk in &mut section.hunks {
                for index in &mut hunk.runs {
                    *index += base;
                }
            }
            runs.extend(local);
        }
        sections.push(section);
    }
    Some(Parsed {
        lines,
        sections,
        runs,
    })
}

fn parse_section(lines: &[&str], start: usize, end: usize) -> (Section, Vec<Run>) {
    let mut section = Section {
        start,
        end,
        readable: false,
        whole_deletion: false,
        whole_addition: false,
        hunks: Vec::new(),
    };
    let mut runs: Vec<Run> = Vec::new();
    if start >= end {
        return (section, runs);
    }
    let first_hunk = (start + 1..end)
        .find(|index| lines[*index].starts_with("@@ "))
        .unwrap_or(end);
    let Some(old_index) = (start + 1..first_hunk).find(|index| lines[*index].starts_with("--- "))
    else {
        return (section, runs);
    };
    let Some(new_index) =
        (old_index + 1..first_hunk).find(|index| lines[*index].starts_with("+++ "))
    else {
        return (section, runs);
    };
    let old = lines[old_index].trim_end_matches('\n');
    let new = lines[new_index].trim_end_matches('\n');
    let old_path = match old.strip_prefix("--- a/") {
        Some(path) => Some(path.to_owned()),
        None if old == "--- /dev/null" => None,
        None => return (section, runs),
    };
    let new_path = match new.strip_prefix("+++ b/") {
        Some(path) => Some(path.to_owned()),
        None if new == "+++ /dev/null" => None,
        None => return (section, runs),
    };

    let mut hunks = Vec::new();
    let mut at = first_hunk;
    while at < end {
        let Some((old_start, old_count, new_start, new_count, tail)) = parse_header(lines[at])
        else {
            return (section, runs);
        };
        let hunk_end = (at + 1..end)
            .find(|index| lines[*index].starts_with("@@ "))
            .unwrap_or(end);
        let mut hunk = Hunk {
            header: at,
            end: hunk_end,
            old_start,
            old_count,
            new_start,
            new_count,
            tail,
            runs: Vec::new(),
        };
        let mut old_line = old_start;
        let mut new_line = new_start;
        let mut body = at + 1;
        while body < hunk_end {
            match lines[body].as_bytes().first() {
                Some(b' ') => {
                    old_line += 1;
                    new_line += 1;
                    body += 1;
                }
                Some(b'-') => {
                    let Some(path) = old_path.as_ref() else {
                        return (section, runs);
                    };
                    let Some((run, next)) =
                        collect_run(lines, body, hunk_end, true, old_line, path)
                    else {
                        return (section, runs);
                    };
                    old_line += run.count();
                    hunk.runs.push(runs.len());
                    runs.push(run);
                    body = next;
                }
                Some(b'+') => {
                    let Some(path) = new_path.as_ref() else {
                        return (section, runs);
                    };
                    let Some((run, next)) =
                        collect_run(lines, body, hunk_end, false, new_line, path)
                    else {
                        return (section, runs);
                    };
                    new_line += run.count();
                    hunk.runs.push(runs.len());
                    runs.push(run);
                    body = next;
                }
                Some(b'\\') => body += 1,
                _ => return (section, runs),
            }
        }
        hunks.push(hunk);
        at = hunk_end;
    }
    section.hunks = hunks;
    section.readable = true;
    section.whole_deletion =
        new_path.is_none() && section.hunks.len() == 1 && whole(&section.hunks[0], &runs, true);
    section.whole_addition =
        old_path.is_none() && section.hunks.len() == 1 && whole(&section.hunks[0], &runs, false);
    (section, runs)
}

fn whole(hunk: &Hunk, runs: &[Run], removal: bool) -> bool {
    if hunk.runs.len() != 1 {
        return false;
    }
    let run = &runs[hunk.runs[0]];
    run.addition != removal
        && run.first == hunk.header + 1
        && run.last == hunk.end
        && run.start == 1
}

fn collect_run(
    lines: &[&str],
    first: usize,
    end: usize,
    removal: bool,
    start: usize,
    path: &str,
) -> Option<(Run, usize)> {
    let marker = if removal { '-' } else { '+' };
    let mut body = first;
    let mut bytes = Vec::new();
    let mut count = 0;
    while body < end && lines[body].starts_with(marker) {
        bytes.extend_from_slice(&lines[body].as_bytes()[1..]);
        count += 1;
        body += 1;
    }
    if count == 0 {
        return None;
    }
    if body < end && lines[body].starts_with('\\') {
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        body += 1;
    }
    Some((
        Run {
            first,
            last: body,
            bytes,
            path: path.to_owned(),
            start,
            end: start + count - 1,
            addition: !removal,
        },
        body,
    ))
}

fn parse_header(line: &str) -> Option<(usize, usize, usize, usize, String)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_range, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix('+')?;
    let (new_range, tail) = rest.split_once(" @@")?;
    let (old_start, old_count) = parse_range(old_range)?;
    let (new_start, new_count) = parse_range(new_range)?;
    Some((
        old_start,
        old_count,
        new_start,
        new_count,
        tail.trim_end_matches('\n').to_owned(),
    ))
}

fn parse_range(text: &str) -> Option<(usize, usize)> {
    match text.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((text.parse().ok()?, 1)),
    }
}

#[cfg(test)]
mod tests;
