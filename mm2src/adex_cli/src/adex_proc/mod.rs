mod adex_proc_impl;
mod command;
mod response_handler;
mod smart_fraction_fmt;

pub(super) use adex_proc_impl::AdexProc;
pub(super) use response_handler::{ResponseHandler, ResponseHandlerImpl};
pub(super) use smart_fraction_fmt::SmarFractPrecision;

#[derive(Clone)]
pub(super) struct OrderbookConfig {
    pub(super) uuids: bool,
    pub(super) min_volume: bool,
    pub(super) max_volume: bool,
    pub(super) publics: bool,
    pub(super) address: bool,
    pub(super) age: bool,
    pub(super) conf_settings: bool,
    pub(super) asks_limit: Option<usize>,
    pub(super) bids_limit: Option<usize>,
}
