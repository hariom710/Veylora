//! Phase 9 integration tests: Layer-1 app login (app_users).
//!
//! Covers: signup/login/logout, uniform login errors, duplicate email,
//! seeded default admin flag, password change, and admin user-management
//! constraints. Uses a temp app DB — never the real %APPDATA% location.

use std::path::PathBuf;

use veylora::app_auth::{AppAuth, ADMIN_DEFAULT_PASSWORD, ADMIN_EMAIL};

fn tmp_db(tag: &str) -> PathBuf {
    let dir = std::env::current_dir().unwrap().join(format!(
        "target/debug/test-phase9-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("app.db")
}

#[test]
fn phase9_signup_login_logout_roundtrip() {
    let auth = AppAuth::with_db_path(tmp_db("roundtrip"));

    assert!(!auth.status().logged_in, "fresh install starts logged out");

    auth.signup("user@example.com", "longenough")
        .expect("signup should succeed");
    assert!(
        auth.login("user@example.com", "wrongpass").is_err(),
        "wrong password must fail"
    );

    let status = auth
        .login("user@example.com", "longenough")
        .expect("login should succeed");
    assert!(status.logged_in);
    assert_eq!(status.email.as_deref(), Some("user@example.com"));
    assert_eq!(status.role.as_deref(), Some("user"));
    assert!(!status.used_default_password, "regular user has no default flag");

    auth.logout();
    assert!(!auth.status().logged_in, "logout clears the session");

    let _ = std::fs::remove_dir_all(tmp_db("roundtrip").parent().unwrap());
}

#[test]
fn phase9_login_error_identical_for_unknown_email_and_wrong_password() {
    let auth = AppAuth::with_db_path(tmp_db("uniform-error"));

    let unknown = auth
        .login("nobody@example.com", "whatever123")
        .unwrap_err();
    let wrong = auth.login(ADMIN_EMAIL, "definitely-wrong").unwrap_err();

    assert_eq!(unknown, "Invalid email or password");
    assert_eq!(
        unknown, wrong,
        "must not reveal whether the account exists"
    );

    let _ = std::fs::remove_dir_all(tmp_db("uniform-error").parent().unwrap());
}

#[test]
fn phase9_duplicate_email_rejected_case_insensitive() {
    let auth = AppAuth::with_db_path(tmp_db("duplicate"));

    auth.signup("dup@example.com", "longenough")
        .expect("first signup succeeds");
    let err = auth
        .signup("dup@example.com", "longenough")
        .unwrap_err();
    assert_eq!(err, "Email already in use");

    let err_ci = auth
        .signup("DUP@example.com", "longenough")
        .unwrap_err();
    assert_eq!(err_ci, "Email already in use", "UNIQUE is case-insensitive");

    let _ = std::fs::remove_dir_all(tmp_db("duplicate").parent().unwrap());
}

#[test]
fn phase9_seeded_default_admin_login_flagged() {
    let auth = AppAuth::with_db_path(tmp_db("seed"));

    // First DB open seeds the default admin.
    let status = auth
        .login(ADMIN_EMAIL, ADMIN_DEFAULT_PASSWORD)
        .expect("seeded admin can log in");
    assert_eq!(status.role.as_deref(), Some("admin"));
    assert!(
        status.used_default_password,
        "default-password banner flag is set"
    );

    let _ = std::fs::remove_dir_all(tmp_db("seed").parent().unwrap());
}

#[test]
fn phase9_change_password_invalidates_old_and_clears_flag() {
    let auth = AppAuth::with_db_path(tmp_db("chpw"));

    auth.login(ADMIN_EMAIL, ADMIN_DEFAULT_PASSWORD)
        .expect("seeded admin logs in");

    let err = auth
        .change_password("not-the-old-one", "brandnew123")
        .unwrap_err();
    assert_eq!(err, "Invalid password");

    auth.change_password(ADMIN_DEFAULT_PASSWORD, "brandnew123")
        .expect("change with correct old password");
    assert!(
        !auth.status().used_default_password,
        "banner flag clears after change"
    );

    auth.logout();
    assert!(
        auth.login(ADMIN_EMAIL, ADMIN_DEFAULT_PASSWORD).is_err(),
        "old password no longer works"
    );
    let status = auth
        .login(ADMIN_EMAIL, "brandnew123")
        .expect("new password works");
    assert!(!status.used_default_password);

    let _ = std::fs::remove_dir_all(tmp_db("chpw").parent().unwrap());
}

#[test]
fn phase9_admin_delete_constraints() {
    let auth = AppAuth::with_db_path(tmp_db("delete"));
    auth.login(ADMIN_EMAIL, ADMIN_DEFAULT_PASSWORD)
        .expect("seeded admin logs in");

    let users = auth.list_users().expect("admin can list users");
    let own_id = users
        .iter()
        .find(|u| u.email == ADMIN_EMAIL)
        .expect("seeded admin row exists")
        .id;

    // Only one admin exists: even an admin cannot remove the last one.
    let err = auth.delete_user(own_id).unwrap_err();
    assert_eq!(err, "Cannot delete the last admin");

    // With a second admin present, self-delete is blocked instead.
    auth.create_user("second@example.com", "longenough", "admin")
        .expect("second admin created");
    let err = auth.delete_user(own_id).unwrap_err();
    assert_eq!(err, "Cannot delete your own account");

    // Deleting an ordinary user works.
    auth.create_user("victim@example.com", "longenough", "user")
        .expect("user created");
    let users = auth.list_users().expect("list after creates");
    let victim = users
        .iter()
        .find(|u| u.email == "victim@example.com")
        .expect("victim row exists");
    auth.delete_user(victim.id).expect("admin can delete a user");
    let users = auth.list_users().expect("list after delete");
    assert!(!users.iter().any(|u| u.email == "victim@example.com"));

    let _ = std::fs::remove_dir_all(tmp_db("delete").parent().unwrap());
}

#[test]
fn phase9_non_admin_cannot_administer() {
    let auth = AppAuth::with_db_path(tmp_db("user-role"));

    auth.signup("plain@example.com", "longenough")
        .expect("signup");
    auth.login("plain@example.com", "longenough")
        .expect("user logs in");

    assert_eq!(auth.list_users().unwrap_err(), "Admin only");
    assert_eq!(
        auth.create_user("x@example.com", "longenough", "user").unwrap_err(),
        "Admin only"
    );
    assert_eq!(auth.delete_user(1).unwrap_err(), "Admin only");

    let _ = std::fs::remove_dir_all(tmp_db("user-role").parent().unwrap());
}
