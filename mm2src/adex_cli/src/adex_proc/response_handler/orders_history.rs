use anyhow::Result;
use itertools::Itertools;
use std::cell::RefMut;
use std::io::Write;
use term_table::{row::Row,
                 table_cell::{Alignment, TableCell},
                 Table as TermTable, TableStyle};

use crate::adex_proc::response_handler::formatters::{format_confirmation_settings, format_datetime, format_f64,
                                                     format_historical_changes, format_ratio, taker_order_header_row,
                                                     taker_order_rows};
use common::{write_safe::io::WriteSafeIO, write_safe_io, writeln_safe_io};
use mm2_rpc::data::legacy::{FilteringOrder, MakerOrderForRpc, Mm2RpcResult, OrderForRpc, OrdersHistoryResponse,
                            UuidParseError};

use super::super::OrdersHistorySettings;
use super::formatters::write_maker_match;

pub(super) fn on_orders_history(
    mut writer: RefMut<'_, dyn Write>,
    mut response: Mm2RpcResult<OrdersHistoryResponse>,
    settings: OrdersHistorySettings,
) -> Result<()> {
    macro_rules! write_result {
        ($rows: ident, $header_fn: ident, $legend: literal) => {
            if $rows.is_empty() {
                writeln_safe_io!(writer, concat!($legend, " not found"));
            } else {
                let mut table = term_table_blank(TableStyle::thin(), false, true, true);
                table.add_row($header_fn());
                table.add_row(Row::new(vec![TableCell::new("")]));
                table.rows.extend($rows.drain(..));
                write_safe_io!(writer, concat!($legend, "\n{}"), table.render().replace('\0', ""))
            }
        };
    }
    if settings.common {
        let orders = response.result.orders.drain(..);
        let mut rows: Vec<Row> = orders.map(filtering_order_row).try_collect()?;
        write_result!(rows, filtering_order_header_row, "Orders history:");
    }

    let mut maker_rows = vec![];
    let mut taker_rows = vec![];

    if settings.makers_detailed || settings.takers_detailed {
        for order in response.result.details.drain(..) {
            match order {
                OrderForRpc::Maker(order) => maker_order_rows(&order)?.drain(..).for_each(|row| maker_rows.push(row)),
                OrderForRpc::Taker(order) => taker_order_rows(&order)?.drain(..).for_each(|row| taker_rows.push(row)),
            }
        }
    }

    if settings.takers_detailed {
        write_result!(taker_rows, taker_order_header_row, "Taker orders history detailed:");
    }
    if settings.makers_detailed {
        write_result!(maker_rows, maker_order_header_row, "Maker orders history detailed:");
    }
    if settings.warnings {
        let warnings = response.result.warnings.drain(..);
        let mut rows: Vec<Row> = warnings.map(uuid_parse_error_row).collect();
        write_result!(rows, uuid_parse_error_header_row, "Uuid parse errors:");
    }

    Ok(())
}

fn filtering_order_header_row() -> Row<'static> {
    Row::new(vec![
        TableCell::new_with_alignment_and_padding("uuid", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Type", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Action", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Base", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Rel", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Volume", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Price", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Status", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Created", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Updated", 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding("Was taker", 1, Alignment::Left, false),
    ])
}

fn filtering_order_row(order: FilteringOrder) -> Result<Row<'static>> {
    Ok(Row::new(vec![
        TableCell::new_with_alignment_and_padding(&order.uuid, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(&order.order_type, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(&order.initial_action, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(&order.base, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(&order.rel, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(format_f64(order.volume, 2, 5)?, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(format_f64(order.price, 2, 5)?, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(&order.status, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(format_datetime(order.created_at as u64)?, 1, Alignment::Left, false),
        TableCell::new_with_alignment_and_padding(
            format_datetime(order.last_updated as u64)?,
            1,
            Alignment::Left,
            false,
        ),
        TableCell::new_with_alignment_and_padding(order.was_taker != 0, 1, Alignment::Left, false),
    ]))
}

fn uuid_parse_error_header_row() -> Row<'static> { Row::new(vec![TableCell::new("uuid"), TableCell::new("error")]) }

fn uuid_parse_error_row(uuid_parse_error: UuidParseError) -> Row<'static> {
    Row::new(vec![
        TableCell::new(uuid_parse_error.uuid),
        TableCell::new(uuid_parse_error.warning),
    ])
}

fn maker_order_header_row() -> Row<'static> {
    Row::new(vec![
        TableCell::new("base,rel"),
        TableCell::new("price"),
        TableCell::new("uuid"),
        TableCell::new("created at,\nupdated at"),
        TableCell::new("min base vol,\nmax base vol"),
        TableCell::new("swaps"),
        TableCell::new("conf_settings"),
        TableCell::new("history changes"),
        TableCell::new("orderbook ticker\nbase, rel"),
    ])
}

fn maker_order_rows(order: &MakerOrderForRpc) -> Result<Vec<Row<'static>>> {
    let mut rows = vec![Row::new(vec![
        TableCell::new(format!("{},{}", order.base, order.rel)),
        TableCell::new(format_ratio(&order.price_rat, 2, 5)?),
        TableCell::new(order.uuid),
        TableCell::new(format!(
            "{},\n{}",
            format_datetime(order.created_at)?,
            order.updated_at.map_or(Ok("".to_string()), format_datetime)?
        )),
        TableCell::new(format!(
            "{},\n{}",
            format_ratio(&order.min_base_vol_rat, 2, 5)?,
            format_ratio(&order.max_base_vol_rat, 2, 5)?
        )),
        TableCell::new(if order.started_swaps.is_empty() {
            "empty".to_string()
        } else {
            order.started_swaps.iter().join(",\n")
        }),
        TableCell::new(
            order
                .conf_settings
                .as_ref()
                .map_or_else(|| "none".to_string(), format_confirmation_settings),
        ),
        TableCell::new(order.changes_history.as_ref().map_or_else(
            || "none".to_string(),
            |val| {
                val.iter()
                    .map(|val| format_historical_changes(val, "\n").unwrap_or_else(|_| "error".into()))
                    .join(",\n")
            },
        )),
        TableCell::new(format!(
            "{}\n{}",
            order
                .base_orderbook_ticker
                .as_ref()
                .map_or_else(|| "none".to_string(), String::clone),
            order
                .rel_orderbook_ticker
                .as_ref()
                .map_or_else(|| "none".to_string(), String::clone)
        )),
    ])];

    if order.matches.is_empty() {
        return Ok(rows);
    }
    rows.push(Row::new(vec![TableCell::new_with_col_span("matches", 10)]));
    for (uuid, m) in &order.matches {
        let mut matches_str = Vec::new();
        let mut bbox: Box<dyn Write> = Box::new(&mut matches_str);
        write_maker_match(bbox.as_mut(), uuid, m)?;
        drop(bbox);
        rows.push(Row::new(vec![TableCell::new_with_col_span(
            String::from_utf8(matches_str).unwrap(),
            10,
        )]));
    }
    Ok(rows)
}

fn term_table_blank(style: TableStyle, sep_row: bool, bottom_border: bool, top_border: bool) -> TermTable<'static> {
    let mut term_table = TermTable::new();
    term_table.style = style;
    term_table.separate_rows = sep_row;
    term_table.has_bottom_boarder = bottom_border;
    term_table.has_top_boarder = top_border;
    term_table
}