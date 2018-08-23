/******************************************************************************
 * Copyright © 2014-2018 The SuperNET Developers.                             *
 *                                                                            *
 * See the AUTHORS, DEVELOPER-AGREEMENT and LICENSE files at                  *
 * the top-level directory of this distribution for the individual copyright  *
 * holder information and the developer policies on copyright and licensing.  *
 *                                                                            *
 * Unless otherwise agreed in a custom licensing agreement, no part of the    *
 * SuperNET software, including this file may be copied, modified, propagated *
 * or distributed except according to the terms contained in the LICENSE file *
 *                                                                            *
 * Removal or modification of this copyright notice is prohibited.            *
 *                                                                            *
 ******************************************************************************/
//
//  etomiclib.rs
//  marketmaker
//
//  Copyright © 2017-2018 SuperNET. All rights reserved.
//
extern crate web3;
extern crate ethabi;
extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate ethkey;
extern crate rlp;
extern crate hex;
extern crate regex;
extern crate libc;
#[macro_use]
extern crate unwrap;

use ethcore_transaction::{ Action, Transaction };
use ethereum_types::{ U256, H160, H256 };
use ethkey::{ KeyPair, Secret, Public, public_to_address };
use ethabi::{ Contract, Token, Error as EthAbiError };
use web3::futures::Future;
use web3::transports::{ Http, EventLoopHandle };
use web3::{ Web3 };
use web3::types::{ Transaction as Web3Transaction, TransactionId, BlockId, BlockNumber, CallRequest, Bytes };
use web3::confirm::TransactionReceiptBlockNumberCheck;
use std::time::Duration;
use std::sync::{ Arc, RwLock, Mutex };
use std::thread;
use std::collections::HashMap;
use std::os::raw::{ c_char, c_void, c_int };
use std::ffi::CString;
use std::ffi::CStr;
use std::str::FromStr;
use regex::Regex;

static ALICE_ABI: &'static str = r#"[{"constant":false,"inputs":[{"name":"_dealId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_bob","type":"address"},{"name":"_aliceHash","type":"bytes20"},{"name":"_bobHash","type":"bytes20"},{"name":"_tokenAddress","type":"address"}],"name":"initErc20Deal","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"_dealId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_tokenAddress","type":"address"},{"name":"_alice","type":"address"},{"name":"_bobHash","type":"bytes20"},{"name":"_aliceSecret","type":"bytes"}],"name":"bobClaimsPayment","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"_dealId","type":"bytes32"},{"name":"_bob","type":"address"},{"name":"_aliceHash","type":"bytes20"},{"name":"_bobHash","type":"bytes20"}],"name":"initEthDeal","outputs":[],"payable":true,"stateMutability":"payable","type":"function"},{"constant":true,"inputs":[{"name":"","type":"bytes32"}],"name":"deals","outputs":[{"name":"dealHash","type":"bytes20"},{"name":"state","type":"uint8"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_dealId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_tokenAddress","type":"address"},{"name":"_bob","type":"address"},{"name":"_aliceHash","type":"bytes20"},{"name":"_bobSecret","type":"bytes"}],"name":"aliceClaimsPayment","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"inputs":[],"payable":false,"stateMutability":"nonpayable","type":"constructor"}]"#;
static BOB_ABI: &'static str = r#"[{"constant":true,"inputs":[{"name":"","type":"bytes32"}],"name":"payments","outputs":[{"name":"paymentHash","type":"bytes20"},{"name":"lockTime","type":"uint64"},{"name":"state","type":"uint8"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_secret","type":"bytes32"},{"name":"_bob","type":"address"},{"name":"_tokenAddress","type":"address"}],"name":"aliceClaimsPayment","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_secret","type":"bytes32"},{"name":"_alice","type":"address"},{"name":"_tokenAddress","type":"address"}],"name":"bobClaimsDeposit","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"","type":"bytes32"}],"name":"deposits","outputs":[{"name":"depositHash","type":"bytes20"},{"name":"lockTime","type":"uint64"},{"name":"state","type":"uint8"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_bob","type":"address"},{"name":"_tokenAddress","type":"address"},{"name":"_secretHash","type":"bytes20"}],"name":"aliceClaimsDeposit","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_alice","type":"address"},{"name":"_secretHash","type":"bytes20"},{"name":"_lockTime","type":"uint64"}],"name":"bobMakesEthPayment","outputs":[],"payable":true,"stateMutability":"payable","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_alice","type":"address"},{"name":"_secretHash","type":"bytes20"},{"name":"_tokenAddress","type":"address"},{"name":"_lockTime","type":"uint64"}],"name":"bobMakesErc20Deposit","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_alice","type":"address"},{"name":"_secretHash","type":"bytes20"},{"name":"_tokenAddress","type":"address"},{"name":"_lockTime","type":"uint64"}],"name":"bobMakesErc20Payment","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_alice","type":"address"},{"name":"_secretHash","type":"bytes20"},{"name":"_lockTime","type":"uint64"}],"name":"bobMakesEthDeposit","outputs":[],"payable":true,"stateMutability":"payable","type":"function"},{"constant":false,"inputs":[{"name":"_txId","type":"bytes32"},{"name":"_amount","type":"uint256"},{"name":"_alice","type":"address"},{"name":"_tokenAddress","type":"address"},{"name":"_secretHash","type":"bytes20"}],"name":"bobClaimsPayment","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"inputs":[],"payable":false,"stateMutability":"nonpayable","type":"constructor"}]"#;
static ERC20_ABI: &'static str = r#"[{"constant":true,"inputs":[],"name":"name","outputs":[{"name":"","type":"string"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_spender","type":"address"},{"name":"_value","type":"uint256"}],"name":"approve","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[],"name":"totalSupply","outputs":[{"name":"","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_from","type":"address"},{"name":"_to","type":"address"},{"name":"_value","type":"uint256"}],"name":"transferFrom","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[],"name":"decimals","outputs":[{"name":"","type":"uint8"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_spender","type":"address"},{"name":"_subtractedValue","type":"uint256"}],"name":"decreaseApproval","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"_owner","type":"address"}],"name":"balanceOf","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"symbol","outputs":[{"name":"","type":"string"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_to","type":"address"},{"name":"_value","type":"uint256"}],"name":"transfer","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"_spender","type":"address"},{"name":"_addedValue","type":"uint256"}],"name":"increaseApproval","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"_owner","type":"address"},{"name":"_spender","type":"address"}],"name":"allowance","outputs":[{"name":"","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"inputs":[],"payable":false,"stateMutability":"nonpayable","type":"constructor"},{"anonymous":false,"inputs":[{"indexed":true,"name":"owner","type":"address"},{"indexed":true,"name":"spender","type":"address"},{"indexed":false,"name":"value","type":"uint256"}],"name":"Approval","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"from","type":"address"},{"indexed":true,"name":"to","type":"address"},{"indexed":false,"name":"value","type":"uint256"}],"name":"Transfer","type":"event"}]"#;
static ALICE_CONTRACT: &'static str = "e1d4236c5774d35dc47dcc2e5e0ccfc463a3289c";
static BOB_CONTRACT: &'static str = "2a8e4f9ae69c86e277602c6802085febc4bd5986";

include!("c_headers/etomiclib.rs");

#[repr(C)]
pub struct EthClient {
    pub web3: Web3<Http>,
    key_pair: KeyPair,
    _event_loop: EventLoopHandle,
    transactions: RwLock<HashMap<H256, Web3Transaction>>,
    block_number: RwLock<u64>,
    current_nonce: Mutex<U256>,
    pub alice_abi: Contract,
    pub bob_abi: Contract,
    pub erc20_abi: Contract
}

impl EthClient {
    pub fn new(secret: Vec<u8>) -> Self {
        let (event_loop, transport) = web3::transports::Http::new("http://195.201.0.6:8545").unwrap();
        let web3 = web3::Web3::new(transport);
        let key_pair = KeyPair::from_secret_slice(&secret).unwrap();
        let current_nonce = web3.eth().parity_next_nonce(key_pair.address()).wait().unwrap();
        let alice_abi = unwrap!(Contract::load(ALICE_ABI.as_bytes()), "Could not load ALICE_ABI, is it valid?");
        let bob_abi = unwrap!(Contract::load(BOB_ABI.as_bytes()), "Could not load BOB_ABI, is it valid?");
        let erc20_abi = unwrap!(Contract::load(ERC20_ABI.as_bytes()), "Could not load ERC20_ABI, is it valid?");

        EthClient {
            web3,
            _event_loop: event_loop,
            key_pair,
            transactions: RwLock::new(HashMap::new()),
            block_number: RwLock::new(3707894),
            current_nonce: Mutex::new(current_nonce),
            alice_abi,
            bob_abi,
            erc20_abi
        }
    }

    pub fn sign_and_send_transaction(
        &self,
        value: U256,
        action: Action,
        data: Vec<u8>,
        gas: U256,
        gas_price: U256
    ) -> Result<H256, web3::Error> {
        let mut nonce_lock = self.current_nonce.lock().unwrap();
        let nonce = self.web3.eth().parity_next_nonce(self.key_pair.address()).wait()?;
        let tx = Transaction {
            nonce,
            value,
            action,
            data,
            gas,
            gas_price
        };

        let signed = tx.sign(self.key_pair.secret(), None);
        *nonce_lock = nonce;
        self.web3.eth().send_raw_transaction(web3::types::Bytes(rlp::encode(&signed).to_vec())).wait()
    }

    pub fn approve_erc20(
        &self,
        token_address: H160,
        spender_address: H160,
        mut amount: U256,
        decimals: u8
    ) -> Result<H256, web3::Error> {
        let approve = unwrap!(
            self.erc20_abi.function("approve"),
            "Could not load approve function from ERC20 contract. Is ERC20_ABI valid?"
        );

        if decimals < 18 {
            amount = amount / U256::exp10((18 - decimals) as usize)
        }

        let encoded = approve.encode_input(&[
            Token::Address(spender_address),
            Token::Uint(amount)
        ]).unwrap();

        self.sign_and_send_transaction(
            0.into(),
            Action::Call(token_address),
            encoded,
            U256::from(210000),
            U256::from_dec_str("10000000000").unwrap()
        )
    }

    pub fn send_alice_payment_eth(
        &self,
        id: Vec<u8>,
        bob_address: Vec<u8>,
        alice_hash: Vec<u8>,
        bob_hash: Vec<u8>,
        value: U256
    ) -> Result<H256, web3::Error> {
        let init_eth_deal = unwrap!(
            self.alice_abi.function("initEthDeal"),
            "Could not load initEthDeal function from Alice contract. Is ALICE_ABI valid?"
        );

        let encoded = init_eth_deal.encode_input(&[
            Token::FixedBytes(id),
            Token::Address(H160::from(bob_address.as_slice())),
            Token::FixedBytes(alice_hash),
            Token::FixedBytes(bob_hash)
        ]).unwrap();

        self.sign_and_send_transaction(
            value,
            Action::Call(H160::from(ALICE_CONTRACT)),
            encoded,
            U256::from(210000),
            U256::from_dec_str("10000000000").unwrap()
        )
    }

    pub fn send_alice_payment_erc20(
        &self,
        id: Vec<u8>,
        bob_address: H160,
        alice_hash: Vec<u8>,
        bob_hash: Vec<u8>,
        mut value: U256,
        token_address: H160,
        decimals: u8
    ) -> Result<H256, web3::Error> {
        let function = unwrap!(
            self.alice_abi.function("initErc20Deal"),
            "Could not get initErc20Deal Alice contract function, is Alice ABI valid?"
        );

        if decimals < 18 {
            value = value / U256::exp10((18 - decimals) as usize);
        }

        let encoded = function.encode_input(&[
            Token::FixedBytes(id),
            Token::Uint(value),
            Token::Address(bob_address),
            Token::FixedBytes(alice_hash),
            Token::FixedBytes(bob_hash),
            Token::Address(token_address)
        ]).unwrap();

        self.sign_and_send_transaction(
            0.into(),
            Action::Call(H160::from(ALICE_CONTRACT)),
            encoded,
            U256::from(210000),
            U256::from_dec_str("10000000000").unwrap()
        )
    }

    pub fn alice_reclaims_payment(
        &self,
        id: Vec<u8>,
        bob_address: H160,
        token_address: H160,
        alice_hash: Vec<u8>,
        bob_priv: Vec<u8>,
        mut value: U256,
        decimals: u8
    ) -> Result<H256, web3::Error> {
        let alice_claims_payment = unwrap!(
            self.alice_abi.function("aliceClaimsPayment"),
            "Could not load aliceClaimsPayment function, is ALICE_ABI valid?"
        );

        if token_address != H160::zero() && decimals < 18 {
            value = value / U256::exp10((18 - decimals) as usize);
        }

        let encoded = alice_claims_payment.encode_input(&[
            Token::FixedBytes(id),
            Token::Uint(value),
            Token::Address(token_address),
            Token::Address(bob_address),
            Token::FixedBytes(alice_hash),
            Token::Bytes(bob_priv)
        ]).unwrap();

        self.sign_and_send_transaction(
            0.into(),
            Action::Call(H160::from(ALICE_CONTRACT)),
            encoded,
            U256::from(210000),
            U256::from_dec_str("10000000000").unwrap()
        )
    }

    pub fn bob_spends_alice_payment(
        &self,
        id: Vec<u8>,
        alice_address: H160,
        token_address: H160,
        alice_priv: Vec<u8>,
        bob_hash: Vec<u8>,
        mut value: U256,
        decimals: u8
    ) -> Result<H256, web3::Error> {
        let abi = Contract::load(ALICE_ABI.as_bytes()).unwrap();
        let function = abi.function("bobClaimsPayment").unwrap();

        if token_address != H160::zero() && decimals < 18 {
            value = value / U256::exp10((18 - decimals) as usize);
        }

        let encoded = function.encode_input(&[
            Token::FixedBytes(id),
            Token::Uint(value),
            Token::Address(token_address),
            Token::Address(alice_address),
            Token::FixedBytes(bob_hash),
            Token::Bytes(alice_priv)
        ]).unwrap();

        self.sign_and_send_transaction(
            0.into(),
            Action::Call(H160::from(ALICE_CONTRACT)),
            encoded,
            U256::from(210000),
            U256::from_dec_str("10000000000").unwrap()
        )
    }

    pub fn bob_sends_eth_deposit(
        &self,
        id: Vec<u8>,
        alice_address: Vec<u8>,
        bob_hash: Vec<u8>,
        timestamp: u64
    ) -> H256 {
        let abi = Contract::load(BOB_ABI.as_bytes()).unwrap();
        let bob_sends_eth_deposit = abi.function("bobMakesEthDeposit").unwrap();

        let encoded = bob_sends_eth_deposit.encode_input(&[
            Token::FixedBytes(id),
            Token::Address(H160::from(alice_address.as_slice())),
            Token::FixedBytes(bob_hash),
            Token::Uint(U256::from(timestamp))
        ]).unwrap();

        let tx = Transaction {
            nonce: self.web3.eth().parity_next_nonce(self.key_pair.address()).wait().unwrap(),
            value: U256::from_dec_str("10000000000000000").unwrap(),
            action: Action::Call(H160::from(BOB_CONTRACT)),
            data: encoded,
            gas: U256::from(210000),
            gas_price: U256::from_dec_str("10000000000").unwrap()
        };

        let t = tx.sign(self.key_pair.secret(), None);

        self.web3.eth().send_raw_transaction(web3::types::Bytes(rlp::encode(&t).to_vec())).wait().unwrap()
    }

    pub fn bob_refunds_deposit(
        &self,
        id: Vec<u8>,
        alice_address: Vec<u8>,
        bob_secret: Vec<u8>
    ) -> H256 {
        let abi = Contract::load(BOB_ABI.as_bytes()).unwrap();
        let bob_refunds_deposit = abi.function("bobClaimsDeposit").unwrap();

        let encoded = bob_refunds_deposit.encode_input(&[
            Token::FixedBytes(id),
            Token::Uint(U256::from_dec_str("10000000000000000").unwrap()),
            Token::FixedBytes(bob_secret),
            Token::Address(H160::from(alice_address.as_slice())),
            Token::Address(H160::new()),
        ]).unwrap();

        let tx = Transaction {
            nonce: self.web3.eth().parity_next_nonce(self.key_pair.address()).wait().unwrap(),
            value: U256::from(0),
            action: Action::Call(H160::from(BOB_CONTRACT)),
            data: encoded,
            gas: U256::from(210000),
            gas_price: U256::from_dec_str("10000000000").unwrap()
        };

        let t = tx.sign(self.key_pair.secret(), None);

        self.web3.eth().send_raw_transaction(web3::types::Bytes(rlp::encode(&t).to_vec())).wait().unwrap()
    }

    pub fn bob_sends_eth_payment(
        &self,
        id: Vec<u8>,
        alice_address: Vec<u8>,
        alice_hash: Vec<u8>,
        timestamp: u64
    ) -> H256 {
        let abi = Contract::load(BOB_ABI.as_bytes()).unwrap();
        let bob_sends_eth_payment = abi.function("bobMakesEthPayment").unwrap();

        let encoded = bob_sends_eth_payment.encode_input(&[
            Token::FixedBytes(id),
            Token::Address(H160::from(alice_address.as_slice())),
            Token::FixedBytes(alice_hash),
            Token::Uint(U256::from(timestamp))
        ]).unwrap();

        let tx = Transaction {
            nonce: self.web3.eth().parity_next_nonce(self.key_pair.address()).wait().unwrap(),
            value: U256::from_dec_str("10000000000000000").unwrap(),
            action: Action::Call(H160::from(BOB_CONTRACT)),
            data: encoded,
            gas: U256::from(210000),
            gas_price: U256::from_dec_str("10000000000").unwrap()
        };

        let t = tx.sign(self.key_pair.secret(), None);

        self.web3.eth().send_raw_transaction(web3::types::Bytes(rlp::encode(&t).to_vec())).wait().unwrap()
    }

    pub fn alice_claims_bob_payment(
        &self,
        id: Vec<u8>,
        alice_secret: Vec<u8>,
        bob_address: Vec<u8>
    ) -> H256 {
        let abi = Contract::load(BOB_ABI.as_bytes()).unwrap();
        let alice_claims_payment = abi.function("aliceClaimsPayment").unwrap();

        let encoded = alice_claims_payment.encode_input(&[
            Token::FixedBytes(id),
            Token::Uint(U256::from_dec_str("10000000000000000").unwrap()),
            Token::FixedBytes(alice_secret),
            Token::Address(H160::from(bob_address.as_slice())),
            Token::Address(H160::new())
        ]).unwrap();

        let tx = Transaction {
            nonce: self.web3.eth().parity_next_nonce(self.key_pair.address()).wait().unwrap(),
            value: U256::from(0),
            action: Action::Call(H160::from(BOB_CONTRACT)),
            data: encoded,
            gas: U256::from(210000),
            gas_price: U256::from_dec_str("10000000000").unwrap()
        };

        let t = tx.sign(self.key_pair.secret(), None);

        self.web3.eth().send_raw_transaction(web3::types::Bytes(rlp::encode(&t).to_vec())).wait().unwrap()
    }

    pub fn get_tx(&self, tx_id: H256) -> Web3Transaction {
        self.web3.eth().transaction(TransactionId::Hash(tx_id)).wait().unwrap().unwrap()
    }

    pub fn wait_confirm(&self, tx_id: H256) {
        let check = TransactionReceiptBlockNumberCheck::new(self.web3.eth().clone(), tx_id);
        let duration = Duration::from_secs(1);
        let wait = self.web3.wait_for_confirmations(duration, 1, check).wait();
    }

    pub fn my_address(&self) -> H160 {
        self.key_pair.address()
    }

    pub fn find_bob_tx_spend(&self, tx_id: Vec<u8>, function: &'static str) -> Result<Web3Transaction, &'static str> {
        let abi = Contract::load(BOB_ABI.as_bytes()).unwrap();
        let eth_function = abi.function(function).unwrap();
        let transactions = self.transactions.read().unwrap();
        let option = transactions.iter().find(
            |(ref _x, ref y)| {
                if y.to == Some(H160::from(BOB_CONTRACT)) {
                    if y.input.0.as_slice()[0..4] == eth_function.short_signature() {
                        let decoded = eth_function.decode_input(&y.input.0).unwrap();
                        println!("Decoded: {:?}", decoded);
                        decoded[0] == Token::FixedBytes(tx_id.clone())
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        );
        match option {
            Some((_x, y)) => Ok(y.clone()),
            None => Err("Transaction spend was not found")
        }
    }
}

#[no_mangle]
pub extern "C" fn eth_client(private_key: *const c_char) -> *mut EthClient {
    unsafe {
        let slice = CStr::from_ptr(private_key).to_str().unwrap();
        let eth_client = EthClient::new(hex::decode(&slice[2..]).unwrap());
        Box::into_raw(Box::new(eth_client))
    }
}

#[no_mangle]
pub extern "C" fn eth_client_destruct(eth_client: *mut EthClient) {
    unsafe {
        Box::from_raw(eth_client);
    }
}

/* The original C code will be replaced with the corresponding Rust code in small increments,
   allowing Git history to catch up and show the function-level diffs.
#include "etomiclib.h"
#include "etomiccurl.h"
#include <iostream>
#include <regex>
#include <cpp-ethereum/libethcore/Common.h>
#include <cpp-ethereum/libethcore/CommonJS.h>
#include <cpp-ethereum/libethcore/TransactionBase.h>
#include <inttypes.h>

using namespace dev;
using namespace dev::eth;

char *stringStreamToChar(std::stringstream& ss)
{
    const std::string tmp = ss.str();
    auto result = (char*)malloc(strlen(tmp.c_str()) + 1);
    strcpy(result, tmp.c_str());
    return result;
}

TransactionSkeleton txDataToSkeleton(BasicTxData txData)
{
    TransactionSkeleton tx;
    tx.from = jsToAddress(txData.from);
    tx.to = jsToAddress(txData.to);
    tx.value = jsToU256(txData.amount);
    tx.gas = 200000;
    tx.gasPrice = getGasPriceFromStation(1) * boost::multiprecision::pow(u256(10), 9);
    tx.nonce = getNonce(txData.from);
    return tx;
}

char *signTx(TransactionSkeleton& tx, char* secret)
{
    Secret secretKey(secret);
    TransactionBase baseTx(tx, secretKey);
    RLPStream rlpStream;
    baseTx.streamRLP(rlpStream);
    std::stringstream ss;
    ss << rlpStream.out();
    return stringStreamToChar(ss);
}
*/
#[no_mangle]
pub extern "C" fn approve_erc20(input: ApproveErc20Input, eth_client: *mut EthClient) -> *mut c_char
{
    unsafe {
        let token_addr = hex::decode(CStr::from_ptr(input.token_address[2..].as_ptr()).to_str().unwrap()).unwrap();
        let spender_addr = hex::decode(CStr::from_ptr(input.spender[2..].as_ptr()).to_str().unwrap()).unwrap();

        let tx_id = (*eth_client).approve_erc20(
            H160::from(token_addr.as_slice()),
            H160::from(spender_addr.as_slice()),
            U256::from(input.amount) * U256::exp10(10),
            input.decimals
        );
        match tx_id {
            Ok(tx) => {
                let mut str = String::from("0x");
                str.push_str(&hex::encode(tx.0));
                CString::new(str).unwrap().into_raw()
            },
            Err(e) => {
                println!("Error ERC20 approve: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn alice_sends_eth_payment(
    input: AliceSendsEthPaymentInput,
    eth_client: *mut EthClient
) -> *mut c_char {
    unsafe {
        let tx_id = (*eth_client).send_alice_payment_eth(
            hex::decode(CStr::from_ptr(input.deal_id[2..].as_ptr()).to_str().unwrap()).unwrap(),
            hex::decode(CStr::from_ptr(input.bob_address[2..].as_ptr()).to_str().unwrap()).unwrap(),
            hex::decode(CStr::from_ptr(input.alice_hash[2..].as_ptr()).to_str().unwrap()).unwrap(),
            hex::decode(CStr::from_ptr(input.bob_hash[2..].as_ptr()).to_str().unwrap()).unwrap(),
            U256::from(input.amount) * U256::exp10(10)
        );
        match tx_id {
            Ok(tx) => {
                let mut str = String::from("0x");
                str.push_str(&hex::encode(tx.0));
                CString::new(str).unwrap().into_raw()
            },
            Err(e) => {
                println!("Error sending Alice ETH payment: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn verify_alice_eth_payment_data(
    input: AliceSendsEthPaymentInput,
    data: *const c_char
) -> u8 {
    unsafe {
        let data_slice = CStr::from_ptr(data).to_str().unwrap();
        let decoded = hex::decode(&data_slice[2..]).unwrap();
        let abi = Contract::load(ALICE_ABI.as_bytes()).unwrap();
        let init_eth_deal = abi.function("initEthDeal").unwrap();

        let deal_id_slice = CStr::from_ptr(input.deal_id[2..].as_ptr()).to_str().unwrap();
        let address_slice = CStr::from_ptr(input.bob_address[2..].as_ptr()).to_str().unwrap();
        let alice_hash_slice = CStr::from_ptr(input.alice_hash[2..].as_ptr()).to_str().unwrap();
        let bob_hash_slice = CStr::from_ptr(input.bob_hash[2..].as_ptr()).to_str().unwrap();

        let encoded = init_eth_deal.encode_input(&[
            Token::FixedBytes(hex::decode(deal_id_slice).unwrap()),
            Token::Address(H160::from_str(address_slice).unwrap()),
            Token::FixedBytes(hex::decode(alice_hash_slice).unwrap()),
            Token::FixedBytes(hex::decode(bob_hash_slice).unwrap())
        ]).unwrap();
        (decoded == encoded) as u8
    }
}

#[no_mangle]
pub extern "C" fn alice_sends_erc20_payment(
    input: AliceSendsErc20PaymentInput,
    eth_client: *mut EthClient
) -> *mut c_char {
    unsafe {
        let bob_addr = hex::decode(CStr::from_ptr(input.bob_address[2..].as_ptr()).to_str().unwrap()).unwrap();
        let token_addr = hex::decode(CStr::from_ptr(input.token_address[2..].as_ptr()).to_str().unwrap()).unwrap();

        let tx_id = (*eth_client).send_alice_payment_erc20(
            hex::decode(CStr::from_ptr(input.deal_id[2..].as_ptr()).to_str().unwrap()).unwrap(),
            H160::from(bob_addr.as_slice()),
            hex::decode(CStr::from_ptr(input.alice_hash[2..].as_ptr()).to_str().unwrap()).unwrap(),
            hex::decode(CStr::from_ptr(input.bob_hash[2..].as_ptr()).to_str().unwrap()).unwrap(),
            U256::from(input.amount) * U256::exp10(10),
            H160::from(token_addr.as_slice()),
            input.decimals
        );
        match tx_id {
            Ok(tx) => {
                let mut str = String::from("0x");
                str.push_str(&hex::encode(tx.0));
                CString::new(str).unwrap().into_raw()
            },
            Err(e) => {
                println!("Error sending Alice ERC20 payment: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn verify_alice_erc20_payment_data(
    input: AliceSendsErc20PaymentInput,
    data: *const c_char
) -> u8 {
    unsafe {
        let data_slice = CStr::from_ptr(data).to_str().unwrap();
        let decoded = hex::decode(&data_slice[2..]).unwrap();
        let abi = unwrap!(
            Contract::load(ALICE_ABI.as_bytes()),
            "Could not load ALICE_ABI, is it valid?"
        );
        let init_erc20_deal = unwrap!(
            abi.function("initErc20Deal"),
            "Could not load initErc20Deal ALICE_ABI function, is it valid?"
        );

        let mut value = U256::from(input.amount) * U256::exp10(10);
        if input.decimals < 18 {
            value = value / U256::exp10((18 - input.decimals) as usize);
        }

        let deal_id_slice = CStr::from_ptr(input.deal_id[2..].as_ptr()).to_str().unwrap();
        let bob_address_slice = CStr::from_ptr(input.bob_address[2..].as_ptr()).to_str().unwrap();
        let alice_hash_slice = CStr::from_ptr(input.alice_hash[2..].as_ptr()).to_str().unwrap();
        let bob_hash_slice = CStr::from_ptr(input.bob_hash[2..].as_ptr()).to_str().unwrap();
        let token_address_slice = CStr::from_ptr(input.token_address[2..].as_ptr()).to_str().unwrap();

        let encoded = init_erc20_deal.encode_input(&[
            Token::FixedBytes(hex::decode(deal_id_slice).unwrap()),
            Token::Uint(value),
            Token::Address(H160::from_str(bob_address_slice).unwrap()),
            Token::FixedBytes(hex::decode(alice_hash_slice).unwrap()),
            Token::FixedBytes(hex::decode(bob_hash_slice).unwrap()),
            Token::Address(H160::from_str(token_address_slice).unwrap()),
        ]).unwrap();
        (decoded == encoded) as u8
    }
}

#[no_mangle]
pub extern "C" fn alice_reclaims_payment(
    input: AliceReclaimsPaymentInput,
    eth_client: *mut EthClient
) -> *mut c_char {
    unsafe {
        let bob_addr_slice = CStr::from_ptr(input.bob_address[2..].as_ptr()).to_str().unwrap();
        let token_addr_slice = CStr::from_ptr(input.token_address[2..].as_ptr()).to_str().unwrap();

        let tx_id = (*eth_client).alice_reclaims_payment(
            hex::decode(CStr::from_ptr(input.deal_id[2..].as_ptr()).to_str().unwrap()).unwrap(),
            H160::from_str(bob_addr_slice).unwrap(),
            H160::from_str(token_addr_slice).unwrap(),
            hex::decode(CStr::from_ptr(input.alice_hash[2..].as_ptr()).to_str().unwrap()).unwrap(),
            hex::decode(CStr::from_ptr(input.bob_secret[2..].as_ptr()).to_str().unwrap()).unwrap(),
            U256::from(input.amount) * U256::exp10(10),
            input.decimals
        );
        match tx_id {
            Ok(tx) => {
                let mut str = String::from("0x");
                str.push_str(&hex::encode(tx.0));
                CString::new(str).unwrap().into_raw()
            },
            Err(e) => {
                println!("Error sending Alice reclaim: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn bob_spends_alice_payment(
    input: BobSpendsAlicePaymentInput,
    eth_client: *mut EthClient
) -> *mut c_char {
    unsafe {
        let alice_addr_slice = CStr::from_ptr(input.alice_address[2..].as_ptr()).to_str().unwrap();
        let token_addr_slice = CStr::from_ptr(input.token_address[2..].as_ptr()).to_str().unwrap();

        let tx_id = (*eth_client).bob_spends_alice_payment(
            hex::decode(CStr::from_ptr(input.deal_id[2..].as_ptr()).to_str().unwrap()).unwrap(),
            H160::from_str(alice_addr_slice).unwrap(),
            H160::from_str(token_addr_slice).unwrap(),
            hex::decode(CStr::from_ptr(input.alice_secret[2..].as_ptr()).to_str().unwrap()).unwrap(),
            hex::decode(CStr::from_ptr(input.bob_hash[2..].as_ptr()).to_str().unwrap()).unwrap(),
            U256::from(input.amount) * U256::exp10(10),
            input.decimals
        );
        match tx_id {
            Ok(tx) => {
                let mut str = String::from("0x");
                str.push_str(&hex::encode(tx.0));
                CString::new(str).unwrap().into_raw()
            },
            Err(e) => {
                println!("Error sending Bob spend: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}
/*
std::stringstream bobSendsEthDepositData(BobSendsEthDepositInput input)
{
    u256 lockTime = input.lockTime;
    std::stringstream ss;
    ss << "0xdd23795f"
       << toHex(jsToBytes(input.depositId))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.aliceAddress))
       << toHex(jsToBytes(input.bobHash))
       << "000000000000000000000000"
       << toHex(toBigEndian(lockTime));
    return ss;
}

char* bobSendsEthDeposit(BobSendsEthDepositInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss = bobSendsEthDepositData(input);
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTx(rawTx);
    free(rawTx);
    return result;
}

uint8_t verifyBobEthDepositData(BobSendsEthDepositInput input, char *data)
{
    std::stringstream ss = bobSendsEthDepositData(input);
    if (strcmp(ss.str().c_str(), data) != 0) {
        printf("Bob deposit data %s != expected %s\n", data, ss.str().c_str());
        return 0;
    }
    return 1;
}

std::stringstream bobSendsErc20DepositData(BobSendsErc20DepositInput input)
{
    uint8_t decimals;
    if (input.decimals > 0) {
        decimals = input.decimals;
    } else {
        decimals = getErc20Decimals(input.tokenAddress);
    }

    u256 amount = jsToU256(input.amount);
    u256 lockTime = input.lockTime;
    if (decimals < 18) {
        amount /= boost::multiprecision::pow(u256(10), 18 - decimals);
    }
    std::stringstream ss;
    ss << "0x5d567259"
       << toHex(jsToBytes(input.depositId))
       << toHex(toBigEndian(amount))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.aliceAddress))
       << toHex(jsToBytes(input.bobHash))
       << "000000000000000000000000"
       << "000000000000000000000000"
       << toHex(jsToAddress(input.tokenAddress))
       << toHex(toBigEndian(lockTime));
    return ss;
}

char* bobSendsErc20Deposit(BobSendsErc20DepositInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss = bobSendsErc20DepositData(input);
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTxWaitConfirm(rawTx);
    free(rawTx);
    return result;
}

uint8_t verifyBobErc20DepositData(BobSendsErc20DepositInput input, char *data)
{
    std::stringstream ss = bobSendsErc20DepositData(input);
    if (strcmp(ss.str().c_str(), data) != 0) {
        printf("Bob deposit data %s != expected %s\n", data, ss.str().c_str());
        return 0;
    }
    return 1;
}

char* bobRefundsDeposit(BobRefundsDepositInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss;
    u256 amount = jsToU256(input.amount);
    dev::Address tokenAddress = jsToAddress(input.tokenAddress);
    if (tokenAddress != ZeroAddress) {
        uint8_t decimals;
        if (input.decimals > 0) {
            decimals = input.decimals;
        } else {
            decimals = getErc20Decimals(input.tokenAddress);
        }

        if (decimals < 18) {
            amount /= boost::multiprecision::pow(u256(10), 18 - decimals);
        }
    }
    ss << "0x1f7a72f7"
       << toHex(jsToBytes(input.depositId))
       << toHex(toBigEndian(amount))
       << toHex(jsToBytes(input.bobSecret))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.aliceAddress))
       << "000000000000000000000000"
       << toHex(tokenAddress);
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTxWaitConfirm(rawTx);
    free(rawTx);
    return result;
}

char* aliceClaimsBobDeposit(AliceClaimsBobDepositInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss;
    u256 amount = jsToU256(input.amount);
    dev::Address tokenAddress = jsToAddress(input.tokenAddress);
    if (tokenAddress != ZeroAddress) {
        uint8_t decimals;
        if (input.decimals > 0) {
            decimals = input.decimals;
        } else {
            decimals = getErc20Decimals(input.tokenAddress);
        }

        if (decimals < 18) {
            amount /= boost::multiprecision::pow(u256(10), 18 - decimals);
        }
    }
    ss << "0x4b915a68"
       << toHex(jsToBytes(input.depositId))
       << toHex(toBigEndian(amount))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.bobAddress))
       << "000000000000000000000000"
       << toHex(tokenAddress)
       << toHex(jsToBytes(input.bobHash))
       << "000000000000000000000000";
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTxWaitConfirm(rawTx);
    free(rawTx);
    return result;
}

std::stringstream bobSendsEthPaymentData(BobSendsEthPaymentInput input)
{
    u256 lockTime = input.lockTime;
    std::stringstream ss;
    ss << "0x5ab30d95"
       << toHex(jsToBytes(input.paymentId))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.aliceAddress))
       << toHex(jsToBytes(input.aliceHash))
       << "000000000000000000000000"
       << toHex(toBigEndian(lockTime));
    return ss;
}

char* bobSendsEthPayment(BobSendsEthPaymentInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss = bobSendsEthPaymentData(input);
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTxWaitConfirm(rawTx);
    free(rawTx);
    return result;
}

uint8_t verifyBobEthPaymentData(BobSendsEthPaymentInput input, char *data)
{
    std::stringstream ss = bobSendsEthPaymentData(input);
    if (strcmp(ss.str().c_str(), data) != 0) {
        printf("Bob payment data %s != expected %s\n", data, ss.str().c_str());
        return 0;
    }
    return 1;
}

std::stringstream bobSendsErc20PaymentData(BobSendsErc20PaymentInput input)
{
    u256 amount = jsToU256(input.amount);
    u256 lockTime = input.lockTime;
    uint8_t decimals;
    if (input.decimals > 0) {
        decimals = input.decimals;
    } else {
        decimals = getErc20Decimals(input.tokenAddress);
    }

    if (decimals < 18) {
        amount /= boost::multiprecision::pow(u256(10), 18 - decimals);
    }
    std::stringstream ss;
    ss << "0xb8a15b1d"
       << toHex(jsToBytes(input.paymentId))
       << toHex(toBigEndian(amount))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.aliceAddress))
       << toHex(jsToBytes(input.aliceHash))
       << "000000000000000000000000"
       << "000000000000000000000000"
       << toHex(jsToAddress(input.tokenAddress))
       << toHex(toBigEndian(lockTime));
    return ss;
}

char* bobSendsErc20Payment(BobSendsErc20PaymentInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss = bobSendsErc20PaymentData(input);
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTxWaitConfirm(rawTx);
    free(rawTx);
    return result;
}

uint8_t verifyBobErc20PaymentData(BobSendsErc20PaymentInput input, char *data)
{
    std::stringstream ss = bobSendsErc20PaymentData(input);
    if (strcmp(ss.str().c_str(), data) != 0) {
        printf("Bob payment data %s != expected %s\n", data, ss.str().c_str());
        return 0;
    }
    return 1;
}

char* bobReclaimsBobPayment(BobReclaimsBobPaymentInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss;
    u256 amount = jsToU256(input.amount);
    dev::Address tokenAddress = jsToAddress(input.tokenAddress);
    if (tokenAddress != ZeroAddress) {
        uint8_t decimals;
        if (input.decimals > 0) {
            decimals = input.decimals;
        } else {
            decimals = getErc20Decimals(input.tokenAddress);
        }

        if (decimals < 18) {
            amount /= boost::multiprecision::pow(u256(10), 18 - decimals);
        }
    }
    ss << "0xe45ef4ad"
       << toHex(jsToBytes(input.paymentId))
       << toHex(toBigEndian(amount))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.aliceAddress))
       << "000000000000000000000000"
       << toHex(tokenAddress)
       << toHex(jsToBytes(input.aliceHash))
       << "000000000000000000000000";
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTxWaitConfirm(rawTx);
    free(rawTx);
    return result;
}

char* aliceSpendsBobPayment(AliceSpendsBobPaymentInput input, BasicTxData txData)
{
    TransactionSkeleton tx = txDataToSkeleton(txData);
    std::stringstream ss;
    u256 amount = jsToU256(input.amount);
    dev::Address tokenAddress = jsToAddress(input.tokenAddress);
    if (tokenAddress != ZeroAddress) {
        uint8_t decimals;
        if (input.decimals > 0) {
            decimals = input.decimals;
        } else {
            decimals = getErc20Decimals(input.tokenAddress);
        }

        if (decimals < 18) {
            amount /= boost::multiprecision::pow(u256(10), 18 - decimals);
        }
    }
    ss << "0x113ee583"
       << toHex(jsToBytes(input.paymentId))
       << toHex(toBigEndian(amount))
       << toHex(jsToBytes(input.aliceSecret))
       << "000000000000000000000000"
       << toHex(jsToAddress(input.bobAddress))
       << "000000000000000000000000"
       << toHex(tokenAddress);
    tx.data = jsToBytes(ss.str());
    char* rawTx = signTx(tx, txData.secretKey);
    char* result = sendRawTxWaitConfirm(rawTx);
    free(rawTx);
    return result;
}
*/
#[no_mangle]
pub extern "C" fn priv_key_2_addr(priv_key: *const c_char) -> *mut c_char
{
    unsafe {
        let priv_key_slice = CStr::from_ptr(priv_key).to_str().unwrap();
        let secret = Secret::from_str(priv_key_slice).unwrap();
        let key_pair = KeyPair::from_secret(secret).unwrap();
        let mut str = String::from("0x");
        str.push_str(&hex::encode(key_pair.address().0));
        CString::new(str).unwrap().into_raw()
    }
}

#[no_mangle]
pub extern "C" fn pub_key_2_addr(pub_key: *const c_char) -> *mut c_char
{
    unsafe {
        let pub_key_slice = CStr::from_ptr(pub_key).to_str().unwrap();
        let public = Public::from_str(pub_key_slice).unwrap();
        let mut str = String::from("0x");
        str.push_str(&hex::encode(public_to_address(&public).0));
        CString::new(str).unwrap().into_raw()
    }
}

#[no_mangle]
pub extern "C" fn priv_key_2_pub_key(priv_key: *const c_char) -> *mut c_char
{
    unsafe {
        let priv_key_slice = CStr::from_ptr(priv_key).to_str().unwrap();
        let secret = Secret::from_str(priv_key_slice).unwrap();
        let key_pair = KeyPair::from_secret(secret).unwrap();
        let pub_str = format!("{:02x}", key_pair.public());
        CString::new(pub_str).unwrap().into_raw()
    }
}

#[no_mangle]
pub extern "C" fn get_eth_balance(address: *const c_char, error: *const c_int, eth_client: *mut EthClient) -> u64
{
    unsafe {
        let address_slice = CStr::from_ptr(address).to_str().unwrap();
        let result = (*eth_client).web3.eth().balance(
            H160::from_str(&address_slice[2..]).unwrap(),
            Some(BlockNumber::Latest)
        ).wait().unwrap();
        (result / U256::exp10(10)).into()
    }
}

#[no_mangle]
pub extern "C" fn get_erc20_balance(
    address: *const c_char,
    token_address: *const c_char,
    set_decimals: u8,
    error: *const c_int,
    eth_client: *mut EthClient
) -> u64
{
    unsafe {
        let abi = unwrap!(Contract::load(ERC20_ABI.as_bytes()), "Could not parse ERC20 ABI, is it valid?");
        let function = unwrap!(abi.function("balanceOf"), "Could not get ERC20 balanceOf function, is ERC20 ABI valid?");
        let address_slice = CStr::from_ptr(address).to_str().unwrap();
        let token_address_slice = CStr::from_ptr(token_address).to_str().unwrap();
        let encoded = function.encode_input(&[
            Token::Address(H160::from_str(&address_slice[2..]).unwrap()),
        ]).unwrap();
        let output = (*eth_client).web3.eth().call(
            CallRequest {
                from: None,
                to: H160::from_str(&token_address_slice[2..]).unwrap(),
                gas: None,
                gas_price: None,
                value: None,
                data: Some(web3::types::Bytes(encoded))
            }, Some(BlockNumber::Latest)
        ).wait().unwrap();
        let decoded = function.decode_output(&output.0).unwrap();
        let mut result = match decoded[0] {
            Token::Uint(number) => number,
            _ => panic!("balanceOf call result must be uint, check ERC20 contract ABI")
        };

        if set_decimals < 18 {
            result = result * U256::exp10((18 - set_decimals) as usize);
        }
        (result / U256::exp10(10)).into()
    }
}

#[no_mangle]
pub extern "C" fn get_erc20_allowance(
    owner: *const c_char,
    spender: *const c_char,
    token_address: *const c_char,
    set_decimals: u8,
    eth_client: *mut EthClient
) -> u64
{
    unsafe {
        let abi = unwrap!(Contract::load(ERC20_ABI.as_bytes()), "Could not parse ERC20 ABI, is it valid?");
        let function = unwrap!(abi.function("allowance"), "Could not get ERC20 allowance function, is ERC20 ABI valid?");
        let owner_slice = CStr::from_ptr(owner).to_str().unwrap();
        let spender_slice = CStr::from_ptr(spender).to_str().unwrap();
        let address_slice = CStr::from_ptr(token_address).to_str().unwrap();
        let encoded = function.encode_input(&[
            Token::Address(H160::from_str(&owner_slice[2..]).unwrap()),
            Token::Address(H160::from_str(&spender_slice[2..]).unwrap())
        ]).unwrap();
        let output = (*eth_client).web3.eth().call(
            CallRequest {
                from: None,
                to: H160::from_str(&address_slice[2..]).unwrap(),
                gas: None,
                gas_price: None,
                value: None,
                data: Some(web3::types::Bytes(encoded))
            }, Some(BlockNumber::Latest)
        ).wait().unwrap();
        let decoded = function.decode_output(&output.0).unwrap();
        let mut result = match decoded[0] {
            Token::Uint(number) => number,
            _ => panic!("Allowance call result must be uint, check ERC20 contract ABI")
        };

        if set_decimals < 18 {
            result = result * U256::exp10((18 - set_decimals) as usize);
        }
        (result / U256::exp10(10)).into()
    }
}

#[no_mangle]
pub extern "C" fn get_erc20_decimals(token_address: *const c_char, eth_client: *mut EthClient) -> u8 {
    unsafe {
        let slice = CStr::from_ptr(token_address).to_str().unwrap();
        let abi = Contract::load(ERC20_ABI.as_bytes()).unwrap();
        let function = abi.function("decimals").unwrap();
        let encoded = function.encode_input(&[]).unwrap();
        let result = (*eth_client).web3.eth().call(
            CallRequest {
                from: None,
                to: H160::from_str(&slice[2..]).unwrap(),
                gas: None,
                gas_price: None,
                value: None,
                data: Some(web3::types::Bytes(encoded))
            }, Some(BlockNumber::Latest)
        ).wait();
        match result {
            Ok(output) => {
                let tokens = function.decode_output(&output.0);
                match tokens {
                    Ok(res) => match res[0] {
                        Token::Uint(dec) => (u64::from(dec)) as u8,
                        _ => 0
                    },
                    Err(_e) => 0
                }
            },
            Err(_e) => 0
        }
    }
}
/*
void uint8arrayToHex(char *dest, uint8_t *input, int len)
{
    strcpy(dest, "0x");
    for (int i = 0; i < len; i++)
    {
        sprintf(dest + (i + 1) * 2, "%02x", input[i]);
    }
    dest[(len + 1) * 2] = '\0';
}
*/
#[no_mangle]
pub extern "C" fn wei_to_satoshi(wei: *const c_char) -> u64
{
    unsafe {
        let wei_slice = CStr::from_ptr(wei).to_str().unwrap();
        (U256::from_str(&wei_slice[2..]).unwrap() / U256::exp10(10)).into()
    }
}

#[no_mangle]
pub extern "C" fn send_eth(
    to: *const c_char,
    amount: u64,
    gas: u64,
    gas_price: u64,
    default_gas_on_err: u8,
    eth_client: *mut EthClient
) -> *mut c_char
{
    let value = U256::from(amount) * U256::exp10(10);
    unsafe {
        let to_slice = CStr::from_ptr(to).to_str().unwrap();
        let to_address_h160 = H160::from_str(&to_slice[2..]).unwrap();

        let tx_id = (*eth_client).sign_and_send_transaction(
            value,
            Action::Call(to_address_h160),
            vec![],
            U256::from(21000),
            U256::exp10(10)
        );
        match tx_id {
            Ok(tx) => {
                let mut str = String::from("0x");
                str.push_str(&hex::encode(tx.0));
                CString::new(str).unwrap().into_raw()
            },
            Err(e) => {
                println!("Got error trying so send the transaction: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn estimate_erc20_gas(
    token_address: *const c_char,
    to: *const c_char,
    amount: u64,
    decimals: u8,
    eth_client: *mut EthClient
) -> u64
{
    let abi = Contract::load(ERC20_ABI.as_bytes()).unwrap();
    let function = abi.function("transfer").unwrap();
    let mut value = U256::from(amount) * U256::exp10(10);
    if decimals < 18 {
        value = value / U256::exp10((18 - decimals) as usize);
    }
    unsafe {
        let address_slice = CStr::from_ptr(token_address).to_str().unwrap();
        let to_slice = CStr::from_ptr(to).to_str().unwrap();
        let encoded = function.encode_input(&[
            Token::Address(H160::from_str(&to_slice[2..]).unwrap()),
            Token::Uint(value)
        ]).unwrap();
        let request = CallRequest {
            from: Some((*eth_client).my_address()),
            value: Some(U256::from(0)),
            to: H160::from_str(&address_slice[2..]).unwrap(),
            data: Some(Bytes(encoded)),
            gas_price: None,
            gas: None
        };
        let result = (*eth_client).web3.eth().estimate_gas(
            request,
            Some(BlockNumber::Latest)
        ).wait().unwrap();
        result.into()
    }
}

#[no_mangle]
pub extern "C" fn send_erc20(
    token_address: *const c_char,
    to: *const c_char,
    amount: u64,
    gas: u64,
    gas_price: u64,
    default_gas_on_err: u8,
    decimals: u8,
    eth_client: *mut EthClient
) -> *mut c_char
{
    let abi = Contract::load(ERC20_ABI.as_bytes()).unwrap();
    let function = abi.function("transfer").unwrap();
    let mut value = U256::from(amount) * U256::exp10(10);
    if decimals < 18 {
        value = value / U256::exp10((18 - decimals) as usize);
    }
    unsafe {
        let to_slice = CStr::from_ptr(to).to_str().unwrap();
        let encoded = function.encode_input(&[
            Token::Address(H160::from_str(&to_slice[2..]).unwrap()),
            Token::Uint(value)
        ]).unwrap();

        let token_address_slice = CStr::from_ptr(token_address).to_str().unwrap();
        let token_address_h160 = H160::from_str(&token_address_slice[2..]).unwrap();

        let tx_id = (*eth_client).sign_and_send_transaction(
            U256::from(0),
            Action::Call(token_address_h160),
            encoded,
            U256::from(200000),
            U256::exp10(10)
        );
        match tx_id {
            Ok(tx) => {
                let mut str = String::from("0x");
                str.push_str(&hex::encode(tx.0));
                CString::new(str).unwrap().into_raw()
            },
            Err(e) => {
                println!("Got error trying so send the transaction: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn verify_alice_erc20_fee_data(
    to: *const c_char,
    amount: u64,
    data: *const c_char,
    decimals: u8
) -> u8 {
    let abi = Contract::load(ERC20_ABI.as_bytes()).unwrap();
    let function = abi.function("transfer").unwrap();
    let mut value = U256::from(amount) * U256::exp10(10);
    if decimals < 18 {
        value = value / U256::exp10((18 - decimals) as usize);
    }
    unsafe {
        let to_slice = CStr::from_ptr(to).to_str().unwrap();
        let encoded = function.encode_input(&[
            Token::Address(H160::from_str(&to_slice[2..]).unwrap()),
            Token::Uint(value)
        ]).unwrap();
        let data_slice = CStr::from_ptr(data).to_str().unwrap();
        let data_decoded = hex::decode(&data_slice[2..]).unwrap();
        (data_decoded == encoded) as u8
    }
}

#[no_mangle]
pub extern "C" fn alice_payment_status(payment_tx_id: *const c_char, eth_client: *mut EthClient) -> u64 {
    unsafe {
        let slice = CStr::from_ptr(payment_tx_id).to_str().unwrap();

        let function = unwrap!(
            (*eth_client).alice_abi.function("deals"),
            "Could not load deals function of Alice contract. Is ALICE_ABI valid?"
        );

        let encoded = function.encode_input(&[
            Token::FixedBytes(hex::decode(&slice[2..]).unwrap())
        ]).unwrap();
        let res = (*eth_client).web3.eth().call(CallRequest {
            from: None,
            to: H160::from(ALICE_CONTRACT),
            gas: None,
            gas_price: None,
            value: None,
            data: Some(web3::types::Bytes(encoded))
        }, Some(BlockNumber::Latest)).wait().unwrap();
        let tokens = function.decode_output(&res.0).unwrap();
        match tokens[1] {
            Token::Uint(number) => number.into(),
            _ => panic!("Payment status must be Uint, check Alice contract ABI")
        }
    }
}

#[no_mangle]
pub extern "C" fn bob_deposit_status(deposit_tx_id: *const c_char, eth_client: *mut EthClient) -> u64 {
    unsafe {
        let slice = CStr::from_ptr(deposit_tx_id).to_str().unwrap();
        let abi = Contract::load(BOB_ABI.as_bytes()).unwrap();
        let function = abi.function("deposits").unwrap();
        let encoded = function.encode_input(&[
            Token::FixedBytes(hex::decode(&slice[2..]).unwrap())
        ]).unwrap();
        let res = (*eth_client).web3.eth().call(CallRequest {
            from: None,
            to: H160::from(BOB_CONTRACT),
            gas: None,
            gas_price: None,
            value: None,
            data: Some(web3::types::Bytes(encoded))
        }, Some(BlockNumber::Latest)).wait().unwrap();
        let tokens = function.decode_output(&res.0).unwrap();
        match tokens[2] {
            Token::Uint(number) => number.into(),
            _ => panic!("Deposit status must be Uint, check bob contract ABI")
        }
    }
}

#[no_mangle]
pub extern "C" fn bob_payment_status(payment_tx_id: *const c_char, eth_client: *mut EthClient) -> u64 {
    unsafe {
        let slice = CStr::from_ptr(payment_tx_id).to_str().unwrap();
        let abi = Contract::load(BOB_ABI.as_bytes()).unwrap();
        let function = abi.function("payments").unwrap();
        let encoded = function.encode_input(&[
            Token::FixedBytes(hex::decode(&slice[2..]).unwrap())
        ]).unwrap();
        let res = (*eth_client).web3.eth().call(CallRequest {
            from: None,
            to: H160::from(BOB_CONTRACT),
            gas: None,
            gas_price: None,
            value: None,
            data: Some(web3::types::Bytes(encoded))
        }, Some(BlockNumber::Latest)).wait().unwrap();
        let tokens = function.decode_output(&res.0).unwrap();
        match tokens[2] {
            Token::Uint(number) => number.into(),
            _ => panic!("Payment status must be Uint, check bob contract ABI")
        }
    }
}

#[no_mangle]
pub extern "C" fn compare_addresses(address1: *const c_char, address2: *const c_char) -> u8 {
    unsafe {
        let slice1 = CStr::from_ptr(address1).to_str().unwrap();
        let slice2 = CStr::from_ptr(address2).to_str().unwrap();
        let hash1 = H160::from_str(&slice1[2..]).unwrap();
        let hash2 = H160::from_str(&slice2[2..]).unwrap();
        (hash1 == hash2) as u8
    }
}

#[no_mangle]
pub extern "C" fn is_valid_address(address: *const c_char) -> u8 {
    unsafe {
        let slice = CStr::from_ptr(address).to_str().unwrap();
        let re = Regex::new(r"^(0x|0X)?[a-fA-F0-9]{40}$").unwrap();
        re.is_match(slice) as u8
    }
}

/// Workaround to avoid Undefined symbols for architecture x86_64 "_je_malloc_usable_size"
/// It's dangerous and seems like it might cause unexpected behaviour
/// However seems like there is no other "easy" ways to fix now as there are too many
/// projects depending on "heapsize" crate
/// https://github.com/paritytech/parity-ethereum/issues/9167
/// https://github.com/servo/heapsize/issues/80
#[no_mangle]
#[cfg(target_os = "macos")]
pub extern fn je_malloc_usable_size(_ptr: *const c_void) -> usize {
    unsafe {
        extern "C" {
            fn malloc_usable_size(ptr: *const c_void) -> usize;
        }
        return malloc_usable_size(_ptr);
    }
}

#[cfg(test)]
#[test]
fn test_get_eth_balance() {
    let address = CString::new("0xbAB36286672fbdc7B250804bf6D14Be0dF69fa29").unwrap();
    let priv_key = CString::new("0x809465b17d0a4ddb3e4c69e8f23c2cabad868f51f8bed5c765ad1d6516c3306f").unwrap();
    let error : c_int = 0;

    let result = get_eth_balance(
        address.as_ptr(),
        &error,
        eth_client(priv_key.as_ptr())
    );
    assert!(result > 0);
}

#[cfg(test)]
#[test]
fn test_get_erc20_balance() {
    let address = CString::new("0xbAB36286672fbdc7B250804bf6D14Be0dF69fa29").unwrap();
    let token_address = CString::new("0xd53315FeE75569ebaAb9d65fcAA94B5E836904Ea").unwrap();
    let priv_key = CString::new("0x809465b17d0a4ddb3e4c69e8f23c2cabad868f51f8bed5c765ad1d6516c3306f").unwrap();
    let error : c_int = 0;

    let result = get_erc20_balance(
        address.as_ptr(),
        token_address.as_ptr(),
        8,
        &error,
        eth_client(priv_key.as_ptr())
    );
    assert!(result > 0);
}

#[cfg(test)]
#[test]
fn test_get_erc20_allowance() {
    let owner = CString::new("0xbAB36286672fbdc7B250804bf6D14Be0dF69fa29").unwrap();
    let spender = CString::new("0xbAB36286672fbdc7B250804bf6D14Be0dF69fa29").unwrap();
    let token_address = CString::new("0xd53315FeE75569ebaAb9d65fcAA94B5E836904Ea").unwrap();
    let priv_key = CString::new("0x809465b17d0a4ddb3e4c69e8f23c2cabad868f51f8bed5c765ad1d6516c3306f").unwrap();

    let result = get_erc20_allowance(
        owner.as_ptr(),
        spender.as_ptr(),
        token_address.as_ptr(),
        8,
        eth_client(priv_key.as_ptr())
    );
    assert_eq!(result, 0);
}

#[cfg(test)]
#[test]
fn test_wei_to_satoshi() {
    let wei = CString::new("0x7526ea4b2401").unwrap();
    let satoshi = wei_to_satoshi(wei.as_ptr());
    assert_eq!(satoshi, 12881);
}

#[cfg(test)]
#[test]
fn test_verify_alice_eth_payment_data() {
    let mut alice_hash : [c_char; 65usize] = [0; 65];
    let mut bob_hash : [c_char; 65usize] = [0; 65];
    let mut bob_address : [c_char; 65usize] = [0; 65];
    let mut deal_id : [c_char; 70usize] = [0; 70];
    unsafe {
        libc::strcpy(alice_hash.as_mut_ptr(), CString::new("0x9e2750ff62c3ae22f441fc51fe4422b4d1f5d414").unwrap().as_ptr());
        libc::strcpy(bob_hash.as_mut_ptr(), CString::new("0x54be0b08698ebd55a43fbb225c124d45fff16366").unwrap().as_ptr());
        libc::strcpy(bob_address.as_mut_ptr(), CString::new("0x4b2d0d6c2c785217457b69b922a2a9cea98f71e9").unwrap().as_ptr());
        libc::strcpy(deal_id.as_mut_ptr(), CString::new("0xac010fc07a69dedd0536ffeca2a1d0685b7be444fdf68c9028b5b169aa0905c3").unwrap().as_ptr());
    }

    let valid_data = CString::new("0x47c7b6e2ac010fc07a69dedd0536ffeca2a1d0685b7be444fdf68c9028b5b169aa0905c30000000000000000000000004b2d0d6c2c785217457b69b922a2a9cea98f71e99e2750ff62c3ae22f441fc51fe4422b4d1f5d41400000000000000000000000054be0b08698ebd55a43fbb225c124d45fff16366000000000000000000000000").unwrap();

    let input = AliceSendsEthPaymentInput {
        amount: 0,
        alice_hash,
        bob_hash,
        bob_address,
        deal_id
    };

    assert_eq!(verify_alice_eth_payment_data(input, valid_data.as_ptr()), 1);

    let invalid_data = CString::new("0xc7b6e2ac010fc07a69dedd0536ffeca2a1d0685b7be444fdf68c9028b5b169aa0905c30000000000000000000000004b2d0d6c2c785217457b69b922a2a9cea98f71e99e2750ff62c3ae22f441fc51fe4422b4d1f5d41400000000000000000000000054be0b08698ebd55a43fbb225c124d45fff16366000000000000000000000000").unwrap();
    assert_eq!(verify_alice_eth_payment_data(input, invalid_data.as_ptr()), 0);
}

#[cfg(test)]
#[test]
fn test_verify_alice_erc20_payment_data() {
    let mut alice_hash : [c_char; 65usize] = [0; 65];
    let mut bob_hash : [c_char; 65usize] = [0; 65];
    let mut bob_address : [c_char; 65usize] = [0; 65];
    let mut token_address : [c_char; 65usize] = [0; 65];
    let mut deal_id : [c_char; 70usize] = [0; 70];
    unsafe {
        libc::strcpy(alice_hash.as_mut_ptr(), CString::new("0xb3b7e2df561771e71335c7ab6af75f07ef5fdbdb").unwrap().as_ptr());
        libc::strcpy(bob_hash.as_mut_ptr(), CString::new("0x339b417b1924f3cbf03aa156cd993368ca66ea88").unwrap().as_ptr());
        libc::strcpy(bob_address.as_mut_ptr(), CString::new("0x4b2d0d6c2c785217457b69b922a2a9cea98f71e9").unwrap().as_ptr());
        libc::strcpy(deal_id.as_mut_ptr(), CString::new("0x08b343ceda5196d3ae8fd0822b44bff1a8ed43a0354fd0a17f4a52e1bbb0e5e5").unwrap().as_ptr());
        libc::strcpy(token_address.as_mut_ptr(), CString::new("0xc0eb7aed740e1796992a08962c15661bdeb58003").unwrap().as_ptr());
    }

    let valid_data = CString::new("0x184db3bf08b343ceda5196d3ae8fd0822b44bff1a8ed43a0354fd0a17f4a52e1bbb0e5e5000000000000000000000000000000000000000000000000016397531f91a0000000000000000000000000004b2d0d6c2c785217457b69b922a2a9cea98f71e9b3b7e2df561771e71335c7ab6af75f07ef5fdbdb000000000000000000000000339b417b1924f3cbf03aa156cd993368ca66ea88000000000000000000000000000000000000000000000000c0eb7aed740e1796992a08962c15661bdeb58003").unwrap();

    let input = AliceSendsErc20PaymentInput {
        amount: 10009000,
        alice_hash,
        bob_hash,
        bob_address,
        token_address,
        deal_id,
        decimals: 18
    };

    assert_eq!(verify_alice_erc20_payment_data(input, valid_data.as_ptr()), 1);

    let invalid_data = CString::new("0xc7b6e2ac010fc07a69dedd0536ffeca2a1d0685b7be444fdf68c9028b5b169aa0905c30000000000000000000000004b2d0d6c2c785217457b69b922a2a9cea98f71e99e2750ff62c3ae22f441fc51fe4422b4d1f5d41400000000000000000000000054be0b08698ebd55a43fbb225c124d45fff16366000000000000000000000000").unwrap();
    assert_eq!(verify_alice_erc20_payment_data(input, invalid_data.as_ptr()), 0);
}

#[cfg(test)]
#[test]
fn test_verify_alice_erc_fee_data() {
    let to = CString::new("0x3f17f1962b36e491b30a40b2405849e597ba5fb5").unwrap();
    let amount = 12881;
    let valid_data = CString::new("0xa9059cbb0000000000000000000000003f17f1962b36e491b30a40b2405849e597ba5fb500000000000000000000000000000000000000000000000000007526ea4b2400").unwrap();
    let decimals = 18;
    assert_eq!(verify_alice_erc20_fee_data(
        to.as_ptr(),
        amount,
        valid_data.as_ptr(),
        decimals
    ), 1);

    let invalid_data = CString::new("0xa9059cbb0000000000000000000000003f17f1962b36e491b30a40b2405849e597ba5fb500000000000000000000000000000000000000000000000000007526ea4b2401").unwrap();
    assert_eq!(verify_alice_erc20_fee_data(
        to.as_ptr(),
        amount,
        invalid_data.as_ptr(),
        decimals
    ), 0);
}

#[cfg(test)]
#[test]
fn test_alice_payment_status() {
    let tx_id = CString::new("0x781d3bd164d6e0b6abeacb34b680a2dd43ee2e5dadad45f631bb21d06e792d98").unwrap();
    let priv_key = CString::new("0x809465b17d0a4ddb3e4c69e8f23c2cabad868f51f8bed5c765ad1d6516c3306f").unwrap();
    assert_eq!(alice_payment_status(tx_id.as_ptr(), eth_client(priv_key.as_ptr())), 2);
}

#[cfg(test)]
#[test]
fn test_bob_payment_status() {
    let tx_id = CString::new("0x301e0ab4824d87e764a1ef4dea49618e207aac8d80ffbb22de75152a5c25adc0").unwrap();
    let priv_key = CString::new("0x809465b17d0a4ddb3e4c69e8f23c2cabad868f51f8bed5c765ad1d6516c3306f").unwrap();
    assert_eq!(bob_payment_status(tx_id.as_ptr(), eth_client(priv_key.as_ptr())), 2);
}

#[cfg(test)]
#[test]
fn test_bob_deposit_status() {
    let tx_id = CString::new("0xd4116948f7b9a8e06b84417a48db0e34213b25e8fa3b50a7888fcb049fbf430d").unwrap();
    let priv_key = CString::new("0x809465b17d0a4ddb3e4c69e8f23c2cabad868f51f8bed5c765ad1d6516c3306f").unwrap();
    assert_eq!(bob_deposit_status(tx_id.as_ptr(), eth_client(priv_key.as_ptr())), 3);
}

#[cfg(test)]
#[test]
fn test_compare_addresses() {
    let address1 = CString::new("0xe1d4236c5774d35dc47dcc2e5e0ccfc463a3289c").unwrap();
    let address2 = CString::new("0xe1D4236C5774D35Dc47dcc2E5E0CcFc463A3289c").unwrap();

    assert_eq!(compare_addresses(address1.as_ptr(), address2.as_ptr()), 1);

    let address1 = CString::new("0xe1d4236c5774d35dc47dcc2e5e0ccfc463a3289c").unwrap();
    let address2 = CString::new("0x2a8e4f9ae69c86e277602c6802085febc4bd5986").unwrap();

    assert_eq!(compare_addresses(address1.as_ptr(), address2.as_ptr()), 0);
}

#[cfg(test)]
#[test]
fn test_is_valid_address() {
    let address = CString::new("0xe1d4236c5774d35dc47dcc2e5e0ccfc463a3289c").unwrap();

    assert_eq!(is_valid_address(address.as_ptr()), 1);

    let address = CString::new("e1d4236c5774d35dc47dcc2e5e0ccfc463a3289c").unwrap();

    assert_eq!(is_valid_address(address.as_ptr()), 1);

    let address = CString::new("0xe1D4236C5774D35Dc47dcc2E5E0CcFc463A3289c").unwrap();

    assert_eq!(is_valid_address(address.as_ptr()), 1);

    let address = CString::new("e1D4236C5774D35Dc47dcc2E5E0CcFc463A3289c").unwrap();

    assert_eq!(is_valid_address(address.as_ptr()), 1);

    let address = CString::new("0e1d4236c5774d35dc47dcc2e5e0ccfc463a3289c").unwrap();

    assert_eq!(is_valid_address(address.as_ptr()), 0);
}