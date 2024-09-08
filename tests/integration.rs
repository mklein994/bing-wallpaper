macro_rules! project {
    ($dir:literal) => {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/local/",
            $dir,
            "/bing_wallpaper"
        )
    };

    ($dir:literal, $file:literal) => {
        concat!(project!($dir), "/", $file)
    };
}

const OPT_ARGS: &[&str] = &[
    "--config-path",
    project!("config", "config.json"),
    "--data-path",
    project!("share"),
    "--state-path",
    project!("state", "image_index.json"),
];

fn get_output<I, S>(args: I) -> (String, String)
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_bing-wallpaper"))
        .args(OPT_ARGS)
        .args(args)
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    (stdout, stderr)
}

#[test]
fn list_images() {
    let (stdout, stderr) = get_output(["list-images"]);
    insta::assert_snapshot!(stdout);
    insta::assert_snapshot!(stderr);
}

#[test]
fn end_to_end_test() {
    let (stdout, stderr) = get_output(["project-dirs"]);
    insta::with_settings!({filters => vec![
    (env!("CARGO_MANIFEST_DIR"), ""),
    ]}, {
        insta::assert_snapshot!(stdout);
        insta::assert_snapshot!(stderr);
    });
}
