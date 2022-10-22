use crate::privkey::{bip39_priv_key_from_seed, key_pair_from_secret, PrivKeyError};
use crate::{mm2_internal_der_path, Bip32DerPathOps, Bip32Error, Bip44PathToCoin, CryptoInitError, CryptoInitResult};
use bip32::{ChildNumber, DerivationPath, ExtendedPrivateKey};
use keys::{hash::H256, KeyPair, Private, Public as PublicKey};
use mm2_err_handle::prelude::*;
use std::str::FromStr;
use std::sync::Arc;

const HARDENED: bool = true;
const NON_HARDENED: bool = false;

#[derive(Clone)]
pub struct HDAccountArc(Arc<HDAccountCtx>);

pub struct HDAccountCtx {
    bip39_priv_key: ExtendedPrivateKey<secp256k1::SecretKey>,
    /// secp256k1 key pair derived at `mm2_internal_der_path`.
    /// It's used for mm2 internal purposes such as signing P2P messages.
    mm2_internal_key_pair: KeyPair,
    /// `hd_account` actually means an `address` index at the `m/purpose'/coin'/account'/chain/address` path.
    /// This account is set globally for every activated coin.
    hd_account: ChildNumber,
}

impl HDAccountCtx {
    pub fn new(passphrase: &str, hd_account: u32) -> CryptoInitResult<HDAccountCtx> {
        let bip39_priv_key = bip39_priv_key_from_seed(passphrase)?;

        let hd_account = ChildNumber::new(hd_account, NON_HARDENED)
            .map_to_mm(|error| CryptoInitError::InvalidHdAccount { hd_account, error })?;
        let derivation_path = mm2_internal_der_path(Some(hd_account));

        let mut internal_priv_key = bip39_priv_key.clone();
        for child in derivation_path {
            internal_priv_key = internal_priv_key
                .derive_child(child)
                .map_to_mm(|e| CryptoInitError::InvalidPassphrase(PrivKeyError::InvalidPrivKey(e.to_string())))?;
        }

        let mm2_internal_key_pair = key_pair_from_secret(internal_priv_key.private_key().as_ref())?;

        Ok(HDAccountCtx {
            bip39_priv_key,
            mm2_internal_key_pair,
            hd_account,
        })
    }

    pub fn mm2_internal_pubkey(&self) -> PublicKey { *self.mm2_internal_key_pair.public() }

    pub fn mm2_internal_privkey(&self) -> &Private { self.mm2_internal_key_pair.private() }

    /// Derives a `secp256k1::SecretKey` from [`HDAccountCtx::bip39_priv_key`]
    /// at the given `m/purpose'/coin_type'/account_id'/chain/address_id` derivation path,
    /// where:
    /// * `m/purpose'/coin_type'` is specified by `derivation_path`.
    /// * `account_id = 0`, `chain = 0`.
    /// * `address_id = HDAccountCtx::hd_account`.
    ///
    /// Returns the `secp256k1::Private` Secret 256-bit key
    pub fn derive_secp256k1_secret(&self, derivation_path: Bip44PathToCoin) -> MmResult<H256, Bip32Error> {
        const ACCOUNT_ID: u32 = 0;
        const CHAIN_ID: u32 = 0;

        let mut account_der_path = derivation_path.to_derivation_path();
        account_der_path.push(ChildNumber::new(ACCOUNT_ID, HARDENED).unwrap());
        account_der_path.push(ChildNumber::new(CHAIN_ID, NON_HARDENED).unwrap());
        account_der_path.push(self.hd_account);

        let mut priv_key = self.bip39_priv_key.clone();
        for child in account_der_path {
            priv_key = priv_key.derive_child(child)?;
        }

        let secret = *priv_key.private_key().as_ref();
        Ok(H256::from(secret))
    }
}
