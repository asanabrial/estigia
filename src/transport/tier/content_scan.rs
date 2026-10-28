//! The content scan `review_tier` reads beside the path-derived producers: a
//! process-spawn construct or a dangerous sink in the lines a change adds, or an
//! interpreter directive on the first line of a file either side holds.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::{
    DiffStat, Failure, How, RiskReason, SCAN_BYTE_BUDGET, ascii_lower, blob_contents, blob_sizes,
    go_base, go_ext, has_excluded_risk_segment, is_configuration_review_path, is_inspectable_stat,
    pinned_diff_args, run,
};

const REASON_PROCESS_BOUNDARY: &str = "process_boundary";
const REASON_PROCESS_SCAN_LIMIT: &str = "process_scan_limit";
const REASON_DANGEROUS_SINK: &str = "dangerous_sink";
const SIGNAL_SHELL_PROCESS: &str = "shell_process";

const PATHS_PER_DIFF: usize = 200;

const PROCESS_BOUNDARY_PATTERN: &str = r"(?i)((^|[^[:alnum:]_.])(subprocess|child_process|execute_process|exec)([^[:alnum:]_]|$))|(execSync\(|getRuntime\(\)\.exec\(|ProcessBuilder|os\.system\(|os\.exec[lv]p?e?\(|posix_spawn|proc_open\(|shell_exec\(|passthru\(|popen\(|Process\.Start\()";

static PROCESS_BOUNDARY: LazyLock<Result<Regex, String>> =
    LazyLock::new(|| Regex::new(PROCESS_BOUNDARY_PATTERN).map_err(|error| error.to_string()));

const TLS: &str = "tls_verification";
const DESERIALIZATION: &str = "unsafe_deserialization";
const EVAL: &str = "eval";
const SHELL: &str = "shell_composition";
const CREDENTIAL_HASH: &str = "weak_credential_hash";
const CORS: &str = "credentialed_cors";
const WRITABLE: &str = "world_writable";
const JWT: &str = "unverified_jwt";
const CSRF: &str = "csrf_opt_out";
const XXE: &str = "xxe";

const ALL_SOURCE: &str = ".go .java .kt .kts .js .jsx .ts .tsx .py .php .rb .rs .cs";
const JAVA: &str = ".java .kt .kts";
const JS: &str = ".js .jsx .ts .tsx";
const SHELLS: &str = ".sh .bash .zsh";

struct Sink {
    family: &'static str,
    name: &'static str,
    extensions: &'static str,
    expression: &'static str,
}

const fn sink(
    family: &'static str,
    name: &'static str,
    extensions: &'static str,
    expression: &'static str,
) -> Sink {
    Sink {
        family,
        name,
        extensions,
        expression,
    }
}

const UNSAFE_YAML_PYTHON: &str = "unsafe YAML Python";

const SINKS: &[Sink] = &[
    sink(
        TLS,
        "TLS Go",
        ".go",
        r##"\bInsecureSkipVerify\s*:\s*true\b"##,
    ),
    sink(
        TLS,
        "TLS JS",
        JS,
        r##"\brejectUnauthorized\s*:\s*false\b|\bNODE_TLS_REJECT_UNAUTHORIZED\s*=\s*['"]?0\b"##,
    ),
    sink(
        TLS,
        "TLS Python",
        ".py",
        r##"\bverify\s*=\s*False\b|\bCERT_NONE\b|\bcheck_hostname\s*=\s*False\b"##,
    ),
    sink(
        TLS,
        "TLS Rust",
        ".rs",
        r##"\.danger_accept_invalid_certs\s*\(\s*true\s*\)"##,
    ),
    sink(TLS, "TLS Ruby", ".rb", r##"OpenSSL::SSL::VERIFY_NONE\b"##),
    sink(
        TLS,
        "TLS PHP",
        ".php",
        r##"\bCURLOPT_SSL_VERIFYPEER\s*=>\s*(?:false|0)\b"##,
    ),
    sink(
        TLS,
        "TLS C sharp",
        ".cs",
        r##"ServerCertificateValidationCallback\s*\+=?\s*(?:\([^)]*\)|\w+)\s*=>\s*true\b"##,
    ),
    sink(
        TLS,
        "TLS shell",
        SHELLS,
        r##"\bcurl\s+(?:[^;|]*\s)?(?:--insecure|-k)(?:\s|$)"##,
    ),
    sink(
        TLS,
        "TLS Java trust all",
        JAVA,
        r##"\bcheckServerTrusted\s*\([^)]*\)\s*\{\s*\}|\bHostnameVerifier\b[^;\n]*\([^)]*\)\s*->\s*true\b"##,
    ),
    sink(
        DESERIALIZATION,
        "pickle",
        ".py",
        r##"\bpickle\.loads?\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        UNSAFE_YAML_PYTHON,
        ".py",
        r##"\byaml\.(?:unsafe_load|load)\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        "marshal Python",
        ".py",
        r##"\bmarshal\.loads?\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        "Java object stream",
        JAVA,
        r##"\bObjectInputStream\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        "C sharp formatter",
        ".cs",
        r##"\bBinaryFormatter\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        "PHP unserialize",
        ".php",
        r##"(?:^|[^\w.])unserialize\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        "Ruby marshal",
        ".rb",
        r##"\bMarshal\.load\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        "Ruby YAML",
        ".rb",
        r##"\bYAML\.load\s*\("##,
    ),
    sink(
        DESERIALIZATION,
        "C sharp type names",
        ".cs",
        r##"\bTypeNameHandling\s*(?:=|:)\s*TypeNameHandling\.(?:All|Auto)\b"##,
    ),
    sink(
        DESERIALIZATION,
        "node serialize",
        JS,
        r##"\b(?:serialize|nodeSerialize)\.unserialize\s*\("##,
    ),
    sink(
        EVAL,
        "eval",
        ".js .jsx .ts .tsx .py .php .rb",
        r##"(?:^|[^\w.])eval\s*\("##,
    ),
    sink(
        EVAL,
        "Function constructor",
        JS,
        r##"\bnew\s+Function\s*\("##,
    ),
    sink(EVAL, "node VM", JS, r##"\bvm\.runInNewContext\s*\("##),
    sink(
        EVAL,
        "PHP create_function",
        ".php",
        r##"\bcreate_function\s*\("##,
    ),
    sink(
        EVAL,
        "Ruby string eval",
        ".rb",
        r##"\b(?:instance_eval|class_eval)\s*\(\s*["'`]"##,
    ),
    sink(
        SHELL,
        "Python shell",
        ".py",
        r##"\bsubprocess\.(?:run|Popen|call|check_call|check_output)\s*\(\s*(?:[A-Za-z_]\w*|f["']|["'][^"']*["']\s*\+)[^;\n]*\bshell\s*=\s*True\b"##,
    ),
    sink(
        SHELL,
        "Python system composition",
        ".py",
        r##"\bos\.system\s*\(\s*(?:f["']|[^)\n]*(?:\+\s*[A-Za-z_]\w*|[A-Za-z_]\w*\s*\+|%\s*[A-Za-z_]\w*|\.format\s*\(\s*[A-Za-z_]\w*))"##,
    ),
    sink(
        SHELL,
        "PHP shell variable",
        ".php",
        r##"\b(?:system|exec|shell_exec|passthru)\s*\(\s*\$\w+|`[^`]*\$\w+[^`]*`"##,
    ),
    sink(
        SHELL,
        "Ruby interpolated shell",
        ".rb",
        r##"`[^`]*#\{[^}]+\}[^`]*`|%x[({][^)}]*#\{[^}]+\}"##,
    ),
    sink(
        SHELL,
        "composed sh -c",
        ".go .java .kt .kts .js .jsx .ts .tsx .py .rb .rs .cs",
        r##"(?:["'](?:sh|bash)["']\s*,\s*["']-c["']\s*,\s*(?:\w+|["'][^"']*["']\s*\+|f["']|`))"##,
    ),
    sink(
        CREDENTIAL_HASH,
        "weak credential hash",
        ALL_SOURCE,
        r##"(?i)\b(?:hashlib\.)?(?:md5|sha1)(?:\.Sum)?\s*\(\s*\$?\w*(?:password|passwd|secret|token)\w*\b|\bcreateHash\s*\(\s*['"](?:md5|sha1)['"]\s*\)\s*\.update\s*\(\s*\w*(?:password|passwd|secret|token)\w*\b|\bMessageDigest\.getInstance\s*\(\s*['"](?:MD5|SHA-1)['"]\s*\)\.digest\s*\(\s*\w*(?:password|passwd|secret|token)\w*\b"##,
    ),
    sink(
        CORS,
        "credentialed wildcard CORS",
        ALL_SOURCE,
        r##"(?i)Access-Control-Allow-Origin[^;\n]*\*[^;\n]*;[^\n]*Access-Control-Allow-Credentials[^\n]*true"##,
    ),
    sink(
        WRITABLE,
        "world writable API",
        ".go .js .jsx .ts .tsx .py .php .rb .rs .cs",
        r##"(?i)\b(?:chmod|Chmod|mkdir|WriteFile)\s*\([^\n]*\b(?:0o777|0777|0666)\b|\bumask\s*\(\s*0\s*\)"##,
    ),
    sink(
        WRITABLE,
        "world writable shell",
        SHELLS,
        r##"\bchmod\s+(?:-R\s+)?(?:777\b|o\+w\b)"##,
    ),
    sink(
        JWT,
        "JWT Python bypass",
        ".py",
        r##"\bjwt\.decode\s*\([^\n]*(?:\bverify\s*=\s*False\b|["']verify_signature["']\s*:\s*False\b|\balgorithms\s*=\s*\[\s*["']none["']\s*\])"##,
    ),
    sink(
        JWT,
        "JWT Go unverified",
        ".go",
        r##"\b(?:jwt|\w+)\.ParseUnverified\s*\("##,
    ),
    sink(
        JWT,
        "JWT Java unsecured",
        JAVA,
        r##"\.parseUnsecuredClaims\s*\("##,
    ),
    sink(
        CSRF,
        "CSRF Python",
        ".py",
        r##"@csrf_exempt\b|\bWTF_CSRF_ENABLED\s*=\s*False\b"##,
    ),
    sink(
        CSRF,
        "CSRF Spring",
        JAVA,
        r##"\.csrf\s*\(\s*\)\.disable\s*\("##,
    ),
    sink(
        CSRF,
        "CSRF Rails",
        ".rb",
        r##"\bskip_before_action\s+:verify_authenticity_token\b"##,
    ),
    sink(
        CSRF,
        "CSRF config",
        ".js .jsx .ts .tsx .php .rb",
        r##"\bcsrf\s*:\s*false\b|\bcsrf\s*=>\s*false\b"##,
    ),
    sink(
        CSRF,
        "CSRF C sharp",
        ".cs",
        r##"\[IgnoreAntiforgeryToken\]"##,
    ),
    sink(
        XXE,
        "XXE Python",
        ".py",
        r##"\bresolve_entities\s*=\s*True\b"##,
    ),
    sink(
        XXE,
        "XXE Java",
        JAVA,
        r##"\bXMLInputFactory\.IS_SUPPORTING_EXTERNAL_ENTITIES\s*,\s*true\b|\.setExpandEntityReferences\s*\(\s*true\s*\)"##,
    ),
    sink(XXE, "XXE PHP", ".php", r##"\bLIBXML_NOENT\b"##),
    sink(XXE, "XXE C sharp", ".cs", r##"\bDtdProcessing\.Parse\b"##),
];

struct CompiledSink {
    sink: &'static Sink,
    expression: Regex,
}

static CATALOG: LazyLock<Result<Vec<CompiledSink>, String>> = LazyLock::new(|| {
    SINKS
        .iter()
        .map(|sink| {
            Regex::new(sink.expression)
                .map(|expression| CompiledSink { sink, expression })
                .map_err(|error| format!("dangerous-sink pattern {:?}: {error}", sink.name))
        })
        .collect()
});

static YAML_LOAD_CALL: LazyLock<Result<Regex, String>> =
    LazyLock::new(|| Regex::new(r"\byaml\.load\s*\(").map_err(|error| error.to_string()));
static SAFE_YAML_LOADER: LazyLock<Result<Regex, String>> = LazyLock::new(|| {
    Regex::new(r"\bLoader\s*=\s*(?:yaml\.)?C?SafeLoader\b").map_err(|error| error.to_string())
});

fn compiled<T>(slot: &'static Result<T, String>) -> Result<&'static T, Failure> {
    slot.as_ref().map_err(|message| {
        Failure::Read(format!("content scan pattern did not compile: {message}"))
    })
}

pub(super) fn is_test_risk_path(logical_path: &str) -> bool {
    let lower = ascii_lower(logical_path);
    if lower.split('/').any(|segment| {
        matches!(
            segment,
            "test" | "tests" | "__tests__" | "testdata" | "spec"
        )
    }) {
        return true;
    }
    let name = go_base(&lower);
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name.ends_with("_test.go")
        || name.contains(".test.")
        || name.contains(".spec.")
        || (name.starts_with("test_") && name.ends_with(".py"))
        || name.ends_with("_test.py")
}

fn is_content_scan_eligible(stat: &DiffStat) -> bool {
    if !is_inspectable_stat(stat) || has_excluded_risk_segment(&stat.path) {
        return false;
    }
    let extension = ascii_lower(go_ext(&stat.path));
    super::SEMANTIC_SOURCE_EXTENSIONS.contains(&extension.as_str())
        || extension == ".txt"
        || is_configuration_review_path(&stat.path)
}

fn has_unsafe_yaml_load(line: &str) -> Result<bool, Failure> {
    let call = compiled(&YAML_LOAD_CALL)?;
    let safe = compiled(&SAFE_YAML_LOADER)?;
    for found in call.find_iter(line) {
        let start = found.end();
        let mut depth = 1;
        let mut end = line.len();
        for (offset, byte) in line.as_bytes()[start..].iter().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        if !safe.is_match(&line[start..end]) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn dangerous_sink_families(
    logical_path: &str,
    line: &str,
) -> Result<BTreeSet<&'static str>, Failure> {
    let mut families = BTreeSet::new();
    if is_test_risk_path(logical_path) {
        return Ok(families);
    }
    let trimmed = line.trim();
    if ["//", "#", "--", "/*", "*", "<!--", "'''", "\"\"\""]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return Ok(families);
    }
    let extension = ascii_lower(go_ext(logical_path));
    for entry in compiled(&CATALOG)? {
        let sink = entry.sink;
        if !sink.extensions.split(' ').any(|known| known == extension)
            || !entry.expression.is_match(line)
        {
            continue;
        }
        if sink.name == UNSAFE_YAML_PYTHON
            && !(line.contains("yaml.unsafe_load(") || has_unsafe_yaml_load(line)?)
        {
            continue;
        }
        families.insert(sink.family);
    }
    Ok(families)
}

fn scan_limit(path: &str) -> Vec<RiskReason> {
    vec![RiskReason {
        code: REASON_PROCESS_SCAN_LIMIT,
        signal: Some(SIGNAL_SHELL_PROCESS),
        path: Some(path.to_owned()),
    }]
}

fn present_sides(stat: &DiffStat) -> impl Iterator<Item = &str> {
    [
        (&stat.old_mode, &stat.old_object),
        (&stat.new_mode, &stat.new_object),
    ]
    .into_iter()
    .filter(|(mode, _)| !mode.is_empty() && *mode != "000000")
    .map(|(_, object)| object.as_str())
}

pub(super) fn has_shebang_line(content: &str) -> bool {
    if content.contains('\0') {
        return false;
    }
    let first = content
        .strip_prefix('\u{feff}')
        .unwrap_or(content)
        .lines()
        .next()
        .unwrap_or("");
    first.starts_with("#!") && !first.starts_with("#![")
}

pub(super) fn content_scan_reasons(
    at: &Path,
    base: &str,
    head: &str,
    stats: &[DiffStat],
) -> Result<Vec<RiskReason>, Failure> {
    let scanned: Vec<&DiffStat> = stats
        .iter()
        .filter(|stat| is_content_scan_eligible(stat) && !is_test_risk_path(&stat.path))
        .collect();
    if scanned.is_empty() {
        return Ok(Vec::new());
    }

    let sides: Vec<(&DiffStat, &str)> = scanned
        .iter()
        .flat_map(|stat| present_sides(stat).map(move |object| (*stat, object)))
        .collect();
    let objects: Vec<&str> = sides.iter().map(|(_, object)| *object).collect();

    let mut total: u64 = 0;
    for ((stat, _), size) in sides.iter().zip(blob_sizes(at, &objects)?) {
        total += size;
        if total > SCAN_BYTE_BUDGET {
            return Ok(scan_limit(&stat.path));
        }
    }

    let mut reasons: Vec<RiskReason> = Vec::new();
    for ((stat, _), content) in sides.iter().zip(blob_contents(at, &objects)?) {
        let named = reasons
            .iter()
            .any(|reason| reason.path.as_deref() == Some(stat.path.as_str()));
        if !named && has_shebang_line(&content) {
            reasons.push(RiskReason {
                code: REASON_PROCESS_BOUNDARY,
                signal: Some(SIGNAL_SHELL_PROCESS),
                path: Some(stat.path.clone()),
            });
        }
    }

    for batch in scanned.chunks(PATHS_PER_DIFF) {
        if let Some(limit) = scan_added_lines(at, base, head, batch, &mut reasons)? {
            return Ok(limit);
        }
    }
    Ok(reasons)
}

fn scan_added_lines(
    at: &Path,
    base: &str,
    head: &str,
    batch: &[&DiffStat],
    reasons: &mut Vec<RiskReason>,
) -> Result<Option<Vec<RiskReason>>, Failure> {
    let pathspecs: Vec<String> = batch
        .iter()
        .map(|stat| format!(":(literal){}", stat.path))
        .collect();
    let mut rest: Vec<&str> = vec!["--text", "--unified=0", base, head, "--"];
    rest.extend(pathspecs.iter().map(String::as_str));
    let output = run(&pinned_diff_args(&rest), Some(at), How::read())?;
    let first = batch[0].path.as_str();
    if output.stdout.len() as u64 > SCAN_BYTE_BUDGET {
        return Ok(Some(scan_limit(first)));
    }

    let known: BTreeSet<&str> = batch.iter().map(|stat| stat.path.as_str()).collect();
    let process = compiled(&PROCESS_BOUNDARY)?;
    let mut current = "";
    let mut in_hunk = false;
    let mut process_seen = false;
    let mut sinks_seen: BTreeSet<&'static str> = BTreeSet::new();
    for line in output.stdout.split('\n') {
        if line.starts_with("diff --git ") {
            let Some(separator) = line.rfind(" b/") else {
                return Ok(Some(scan_limit(first)));
            };
            current = &line[separator + 3..];
            in_hunk = false;
            process_seen = false;
            sinks_seen.clear();
            if !known.contains(current) {
                return Ok(Some(scan_limit(first)));
            }
        } else if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk && let Some(added) = line.strip_prefix('+') {
            if !process_seen && process.is_match(added) {
                reasons.push(RiskReason {
                    code: REASON_PROCESS_BOUNDARY,
                    signal: Some(SIGNAL_SHELL_PROCESS),
                    path: Some(current.to_owned()),
                });
                process_seen = true;
            }
            for family in dangerous_sink_families(current, added)? {
                if sinks_seen.insert(family) {
                    reasons.push(RiskReason {
                        code: REASON_DANGEROUS_SINK,
                        signal: Some(family),
                        path: Some(current.to_owned()),
                    });
                }
            }
        }
    }
    Ok(None)
}
