use anyhow::{anyhow, Result};
use derive_more::Display;
use log::error;
use mm2_rpc::data::version2::{MmRpcRequest, MmRpcVersion};
use serde::Serialize;

use crate::error_anyhow;

#[derive(Serialize, Clone)]
pub(super) struct Command<T>
where
    T: Serialize + Sized,
{
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(self) flatten_data: Option<T>,
    pub(self) userpass: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(self) method: Option<Method>,
}

#[derive(Serialize, Clone, Display)]
#[serde(rename_all = "snake_case")]
pub(super) enum Method {
    Stop,
    Version,
    #[serde(rename = "my_balance")]
    GetBalance,
    #[serde(rename = "get_enabled_coins")]
    GetEnabledCoins,
    #[serde(rename = "orderbook")]
    GetOrderbook,
    Sell,
    Buy,
    CancelOrder,
    CancelAllOrders,
    OrderStatus,
    BestOrders,
    #[serde(rename = "setprice")]
    SetPrice,
    MyOrders,
    OrderbookDepth,
    #[serde(rename = "orders_history_by_filter")]
    OrdersHistory,
    UpdateMakerOrder,
}

#[derive(Serialize, Clone, Copy, Display)]
pub(super) struct Dummy {}

impl<T> Command<T>
where
    T: Serialize + Sized,
{
    pub(super) fn builder() -> CommandBuilder<T> { CommandBuilder::new() }
}

pub(super) struct CommandBuilder<T> {
    userpass: Option<String>,
    method: Option<Method>,
    flatten_data: Option<T>,
}

impl<T> CommandBuilder<T>
where
    T: Serialize,
{
    fn new() -> Self {
        CommandBuilder {
            userpass: None,
            method: None,
            flatten_data: None,
        }
    }

    pub(super) fn userpass(&mut self, userpass: String) -> &mut Self {
        self.userpass = Some(userpass);
        self
    }

    pub(super) fn method(&mut self, method: Method) -> &mut Self {
        self.method = Some(method);
        self
    }

    pub(super) fn flatten_data(&mut self, flatten_data: T) -> &mut Self {
        self.flatten_data = Some(flatten_data);
        self
    }

    pub(super) fn build(&mut self) -> Result<Command<T>> {
        Ok(Command {
            userpass: self
                .userpass
                .take()
                .ok_or_else(|| error_anyhow!("Build command failed, no userpass"))?,
            method: self.method.take(),
            flatten_data: self.flatten_data.take(),
        })
    }

    pub(crate) fn build_v2(&mut self) -> Result<MmRpcRequest<Method, T>> {
        let mm2_rpc_request = MmRpcRequest {
            mmrpc: MmRpcVersion::V2,
            userpass: Some(
                self.userpass
                    .take()
                    .ok_or_else(|| error_anyhow!("Build command failed, no userpass"))?,
            ),
            method: self
                .method
                .take()
                .ok_or_else(|| error_anyhow!("Failed to get method, not set"))?,
            params: self
                .flatten_data
                .take()
                .ok_or_else(|| error_anyhow!("Failed to get flatten_data, not set"))?,
            id: None,
        };

        Ok(mm2_rpc_request)
    }
}

impl<T: Serialize + Clone> std::fmt::Display for Command<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut cmd: Self = self.clone();
        cmd.userpass = "***********".to_string();
        writeln!(
            f,
            "{}",
            serde_json::to_string(&cmd).unwrap_or_else(|_| "Unknown".to_string())
        )
    }
}