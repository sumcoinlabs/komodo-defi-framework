mod indexeddb;

use blake2b_simd::State;
pub use indexeddb::ZcashParamsWasmImpl;
use mm2_err_handle::prelude::*;
use mm2_net::wasm::http::FetchRequest;

const DOWNLOAD_URL: &str = "https://komodoplatform.com/downloads";
const SAPLING_SPEND_NAME: &str = "sapling-spend.params";
const SAPLING_OUTPUT_NAME: &str = "sapling-output.params";
const SAPLING_SPEND_HASH: &str = "8270785a1a0d0bc77196f000ee6d221c9c9894f55307bd9357c3f0105d31ca63991ab91324160d8f53e2bbd3c2633a6eb8bdf5205d822e7f3f73edac51b2b70c";
const SAPLING_OUTPUT_HASH: &str = "657e3d38dbb5cb5e7dd2970e8b03d69b4787dd907285b5a7f0790dcc8072f60bf593b32cc2d1c030e00ff5ae64bf84c5c3beb84ddc841d48264b4a171744d028";

#[derive(Debug, derive_more::Display)]
pub enum ZcashParamsBytesError {
    IO(String),
}

async fn fetch_params(name: &str, expected_hash: &str) -> MmResult<Vec<u8>, ZcashParamsBytesError> {
    let (status, file) = FetchRequest::get(&format!("{DOWNLOAD_URL}/{name}"))
        .cors()
        .request_array()
        .await
        .mm_err(|err| ZcashParamsBytesError::IO(err.to_string()))?;

    assert_eq!(200, status);

    let hash = State::new().update(&file).finalize().to_hex();
    // Verify parameter file hash.
    if &hash != expected_hash {
        return Err(ZcashParamsBytesError::IO(format!(
            "{} failed validation (expected: {}, actual: {}, fetched {} bytes)",
            name,
            expected_hash,
            hash,
            file.len()
        ))
        .into());
    }

    Ok(file)
}

pub async fn download_parameters() -> MmResult<(Vec<u8>, Vec<u8>), ZcashParamsBytesError> {
    Ok((
        fetch_params(SAPLING_SPEND_NAME, SAPLING_SPEND_HASH).await?,
        fetch_params(SAPLING_OUTPUT_NAME, SAPLING_OUTPUT_HASH).await?,
    ))
}

use common::log::wasm_log::register_wasm_log;
use mm2_err_handle::prelude::MmResult;
use mm2_test_helpers::for_tests::mm_ctx_with_custom_db;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
async fn test_download_save_and_get_params() {
    register_wasm_log();
    let ctx = mm_ctx_with_custom_db();
    let db = ZcashParamsWasmImpl::new(ctx).await.unwrap();
    // download params
    let (sapling_spend, sapling_output) = download_parameters().await.unwrap();
    // save params
    db.save_params(&sapling_spend, &sapling_output).await.unwrap();
    // get params
    let (sapling_spend_db, sapling_output_db) = db.get_params().await.unwrap();
    assert_eq!(sapling_spend, sapling_spend_db);
    assert_eq!(sapling_output, sapling_output_db);
}

#[wasm_bindgen_test]
async fn test_check_for_no_params() {
    register_wasm_log();
    let ctx = mm_ctx_with_custom_db();
    let db = ZcashParamsWasmImpl::new(ctx).await.unwrap();
    // check for no params
    let check_params = db.check_params().await.unwrap();
    assert_eq!(false, check_params)
}