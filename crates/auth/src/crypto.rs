use mcp_common::{Error, Result};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};

const NONCE_SIZE: usize = 12;
const KEY_SIZE: usize = 32;

#[derive(Clone)]
pub struct CryptoService {
    key: Vec<u8>,
}

impl CryptoService {
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.len() != KEY_SIZE {
            return Err(Error::Config(format!(
                "Encryption key must be {} bytes",
                KEY_SIZE
            )));
        }

        Ok(Self { key: key.to_vec() })
    }

    pub fn from_hex(hex_key: &str) -> Result<Self> {
        let key = hex::decode(hex_key)
            .map_err(|e| Error::Config(format!("Invalid hex key: {}", e)))?;
        Self::new(&key)
    }

    pub fn generate_key() -> Result<String> {
        let rng = SystemRandom::new();
        let mut key = vec![0u8; KEY_SIZE];
        rng.fill(&mut key)
            .map_err(|_| Error::Internal("Failed to generate random encryption key".into()))?;
        Ok(hex::encode(key))
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let rng = SystemRandom::new();

        // Generate nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rng.fill(&mut nonce_bytes)
            .map_err(|_| Error::Internal("Failed to generate nonce".into()))?;

        // Create key and seal
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.key)
            .map_err(|_| Error::Internal("Failed to create encryption key".into()))?;
        let key = LessSafeKey::new(unbound_key);

        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let mut ciphertext = plaintext.to_vec();
        key.seal_in_place_append_tag(nonce, Aad::empty(), &mut ciphertext)
            .map_err(|_| Error::Internal("Encryption failed".into()))?;

        Ok((ciphertext, nonce_bytes.to_vec()))
    }

    pub fn decrypt(&self, ciphertext: &[u8], nonce_bytes: &[u8]) -> Result<Vec<u8>> {
        if nonce_bytes.len() != NONCE_SIZE {
            return Err(Error::BadRequest("Invalid nonce size".into()));
        }

        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.key)
            .map_err(|_| Error::Internal("Failed to create decryption key".into()))?;
        let key = LessSafeKey::new(unbound_key);

        let mut nonce_array = [0u8; NONCE_SIZE];
        nonce_array.copy_from_slice(nonce_bytes);
        let nonce = Nonce::assume_unique_for_key(nonce_array);

        let mut plaintext = ciphertext.to_vec();
        let plaintext = key
            .open_in_place(nonce, Aad::empty(), &mut plaintext)
            .map_err(|_| Error::Internal("Decryption failed".into()))?;

        Ok(plaintext.to_vec())
    }

    pub fn encrypt_string(&self, plaintext: &str) -> Result<(Vec<u8>, Vec<u8>)> {
        self.encrypt(plaintext.as_bytes())
    }

    pub fn decrypt_string(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<String> {
        let plaintext = self.decrypt(ciphertext, nonce)?;
        String::from_utf8(plaintext)
            .map_err(|e| Error::Internal(format!("Invalid UTF-8: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = CryptoService::generate_key().unwrap();
        let crypto = CryptoService::from_hex(&key).unwrap();

        let plaintext = "Hello, World!";
        let (ciphertext, nonce) = crypto.encrypt_string(plaintext).unwrap();
        let decrypted = crypto.decrypt_string(&ciphertext, &nonce).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = CryptoService::generate_key().unwrap();
        let key2 = CryptoService::generate_key().unwrap();

        let crypto1 = CryptoService::from_hex(&key1).unwrap();
        let crypto2 = CryptoService::from_hex(&key2).unwrap();

        let plaintext = "Secret message";
        let (ciphertext, nonce) = crypto1.encrypt_string(plaintext).unwrap();

        let result = crypto2.decrypt_string(&ciphertext, &nonce);
        assert!(result.is_err());
    }
}
