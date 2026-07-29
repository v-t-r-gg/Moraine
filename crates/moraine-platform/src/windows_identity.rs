//! Stable current-account identity used to describe Windows runtime endpoints.

use sha2::{Digest, Sha256};

use crate::PlatformError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsUserIdentity {
    pub sid: String,
    pub scope_id: String,
}

fn validate_sid(sid: &str) -> Result<(), PlatformError> {
    let mut parts = sid.split('-');
    if parts.next() != Some("S") {
        return Err(PlatformError::new(
            "invalid_windows_sid",
            "Windows SID must start with S-",
        ));
    }
    let Some(revision) = parts.next() else {
        return Err(PlatformError::new(
            "invalid_windows_sid",
            "Windows SID is missing its revision",
        ));
    };
    let Some(authority) = parts.next() else {
        return Err(PlatformError::new(
            "invalid_windows_sid",
            "Windows SID is missing its identifier authority",
        ));
    };
    if revision != "1"
        || authority.is_empty()
        || !authority.bytes().all(|byte| byte.is_ascii_digit())
        || !parts.clone().any(|part| !part.is_empty())
        || !parts.all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(PlatformError::new(
            "invalid_windows_sid",
            format!("invalid Windows SID {sid}"),
        ));
    }
    Ok(())
}

pub fn scope_id_from_sid(sid: &str) -> Result<String, PlatformError> {
    validate_sid(sid)?;
    let digest = Sha256::digest(sid.as_bytes());
    Ok(hex::encode(digest)[..12].to_owned())
}

pub fn named_pipe_name_from_scope(scope_id: &str) -> String {
    format!(r"\\.\pipe\moraine.capture.v1.{scope_id}")
}

#[cfg(target_os = "windows")]
pub fn current_windows_user_identity() -> Result<WindowsUserIdentity, PlatformError> {
    use std::ffi::c_void;

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(|error| {
            PlatformError::new(
                "windows_identity_unavailable",
                format!("could not open the current process token: {error}"),
            )
        })?;

        let result = (|| {
            let mut required = 0;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut required);
            if required == 0 {
                return Err(PlatformError::new(
                    "windows_identity_unavailable",
                    "could not determine the current token SID size",
                ));
            }
            let mut buffer = vec![0u8; required as usize];
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast::<c_void>()),
                required,
                &mut required,
            )
            .map_err(|error| {
                PlatformError::new(
                    "windows_identity_unavailable",
                    format!("could not read the current token SID: {error}"),
                )
            })?;

            let token_user = &*(buffer.as_ptr().cast::<TOKEN_USER>());
            let mut sid = PWSTR::null();
            ConvertSidToStringSidW(token_user.User.Sid, &mut sid).map_err(|error| {
                PlatformError::new(
                    "windows_identity_unavailable",
                    format!("could not format the current token SID: {error}"),
                )
            })?;
            let sid_string = sid.to_string().map_err(|error| {
                PlatformError::new(
                    "windows_identity_unavailable",
                    format!("current token SID is not valid Unicode: {error}"),
                )
            });
            LocalFree(Some(HLOCAL(sid.0.cast())));
            let sid = sid_string?;
            validate_sid(&sid)?;
            Ok(WindowsUserIdentity {
                scope_id: scope_id_from_sid(&sid)?,
                sid,
            })
        })();
        let _ = CloseHandle(token);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sid_scope_is_stable_distinct_and_fixed_width() {
        let first = scope_id_from_sid("S-1-5-21-1000-2000-3000-1001").unwrap();
        let repeated = scope_id_from_sid("S-1-5-21-1000-2000-3000-1001").unwrap();
        let second = scope_id_from_sid("S-1-5-21-1000-2000-3000-1002").unwrap();
        assert_eq!(first, "d07be4ed3160");
        assert_eq!(first, repeated);
        assert_ne!(first, second);
        assert_eq!(first.len(), 12);
        assert!(first
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
    }

    #[test]
    fn pipe_name_contains_only_the_versioned_scope() {
        let scope = scope_id_from_sid("S-1-5-21-1000-2000-3000-1001").unwrap();
        let pipe = named_pipe_name_from_scope(&scope);
        assert_eq!(pipe, format!(r"\\.\pipe\moraine.capture.v1.{scope}"));
        assert!(!pipe.contains("1000-2000"));
        assert!(!pipe.contains("Users"));
        assert!(!pipe.contains("Moraine.exe"));
    }

    #[test]
    fn invalid_sid_is_rejected_before_hashing() {
        let error = scope_id_from_sid("not-a-sid").unwrap_err();
        assert_eq!(error.code, "invalid_windows_sid");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn current_identity_resolves_to_its_pipe() {
        let identity = current_windows_user_identity().unwrap();
        assert!(identity.sid.starts_with("S-1-"));
        assert_eq!(identity.scope_id, scope_id_from_sid(&identity.sid).unwrap());
        assert_eq!(
            named_pipe_name_from_scope(&identity.scope_id),
            format!(r"\\.\pipe\moraine.capture.v1.{}", identity.scope_id)
        );
    }
}
