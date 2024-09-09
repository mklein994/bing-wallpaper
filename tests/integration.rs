use std::sync::LazyLock;

macro_rules! project_file {
    ($base:literal, $dir:literal) => {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/",
            $base,
            "/",
            $dir,
            "/bing_wallpaper"
        )
    };

    ($base:literal, $dir:literal, $file:literal) => {
        concat!(project_file!($base, $dir), "/", $file)
    };
}

macro_rules! project {
    ($base:literal) => {
        &[
            "--config-path", project_file!($base, "config", "config.json"),
            "--data-path", project_file!($base, "share"),
            "--state-path", project_file!($base, "state", "image_index.json"),
        ]
    }
}

fn get_output<I, V, S, T>(project: I, args: V) -> (String, String)
where
    I: IntoIterator<Item = S>,
    V: IntoIterator<Item = T>,
    S: AsRef<std::ffi::OsStr>,
    T: AsRef<std::ffi::OsStr>,
{
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_bing-wallpaper"))
        .args(project)
        .args(args)
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    (stdout, stderr)
}

static PATH_FILTER: LazyLock<String> = LazyLock::new(|| regex::escape(env!("CARGO_MANIFEST_DIR")));

macro_rules! t {
    ($project:expr, $args:expr) => {
        let (stdout, stderr) = get_output($project, $args);
        insta::with_settings!({filters => vec![
            (&*PATH_FILTER.as_str(), ""),
        ]}, {
            insta::assert_snapshot!(stdout);
            insta::assert_snapshot!(stderr);
        });
    }
}

#[test]
fn list_images() {
    let (stdout, stderr) = get_output(project!("local"), ["list-images"]);
    insta::assert_snapshot!(stdout);
    insta::assert_snapshot!(stderr);
}

#[test]
fn end_to_end_test() {
    let (stdout, stderr) = get_output(project!("local"), ["project-dirs"]);
    insta::with_settings!({filters => vec![
    (env!("CARGO_MANIFEST_DIR"), ""),
    ]}, {
        insta::assert_snapshot!(stdout);
        insta::assert_snapshot!(stderr);
    });
}

#[test]
fn list_existing_images() {
    t!(
        project!("local-state-has-images"),
        ["list-images", "-f", "title,path"]
    );
}
