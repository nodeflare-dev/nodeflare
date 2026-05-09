pub mod jwt;
pub mod github;
pub mod google;
pub mod api_key;
pub mod password;
pub mod crypto;
pub mod revocation;

pub use jwt::{Claims, JwtService};
pub use github::GitHubOAuth;
pub use google::GoogleOAuth;
pub use api_key::ApiKeyService;
pub use password::{hash_password, verify_password};
pub use crypto::CryptoService;
pub use revocation::{is_token_revoked, revoke_token, revoke_all_user_tokens, get_user_revocation_timestamp};
