//! Password hashing and verification helper module

use anyhow::Result;
use bcrypt::{hash, verify, DEFAULT_COST};

/// Hash a password using bcrypt with default cost
pub fn hash_password(password: &str) -> Result<String> {
    let hashed = hash(password, DEFAULT_COST)?;
    Ok(hashed)
}

/// Hash a password using bcrypt with custom cost
pub fn hash_password_with_cost(password: &str, cost: u32) -> Result<String> {
    let hashed = hash(password, cost)?;
    Ok(hashed)
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let valid = verify(password, hash)?;
    Ok(valid)
}

/// Validate password strength
pub fn validate_password_strength(password: &str) -> Result<()> {
    if password.len() < 8 {
        return Err(anyhow::anyhow!(
            "Password must be at least 8 characters long"
        ));
    }

    let has_uppercase = password.chars().any(|c| c.is_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_digit(10));
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    if !has_uppercase || !has_lowercase || !has_digit {
        return Err(anyhow::anyhow!(
            "Password must contain uppercase, lowercase, and digits"
        ));
    }

    if password.len() >= 12 && !has_special {
        // For longer passwords, require special characters
        return Err(anyhow::anyhow!(
            "Passwords 12+ characters must include special characters"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "SecurePass123!";
        let hashed = hash_password(password).expect("Failed to hash password");

        // Hash should be different from original
        assert_ne!(password, hashed);

        // Should verify correctly
        assert!(verify_password(password, &hashed).expect("Failed to verify password"));

        // Wrong password should fail
        assert!(!verify_password("WrongPass", &hashed).expect("Failed to verify wrong password"));
    }

    #[test]
    fn test_password_strength_validation() {
        // Too short
        assert!(validate_password_strength("Pass1!").is_err());

        // No uppercase
        assert!(validate_password_strength("password123").is_err());

        // No lowercase
        assert!(validate_password_strength("PASSWORD123").is_err());

        // No digits
        assert!(validate_password_strength("Password!").is_err());

        // Valid password
        assert!(validate_password_strength("Password123").is_ok());

        // Long password without special chars
        assert!(validate_password_strength("VeryLongPassword123").is_err());

        // Long password with special chars
        assert!(validate_password_strength("VeryLongPassword123!").is_ok());
    }
}
