//! Phase 9 integration tests: Layer-1 app login (app_users).
//!
//! Covers: signup/login/logout, uniform login errors, duplicate email,
//! first-signup-becomes-admin, password change, and admin user-management
//! constraints. Uses a temp app DB — never the real %APPDATA% location.

use std::path::PathBuf;

use veylora::app_auth::AppAuth;

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
    assert_eq!(
        status.role.as_deref(),
        Some("admin"),
        "first account on an empty install becomes the admin"
    );

    auth.logout();
    assert!(!auth.status().logged_in, "logout clears the session");

    let _ = std::fs::remove_dir_all(tmp_db("roundtrip").parent().unwrap());
}

#[test]
fn phase9_login_error_identical_for_unknown_email_and_wrong_password() {
    let auth = AppAuth::with_db_path(tmp_db("uniform-error"));

    auth.signup("known@example.com", "longenough")
        .expect("signup");
    let unknown = auth.login("nobody@example.com", "whatever123").unwrap_err();
    let wrong = auth
        .login("known@example.com", "definitely-wrong")
        .unwrap_err();

    assert_eq!(unknown, "Invalid email or password");
    assert_eq!(unknown, wrong, "must not reveal whether the account exists");

    let _ = std::fs::remove_dir_all(tmp_db("uniform-error").parent().unwrap());
}

#[test]
fn phase9_duplicate_email_rejected_case_insensitive() {
    let auth = AppAuth::with_db_path(tmp_db("duplicate"));

    auth.signup("dup@example.com", "longenough")
        .expect("first signup succeeds");
    let err = auth.signup("dup@example.com", "longenough").unwrap_err();
    assert_eq!(err, "Email already in use");

    let err_ci = auth.signup("DUP@example.com", "longenough").unwrap_err();
    assert_eq!(err_ci, "Email already in use", "UNIQUE is case-insensitive");

    let _ = std::fs::remove_dir_all(tmp_db("duplicate").parent().unwrap());
}

#[test]
fn phase9_first_signup_is_admin_later_signups_are_users() {
    let auth = AppAuth::with_db_path(tmp_db("first-admin"));

    // Nothing is seeded: the empty DB logs nobody in by default.
    assert!(!auth.status().logged_in, "fresh install starts logged out");

    auth.signup("boss@example.com", "longenough")
        .expect("first signup succeeds");
    let status = auth
        .login("boss@example.com", "longenough")
        .expect("first account logs in");
    assert_eq!(
        status.role.as_deref(),
        Some("admin"),
        "first account becomes the admin"
    );

    auth.logout();
    auth.signup("worker@example.com", "longenough")
        .expect("second signup succeeds");
    let status = auth
        .login("worker@example.com", "longenough")
        .expect("second account logs in");
    assert_eq!(
        status.role.as_deref(),
        Some("user"),
        "later signups are regular users"
    );

    let _ = std::fs::remove_dir_all(tmp_db("first-admin").parent().unwrap());
}

#[test]
fn phase9_change_password_invalidates_old() {
    let auth = AppAuth::with_db_path(tmp_db("chpw"));

    auth.signup("boss@example.com", "longenough")
        .expect("first signup is admin");
    auth.login("boss@example.com", "longenough")
        .expect("admin logs in");

    let err = auth
        .change_password("not-the-old-one", "brandnew123")
        .unwrap_err();
    assert_eq!(err, "Invalid password");

    auth.change_password("longenough", "brandnew123")
        .expect("change with correct old password");

    auth.logout();
    assert!(
        auth.login("boss@example.com", "longenough").is_err(),
        "old password no longer works"
    );
    let status = auth
        .login("boss@example.com", "brandnew123")
        .expect("new password works");
    assert_eq!(status.role.as_deref(), Some("admin"));

    let _ = std::fs::remove_dir_all(tmp_db("chpw").parent().unwrap());
}

#[test]
fn phase9_admin_delete_constraints() {
    let auth = AppAuth::with_db_path(tmp_db("delete"));
    auth.signup("boss@example.com", "longenough")
        .expect("first signup is admin");
    auth.login("boss@example.com", "longenough")
        .expect("admin logs in");

    let users = auth.list_users().expect("admin can list users");
    let own_id = users
        .iter()
        .find(|u| u.email == "boss@example.com")
        .expect("admin row exists")
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
    auth.delete_user(victim.id)
        .expect("admin can delete a user");
    let users = auth.list_users().expect("list after delete");
    assert!(!users.iter().any(|u| u.email == "victim@example.com"));

    let _ = std::fs::remove_dir_all(tmp_db("delete").parent().unwrap());
}

#[test]
fn phase9_non_admin_cannot_administer() {
    let auth = AppAuth::with_db_path(tmp_db("user-role"));

    // First account claims the admin role; the second is an ordinary user.
    auth.signup("boss@example.com", "longenough")
        .expect("first signup (admin)");
    auth.signup("plain@example.com", "longenough")
        .expect("second signup (user)");
    auth.login("plain@example.com", "longenough")
        .expect("user logs in");

    assert_eq!(auth.list_users().unwrap_err(), "Admin only");
    assert_eq!(
        auth.create_user("x@example.com", "longenough", "user")
            .unwrap_err(),
        "Admin only"
    );
    assert_eq!(auth.delete_user(1).unwrap_err(), "Admin only");

    let _ = std::fs::remove_dir_all(tmp_db("user-role").parent().unwrap());
}
