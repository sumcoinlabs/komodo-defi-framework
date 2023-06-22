use clap::{Args, ValueEnum};
use common::serde_derive::Serialize;
use mm2_number::MmNumber;
use mm2_rpc::data::legacy::{MatchBy, OrderType, SellBuyRequest};
use rpc::v1::types::H256 as H256Json;
use std::collections::HashSet;
use std::mem::take;
use std::str::FromStr;
use uuid::Uuid;

use super::parse_mm_number;

#[derive(Args)]
#[command(about = "Puts a selling coins request")]
pub struct SellOrderArgs {
    #[command(flatten)]
    pub order_cli: OrderArgs,
}

#[derive(Args)]
#[command(about = "Puts a buying coins request")]
pub struct BuyOrderArgs {
    #[command(flatten)]
    pub order_cli: OrderArgs,
}

#[derive(Args, Serialize, Debug)]
pub struct OrderArgs {
    #[arg(help = "Base currency of a pair")]
    pub base: String,
    #[arg(help = "Related currency")]
    pub rel: String,
    #[arg(help = "Amount of coins the user is willing to sell/buy of the base coin", value_parser=parse_mm_number )]
    pub volume: MmNumber,
    #[arg(help = "Price in rel the user is willing to receive/pay per one unit of the base coin", value_parser=parse_mm_number)]
    pub price: MmNumber,
    #[arg(long, value_enum, default_value_t = OrderTypeCli::GoodTillCancelled, help="The GoodTillCancelled order is automatically converted to a maker order if not matched in 30 seconds, and this maker order stays in the orderbook until explicitly cancelled. On the other hand, a FillOrKill is cancelled if not matched within 30 seconds")]
    pub order_type: OrderTypeCli,
    #[arg(long,
          help = "Amount of base coin that will be used as min_volume of GoodTillCancelled order after conversion to maker", 
          value_parser=parse_mm_number
    )]
    pub min_volume: Option<MmNumber>,
    #[arg(
        short = 'u',
        long = "uuid",
        help = "The created order is matched using a set of uuid"
    )]
    pub match_uuids: Vec<Uuid>,
    #[arg(short='p',
          long="public",
          value_parser=H256Json::from_str,
          help="The created order is matched using a set of publics to select specific nodes (ignored if uuids not empty)")]
    pub match_publics: Vec<H256Json>,
    #[arg(
        long,
        help = "Number of required blockchain confirmations for base coin atomic swap transaction"
    )]
    pub base_confs: Option<u64>,
    #[arg(
        long,
        help = "Whether dPoW notarization is required for base coin atomic swap transaction"
    )]
    pub base_nota: Option<bool>,
    #[arg(
        long,
        help = "Number of required blockchain confirmations for rel coin atomic swap transaction"
    )]
    pub rel_confs: Option<u64>,
    #[arg(
        long,
        help = "Whether dPoW notarization is required for rel coin atomic swap transaction"
    )]
    pub rel_nota: Option<bool>,
    #[arg(
        long,
        help = "If true, each order's short record history is stored else the only order status will be temporarily stored while in progress"
    )]
    pub save_in_history: bool,
}

#[derive(Debug, Copy, Clone, ValueEnum, Serialize)]
pub enum OrderTypeCli {
    FillOrKill,
    GoodTillCancelled,
}

impl From<OrderTypeCli> for OrderType {
    fn from(value: OrderTypeCli) -> Self {
        match value {
            OrderTypeCli::GoodTillCancelled => OrderType::GoodTillCancelled,
            OrderTypeCli::FillOrKill => OrderType::FillOrKill,
        }
    }
}

impl From<&mut OrderArgs> for SellBuyRequest {
    fn from(value: &mut OrderArgs) -> Self {
        let match_by = if !value.match_uuids.is_empty() {
            MatchBy::Orders(HashSet::from_iter(value.match_uuids.drain(..)))
        } else if !value.match_publics.is_empty() {
            MatchBy::Pubkeys(HashSet::from_iter(value.match_publics.drain(..)))
        } else {
            MatchBy::Any
        };

        let will_be_substituted = String::new();
        SellBuyRequest {
            base: take(&mut value.base),
            rel: take(&mut value.rel),
            price: take(&mut value.price),
            volume: take(&mut value.volume),
            timeout: None,
            duration: None,
            method: will_be_substituted,
            gui: None,
            dest_pub_key: H256Json::default(),
            match_by,
            order_type: value.order_type.into(),
            base_confs: value.base_confs,
            base_nota: value.base_nota,
            rel_confs: value.rel_confs,
            rel_nota: value.rel_nota,
            min_volume: take(&mut value.min_volume),
            save_in_history: value.save_in_history,
        }
    }
}