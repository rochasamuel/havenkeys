// Declares the complete list of app commands. tauri-build generates an
// `allow-<command>` permission for each; only those granted in
// capabilities/main.json are callable from the renderer.
const COMMANDS: &[&str] = &[
    "vault_status",
    "unlock_vault",
    "lock_vault",
    "change_master_password",
    "record_activity",
    "list_items",
    "get_item",
    "reveal_secret",
    "password_history",
    "list_passkeys",
    "delete_passkey",
    "device_status",
    "account_status",
    "activate_account",
    "sign_in",
    "sign_out",
    "list_devices",
    "revoke_device",
    "remove_device",
    "get_emergency_kit",
    "reveal_account_secret_key",
    "copy_account_field",
    "sync_now",
    "resync_vault",
    "reveal_previous_password",
    "get_totp_code",
    "copy_secret",
    "open_website",
    "scan_totp_qr",
    "create_item",
    "update_item",
    "delete_item",
    "generate_password",
    "copy_generated_password",
    "get_settings",
    "update_settings",
    "launch_at_login",
    "set_launch_at_login",
    "import_1pux",
    "delete_import_file",
    "set_ui_language",
    "update_status",
    "check_for_update",
    "install_update",
    "set_update_auto_check",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
