use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nautilus-generator-smoke-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock predates Unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("could not create generator smoke directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("could not remove temporary directory {:?}: {error}", self.0);
        }
    }
}

fn run_generator(work_dir: &Path) -> Output {
    let grammar = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../grammars/grammar_py_example.py")
        .canonicalize()
        .expect("could not resolve bundled Python grammar");
    let mut child = Command::new(env!("CARGO_BIN_EXE_generator"))
        .args(["-g", grammar.to_str().expect("grammar path is not UTF-8")])
        .args(["-t", "20", "-n", "4", "-s"])
        .current_dir(work_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("could not start generator CLI");
    let timeout = Duration::from_secs(10);
    let started = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .expect("could not collect generator output");
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                child
                    .kill()
                    .expect("generator timed out and could not be killed");
                let output = child
                    .wait_with_output()
                    .expect("could not reap timed-out generator");
                panic!(
                    "generator timed out after {timeout:?}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("could not query generator status: {error}");
            }
        }
    }
}

#[test]
fn bundled_python_grammar_generates_requested_corpus() {
    let work_dir = TestDir::new();
    let output = run_generator(work_dir.path());
    assert!(
        output.status.success(),
        "generator exited with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let corpus = work_dir.path().join("corpus");
    let mut files = fs::read_dir(&corpus)
        .expect("generator did not create corpus directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("could not read generated corpus");
    files.sort_by_key(std::fs::DirEntry::file_name);
    assert_eq!(files.len(), 4, "generator did not honor -n 4");

    for file in files {
        assert!(
            file.file_type()
                .expect("could not inspect corpus entry")
                .is_file(),
            "unexpected non-file corpus entry: {:?}",
            file.path()
        );
        let generated = fs::read(file.path()).expect("could not read generated input");
        assert!(!generated.is_empty(), "generated an empty input");
        let generated = std::str::from_utf8(&generated).expect("generated input was not UTF-8");
        assert!(
            generated.chars().all(|c| c.is_ascii_lowercase())
                || (generated.contains("<document>") && generated.ends_with("</document>")),
            "generated input does not match the bundled grammar: {generated:?}"
        );
    }
}
