use vpremises_incus_driver::{ResolvedEnvironment, render};

fn fixture() -> ResolvedEnvironment {
    serde_json::from_str(include_str!("fixtures/resolved.json")).unwrap()
}

#[test]
fn renders_restricted_project_and_bounded_instance() {
    let rendered = render(&fixture()).unwrap();
    let project = &rendered["resource"]["incus_project"]["managed"];
    let instance = &rendered["resource"]["incus_instance"]["managed"];
    assert_eq!(project["config"]["restricted"], "true");
    assert_eq!(project["force_destroy"], false);
    assert_eq!(instance["type"], "virtual-machine");
    assert_eq!(instance["config"]["boot.autostart"], "true");
    assert_eq!(instance["wait_for"][0]["type"], "cloud-init");
}

#[test]
fn rejects_non_cloud_image() {
    let mut raw: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/resolved.json")).unwrap();
    raw["environment"]["incus"]["image"] = "images:debian/13".into();
    let invalid: ResolvedEnvironment = serde_json::from_value(raw).unwrap();
    assert!(render(&invalid).is_err());
}

#[test]
fn rejects_cloud_init_user_injection_and_root_aliases() {
    for user in [
        "",
        "root",
        "Root",
        "ROOT",
        "0",
        "root:0",
        " user",
        "user\nwrite_files:",
        "user\r",
        "user\t",
        "user\0",
        "user: admin",
        "user#comment",
        "用户",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        let mut input = fixture();
        input.service.process.user = user.into();
        assert!(render(&input).is_err(), "accepted invalid user {user:?}");
    }
}

#[test]
fn serializes_cloud_init_with_exact_user_and_fixed_security_fields() {
    for user in ["app", "app-user", "app_01", "_service"] {
        let mut input = fixture();
        input.service.process.user = user.into();
        let rendered = render(&input).unwrap();
        let text =
            rendered["resource"]["incus_instance"]["managed"]["config"]["cloud-init.user-data"]
                .as_str()
                .unwrap();
        let document: serde_json::Value =
            serde_json::from_str(text.strip_prefix("#cloud-config\n").unwrap()).unwrap();
        assert_eq!(document["users"].as_array().unwrap().len(), 1);
        assert_eq!(document["users"][0]["name"], user);
        assert_eq!(document["users"][0]["lock_passwd"], true);
        assert_eq!(document["users"][0]["shell"], "/usr/sbin/nologin");
        assert_eq!(document.as_object().unwrap().len(), 3);
    }
}
