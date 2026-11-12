#![deny(clippy::undocumented_unsafe_blocks)]

use super::*;

use crate::FfiSlice;

use std::str;

use bssl_sys::{
    sk_CRYPTO_BUFFER_num, sk_CRYPTO_BUFFER_value, stack_st_CRYPTO_BUFFER, CRYPTO_BUFFER_data,
    CRYPTO_BUFFER_len, OPENSSL_free, SSL_CTX_set_early_data_enabled, SSL_SESSION_free,
    SSL_SESSION_from_bytes, SSL_SESSION_to_bytes, SSL_get0_peer_certificates, SSL_get_SSL_CTX,
    SSL_get_curve_id, SSL_get_curve_name, SSL_get_early_data_reason,
    SSL_get_peer_signature_algorithm, SSL_get_signature_algorithm_name, SSL_in_early_data,
    SSL_reset_early_data_reject, SSL_set_quic_early_data_context, SSL_set_session, CRYPTO_BUFFER,
};

pub use bssl_sys::SSL_QUIC_METHOD;

#[cfg(test)]
use bssl_sys::{SSL_set_private_key_method, SSL_PRIVATE_KEY_METHOD};

pub(super) static QUICHE_STREAM_METHOD: SSL_QUIC_METHOD = SSL_QUIC_METHOD {
    set_read_secret: Some(g3_set_read_secret),
    set_write_secret: Some(g3_set_write_secret),
    add_handshake_data: Some(g3_add_handshake_data),
    flush_flight: Some(g3_flush_flight),
    send_alert: Some(g3_send_alert),
};

impl Context {
    pub fn set_early_data_enabled(&mut self, enabled: bool) {
        unsafe {
            // SAFETY: `self.as_mut_ptr()` returns a valid pointer to an initialized `SSL_CTX`.
            SSL_CTX_set_early_data_enabled(self.as_mut_ptr(), if enabled { 1 } else { 0 });
        }
    }
}

impl Handshake {
    pub fn set_quic_early_data_context(&mut self, context: &[u8]) -> Result<()> {
        map_result(unsafe {
            // SAFETY: `self.as_mut_ptr()` returns a valid pointer to an initialized `SSL`,
            // and `context` provides a valid pointer and length.
            SSL_set_quic_early_data_context(self.as_mut_ptr(), context.as_ffi_ptr(), context.len())
        })
    }

    pub fn set_session(&mut self, session: &[u8]) -> Result<()> {
        let ctx = unsafe {
            // SAFETY: `self.as_ptr()` returns a valid pointer to an initialized `SSL`.
            SSL_get_SSL_CTX(self.as_ptr())
        };

        if ctx.is_null() {
            return Err(Error::TlsFail);
        }

        let session = unsafe {
            // SAFETY: `session` provides a valid buffer and length, and `ctx` is a valid `SSL_CTX`.
            SSL_SESSION_from_bytes(session.as_ffi_ptr(), session.len(), ctx)
        };

        if session.is_null() {
            return Err(Error::TlsFail);
        }

        let rc = unsafe {
            // SAFETY: `self.as_mut_ptr()` and `session` are valid pointers to `SSL` and
            // `SSL_SESSION`.
            SSL_set_session(self.as_mut_ptr(), session)
        };

        unsafe {
            // SAFETY: `session` was allocated by `SSL_SESSION_from_bytes` with exclusive access,
            // and is safe for us to free.
            SSL_SESSION_free(session);
        }

        map_result(rc)
    }

    pub fn reset_early_data_reject(&mut self) {
        unsafe {
            // SAFETY: `self.as_mut_ptr()` returns a valid pointer to an initialized `SSL`.
            SSL_reset_early_data_reject(self.as_mut_ptr())
        };
    }

    pub fn curve(&self) -> Option<String> {
        let curve_id = unsafe {
            // SAFETY: `self.as_ptr()` returns a valid pointer to an initialized `SSL`.
            SSL_get_curve_id(self.as_ptr())
        };
        if curve_id == 0 {
            return None;
        }

        let curve_name = unsafe {
            // SAFETY: `curve_id` is a valid curve ID returned from `SSL_get_curve_id`.
            SSL_get_curve_name(curve_id)
        };
        if curve_name.is_null() {
            return None;
        }

        let curve = unsafe {
            // SAFETY:
            // - `curve_name` is non-null and points to a valid null-terminated C string.
            // - BoringSSL curve names are static ASCII strings, which are valid UTF-8.
            str::from_utf8_unchecked(ffi::CStr::from_ptr(curve_name).to_bytes())
        };

        Some(curve.to_string())
    }

    pub fn sigalg(&self) -> Option<String> {
        let sigalg_id = unsafe {
            // SAFETY: `self.as_ptr()` returns a valid pointer to an initialized `SSL`.
            SSL_get_peer_signature_algorithm(self.as_ptr())
        };
        if sigalg_id == 0 {
            return None;
        }

        let sigalg_name = unsafe {
            // SAFETY: `sigalg_id` is a valid signature algorithm ID returned from
            // `SSL_get_peer_signature_algorithm`.
            SSL_get_signature_algorithm_name(sigalg_id, 1)
        };
        if sigalg_name.is_null() {
            return None;
        }

        let sigalg = unsafe {
            // SAFETY:
            // - `sigalg_name` is non-null and points to a valid null-terminated C string.
            // - BoringSSL signature algorithm names are static ASCII strings, which are valid
            //   UTF-8.
            str::from_utf8_unchecked(ffi::CStr::from_ptr(sigalg_name).to_bytes())
        };

        Some(sigalg.to_string())
    }

    pub fn peer_cert_chain(&self) -> Option<Vec<Vec<u8>>> {
        let chain: *const stack_st_CRYPTO_BUFFER = unsafe {
            // SAFETY:
            // - `self.as_ptr()` returns a valid pointer to an initialized `SSL`.
            // - The returned stack pointer is valid for the lifetime of `self`.
            map_result_ptr(SSL_get0_peer_certificates(self.as_ptr())).ok()?
        };

        let num = unsafe {
            // SAFETY: `chain` is a valid pointer to `stack_st_CRYPTO_BUFFER`.
            sk_CRYPTO_BUFFER_num(chain)
        };
        if num == 0 {
            return None;
        }

        let mut cert_chain = vec![];
        for i in 0..num {
            let buffer = map_result_ptr(unsafe {
                // SAFETY: `chain` is a valid pointer to `stack_st_CRYPTO_BUFFER` and `i` is within
                // bounds (`0..num`).
                sk_CRYPTO_BUFFER_value(chain, i)
            })
            .ok()?;

            let out_len = unsafe {
                // SAFETY: `buffer` is a valid non-null `CRYPTO_BUFFER`.
                CRYPTO_BUFFER_len(buffer)
            };
            let out = unsafe {
                // SAFETY: `buffer` is a valid non-null `CRYPTO_BUFFER`.
                CRYPTO_BUFFER_data(buffer)
            };
            debug_assert!(!out.is_null() && out_len > 0);
            let slice = unsafe {
                // SAFETY:
                // - `out` is a valid BoringSSL-allocated buffer of size `out_len`.
                // - The buffer is initialized and valid for reads of `out_len` bytes.
                // - BoringSSL guarantees the certificate is never empty per RFC9846.
                slice::from_raw_parts(out, out_len)
            };

            cert_chain.push(slice.to_vec());
        }

        Some(cert_chain)
    }

    pub fn peer_cert(&self) -> Option<Vec<u8>> {
        let chain: *const stack_st_CRYPTO_BUFFER = unsafe {
            // SAFETY:
            // - `self.as_ptr()` returns a valid pointer to an initialized `SSL`.
            // - The returned stack pointer is valid for the lifetime of `self`.
            map_result_ptr(SSL_get0_peer_certificates(self.as_ptr())).ok()?
        };
        let num = unsafe {
            // SAFETY: `chain` is a valid pointer to `stack_st_CRYPTO_BUFFER`.
            sk_CRYPTO_BUFFER_num(chain)
        };
        if num == 0 {
            return None;
        }

        let buffer = map_result_ptr(unsafe {
            // SAFETY: `chain` is a valid pointer to `stack_st_CRYPTO_BUFFER` and index 0 is within
            // bounds (`0 < num`).
            sk_CRYPTO_BUFFER_value(chain, 0)
        })
        .ok()?;

        let out_len = unsafe {
            // SAFETY: `buffer` is a valid non-null `CRYPTO_BUFFER`.
            CRYPTO_BUFFER_len(buffer)
        };

        let out = unsafe {
            // SAFETY: `buffer` is a valid non-null `CRYPTO_BUFFER`.
            CRYPTO_BUFFER_data(buffer)
        };
        debug_assert!(!out.is_null() && out_len > 0);
        let peer_cert = unsafe {
            // SAFETY:
            // - `out` is a valid BoringSSL-allocated buffer of size `out_len`.
            // - The buffer is initialized and valid for reads of `out_len` bytes.
            // - BoringSSL guarantees the certificate is never empty per RFC9846.
            slice::from_raw_parts(out, out_len)
        };

        Some(peer_cert.to_vec())
    }

    // Only used for testing handling of failure during key signing.
    #[cfg(test)]
    pub fn set_failing_private_key_method(&mut self) {
        // SAFETY: C callback invoked by BoringSSL; ignores arguments and unconditionally returns
        // failure.
        unsafe extern "C" fn failing_sign(
            _ssl: *mut SSL,
            _out: *mut u8,
            _out_len: *mut usize,
            _max_out: usize,
            _signature_algorithm: u16,
            _in: *const u8,
            _in_len: usize,
        ) -> ssl_private_key_result_t {
            bssl_sys::ssl_private_key_result_t_ssl_private_key_failure
        }

        // SAFETY: C callback invoked by BoringSSL; ignores arguments and unconditionally returns
        // failure.
        unsafe extern "C" fn failing_decrypt(
            _ssl: *mut SSL,
            _out: *mut u8,
            _out_len: *mut usize,
            _max_out: usize,
            _in: *const u8,
            _in_len: usize,
        ) -> ssl_private_key_result_t {
            bssl_sys::ssl_private_key_result_t_ssl_private_key_failure
        }

        // SAFETY: C callback invoked by BoringSSL; ignores arguments and unconditionally returns
        // failure.
        unsafe extern "C" fn failing_complete(
            _ssl: *mut SSL,
            _out: *mut u8,
            _out_len: *mut usize,
            _max_out: usize,
        ) -> ssl_private_key_result_t {
            bssl_sys::ssl_private_key_result_t_ssl_private_key_failure
        }

        static QUICHE_PRIVATE_KEY_METHOD: SSL_PRIVATE_KEY_METHOD = SSL_PRIVATE_KEY_METHOD {
            decrypt: Some(failing_decrypt),
            sign: Some(failing_sign),
            complete: Some(failing_complete),
        };

        unsafe {
            // SAFETY:
            // - `self.as_mut_ptr()` is a valid pointer to an initialized `SSL`.
            // - `QUICHE_PRIVATE_KEY_METHOD` is a valid static `SSL_PRIVATE_KEY_METHOD`.
            SSL_set_private_key_method(self.as_mut_ptr(), &raw const QUICHE_PRIVATE_KEY_METHOD);
        }
    }

    pub fn is_in_early_data(&self) -> bool {
        unsafe {
            // SAFETY: `self.as_ptr()` returns a valid pointer to an initialized `SSL`.
            SSL_in_early_data(self.as_ptr()) == 1
        }
    }

    pub fn early_data_reason(&self) -> u32 {
        let reuse_reason_status = unsafe {
            // SAFETY: `self.as_ptr()` returns a valid pointer to an initialized `SSL`.
            SSL_get_early_data_reason(self.as_ptr())
        };
        reuse_reason_status as u32
    }
}

pub(super) fn get_session_bytes(session: *mut SSL_SESSION) -> Result<Vec<u8>> {
    let mut out = ptr::null_mut();
    let mut out_len = 0;

    let rc = unsafe {
        // SAFETY:
        // - `session` is a valid `SSL_SESSION` handle.
        // - `out` and `out_len` are valid pointers to receive the allocated buffer and its length.
        SSL_SESSION_to_bytes(session, &raw mut out, &raw mut out_len)
    };
    if rc == 0 || out.is_null() {
        return Err(Error::TlsFail);
    }

    let session_bytes = unsafe {
        // SAFETY:
        // - `out` points to a valid BoringSSL-allocated buffer of length `out_len`.
        // - The buffer is initialized and valid for reads of `out_len` bytes.
        slice::from_raw_parts(out, out_len)
    };
    let session_bytes = session_bytes.to_vec();
    unsafe {
        // SAFETY: `out` was allocated by BoringSSL via `SSL_SESSION_to_bytes` with exclusive
        // access, and is safe for us to free using `OPENSSL_free`.
        OPENSSL_free(out as *mut _)
    };

    Ok(session_bytes)
}

pub(super) const TLS_ERROR: c_int = 3;
