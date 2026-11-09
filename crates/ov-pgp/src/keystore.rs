//! Android Keystore backed master wrapping key.
//!
//! The master wrapping key is an AES-256-GCM key that lives inside the
//! `AndroidKeyStore`.  It is generated with `KeyGenParameterSpec` (GCM block
//! mode, no padding, 256 bit key size) and can be configured to require a
//! user authentication (biometric or device credential) before every use.
//!
//! The TSK wrapping itself is performed by the Keystore: Rust hands the
//! plaintext to `Cipher.doFinal` and receives the ciphertext back.  The
//! master key therefore never leaves the secure hardware.
//!
//! On non-Android platforms the Keystore is not available and the caller has
//! to fall back to [`crate::master_key::SoftwareMasterKey`].

#[cfg(target_os = "android")]
use anyhow::anyhow;
use anyhow::{bail, Result};

use crate::master_key::MasterKeyProvider;

/// Default alias of the master wrapping key.
pub const DEFAULT_ALIAS: &str = "onionvault.master.wrap.v1";

/// `KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT`.
#[cfg(target_os = "android")]
const PURPOSE_ENCRYPT_DECRYPT: i32 = 0x1 | 0x2;

/// `Cipher.ENCRYPT_MODE`.
#[cfg(target_os = "android")]
const CIPHER_ENCRYPT_MODE: i32 = 1;

/// `Cipher.DECRYPT_MODE`.
#[cfg(target_os = "android")]
const CIPHER_DECRYPT_MODE: i32 = 2;

/// GCM authentication tag length in bits.
#[cfg(target_os = "android")]
const GCM_TAG_BITS: i32 = 128;

/// Length of the GCM nonce that the Keystore generates.
#[cfg(target_os = "android")]
const IV_LEN: usize = 12;

/// How long the master key stays unlocked after the user confirmed the
/// prompt of the native UI.
#[cfg(target_os = "android")]
const AUTH_VALIDITY_SECONDS: i32 = 30;

/// A master key that is kept inside the Android Keystore.
#[cfg(target_os = "android")]
pub struct AndroidKeystoreMasterKey {
    vm: jni::JavaVM,
    alias: String,
    require_user_auth: bool,
}

#[cfg(target_os = "android")]
impl AndroidKeystoreMasterKey {
    /// Open (and create if necessary) the master key in the Keystore.
    pub fn open(vm: jni::JavaVM, alias: &str, require_user_auth: bool) -> Result<Self> {
        let provider = Self {
            vm,
            alias: alias.to_string(),
            require_user_auth,
        };
        provider.ensure_key()?;
        Ok(provider)
    }

    /// The alias of the Keystore entry.
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// `true` when the Keystore entry demands a user authentication.
    pub fn requires_user_authentication(&self) -> bool {
        self.require_user_auth
    }

    /// Create the Keystore entry if it does not exist yet.
    fn ensure_key(&self) -> Result<()> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| anyhow!("cannot attach to the Java VM: {e}"))?;

        let keystore = open_keystore(&mut env)?;
        let alias = alias_string(&mut env, &self.alias)?;
        let exists = env
            .call_method(
                &keystore,
                "containsAlias",
                "(Ljava/lang/String;)Z",
                &[(&alias).into()],
            )
            .map_err(|e| anyhow!("cannot query the Android Keystore: {e}"))?
            .z()?;
        if exists {
            return Ok(());
        }

        generate_key(&mut env, &self.alias, self.require_user_auth)
    }
}

#[cfg(target_os = "android")]
impl MasterKeyProvider for AndroidKeystoreMasterKey {
    fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| anyhow!("cannot attach to the Java VM: {e}"))?;

        let keystore = open_keystore(&mut env)?;
        let key = load_key(&mut env, &keystore, &self.alias)?;

        let cipher = new_cipher(&mut env)?;
        env.call_method(
            &cipher,
            "init",
            "(ILjava/security/Key;)V",
            &[CIPHER_ENCRYPT_MODE.into(), (&key).into()],
        )
        .map_err(|e| anyhow!("cannot initialise the Keystore cipher: {e}"))?;

        let iv = env
            .call_method(&cipher, "getIv", "()[B", &[])
            .map_err(|e| anyhow!("cannot read the GCM nonce: {e}"))?
            .l()?;
        let iv = jni::objects::JByteArray::from(iv);
        let iv = env
            .convert_byte_array(&iv)
            .map_err(|e| anyhow!("cannot read the GCM nonce: {e}"))?;
        if iv.len() != IV_LEN {
            bail!(
                "the Keystore returned a {}-byte nonce, expected {IV_LEN}",
                iv.len()
            );
        }

        let plaintext_array = env.byte_array_from_slice(plaintext)?;
        let ciphertext = env
            .call_method(&cipher, "doFinal", "([B)[B", &[(&plaintext_array).into()])
            .map_err(|e| anyhow!("the Keystore refused to encrypt the secret key: {e}"))?
            .l()?;
        let ciphertext = jni::objects::JByteArray::from(ciphertext);
        let ciphertext = env
            .convert_byte_array(&ciphertext)
            .map_err(|e| anyhow!("cannot read the ciphertext: {e}"))?;

        let mut blob = Vec::with_capacity(iv.len() + ciphertext.len());
        blob.extend_from_slice(&iv);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    fn unwrap(&self, blob: &[u8]) -> Result<Vec<u8>> {
        if blob.len() <= IV_LEN {
            bail!("the wrapped blob is too short ({} bytes)", blob.len());
        }
        let (iv, ciphertext) = blob.split_at(IV_LEN);

        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| anyhow!("cannot attach to the Java VM: {e}"))?;

        let keystore = open_keystore(&mut env)?;
        let key = load_key(&mut env, &keystore, &self.alias)?;

        let cipher = new_cipher(&mut env)?;
        let spec_class = env
            .find_class("javax/crypto/spec/GCMParameterSpec")
            .map_err(|e| anyhow!("cannot find GCMParameterSpec: {e}"))?;
        let iv_array = env.byte_array_from_slice(iv)?;
        let spec = env
            .new_object(
                &spec_class,
                "(I[B)V",
                &[GCM_TAG_BITS.into(), (&iv_array).into()],
            )
            .map_err(|e| anyhow!("cannot build the GCM parameters: {e}"))?;

        env.call_method(
            &cipher,
            "init",
            "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
            &[CIPHER_DECRYPT_MODE.into(), (&key).into(), (&spec).into()],
        )
        .map_err(|e| anyhow!("cannot initialise the Keystore cipher: {e}"))?;

        let ciphertext_array = env.byte_array_from_slice(ciphertext)?;
        let plaintext = env
            .call_method(&cipher, "doFinal", "([B)[B", &[(&ciphertext_array).into()])
            .map_err(|e| anyhow!("the Keystore refused to decrypt the secret key: {e}"))?
            .l()?;
        let plaintext = jni::objects::JByteArray::from(plaintext);
        env.convert_byte_array(&plaintext)
            .map_err(|e| anyhow!("cannot read the plaintext: {e}"))
    }

    fn describe(&self) -> String {
        format!(
            "Android Keystore ({}, user authentication {})",
            self.alias,
            if self.require_user_auth {
                "required"
            } else {
                "not required"
            }
        )
    }
}

/// Open and load the `AndroidKeyStore` provider.
#[cfg(target_os = "android")]
fn open_keystore<'local>(env: &mut jni::JNIEnv<'local>) -> Result<jni::objects::JObject<'local>> {
    let class = env
        .find_class("java/security/KeyStore")
        .map_err(|e| anyhow!("cannot find java.security.KeyStore: {e}"))?;
    let name = env
        .new_string("AndroidKeyStore")
        .map_err(|e| anyhow!("cannot create the provider name: {e}"))?;
    let keystore = env
        .call_method(
            &class,
            "getInstance",
            "(Ljava/lang/String;)Ljava/security/KeyStore;",
            &[(&name).into()],
        )
        .map_err(|e| anyhow!("cannot obtain the AndroidKeyStore: {e}"))?
        .l()?;
    let null = jni::objects::JObject::null();
    env.call_method(
        &keystore,
        "load",
        "(Ljava/security/KeyStore$LoadStoreParameter;)V",
        &[(&null).into()],
    )
    .map_err(|e| anyhow!("cannot load the AndroidKeyStore: {e}"))?;
    Ok(keystore)
}

/// Fetch the key from the Keystore.
#[cfg(target_os = "android")]
fn load_key<'local>(
    env: &mut jni::JNIEnv<'local>,
    keystore: &jni::objects::JObject<'local>,
    alias: &str,
) -> Result<jni::objects::JObject<'local>> {
    let key_alias = alias_string(env, alias)?;
    let null = jni::objects::JObject::null();
    let key = env
        .call_method(
            keystore,
            "getKey",
            "(Ljava/lang/String;[C)Ljava/security/Key;",
            &[(&key_alias).into(), (&null).into()],
        )
        .map_err(|e| anyhow!("cannot read the key {alias} from the Keystore: {e}"))?
        .l()?;
    Ok(key)
}

/// Create a fresh AES-256-GCM key inside the Keystore.
#[cfg(target_os = "android")]
fn generate_key(env: &mut jni::JNIEnv<'_>, alias: &str, require_user_auth: bool) -> Result<()> {
    let generator_class = env
        .find_class("javax/crypto/KeyGenerator")
        .map_err(|e| anyhow!("cannot find javax.crypto.KeyGenerator: {e}"))?;
    let algorithm = env
        .new_string("AES")
        .map_err(|e| anyhow!("cannot create the algorithm name: {e}"))?;
    let provider = env
        .new_string("AndroidKeyStore")
        .map_err(|e| anyhow!("cannot create the provider name: {e}"))?;
    let generator = env
        .call_method(
            &generator_class,
            "getInstance",
            "(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;",
            &[(&algorithm).into(), (&provider).into()],
        )
        .map_err(|e| anyhow!("cannot obtain an AndroidKeyStore key generator: {e}"))?
        .l()?;

    let builder_class = env
        .find_class("android/security/keystore/KeyGenParameterSpec$Builder")
        .map_err(|e| anyhow!("cannot find KeyGenParameterSpec.Builder: {e}"))?;
    let key_alias = alias_string(env, alias)?;
    let builder = env
        .new_object(
            &builder_class,
            "(Ljava/lang/String;I)V",
            &[(&key_alias).into(), PURPOSE_ENCRYPT_DECRYPT.into()],
        )
        .map_err(|e| anyhow!("cannot build the KeyGenParameterSpec builder: {e}"))?;

    let builder = set_string_array_method(env, &builder, "setBlockModes", "GCM")?;
    let builder = set_string_array_method(env, &builder, "setEncryptionPaddings", "NoPadding")?;

    let builder = env
        .call_method(
            &builder,
            "setKeySize",
            "(I)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[256.into()],
        )
        .map_err(|e| anyhow!("cannot set the key size: {e}"))?
        .l()?;

    let builder = env
        .call_method(
            &builder,
            "setUserAuthenticationRequired",
            "(Z)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[(require_user_auth as i32).into()],
        )
        .map_err(|e| anyhow!("cannot configure the user authentication: {e}"))?
        .l()?;

    if require_user_auth {
        // The key is unlocked for a short moment after the user confirmed
        // the prompt of the native UI.  Without the validity window the key
        // would demand a `CryptoObject` that is bound to a single cipher
        // operation, which the native UI cannot provide.
        let _builder = env
            .call_method(
                &builder,
                "setUserAuthenticationValidityDurationSeconds",
                "(I)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
                &[AUTH_VALIDITY_SECONDS.into()],
            )
            .map_err(|e| anyhow!("cannot configure the validity of the authentication: {e}"))?
            .l()?;
    }

    let spec = env
        .call_method(
            &builder,
            "build",
            "()Landroid/security/keystore/KeyGenParameterSpec;",
            &[],
        )
        .map_err(|e| anyhow!("cannot build the KeyGenParameterSpec: {e}"))?
        .l()?;

    env.call_method(
        &generator,
        "init",
        "(Ljava/security/spec/AlgorithmParameterSpec;)V",
        &[(&spec).into()],
    )
    .map_err(|e| anyhow!("cannot initialise the key generator: {e}"))?;

    env.call_method(&generator, "generateKey", "()Ljava/security/Key;", &[])
        .map_err(|e| anyhow!("the AndroidKeyStore refused to generate the key: {e}"))?;

    Ok(())
}

/// Call a `Builder` method that takes a `String[]` argument.
#[cfg(target_os = "android")]
fn set_string_array_method<'local>(
    env: &mut jni::JNIEnv<'local>,
    builder: &jni::objects::JObject<'local>,
    method: &str,
    value: &str,
) -> Result<jni::objects::JObject<'local>> {
    let text = env
        .new_string(value)
        .map_err(|e| anyhow!("cannot create {value:?}: {e}"))?;
    let array = env
        .new_object_array(1, "java/lang/String", &text)
        .map_err(|e| anyhow!("cannot create the string array: {e}"))?;
    let signature =
        format!("([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;");
    let result = env
        .call_method(builder, method, &signature, &[(&array).into()])
        .map_err(|e| anyhow!("{method} failed: {e}"))?
        .l()?;
    Ok(result)
}

/// Create an `AES/GCM/NoPadding` cipher.
#[cfg(target_os = "android")]
fn new_cipher<'local>(env: &mut jni::JNIEnv<'local>) -> Result<jni::objects::JObject<'local>> {
    let class = env
        .find_class("javax/crypto/Cipher")
        .map_err(|e| anyhow!("cannot find javax.crypto.Cipher: {e}"))?;
    let transformation = env
        .new_string("AES/GCM/NoPadding")
        .map_err(|e| anyhow!("cannot create the transformation name: {e}"))?;
    let cipher = env
        .call_method(
            &class,
            "getInstance",
            "(Ljava/lang/String;)Ljavax/crypto/Cipher;",
            &[(&transformation).into()],
        )
        .map_err(|e| anyhow!("cannot obtain an AES/GCM cipher: {e}"))?
        .l()?;
    Ok(cipher)
}

/// Helper that creates a `java.lang.String`.
#[cfg(target_os = "android")]
fn alias_string<'local>(
    env: &mut jni::JNIEnv<'local>,
    alias: &str,
) -> Result<jni::objects::JString<'local>> {
    env.new_string(alias)
        .map_err(|e| anyhow!("cannot create the key alias: {e}"))
}

/// The Keystore is only available on Android; everywhere else this helper
/// explains the situation instead of silently degrading.
#[cfg(not(target_os = "android"))]
pub fn open(_alias: &str, _require_user_auth: bool) -> Result<Box<dyn MasterKeyProvider>> {
    bail!("the Android Keystore is only available on Android")
}
