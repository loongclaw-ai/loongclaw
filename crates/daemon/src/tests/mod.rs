use super::*;

fn approval_test_operation(tool_name: &str, payload: Value) -> OperationSpec {
    OperationSpec::ToolCore {
        tool_name: tool_name.to_owned(),
        required_capabilities: BTreeSet::from([Capability::InvokeTool]),
        payload,
        core: None,
    }
}

fn write_temp_risk_profile(path: &Path, body: &str) {
    fs::create_dir_all(
        path.parent()
            .expect("temp risk profile path should have parent directory"),
    )
    .expect("create temp risk profile directory");
    fs::write(path, body).expect("write temp risk profile");
}

fn sign_security_scan_profile_for_test(profile: &SecurityScanProfile) -> (String, String) {
    use ed25519_dalek::{Signer, SigningKey};

    let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
    let signature = signing_key.sign(&security_scan_profile_message(profile));
    let public_key_base64 = BASE64_STANDARD.encode(signing_key.verifying_key().to_bytes());
    let signature_base64 = BASE64_STANDARD.encode(signature.to_bytes());
    (public_key_base64, signature_base64)
}

mod architecture;
mod programmatic;
mod spec_runtime;
