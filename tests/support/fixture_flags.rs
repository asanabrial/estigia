use std::path::Path;

/// The cargo flags that put the `fake_process` example where `fixture` was
/// looked for, each with a leading space: empty for `target/debug`.
///
/// The layout is `target/[<triple>/]<profile>/examples/<fixture>`, so the
/// directory above the profile is either `target` itself or the triple.
pub fn fixture_build_flags(fixture: &Path) -> String {
    let profile_dir = fixture.parent().and_then(Path::parent);
    let profile_name = profile_dir
        .and_then(Path::file_name)
        .map(|segment| segment.to_string_lossy().into_owned());
    let triple_flag = profile_dir
        .and_then(Path::parent)
        .filter(|above| above.file_name().is_some_and(|segment| segment != "target"))
        .and_then(Path::file_name)
        .map(|segment| format!(" --target {}", segment.to_string_lossy()))
        .unwrap_or_default();
    let profile_flag = match profile_name.as_deref() {
        None | Some("debug") => String::new(),
        Some("release") => " --release".to_owned(),
        Some(custom) => format!(" --profile {custom}"),
    };
    format!("{profile_flag}{triple_flag}")
}
