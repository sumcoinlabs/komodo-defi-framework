use crate::client::TrezorSession;
use crate::proto::messages_bitcoin as proto_bitcoin;
use crate::result_handler::ResultHandler;
use crate::utxo::TrezorUtxoCoin;
use crate::{ecdsa_curve_to_string, serialize_derivation_path, TrezorResponse, TrezorResult};
use hw_common::primitives::{DerivationPath, EcdsaCurve, XPub};

// Bitcoin(UTXO) operations.
impl<'a> TrezorSession<'a> {
    pub async fn get_utxo_address<'b>(
        &'b mut self,
        path: DerivationPath,
        coin: TrezorUtxoCoin,
        show_display: bool,
    ) -> TrezorResult<TrezorResponse<'a, 'b, String>> {
        let req = proto_bitcoin::GetAddress {
            address_n: serialize_derivation_path(&path),
            coin_name: Some(coin.to_string()),
            show_display: Some(show_display),
            multisig: None,
            script_type: None,
            ignore_xpub_magic: None,
        };
        let result_handler = ResultHandler::new(|m: proto_bitcoin::Address| Ok(m.address));
        self.call(req, result_handler).await
    }

    pub async fn get_public_key<'b>(
        &'b mut self,
        path: DerivationPath,
        coin: TrezorUtxoCoin,
        ecdsa_curve: EcdsaCurve,
    ) -> TrezorResult<TrezorResponse<'a, 'b, XPub>> {
        let req = proto_bitcoin::GetPublicKey {
            address_n: serialize_derivation_path(&path),
            ecdsa_curve_name: Some(ecdsa_curve_to_string(ecdsa_curve)),
            show_display: None,
            coin_name: Some(coin.to_string()),
            script_type: None,
            ignore_xpub_magic: None,
        };

        let result_handler = ResultHandler::new(|m: proto_bitcoin::PublicKey| Ok(m.xpub));
        self.call(req, result_handler).await
    }
}