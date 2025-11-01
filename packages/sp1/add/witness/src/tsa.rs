use anyhow::Result;
use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::Decode;
use der::{
    Encode,
    asn1::{Int, OctetString},
    oid::db::rfc5912::ID_SHA_256,
};
use price_lib::TsaResponse;
use rand::{RngCore, rng};
use reqwest::Client;
use sha2::{Digest, Sha256};
use spki::AlgorithmIdentifierOwned;
use tracing::{debug, info};
use x509_tsp::{MessageImprint, TimeStampReq, TimeStampResp, TspVersion, TstInfo};

/// Get a timestamp from a TSA endpoint for the given data
pub async fn get_timestamp(data: &[u8], endpoint: &str) -> Result<TsaResponse> {
    info!("Getting timestamp from TSA endpoint: {}", endpoint);

    // 1. Hash the data
    let digest = Sha256::digest(data);
    debug!("SHA256 hash: {}", hex::encode(digest));

    // 2. Build TimeStampReq
    let imprint = MessageImprint {
        hash_algorithm: AlgorithmIdentifierOwned {
            oid: ID_SHA_256,
            parameters: None,
        },
        hashed_message: OctetString::new(digest.to_vec())?,
    };

    let mut nonce_bytes = [0u8; 8];
    rng().fill_bytes(&mut nonce_bytes);

    let ts_req = TimeStampReq {
        version: TspVersion::V1,
        message_imprint: imprint,
        req_policy: None,
        nonce: Some(Int::new(&nonce_bytes)?),
        cert_req: true,
        extensions: None,
    };
    let der_req = ts_req.to_der()?;

    // 3. POST to TSA endpoint
    let client = Client::builder()
        .user_agent("Rust RFC3161 TSA Client/0.1")
        .build()?;

    debug!("Sending timestamp request to {}", endpoint);

    let resp_bytes = client
        .post(endpoint)
        .header("Content-Type", "application/timestamp-query")
        .body(der_req)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    // 4. Parse & basic validation
    let ts_resp = TimeStampResp::from_der(resp_bytes.as_ref())?;
    if ts_resp.time_stamp_token.is_none() {
        anyhow::bail!(
            "TSA did not include a timestamp token. Status: {:?}",
            ts_resp.status.status
        );
    }
    let token = ts_resp.time_stamp_token.unwrap();

    // 5. Extract TSTInfo from the token
    let token_der = token.to_der()?;
    let ci = ContentInfo::from_der(&token_der)?;

    // ci.content is a der::Any, we need to decode it as SignedData
    let signed = SignedData::from_der(ci.content.to_der()?.as_slice())?;

    let econtent = signed
        .encap_content_info
        .econtent
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing eContent"))?;

    // econtent is a der::Any containing an OCTET STRING with TstInfo bytes
    let octet_string = OctetString::from_der(econtent.to_der()?.as_slice())?;
    let tst_info = TstInfo::from_der(octet_string.as_bytes())?;

    // 6. Extract timing information
    let gen_time_der = tst_info.gen_time.to_der()?;
    // GeneralizedTime DER: tag(1) + length(1+) + content
    let time_content = if gen_time_der.len() > 2 {
        match gen_time_der[1] {
            len if len < 0x80 => &gen_time_der[2..],
            0x81 => &gen_time_der[3..],
            0x82 => &gen_time_der[4..],
            _ => &gen_time_der[2..],
        }
    } else {
        &gen_time_der[2..]
    };

    let time_str = std::str::from_utf8(time_content)
        .unwrap_or("Unable to parse time string")
        .to_string();

    debug!("TSA genTime: {}", time_str);

    // Serialize serial number to bytes for storage
    let serial_number_bytes = tst_info.serial_number.to_der()?;

    // 7. Verify TSA certificates and signature
    info!("Verifying TSA certificates and signature");
    let cert_verification = price_lib::verify_certificates(&signed)?;

    if cert_verification.is_valid {
        info!("✅ TSA certificate chain and signature verified successfully");
        if let Some(subject) = &cert_verification.signer_cert_subject {
            info!("   Signer: {}", subject);
        }
        info!("   Certificates in chain: {}", cert_verification.cert_count);
    } else {
        info!("❌ TSA certificate verification failed");
        if let Some(error) = &cert_verification.error_message {
            info!("   Error: {}", error);
        }
    }

    Ok(TsaResponse {
        time_string: time_str,
        serial_number_bytes,
        cert_verified: cert_verification.is_valid,
        cert_count: cert_verification.cert_count,
        signer_cert_subject: cert_verification.signer_cert_subject,
        verification_error: cert_verification.error_message,
    })
}
