use super::*;

#[test]
fn open_refuses_symlink_and_appends_to_existing() {
    use carrova_platform::fake::FakePlatform;
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let target = tmp.path().join("victim");
    std::fs::write(&target, b"keep").unwrap();
    let link = tmp.path().join("log.jsonl");
    carrova_platform::fake::symlink(&target, &link).unwrap();
    let fwd: Arc<dyn EventSink> = Arc::new(crate::session::NullSink);
    assert!(LogSink::open(&p, &link, fwd.clone()).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"keep");

    let real = tmp.path().join("real.jsonl");
    std::fs::write(&real, b"x\n").unwrap();
    let sink = LogSink::open(&p, &real, fwd).unwrap();
    sink.emit(Event::Warning {
        message: "m".into(),
    });
    drop(sink);
    let body = std::fs::read_to_string(&real).unwrap();
    assert!(body.starts_with("x\n{"), "{body}");
}

#[test]
fn scrubs_sensitive_and_home() {
    let r = Redaction {
        sensitive: vec!["home:.ssh".into(), "/h/u/.ssh".into()],
        home: Some("/h/u".into()),
    };
    let body =
        br#"{"event":"action_applied","item":"ssh/keys","target":"home:.ssh/id","action":"create"}
{"event":"warning","message":"cannot read /h/u/.config/x"}
{"event":"rollback_step","target":"/h/u/.ssh/id","restored":true}
torn"#;
    let out = redact_log(body, &r);
    assert!(!out.contains(".ssh"), "{out}");
    assert!(out.contains("~/.config/x"), "{out}");
    assert!(!out.contains("/h/u"), "{out}");
    assert_eq!(out.lines().count(), 3);
}
