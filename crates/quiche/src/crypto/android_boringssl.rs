#![deny(clippy::undocumented_unsafe_blocks)]

use super::{
    derive_pkt_iv, derive_pkt_key, make_nonce, Algorithm, Error, FfiMutSlice, FfiSlice,
    HeaderProtectionMask, Result, HP_MASK_LEN,
};

use std::convert::TryFrom;

use std::mem::size_of;
use std::mem::MaybeUninit;

use libc::c_int;
use libc::c_uint;

use bssl_sys::{
    AES_ecb_encrypt, AES_set_encrypt_key, CRYPTO_chacha_20, EVP_AEAD_CTX_cleanup,
    EVP_AEAD_CTX_init, EVP_AEAD_CTX_open, EVP_AEAD_CTX_seal_scatter, EVP_aead_aes_128_gcm_tls13,
    EVP_aead_aes_256_gcm_tls13, EVP_aead_chacha20_poly1305, HKDF_expand, HKDF_extract, AES_KEY,
    EVP_AEAD, EVP_AEAD_CTX, EVP_MD,
};

impl Algorithm {
    fn get_evp_aead(self) -> *const EVP_AEAD {
        match self {
            // SAFETY: FFI call, this function returns a static immutable lifetime object.
            Algorithm::AES128_GCM => unsafe { EVP_aead_aes_128_gcm_tls13() },
            // SAFETY: FFI call,this function returns a static immutable lifetime object.
            Algorithm::AES256_GCM => unsafe { EVP_aead_aes_256_gcm_tls13() },
            // SAFETY: FFI call, this function returns a static immutable lifetime object.
            Algorithm::ChaCha20_Poly1305 => unsafe { EVP_aead_chacha20_poly1305() },
        }
    }
}

pub(crate) struct PacketKey {
    alg: Algorithm,

    ctx: EVP_AEAD_CTX,

    nonce: Vec<u8>,
}

// SAFETY: the components of the PacketKey are safe to send between threads. Note they are
// *not* safe to Sync, because the EVP_AEAD_CTX contains non-thread-safe state for TLS support.
unsafe impl Send for PacketKey {}

impl Drop for PacketKey {
    fn drop(&mut self) {
        // Safety: `self.ctx` was initialized by `EVP_AEAD_CTX_init` because all
        // paths to create a `PacketKey` do so.
        unsafe {
            EVP_AEAD_CTX_cleanup(&mut self.ctx);
        }
    }
}

impl PacketKey {
    pub fn new(alg: Algorithm, key: Vec<u8>, iv: Vec<u8>, _enc: u32) -> Result<Self> {
        Ok(Self { alg, ctx: make_aead_ctx(alg, &key)?, nonce: iv })
    }

    pub fn from_secret(aead: Algorithm, secret: &[u8], enc: u32) -> Result<Self> {
        let key_len = aead.key_len();
        let nonce_len = aead.nonce_len();

        let mut key = vec![0; key_len];
        let mut iv = vec![0; nonce_len];

        derive_pkt_key(aead, secret, &mut key)?;
        derive_pkt_iv(aead, secret, &mut iv)?;

        let mut pkt_key = Self::new(aead, key, iv, enc)?;

        // Dummy seal operation to prime the AEAD context with the nonce mask.
        //
        // This is needed because BoringCrypto requires the first counter (i.e.
        // packet number) to be zero, which would not be the case for packet
        // number spaces after Initial as the same packet number sequence is
        // shared.
        let _ = pkt_key.seal_with_u64_counter(0, b"", &mut [0_u8; 16], 0, None);

        Ok(pkt_key)
    }

    pub fn open_with_u64_counter(&self, counter: u64, ad: &[u8], buf: &mut [u8]) -> Result<usize> {
        let tag_len: usize = self.alg.tag_len();

        let mut out_len = match buf.len().checked_sub(tag_len) {
            Some(n) => n,
            None => return Err(Error::CryptoFail),
        };

        let max_out_len = out_len;

        let nonce = make_nonce(&self.nonce, counter);

        // Safety: FFI call, the input buffers are all valid, with corresponding
        // ptr and length. The output buffer has at least `max_output`
        // bytes of space and that maximum is passed to
        // `EVP_AEAD_CTX_open` as a limit.
        let rc = unsafe {
            EVP_AEAD_CTX_open(
                &self.ctx,              // ctx
                buf.as_mut_ffi_ptr(),   // out
                &mut out_len,           // out_len
                max_out_len,            // max_out_len
                nonce[..].as_ffi_ptr(), // nonce
                nonce.len(),            // nonce_len
                buf.as_ffi_ptr(),       // inp
                buf.len(),              // in_len
                ad.as_ffi_ptr(),        // ad
                ad.len(),               // ad_len
            )
        };

        if rc != 1 {
            return Err(Error::CryptoFail);
        }

        Ok(out_len)
    }

    pub fn seal_with_u64_counter(
        &mut self,
        counter: u64,
        ad: &[u8],
        buf: &mut [u8],
        in_len: usize,
        extra_in: Option<&[u8]>,
    ) -> Result<usize> {
        let tag_len: usize = self.alg.tag_len();

        let mut out_tag_len = tag_len;

        let (extra_in_ptr, extra_in_len) = match extra_in {
            Some(v) => (v.as_ffi_ptr(), v.len()),

            None => (std::ptr::null(), 0),
        };

        let Some((in_out_buf, remaining_buf)) = buf.split_at_mut_checked(in_len) else {
            return Err(Error::CryptoFail);
        };
        // Make sure all the outputs combined fit in the buffer.
        let max_out_tag_len = tag_len + extra_in_len;
        if max_out_tag_len > remaining_buf.len() {
            return Err(Error::CryptoFail);
        }

        let nonce = make_nonce(&self.nonce, counter);

        let mut out_tag_len = remaining_buf.len();
        // Safety: the buffers are all valid, with corresponding ptr and length.
        // `max_out_tag_len` is passed at the maximum size of `remaining_buf`
        // to ensure that both the extra input and the tag can fit in the `remaining_buf`.
        let rc = unsafe {
            EVP_AEAD_CTX_seal_scatter(
                &mut self.ctx,                  // ctx
                in_out_buf.as_mut_ffi_ptr(),    // out
                remaining_buf.as_mut_ffi_ptr(), // out_tag
                &mut out_tag_len,               // out_tag_len
                max_out_tag_len,                // max_out_tag_len
                nonce[..].as_ffi_ptr(),         // nonce
                nonce.len(),                    // nonce_len
                in_out_buf.as_ffi_ptr(),        // inp
                in_len,                         // in_len
                extra_in_ptr,                   // extra_in
                extra_in_len,                   // extra_in_len
                ad.as_ffi_ptr(),                // ad
                ad.len(),                       // ad_len
            )
        };

        if rc != 1 {
            return Err(Error::CryptoFail);
        }

        Ok(in_len + out_tag_len)
    }
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum HeaderProtectionKey {
    Aes(AES_KEY),

    ChaCha(Vec<u8>),
}

impl HeaderProtectionKey {
    pub fn new(alg: Algorithm, hp_key: Vec<u8>) -> Result<Self> {
        match alg {
            Algorithm::AES128_GCM | Algorithm::AES256_GCM => {
                if (hp_key.len() != alg.key_len()) {
                    return Err(Error::CryptoFail);
                }

                let key_len_bits = alg.key_len() as u32 * 8;

                let mut aes_key = MaybeUninit::<AES_KEY>::zeroed();

                // SAFETY: FFI call, `AES_KEY` has a definite size as generated by `bindgen`, so
                // its allocation is large enough to hold `hp_key`. The return value of this function
                // differs from the usual BoringSSL convention.
                let rc = unsafe {
                    AES_set_encrypt_key(hp_key.as_ptr(), key_len_bits, aes_key.as_mut_ptr())
                };

                if rc != 0 {
                    return Err(Error::CryptoFail);
                }

                // SAFETY:  zeroed memory is safe for AES_KEY as it is a C type.
                let aes_key = unsafe { aes_key.assume_init() };
                Ok(Self::Aes(aes_key))
            }

            Algorithm::ChaCha20_Poly1305 => Ok(Self::ChaCha(hp_key)),
        }
    }

    pub fn new_mask(&self, sample: &[u8]) -> Result<HeaderProtectionMask> {
        match self {
            Self::Aes(aes_key) => {
                if sample.len() != 16 {
                    return Err(Error::CryptoFail);
                }

                let mut block = [0_u8; 16];

                // SAFETY: FFI call, `sample` and `block` are both 16 bytes blocks and non-overlapping.
                unsafe { AES_ecb_encrypt(sample.as_ffi_ptr(), block.as_mut_ffi_ptr(), aes_key, 1) };

                // Downsize the encrypted block to the size of the header
                // protection mask.
                //
                // The length of the slice will always match the size of
                // `HeaderProtectionMask` so the `unwrap()` is safe.
                let new_mask = HeaderProtectionMask::try_from(&block[..HP_MASK_LEN]).unwrap();
                Ok(new_mask)
            }

            Self::ChaCha(key) => {
                const PLAINTEXT: &[u8; HP_MASK_LEN] = &[0_u8; HP_MASK_LEN];

                // Check that the key and sample are the expected sizes for ChaCha20.
                if (key.len() != 32 || sample.len() != 16) {
                    return Err(Error::CryptoFail);
                }

                let mut new_mask = HeaderProtectionMask::default();

                let counter = u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);

                // SAFETY: FFI call, preconditions upheld:
                // - `new_mask` is valid for writes of at least `PLAINTEXT.len()` bytes.
                // - `PLAINTEXT` is valid for reads of `PLAINTEXT.len()` bytes.
                // - `key` is valid for reads of 32 bytes (standard ChaCha20 key size).
                // - `sample[size_of::<u32>()..]` is valid for reads of 12 bytes (standard ChaCha20 nonce size).
                // - The input and output buffers (`PLAINTEXT` and `new_mask`) do not overlap,
                //    or if they do, they are exactly the same pointer (in-place encryption).
                unsafe {
                    CRYPTO_chacha_20(
                        new_mask.as_mut_ffi_ptr(),
                        PLAINTEXT.as_ffi_ptr(),
                        PLAINTEXT.len(),
                        key.as_ffi_ptr(),
                        sample[size_of::<u32>()..].as_ffi_ptr(),
                        counter,
                    );
                };

                Ok(new_mask)
            }
        }
    }
}

fn make_aead_ctx(alg: Algorithm, key: &[u8]) -> Result<EVP_AEAD_CTX> {
    if key.len() < alg.key_len() {
        return Err(Error::CryptoFail);
    }

    let mut ctx = MaybeUninit::uninit();

    let aead = alg.get_evp_aead();

    // SAFETY: FFI call, `key` points to at least `alg.key_len()` bytes.
    let rc = unsafe {
        EVP_AEAD_CTX_init(
            ctx.as_mut_ptr(),
            aead as *const _,
            key.as_ffi_ptr(),
            alg.key_len(),
            alg.tag_len(),
            std::ptr::null_mut(),
        )
    };

    if rc != 1 {
        return Err(Error::CryptoFail);
    }

    // SAFETY: must guarantee that `ctx` is initialized.
    let ctx = unsafe { ctx.assume_init() };
    Ok(ctx)
}

pub(crate) fn hkdf_extract(
    alg: Algorithm,
    out: &mut [u8],
    secret: &[u8],
    salt: &[u8],
) -> Result<()> {
    let mut out_len = out.len();

    // Safety: `EVP_MAX_MD_SIZE` is the maximum output size of
    // `HKDF_extract` so it'll never overrun the buffer.
    let rc = unsafe {
        HKDF_extract(
            out.as_mut_ffi_ptr(),
            &mut out_len,
            alg.get_evp_digest() as *const _,
            secret.as_ffi_ptr(),
            secret.len(),
            salt.as_ffi_ptr(),
            salt.len(),
        )
    };

    if rc != 1 {
        return Err(Error::CryptoFail);
    }

    Ok(())
}

pub(crate) fn hkdf_expand(
    alg: Algorithm,
    out: &mut [u8],
    secret: &[u8],
    info: &[u8],
) -> Result<()> {
    // Safety: `HKDF_expand` writes exactly `out_len` bytes or else
    // returns zero. `evp_md` is valid by construction.
    let rc = unsafe {
        HKDF_expand(
            out.as_mut_ffi_ptr(),
            out.len(),
            alg.get_evp_digest() as *const _, // evp_md
            secret.as_ffi_ptr(),
            secret.len(),
            info.as_ffi_ptr(),
            info.len(),
        )
    };

    if rc != 1 {
        return Err(Error::CryptoFail);
    }

    Ok(())
}
