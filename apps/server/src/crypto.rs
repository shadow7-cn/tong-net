use aes_gcm::{
    aead::{Aead, OsRng},
    Aes256Gcm, KeyInit, Nonce,
};
use argon2::{
    password_hash::{
        rand_core::RngCore, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
    Argon2,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use std::path::Path;
use x25519_dalek::{PublicKey, StaticSecret};

pub fn hash_password(value: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(value.as_bytes(), &salt)
        .map(|value| value.to_string())
        .map_err(|error| error.to_string())
}

pub fn verify_password(hash: &str, value: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(value.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}

pub fn random_token(bytes: usize) -> String {
    let mut value = vec![0u8; bytes];
    OsRng.fill_bytes(&mut value);
    STANDARD.encode(value)
}

pub fn token_hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub fn load_or_create_master_key(path: &Path) -> Result<[u8; 32], String> {
    if let Ok(value) = std::fs::read(path) {
        return value
            .try_into()
            .map_err(|_| "站点主密钥长度无效".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut value = [0u8; 32];
    OsRng.fill_bytes(&mut value);
    std::fs::write(path, value).map_err(|error| error.to_string())?;
    set_private_file(path)?;
    Ok(value)
}

pub fn encrypt(key: &[u8; 32], value: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| error.to_string())?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), value.as_bytes())
        .map_err(|_| "加密敏感配置失败".to_string())?;
    let mut output = nonce.to_vec();
    output.extend(encrypted);
    Ok(STANDARD.encode(output))
}

pub fn decrypt(key: &[u8; 32], value: &str) -> Result<String, String> {
    let payload = STANDARD
        .decode(value)
        .map_err(|_| "敏感配置编码无效".to_string())?;
    if payload.len() <= 12 {
        return Err("敏感配置长度无效".into());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| error.to_string())?;
    let decrypted = cipher
        .decrypt(Nonce::from_slice(&payload[..12]), &payload[12..])
        .map_err(|_| "无法解密敏感配置".to_string())?;
    String::from_utf8(decrypted).map_err(|_| "敏感配置文本无效".to_string())
}

pub fn generate_x25519_keypair() -> (String, String) {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (
        STANDARD.encode(secret.to_bytes()),
        STANDARD.encode(public.as_bytes()),
    )
}

pub fn validate_password(value: &str) -> Result<(), String> {
    if value.chars().count() < 8 {
        return Err("密码至少需要 8 个字符".into());
    }
    Ok(())
}

pub fn validate_device_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    let count = value.chars().count();
    if !(1..=40).contains(&count) || value.chars().any(char::is_control) {
        return Err("设备名称需要 1-40 个字符，且不能包含控制字符".into());
    }
    Ok(value.to_string())
}

#[cfg(unix)]
fn set_private_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn set_private_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hashes_and_verifies() {
        let hash = hash_password("correct-horse").unwrap();
        assert!(verify_password(&hash, "correct-horse"));
        assert!(!verify_password(&hash, "wrong"));
    }

    #[test]
    fn encryption_round_trip() {
        let key = [7u8; 32];
        let encrypted = encrypt(&key, "秘密").unwrap();
        assert_ne!(encrypted, "秘密");
        assert_eq!(decrypt(&key, &encrypted).unwrap(), "秘密");
    }

    #[test]
    fn x25519_keypair_has_expected_size() {
        let (private, public) = generate_x25519_keypair();
        assert_eq!(STANDARD.decode(private).unwrap().len(), 32);
        assert_eq!(STANDARD.decode(public).unwrap().len(), 32);
    }
}
