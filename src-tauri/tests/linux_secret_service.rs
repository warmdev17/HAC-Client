#![cfg(target_os = "linux")]

use hac_core::platform::{NativeSystem, System};
use std::process::Command;

#[test]
#[ignore = "Run only with a private D-Bus and isolated XDG directories; see README"]
fn private_secret_service_round_trip() {
    assert_eq!(std::env::var("HAC_TEST_PRIVATE_BUS").as_deref(), Ok("1"));
    let system = NativeSystem;
    assert!(
        system.key().unwrap().is_none(),
        "Test requires an empty private keyring"
    );
    let key = "fake-key-'\"$`\\=é";
    system.save_key(Some(key)).unwrap();
    let result = (|| -> Result<(), String> {
        assert_eq!(system.key()?.as_deref(), Some(key));
        let output = Command::new("secret-tool")
            .args([
                "lookup",
                "service",
                "com.hac.codexmanager",
                "username",
                "HAC_API_KEY",
                "target",
                "default",
            ])
            .output()
            .map_err(|_| "secret-tool unavailable")?;
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim_end(), key);
        system.save_key(Some("rotated-fake-key"))?;
        assert_eq!(system.key()?.as_deref(), Some("rotated-fake-key"));
        Ok(())
    })();
    system.save_key(None).unwrap();
    assert!(system.key().unwrap().is_none());
    result.unwrap();
}
