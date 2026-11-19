//! Anti-spoofing capture metadata validation and ECDSA P-256 signature verification.

use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const CANONICAL_DOMAIN_TAG: &[u8] = b"BIODEX_CAPTURE_V1";
const MAX_CAPTURE_ID_BYTES: usize = 256;

/// Image acquisition pipeline reported by the mobile client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    /// Live capture directly from the Android CameraX pipeline (competitive-eligible).
    CameraXLive,
    /// Imported from device gallery (encyclopedia-only; invalid for competitive cards).
    Gallery,
}

/// WGS 84 geographic coordinates attached to a capture payload.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    /// Latitude in decimal degrees (`-90.0..=90.0`).
    pub latitude: f64,
    /// Longitude in decimal degrees (`-180.0..=180.0`).
    pub longitude: f64,
    /// Horizontal GPS accuracy radius in meters (`>= 0.0`).
    pub accuracy_meters: f32,
}

/// Hardware IMU sample collected at shutter release time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImuReading {
    /// Accelerometer `[x, y, z]` in `m/s^2`.
    pub accel_xyz: [f32; 3],
    /// Gyroscope `[x, y, z]` in `rad/s`.
    pub gyro_xyz: [f32; 3],
}

/// Signed anti-spoofing capture evidence submitted by the mobile client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureMetadata {
    /// Client-generated capture identifier (e.g., UUID string).
    pub capture_id: String,
    /// Capture timestamp in UTC milliseconds since Unix epoch.
    pub captured_at_utc_ms: i64,
    /// Acquisition pipeline (`camera_x_live` or `gallery`).
    pub source: CaptureSource,
    /// GPS coordinates at shutter time.
    pub coordinates: GeoPoint,
    /// Inertial sensor sample at shutter time.
    pub imu: ImuReading,
    /// Screen-Moiré heuristic score (`0.0` = natural scene, `1.0` = high screen probability).
    pub moire_score: f32,
    /// SHA-256 digest of the captured RAW/JPEG image payload.
    pub image_sha256: [u8; 32],
    /// ECDSA P-256 public key (X.509 SPKI DER or SEC1 bytes).
    pub device_public_key: Vec<u8>,
    /// ECDSA P-256 signature over [`CaptureMetadata::canonical_signing_bytes`] (ASN.1 DER or raw `r||s`).
    pub signature: Vec<u8>,
}

/// Verification errors for capture metadata and ECDSA P-256 signatures.
#[derive(Debug, Error, PartialEq)]
pub enum CryptoError {
    /// Gallery uploads cannot mint competitive or tradeable cards.
    #[error("gallery captures are not permitted for competitive card minting")]
    GallerySourceNotCompetitive,
    /// Capture identifier is empty or exceeds length bounds.
    #[error("capture_id must be non-empty and at most {MAX_CAPTURE_ID_BYTES} bytes")]
    InvalidCaptureId,
    /// Capture timestamp must be positive.
    #[error("captured_at_utc_ms must be greater than zero")]
    InvalidTimestamp,
    /// GPS coordinates or accuracy are non-finite or out of WGS 84 bounds.
    #[error("coordinates are invalid or out of WGS 84 bounds")]
    InvalidCoordinates,
    /// IMU readings contain non-finite values.
    #[error("IMU readings must contain only finite numbers")]
    InvalidImuTelemetry,
    /// Moiré score is non-finite or outside `0.0..=1.0`.
    #[error("moire_score must be a finite value within 0.0..=1.0")]
    InvalidMoireScore,
    /// Moiré score exceeded the allowed threshold for live captures.
    #[error("moire_score {score} exceeded maximum allowed threshold {max_allowed}")]
    MoireThresholdExceeded {
        /// Measured score.
        score: f32,
        /// Configured threshold.
        max_allowed: f32,
    },
    /// Device public key could not be decoded as ECDSA P-256 SPKI DER or SEC1.
    #[error("invalid ECDSA P-256 public key")]
    InvalidPublicKey,
    /// Signature bytes could not be decoded as DER or raw 64-byte `(r, s)`.
    #[error("invalid ECDSA P-256 signature encoding")]
    InvalidSignatureEncoding,
    /// Signature cryptographic verification failed.
    #[error("ECDSA P-256 signature verification failed")]
    SignatureVerificationFailed,
}

impl CaptureMetadata {
    /// Validates structural bounds and builds the deterministic canonical byte message
    /// signed by the Android Hardware Keystore (`SHA256withECDSA`).
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, CryptoError> {
        validate_metadata_fields(self)?;

        let id_bytes = self.capture_id.as_bytes();
        let id_len = u32::try_from(id_bytes.len()).map_err(|_| CryptoError::InvalidCaptureId)?;

        let mut buf = Vec::with_capacity(128 + id_bytes.len());
        buf.extend_from_slice(CANONICAL_DOMAIN_TAG);
        buf.extend_from_slice(&id_len.to_be_bytes());
        buf.extend_from_slice(id_bytes);
        buf.extend_from_slice(&self.captured_at_utc_ms.to_be_bytes());
        buf.push(match self.source {
            CaptureSource::CameraXLive => 0x01,
            CaptureSource::Gallery => 0x02,
        });
        buf.extend_from_slice(&canonical_f64_bits(self.coordinates.latitude));
        buf.extend_from_slice(&canonical_f64_bits(self.coordinates.longitude));
        buf.extend_from_slice(&canonical_f32_bits(self.coordinates.accuracy_meters));
        for axis in self.imu.accel_xyz {
            buf.extend_from_slice(&canonical_f32_bits(axis));
        }
        for axis in self.imu.gyro_xyz {
            buf.extend_from_slice(&canonical_f32_bits(axis));
        }
        buf.extend_from_slice(&canonical_f32_bits(self.moire_score));
        buf.extend_from_slice(&self.image_sha256);

        Ok(buf)
    }
}

/// Validates environment integrity and verifies the ECDSA P-256 signature of a capture
/// intended for competitive minting.
pub fn verify_competitive_capture(
    meta: &CaptureMetadata,
    max_moire_score: f32,
) -> Result<(), CryptoError> {
    if meta.source != CaptureSource::CameraXLive {
        return Err(CryptoError::GallerySourceNotCompetitive);
    }

    let payload = meta.canonical_signing_bytes()?;

    if !max_moire_score.is_finite() || !(0.0..=1.0).contains(&max_moire_score) {
        return Err(CryptoError::InvalidMoireScore);
    }
    if meta.moire_score > max_moire_score {
        return Err(CryptoError::MoireThresholdExceeded {
            score: meta.moire_score,
            max_allowed: max_moire_score,
        });
    }

    let verifying_key = parse_p256_public_key(&meta.device_public_key)?;
    let signature = parse_p256_signature(&meta.signature)?;

    verifying_key
        .verify(&payload, &signature)
        .map_err(|_| CryptoError::SignatureVerificationFailed)
}

fn validate_metadata_fields(meta: &CaptureMetadata) -> Result<(), CryptoError> {
    let trimmed_id = meta.capture_id.trim();
    if trimmed_id.is_empty() || meta.capture_id.len() > MAX_CAPTURE_ID_BYTES {
        return Err(CryptoError::InvalidCaptureId);
    }
    if meta.captured_at_utc_ms <= 0 {
        return Err(CryptoError::InvalidTimestamp);
    }

    let coords = meta.coordinates;
    if !coords.latitude.is_finite()
        || !coords.longitude.is_finite()
        || !coords.accuracy_meters.is_finite()
        || !(-90.0..=90.0).contains(&coords.latitude)
        || !(-180.0..=180.0).contains(&coords.longitude)
        || coords.accuracy_meters < 0.0
    {
        return Err(CryptoError::InvalidCoordinates);
    }

    if !meta
        .imu
        .accel_xyz
        .iter()
        .chain(meta.imu.gyro_xyz.iter())
        .all(|val| val.is_finite())
    {
        return Err(CryptoError::InvalidImuTelemetry);
    }

    if !meta.moire_score.is_finite() || !(0.0..=1.0).contains(&meta.moire_score) {
        return Err(CryptoError::InvalidMoireScore);
    }

    Ok(())
}

fn parse_p256_public_key(bytes: &[u8]) -> Result<VerifyingKey, CryptoError> {
    VerifyingKey::from_public_key_der(bytes)
        .or_else(|_| VerifyingKey::from_sec1_bytes(bytes))
        .map_err(|_| CryptoError::InvalidPublicKey)
}

fn parse_p256_signature(bytes: &[u8]) -> Result<Signature, CryptoError> {
    Signature::from_der(bytes)
        .or_else(|_| Signature::from_slice(bytes))
        .map_err(|_| CryptoError::InvalidSignatureEncoding)
}

fn canonical_f64_bits(value: f64) -> [u8; 8] {
    let normalized = if value == 0.0 { 0.0 } else { value };
    normalized.to_bits().to_be_bytes()
}

fn canonical_f32_bits(value: f32) -> [u8; 4] {
    let normalized = if value == 0.0 { 0.0 } else { value };
    normalized.to_bits().to_be_bytes()
}

#[cfg(test)]
mod tests {
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer;
    use p256::pkcs8::EncodePublicKey;
    use sha2::{Digest, Sha256};

    use super::*;

    fn test_signing_key() -> SigningKey {
        let key_bytes = [0x42_u8; 32];
        SigningKey::from_bytes((&key_bytes).into()).unwrap()
    }

    fn base_unsigned_metadata(source: CaptureSource) -> CaptureMetadata {
        let image_sha256: [u8; 32] = Sha256::digest(b"mock-raw-camera-frame").into();
        CaptureMetadata {
            capture_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            captured_at_utc_ms: 1_795_017_600_000,
            source,
            coordinates: GeoPoint {
                latitude: 19.4326,
                longitude: -99.1332,
                accuracy_meters: 4.5,
            },
            imu: ImuReading {
                accel_xyz: [0.12, -0.05, 9.79],
                gyro_xyz: [0.01, -0.02, 0.0],
            },
            moire_score: 0.08,
            image_sha256,
            device_public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn sign_spki_der(meta: &mut CaptureMetadata, signing_key: &SigningKey) {
        let public_key = p256::PublicKey::from(*signing_key.verifying_key());
        meta.device_public_key = public_key.to_public_key_der().unwrap().into_vec();
        let payload = meta.canonical_signing_bytes().unwrap();
        let sig: Signature = signing_key.sign(&payload);
        meta.signature = sig.to_der().as_bytes().to_vec();
    }

    #[test]
    fn verifies_valid_camerax_capture_with_spki_der_and_asn1_signature() {
        let key = test_signing_key();
        let mut meta = base_unsigned_metadata(CaptureSource::CameraXLive);
        sign_spki_der(&mut meta, &key);

        assert_eq!(verify_competitive_capture(&meta, 0.35), Ok(()));
    }

    #[test]
    fn verifies_valid_camerax_capture_with_sec1_key_and_raw_signature() {
        let key = test_signing_key();
        let mut meta = base_unsigned_metadata(CaptureSource::CameraXLive);
        meta.device_public_key = key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        let payload = meta.canonical_signing_bytes().unwrap();
        let sig: Signature = key.sign(&payload);
        meta.signature = sig.to_bytes().to_vec();

        assert_eq!(verify_competitive_capture(&meta, 0.35), Ok(()));
    }

    #[test]
    fn rejects_tampered_coordinates_or_image_hash() {
        let key = test_signing_key();
        let mut meta = base_unsigned_metadata(CaptureSource::CameraXLive);
        sign_spki_der(&mut meta, &key);

        meta.coordinates.latitude = 20.0;
        assert_eq!(
            verify_competitive_capture(&meta, 0.35),
            Err(CryptoError::SignatureVerificationFailed)
        );
    }

    #[test]
    fn rejects_gallery_source_even_with_valid_signature() {
        let key = test_signing_key();
        let mut meta = base_unsigned_metadata(CaptureSource::Gallery);
        sign_spki_der(&mut meta, &key);

        assert_eq!(
            verify_competitive_capture(&meta, 0.35),
            Err(CryptoError::GallerySourceNotCompetitive)
        );
    }

    #[test]
    fn rejects_excessive_moire_score_and_invalid_coordinates() {
        let key = test_signing_key();
        let mut high_moire = base_unsigned_metadata(CaptureSource::CameraXLive);
        high_moire.moire_score = 0.72;
        sign_spki_der(&mut high_moire, &key);

        assert_eq!(
            verify_competitive_capture(&high_moire, 0.35),
            Err(CryptoError::MoireThresholdExceeded {
                score: 0.72,
                max_allowed: 0.35,
            })
        );

        let mut bad_coords = base_unsigned_metadata(CaptureSource::CameraXLive);
        bad_coords.coordinates.latitude = 95.0;
        assert_eq!(
            verify_competitive_capture(&bad_coords, 0.35),
            Err(CryptoError::InvalidCoordinates)
        );
    }

    #[test]
    fn rejects_malformed_keys_signatures_and_non_finite_telemetry() {
        let key = test_signing_key();
        let mut bad_key = base_unsigned_metadata(CaptureSource::CameraXLive);
        sign_spki_der(&mut bad_key, &key);
        bad_key.device_public_key = vec![1, 2, 3];
        assert_eq!(
            verify_competitive_capture(&bad_key, 0.35),
            Err(CryptoError::InvalidPublicKey)
        );

        let mut bad_sig = base_unsigned_metadata(CaptureSource::CameraXLive);
        sign_spki_der(&mut bad_sig, &key);
        bad_sig.signature = vec![9, 9, 9];
        assert_eq!(
            verify_competitive_capture(&bad_sig, 0.35),
            Err(CryptoError::InvalidSignatureEncoding)
        );

        let mut nan_imu = base_unsigned_metadata(CaptureSource::CameraXLive);
        nan_imu.imu.accel_xyz[0] = f32::NAN;
        assert_eq!(
            verify_competitive_capture(&nan_imu, 0.35),
            Err(CryptoError::InvalidImuTelemetry)
        );

        let mut zero_ts = base_unsigned_metadata(CaptureSource::CameraXLive);
        zero_ts.captured_at_utc_ms = 0;
        assert_eq!(
            verify_competitive_capture(&zero_ts, 0.35),
            Err(CryptoError::InvalidTimestamp)
        );
    }

    #[test]
    fn canonical_signing_bytes_normalizes_negative_zero() {
        let mut a = base_unsigned_metadata(CaptureSource::CameraXLive);
        a.imu.gyro_xyz[2] = -0.0;
        let mut b = base_unsigned_metadata(CaptureSource::CameraXLive);
        b.imu.gyro_xyz[2] = 0.0;

        assert_eq!(
            a.canonical_signing_bytes().unwrap(),
            b.canonical_signing_bytes().unwrap()
        );
    }
}
