#![feature(prelude_import)]
extern crate std;
#[prelude_import]
use std::prelude::rust_2024::*;
use anyhow::Result;
use clap::Parser;
mod client {
    pub mod client {
        use std::str::FromStr;
        use std::sync::Arc;
        use anyhow::{Context, Result};
        use iroh::{
            Endpoint, EndpointId, address_lookup::{self, PkarrPublisher},
            endpoint::presets,
        };
        use tokio::net::{TcpListener, TcpStream};
        use tracing::{error, info, warn};
        use crate::protocols::{
            ack::Ack, file_send::{alpn::FILE_ALPN_V1, file_send_header::FileSendHeader},
            ping::{alpn::PING_ALPN_V1, ping_header::PingHeader},
            proxy::{alpn::TCP_PROXY_ALPN_V1, proxy_header::ProxyHeader},
        };
        use crate::protocols::proxy::proxy_helpers::proxy_streams;
        use crate::socks5;
        use crate::http;
        use crate::client::client_helpers::{
            load_node_id_from_file, write_node_id_to_file,
        };
        enum ProxyType {
            Socks5,
            Http,
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for ProxyType {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(
                    f,
                    match self {
                        ProxyType::Socks5 => "Socks5",
                        ProxyType::Http => "Http",
                    },
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        unsafe impl ::core::clone::TrivialClone for ProxyType {}
        #[automatically_derived]
        impl ::core::clone::Clone for ProxyType {
            #[inline]
            fn clone(&self) -> ProxyType {
                *self
            }
        }
        #[automatically_derived]
        impl ::core::marker::Copy for ProxyType {}
        fn get_proxy_addr_and_type(url: &str) -> (ProxyType, String) {
            let (prefix, addr) = url
                .split_once("://")
                .expect("Invalid format: must be protocol://host:port");
            let typ = match prefix.to_lowercase().as_str() {
                "socks5" => ProxyType::Socks5,
                "http" => ProxyType::Http,
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("Unsupported proxy type: {0}", prefix),
                    );
                }
            };
            (typ, addr.to_string())
        }
        async fn ping_server(
            endpoint: &Endpoint,
            server_node_id: EndpointId,
        ) -> Result<()> {
            const MSG: &str = "ping";
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:36",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(36u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Pinging server {0}", server_node_id)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Pinging server {0}", server_node_id)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let conn = endpoint.connect(server_node_id, PING_ALPN_V1).await?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:38",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(38u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Connected to server, opening stream")
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Connected to server, opening stream")
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let (mut send, mut recv) = conn
                .open_bi()
                .await
                .with_context(|| "Could not get bi_directional channel")?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:40",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(40u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Sending ping to server {0}", server_node_id)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Sending ping to server {0}", server_node_id)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            PingHeader {
                version: 1,
                msg: MSG.to_string(),
            }
                .write_to_stream(&mut send)
                .await?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:42",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(42u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Ping sent") as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Ping sent") as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            send.finish()?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:44",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(44u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Ping finished, waiting for pong response")
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Ping finished, waiting for pong response")
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let pong = PingHeader::from_stream(&mut recv)
                .await
                .with_context(|| "Couldn't receive ping header")?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:46",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(46u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Received pong from server: {0:?}", pong.msg)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Received pong from server: {0:?}", pong.msg)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            if ::anyhow::__private::not(pong.msg == MSG) {
                return ::anyhow::__private::Err(
                    ::anyhow::Error::msg(
                        ::alloc::__export::must_use({
                            ::alloc::fmt::format(
                                format_args!(
                                    "ping/pong message mismatch: got {0:?}",
                                    pong.msg,
                                ),
                            )
                        }),
                    ),
                );
            }
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:48",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(48u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Server is reachable (pong: {0:?})", pong.msg)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Server is reachable (pong: {0:?})", pong.msg)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            Ok(())
        }
        pub async fn run_send_file(
            file_path: String,
            server_node_id_str: Option<String>,
            can_overwrite: bool,
        ) -> Result<()> {
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:53",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(53u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Client send file")
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Client send file")
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let full_path = std::fs::canonicalize(&file_path)
                .with_context(|| ::alloc::__export::must_use({
                    ::alloc::fmt::format(
                        format_args!("Failed to canonicalize path: {0}", file_path),
                    )
                }))?;
            let metadata = tokio::fs::metadata(&full_path).await?;
            let file_size = metadata.len();
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:59",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(59u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!(
                                            "{0}, {1} bytes",
                                            full_path.display(),
                                            file_size,
                                        ) as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!(
                                                                    "{0}, {1} bytes",
                                                                    full_path.display(),
                                                                    file_size,
                                                                ) as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let path = std::path::Path::new(&full_path);
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap()
                .to_string();
            let mut reader = tokio::fs::File::open(&full_path).await?;
            let raw: String = server_node_id_str
                .unwrap_or_else(|| {
                    load_node_id_from_file().expect("Could not get node-id")
                });
            let server_node_id = EndpointId::from_str(&raw)
                .with_context(|| "Could not parse server node id")?;
            let endpoint = Arc::new(
                Endpoint::builder(presets::N0)
                    .address_lookup(PkarrPublisher::n0_dns())
                    .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
                    .bind()
                    .await?,
            );
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:78",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(78u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("creating endpoint")
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("creating endpoint")
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            endpoint.online().await;
            let result = async {
                ping_server(&endpoint, server_node_id).await?;
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:83",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(83u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("connecting") as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("connecting") as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                let conn = endpoint.connect(server_node_id, FILE_ALPN_V1).await?;
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:85",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(85u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("connected to server {0}", server_node_id)
                                                as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("connected to server {0}", server_node_id)
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                let (mut iroh_send, mut iroh_recv) = conn.open_bi().await?;
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:87",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(87u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("opened bidirectional stream")
                                                as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("opened bidirectional stream")
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:88",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(88u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("sending file header")
                                                as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("sending file header")
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                let file_send_header = FileSendHeader {
                    file_name,
                    file_size,
                    version: 1,
                    can_overwrite,
                };
                file_send_header.write_to_stream(&mut iroh_send).await?;
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:91",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(91u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("sent file header, waiting for ack")
                                                as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("sent file header, waiting for ack")
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                let file_send_header_ack = Ack::read_ack(&mut iroh_recv).await?;
                if file_send_header_ack.ack != 0 {
                    return ::anyhow::__private::Err(
                        ::anyhow::Error::msg(
                            ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Server responded with error ack: {0:?}",
                                        file_send_header_ack.msg,
                                    ),
                                )
                            }),
                        ),
                    );
                }
                let bytes = tokio::io::copy(&mut reader, &mut iroh_send).await?;
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:97",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(97u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("Finished sending file: {0} bytes", bytes)
                                                as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("Finished sending file: {0} bytes", bytes)
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                iroh_send.finish()?;
                let file_send_ack = Ack::read_ack(&mut iroh_recv).await?;
                if file_send_ack.ack != 0 {
                    return ::anyhow::__private::Err(
                        ::anyhow::Error::msg(
                            ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!(
                                        "Server responded with error ack: {0:?}",
                                        file_send_ack.msg,
                                    ),
                                )
                            }),
                        ),
                    );
                }
                Ok(())
            }
                .await;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:106",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(106u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("shutting down p2p endpoint")
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("shutting down p2p endpoint")
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            endpoint.close().await;
            result
        }
        pub async fn run_tcp_client(
            listen_addr: String,
            server_node_id_str: Option<String>,
        ) -> Result<()> {
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:113",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(113u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Client mode") as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Client mode") as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let (typ, addr) = get_proxy_addr_and_type(&listen_addr);
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:116",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(116u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!(
                                            "Proxy type: {0:?}, Proxy address: {1}",
                                            typ,
                                            addr,
                                        ) as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!(
                                                                    "Proxy type: {0:?}, Proxy address: {1}",
                                                                    typ,
                                                                    addr,
                                                                ) as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            if let Some(id) = &server_node_id_str {
                write_node_id_to_file(id)?;
            }
            let raw: String = server_node_id_str
                .unwrap_or_else(|| load_node_id_from_file().unwrap());
            let server_node_id = EndpointId::from_str(&raw)
                .with_context(|| "Could not parse server node id")?;
            let endpoint = Arc::new(
                Endpoint::builder(presets::N0)
                    .address_lookup(PkarrPublisher::n0_dns())
                    .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
                    .bind()
                    .await?,
            );
            endpoint.online().await;
            let result = async {
                ping_server(&endpoint, server_node_id).await?;
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:137",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(137u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("Client NodeId: {0}", endpoint.id())
                                                as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("Client NodeId: {0}", endpoint.id())
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:138",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(138u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!(
                                                "Connecting to server NodeId: {0}",
                                                server_node_id,
                                            ) as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!(
                                                                        "Connecting to server NodeId: {0}",
                                                                        server_node_id,
                                                                    ) as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                let listener = TcpListener::bind(&addr).await?;
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client.rs:141",
                                "proxy_rs::client::client",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(141u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!(
                                                "Listening for {0:?} connections on {1}",
                                                typ,
                                                addr,
                                            ) as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!(
                                                                        "Listening for {0:?} connections on {1}",
                                                                        typ,
                                                                        addr,
                                                                    ) as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                loop {
                    {
                        #[doc(hidden)]
                        mod __tokio_select_util {
                            pub(super) enum Out<_0, _1> {
                                _0(_0),
                                _1(_1),
                                Disabled,
                            }
                            pub(super) type Mask = u8;
                        }
                        use ::tokio::macros::support::Pin;
                        const BRANCHES: u32 = 2;
                        let mut disabled: __tokio_select_util::Mask = Default::default();
                        if !true {
                            let mask: __tokio_select_util::Mask = 1 << 0;
                            disabled |= mask;
                        }
                        if !true {
                            let mask: __tokio_select_util::Mask = 1 << 1;
                            disabled |= mask;
                        }
                        let mut output = {
                            let futures_init = (
                                listener.accept(),
                                tokio::signal::ctrl_c(),
                            );
                            let mut futures = (
                                ::tokio::macros::support::IntoFuture::into_future(
                                    futures_init.0,
                                ),
                                ::tokio::macros::support::IntoFuture::into_future(
                                    futures_init.1,
                                ),
                            );
                            let mut futures = &mut futures;
                            ::tokio::macros::support::poll_fn(|cx| {
                                    match ::tokio::macros::support::poll_budget_available(cx) {
                                        ::core::task::Poll::Ready(t) => t,
                                        ::core::task::Poll::Pending => {
                                            return ::core::task::Poll::Pending;
                                        }
                                    };
                                    let mut is_pending = false;
                                    let start = {
                                        ::tokio::macros::support::thread_rng_n(BRANCHES)
                                    };
                                    for i in 0..BRANCHES {
                                        let branch;
                                        #[allow(clippy::modulo_one)]
                                        {
                                            branch = (start + i) % BRANCHES;
                                        }
                                        match branch {
                                            #[allow(unreachable_code)]
                                            0 => {
                                                let mask = 1 << branch;
                                                if disabled & mask == mask {
                                                    continue;
                                                }
                                                let (fut, ..) = &mut *futures;
                                                let mut fut = unsafe {
                                                    ::tokio::macros::support::Pin::new_unchecked(fut)
                                                };
                                                let out = match ::tokio::macros::support::Future::poll(
                                                    fut,
                                                    cx,
                                                ) {
                                                    ::tokio::macros::support::Poll::Ready(out) => out,
                                                    ::tokio::macros::support::Poll::Pending => {
                                                        is_pending = true;
                                                        continue;
                                                    }
                                                };
                                                disabled |= mask;
                                                #[allow(unused_variables)] #[allow(unused_mut)]
                                                match &out {
                                                    result => {}
                                                    _ => continue,
                                                }
                                                return ::tokio::macros::support::Poll::Ready(
                                                    __tokio_select_util::Out::_0(out),
                                                );
                                            }
                                            #[allow(unreachable_code)]
                                            1 => {
                                                let mask = 1 << branch;
                                                if disabled & mask == mask {
                                                    continue;
                                                }
                                                let (_, fut, ..) = &mut *futures;
                                                let mut fut = unsafe {
                                                    ::tokio::macros::support::Pin::new_unchecked(fut)
                                                };
                                                let out = match ::tokio::macros::support::Future::poll(
                                                    fut,
                                                    cx,
                                                ) {
                                                    ::tokio::macros::support::Poll::Ready(out) => out,
                                                    ::tokio::macros::support::Poll::Pending => {
                                                        is_pending = true;
                                                        continue;
                                                    }
                                                };
                                                disabled |= mask;
                                                #[allow(unused_variables)] #[allow(unused_mut)]
                                                match &out {
                                                    _ => {}
                                                    _ => continue,
                                                }
                                                return ::tokio::macros::support::Poll::Ready(
                                                    __tokio_select_util::Out::_1(out),
                                                );
                                            }
                                            _ => {
                                                ::core::panicking::panic_fmt(
                                                    format_args!(
                                                        "internal error: entered unreachable code: {0}",
                                                        format_args!(
                                                            "reaching this means there probably is an off by one bug",
                                                        ),
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                    if is_pending {
                                        ::tokio::macros::support::Poll::Pending
                                    } else {
                                        ::tokio::macros::support::Poll::Ready(
                                            __tokio_select_util::Out::Disabled,
                                        )
                                    }
                                })
                                .await
                        };
                        match output {
                            __tokio_select_util::Out::_0(result) => {
                                let (tcp_stream, peer_addr) = result?;
                                {
                                    use ::tracing::__macro_support::Callsite as _;
                                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                                        static META: ::tracing::Metadata<'static> = {
                                            ::tracing_core::metadata::Metadata::new(
                                                "event src/client/client.rs:147",
                                                "proxy_rs::client::client",
                                                ::tracing::Level::INFO,
                                                ::tracing_core::__macro_support::Option::Some(
                                                    "src/client/client.rs",
                                                ),
                                                ::tracing_core::__macro_support::Option::Some(147u32),
                                                ::tracing_core::__macro_support::Option::Some(
                                                    "proxy_rs::client::client",
                                                ),
                                                ::tracing_core::field::FieldSet::new(
                                                    &["message"],
                                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                                ),
                                                ::tracing::metadata::Kind::EVENT,
                                            )
                                        };
                                        ::tracing::callsite::DefaultCallsite::new(&META)
                                    };
                                    let enabled = ::tracing::Level::INFO
                                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                                        && ::tracing::Level::INFO
                                            <= ::tracing::level_filters::LevelFilter::current()
                                        && {
                                            let interest = __CALLSITE.interest();
                                            !interest.is_never()
                                                && ::tracing::__macro_support::__is_enabled(
                                                    __CALLSITE.metadata(),
                                                    interest,
                                                )
                                        };
                                    if enabled {
                                        (|value_set: ::tracing::field::ValueSet| {
                                            let meta = __CALLSITE.metadata();
                                            ::tracing::Event::dispatch(meta, &value_set);
                                            if match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            } <= ::tracing::log::STATIC_MAX_LEVEL
                                            {
                                                if !::tracing::dispatcher::has_been_set() {
                                                    {
                                                        use ::tracing::log;
                                                        let level = match ::tracing::Level::INFO {
                                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                            _ => ::tracing::log::Level::Trace,
                                                        };
                                                        if level <= log::max_level() {
                                                            let meta = __CALLSITE.metadata();
                                                            let log_meta = log::Metadata::builder()
                                                                .level(level)
                                                                .target(meta.target())
                                                                .build();
                                                            let logger = log::logger();
                                                            if logger.enabled(&log_meta) {
                                                                ::tracing::__macro_support::__tracing_log(
                                                                    meta,
                                                                    logger,
                                                                    log_meta,
                                                                    &value_set,
                                                                )
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    {}
                                                }
                                            } else {
                                                {}
                                            };
                                        })({
                                            #[allow(unused_imports)]
                                            use ::tracing::field::{debug, display, Value};
                                            __CALLSITE
                                                .metadata()
                                                .fields()
                                                .value_set_all(
                                                    &[
                                                        (::tracing::__macro_support::Option::Some(
                                                            &format_args!(
                                                                "Accepted {0:?} connection from {1}",
                                                                typ,
                                                                peer_addr,
                                                            ) as &dyn ::tracing::field::Value,
                                                        )),
                                                    ],
                                                )
                                        });
                                    } else {
                                        if match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        } <= ::tracing::log::STATIC_MAX_LEVEL
                                        {
                                            if !::tracing::dispatcher::has_been_set() {
                                                {
                                                    use ::tracing::log;
                                                    let level = match ::tracing::Level::INFO {
                                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                        _ => ::tracing::log::Level::Trace,
                                                    };
                                                    if level <= log::max_level() {
                                                        let meta = __CALLSITE.metadata();
                                                        let log_meta = log::Metadata::builder()
                                                            .level(level)
                                                            .target(meta.target())
                                                            .build();
                                                        let logger = log::logger();
                                                        if logger.enabled(&log_meta) {
                                                            ::tracing::__macro_support::__tracing_log(
                                                                meta,
                                                                logger,
                                                                log_meta,
                                                                &{
                                                                    #[allow(unused_imports)]
                                                                    use ::tracing::field::{debug, display, Value};
                                                                    __CALLSITE
                                                                        .metadata()
                                                                        .fields()
                                                                        .value_set_all(
                                                                            &[
                                                                                (::tracing::__macro_support::Option::Some(
                                                                                    &format_args!(
                                                                                        "Accepted {0:?} connection from {1}",
                                                                                        typ,
                                                                                        peer_addr,
                                                                                    ) as &dyn ::tracing::field::Value,
                                                                                )),
                                                                            ],
                                                                        )
                                                                },
                                                            )
                                                        }
                                                    }
                                                }
                                            } else {
                                                {}
                                            }
                                        } else {
                                            {}
                                        };
                                    }
                                };
                                let ep = endpoint.clone();
                                tokio::spawn(async move {
                                    let result = match typ {
                                        ProxyType::Socks5 => {
                                            handle_socks5(tcp_stream, ep, server_node_id).await
                                        }
                                        ProxyType::Http => {
                                            handle_http(tcp_stream, ep, server_node_id).await
                                        }
                                    };
                                    if let Err(e) = result {
                                        {
                                            use ::tracing::__macro_support::Callsite as _;
                                            static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                                                static META: ::tracing::Metadata<'static> = {
                                                    ::tracing_core::metadata::Metadata::new(
                                                        "event src/client/client.rs:156",
                                                        "proxy_rs::client::client",
                                                        ::tracing::Level::ERROR,
                                                        ::tracing_core::__macro_support::Option::Some(
                                                            "src/client/client.rs",
                                                        ),
                                                        ::tracing_core::__macro_support::Option::Some(156u32),
                                                        ::tracing_core::__macro_support::Option::Some(
                                                            "proxy_rs::client::client",
                                                        ),
                                                        ::tracing_core::field::FieldSet::new(
                                                            &["message"],
                                                            ::tracing_core::callsite::Identifier(&__CALLSITE),
                                                        ),
                                                        ::tracing::metadata::Kind::EVENT,
                                                    )
                                                };
                                                ::tracing::callsite::DefaultCallsite::new(&META)
                                            };
                                            let enabled = ::tracing::Level::ERROR
                                                <= ::tracing::level_filters::STATIC_MAX_LEVEL
                                                && ::tracing::Level::ERROR
                                                    <= ::tracing::level_filters::LevelFilter::current()
                                                && {
                                                    let interest = __CALLSITE.interest();
                                                    !interest.is_never()
                                                        && ::tracing::__macro_support::__is_enabled(
                                                            __CALLSITE.metadata(),
                                                            interest,
                                                        )
                                                };
                                            if enabled {
                                                (|value_set: ::tracing::field::ValueSet| {
                                                    let meta = __CALLSITE.metadata();
                                                    ::tracing::Event::dispatch(meta, &value_set);
                                                    if match ::tracing::Level::ERROR {
                                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                        _ => ::tracing::log::Level::Trace,
                                                    } <= ::tracing::log::STATIC_MAX_LEVEL
                                                    {
                                                        if !::tracing::dispatcher::has_been_set() {
                                                            {
                                                                use ::tracing::log;
                                                                let level = match ::tracing::Level::ERROR {
                                                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                                    _ => ::tracing::log::Level::Trace,
                                                                };
                                                                if level <= log::max_level() {
                                                                    let meta = __CALLSITE.metadata();
                                                                    let log_meta = log::Metadata::builder()
                                                                        .level(level)
                                                                        .target(meta.target())
                                                                        .build();
                                                                    let logger = log::logger();
                                                                    if logger.enabled(&log_meta) {
                                                                        ::tracing::__macro_support::__tracing_log(
                                                                            meta,
                                                                            logger,
                                                                            log_meta,
                                                                            &value_set,
                                                                        )
                                                                    }
                                                                }
                                                            }
                                                        } else {
                                                            {}
                                                        }
                                                    } else {
                                                        {}
                                                    };
                                                })({
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("Proxy error from {0}: {1:#}", peer_addr, e)
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                });
                                            } else {
                                                if match ::tracing::Level::ERROR {
                                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                    _ => ::tracing::log::Level::Trace,
                                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                                {
                                                    if !::tracing::dispatcher::has_been_set() {
                                                        {
                                                            use ::tracing::log;
                                                            let level = match ::tracing::Level::ERROR {
                                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                                _ => ::tracing::log::Level::Trace,
                                                            };
                                                            if level <= log::max_level() {
                                                                let meta = __CALLSITE.metadata();
                                                                let log_meta = log::Metadata::builder()
                                                                    .level(level)
                                                                    .target(meta.target())
                                                                    .build();
                                                                let logger = log::logger();
                                                                if logger.enabled(&log_meta) {
                                                                    ::tracing::__macro_support::__tracing_log(
                                                                        meta,
                                                                        logger,
                                                                        log_meta,
                                                                        &{
                                                                            #[allow(unused_imports)]
                                                                            use ::tracing::field::{debug, display, Value};
                                                                            __CALLSITE
                                                                                .metadata()
                                                                                .fields()
                                                                                .value_set_all(
                                                                                    &[
                                                                                        (::tracing::__macro_support::Option::Some(
                                                                                            &format_args!("Proxy error from {0}: {1:#}", peer_addr, e)
                                                                                                as &dyn ::tracing::field::Value,
                                                                                        )),
                                                                                    ],
                                                                                )
                                                                        },
                                                                    )
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        {}
                                                    }
                                                } else {
                                                    {}
                                                };
                                            }
                                        };
                                    }
                                });
                            }
                            __tokio_select_util::Out::_1(_) => {
                                {
                                    use ::tracing::__macro_support::Callsite as _;
                                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                                        static META: ::tracing::Metadata<'static> = {
                                            ::tracing_core::metadata::Metadata::new(
                                                "event src/client/client.rs:161",
                                                "proxy_rs::client::client",
                                                ::tracing::Level::INFO,
                                                ::tracing_core::__macro_support::Option::Some(
                                                    "src/client/client.rs",
                                                ),
                                                ::tracing_core::__macro_support::Option::Some(161u32),
                                                ::tracing_core::__macro_support::Option::Some(
                                                    "proxy_rs::client::client",
                                                ),
                                                ::tracing_core::field::FieldSet::new(
                                                    &["message"],
                                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                                ),
                                                ::tracing::metadata::Kind::EVENT,
                                            )
                                        };
                                        ::tracing::callsite::DefaultCallsite::new(&META)
                                    };
                                    let enabled = ::tracing::Level::INFO
                                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                                        && ::tracing::Level::INFO
                                            <= ::tracing::level_filters::LevelFilter::current()
                                        && {
                                            let interest = __CALLSITE.interest();
                                            !interest.is_never()
                                                && ::tracing::__macro_support::__is_enabled(
                                                    __CALLSITE.metadata(),
                                                    interest,
                                                )
                                        };
                                    if enabled {
                                        (|value_set: ::tracing::field::ValueSet| {
                                            let meta = __CALLSITE.metadata();
                                            ::tracing::Event::dispatch(meta, &value_set);
                                            if match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            } <= ::tracing::log::STATIC_MAX_LEVEL
                                            {
                                                if !::tracing::dispatcher::has_been_set() {
                                                    {
                                                        use ::tracing::log;
                                                        let level = match ::tracing::Level::INFO {
                                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                            _ => ::tracing::log::Level::Trace,
                                                        };
                                                        if level <= log::max_level() {
                                                            let meta = __CALLSITE.metadata();
                                                            let log_meta = log::Metadata::builder()
                                                                .level(level)
                                                                .target(meta.target())
                                                                .build();
                                                            let logger = log::logger();
                                                            if logger.enabled(&log_meta) {
                                                                ::tracing::__macro_support::__tracing_log(
                                                                    meta,
                                                                    logger,
                                                                    log_meta,
                                                                    &value_set,
                                                                )
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    {}
                                                }
                                            } else {
                                                {}
                                            };
                                        })({
                                            #[allow(unused_imports)]
                                            use ::tracing::field::{debug, display, Value};
                                            __CALLSITE
                                                .metadata()
                                                .fields()
                                                .value_set_all(
                                                    &[
                                                        (::tracing::__macro_support::Option::Some(
                                                            &format_args!("Shutting down client")
                                                                as &dyn ::tracing::field::Value,
                                                        )),
                                                    ],
                                                )
                                        });
                                    } else {
                                        if match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        } <= ::tracing::log::STATIC_MAX_LEVEL
                                        {
                                            if !::tracing::dispatcher::has_been_set() {
                                                {
                                                    use ::tracing::log;
                                                    let level = match ::tracing::Level::INFO {
                                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                        _ => ::tracing::log::Level::Trace,
                                                    };
                                                    if level <= log::max_level() {
                                                        let meta = __CALLSITE.metadata();
                                                        let log_meta = log::Metadata::builder()
                                                            .level(level)
                                                            .target(meta.target())
                                                            .build();
                                                        let logger = log::logger();
                                                        if logger.enabled(&log_meta) {
                                                            ::tracing::__macro_support::__tracing_log(
                                                                meta,
                                                                logger,
                                                                log_meta,
                                                                &{
                                                                    #[allow(unused_imports)]
                                                                    use ::tracing::field::{debug, display, Value};
                                                                    __CALLSITE
                                                                        .metadata()
                                                                        .fields()
                                                                        .value_set_all(
                                                                            &[
                                                                                (::tracing::__macro_support::Option::Some(
                                                                                    &format_args!("Shutting down client")
                                                                                        as &dyn ::tracing::field::Value,
                                                                                )),
                                                                            ],
                                                                        )
                                                                },
                                                            )
                                                        }
                                                    }
                                                }
                                            } else {
                                                {}
                                            }
                                        } else {
                                            {}
                                        };
                                    }
                                };
                                break;
                            }
                            __tokio_select_util::Out::Disabled => {
                                ::core::panicking::panic_fmt(
                                    format_args!(
                                        "all branches are disabled and there is no else branch",
                                    ),
                                );
                            }
                            _ => {
                                ::core::panicking::panic_fmt(
                                    format_args!(
                                        "internal error: entered unreachable code: {0}",
                                        format_args!("failed to match bind"),
                                    ),
                                );
                            }
                        }
                    }
                }
                Ok(())
            }
                .await;
            endpoint.close().await;
            result
        }
        async fn handle_http(
            mut tcp: TcpStream,
            endpoint: Arc<Endpoint>,
            server_node_id: EndpointId,
        ) -> Result<()> {
            let (host, port, preamble) = http::handshake(&mut tcp).await?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:179",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(179u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("HTTP proxy -> {0}:{1}", host, port)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("HTTP proxy -> {0}:{1}", host, port)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let conn = endpoint.connect(server_node_id, TCP_PROXY_ALPN_V1).await?;
            let (mut iroh_send, iroh_recv) = conn.open_bi().await?;
            let proxy_header = ProxyHeader {
                version: 1,
                host: host.clone(),
                port,
                can_read: true,
                can_write: false,
                can_execute: false,
            };
            proxy_header.write_to_stream(&mut iroh_send).await?;
            if !preamble.is_empty() {
                iroh_send.write_all(&preamble).await?;
            }
            let (tcp_read, tcp_write) = tcp.into_split();
            proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:194",
                            "proxy_rs::client::client",
                            ::tracing::Level::WARN,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(194u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::WARN
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::WARN
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::WARN {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::WARN {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!(
                                            "HTTP proxy connection to {0}:{1} via iroh server closed",
                                            host,
                                            port,
                                        ) as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::WARN {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::WARN {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!(
                                                                    "HTTP proxy connection to {0}:{1} via iroh server closed",
                                                                    host,
                                                                    port,
                                                                ) as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            Ok(())
        }
        async fn handle_socks5(
            mut tcp: TcpStream,
            endpoint: Arc<Endpoint>,
            server_node_id: EndpointId,
        ) -> Result<()> {
            let (host, port) = socks5::handshake(&mut tcp).await?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:204",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(204u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("SOCKS5 CONNECT -> {0}:{1}", host, port)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("SOCKS5 CONNECT -> {0}:{1}", host, port)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:206",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(206u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!(
                                            "Connecting to iroh server {0}",
                                            server_node_id,
                                        ) as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!(
                                                                    "Connecting to iroh server {0}",
                                                                    server_node_id,
                                                                ) as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let conn = endpoint.connect(server_node_id, TCP_PROXY_ALPN_V1).await?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:208",
                            "proxy_rs::client::client",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(208u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Connected.") as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Connected.") as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            let (mut iroh_send, iroh_recv) = conn.open_bi().await?;
            let proxy_header = ProxyHeader {
                version: 1,
                host,
                port,
                can_read: true,
                can_write: false,
                can_execute: false,
            };
            proxy_header.write_to_stream(&mut iroh_send).await?;
            let (tcp_read, tcp_write) = tcp.into_split();
            proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write).await?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client.rs:218",
                            "proxy_rs::client::client",
                            ::tracing::Level::WARN,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(218u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::WARN
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::WARN
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::WARN {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::WARN {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!(
                                            "Connection to {0}:{1} via iroh server closed",
                                            proxy_header.host,
                                            proxy_header.port,
                                        ) as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::WARN {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::WARN {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!(
                                                                    "Connection to {0}:{1} via iroh server closed",
                                                                    proxy_header.host,
                                                                    proxy_header.port,
                                                                ) as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            Ok(())
        }
    }
    pub mod client_helpers {
        use std::path::Path;
        use anyhow::{Context, Result};
        use dialoguer::Select;
        use tracing::info;
        const PATH: &str = ".node-id";
        pub fn load_node_id_from_file() -> Result<String> {
            let ids = load_node_ids()?;
            match ids.len() {
                0 => {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!(
                                "No saved server node IDs. Pass one with -n on first run.",
                            ),
                        );
                        error
                    });
                }
                1 => Ok(ids.into_iter().next().unwrap()),
                _ => {
                    let selection = Select::new()
                        .with_prompt("Select server node ID")
                        .items(&ids)
                        .default(0)
                        .interact()
                        .with_context(|| "Failed to get user selection")?;
                    Ok(ids[selection].clone())
                }
            }
        }
        pub fn write_node_id_to_file(id: &str) -> Result<()> {
            let mut ids = load_node_ids().unwrap_or_default();
            if ids.iter().any(|existing| existing == id) {
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event src/client/client_helpers.rs:29",
                                "proxy_rs::client::client_helpers",
                                ::tracing::Level::INFO,
                                ::tracing_core::__macro_support::Option::Some(
                                    "src/client/client_helpers.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(29u32),
                                ::tracing_core::__macro_support::Option::Some(
                                    "proxy_rs::client::client_helpers",
                                ),
                                ::tracing_core::field::FieldSet::new(
                                    &["message"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = ::tracing::Level::INFO
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && ::tracing::Level::INFO
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &value_set,
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set_all(
                                    &[
                                        (::tracing::__macro_support::Option::Some(
                                            &format_args!("Server node id already saved in {0}", PATH)
                                                as &dyn ::tracing::field::Value,
                                        )),
                                    ],
                                )
                        });
                    } else {
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &{
                                                    #[allow(unused_imports)]
                                                    use ::tracing::field::{debug, display, Value};
                                                    __CALLSITE
                                                        .metadata()
                                                        .fields()
                                                        .value_set_all(
                                                            &[
                                                                (::tracing::__macro_support::Option::Some(
                                                                    &format_args!("Server node id already saved in {0}", PATH)
                                                                        as &dyn ::tracing::field::Value,
                                                                )),
                                                            ],
                                                        )
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    }
                };
                return Ok(());
            }
            ids.push(id.to_string());
            std::fs::write(PATH, ids.join("\n"))
                .with_context(|| ::alloc::__export::must_use({
                    ::alloc::fmt::format(
                        format_args!("failed to write node id file: {0}", PATH),
                    )
                }))?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/client/client_helpers.rs:35",
                            "proxy_rs::client::client_helpers",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/client/client_helpers.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(35u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::client::client_helpers",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Saved server node id to {0}", PATH)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Saved server node id to {0}", PATH)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            Ok(())
        }
        fn load_node_ids() -> Result<Vec<String>> {
            if !Path::new(PATH).exists() {
                return Ok(::alloc::vec::Vec::new());
            }
            let content = std::fs::read_to_string(PATH)
                .with_context(|| ::alloc::__export::must_use({
                    ::alloc::fmt::format(
                        format_args!("failed to read node id file: {0}", PATH),
                    )
                }))?;
            Ok(
                content
                    .lines()
                    .map(str::trim)
                    .filter(|&s| !s.is_empty())
                    .map(String::from)
                    .collect(),
            )
        }
    }
}
mod http {
    use anyhow::{bail, Result};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    /// Perform an HTTP proxy handshake.
    ///
    /// Returns `(host, port, preamble)` where `preamble` is any data that must be
    /// forwarded to the upstream target before streaming the rest of the connection.
    /// For `CONNECT` (HTTPS) this is empty; for plain HTTP it is the original
    /// request headers so the upstream server receives a well-formed request.
    pub async fn handshake(stream: &mut TcpStream) -> Result<(String, u16, Vec<u8>)> {
        let headers = read_headers(stream).await?;
        let headers_str = std::str::from_utf8(&headers)?;
        let first_line = headers_str
            .split_once("\r\n")
            .map(|(l, _)| l)
            .ok_or_else(|| ::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(
                    format_args!("empty HTTP request"),
                );
                error
            }))?;
        let mut parts = first_line.splitn(3, ' ');
        let method = parts
            .next()
            .ok_or_else(|| ::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(
                    format_args!("missing HTTP method"),
                );
                error
            }))?;
        let target = parts
            .next()
            .ok_or_else(|| ::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(
                    format_args!("missing HTTP target"),
                );
                error
            }))?;
        if method == "CONNECT" {
            let (host, port_str) = target
                .rsplit_once(':')
                .ok_or_else(|| ::anyhow::__private::must_use({
                    let error = ::anyhow::__private::format_err(
                        format_args!("invalid CONNECT target: {0}", target),
                    );
                    error
                }))?;
            let port: u16 = port_str.parse()?;
            stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
            Ok((host.to_string(), port, ::alloc::vec::Vec::new()))
        } else {
            let without_scheme = target
                .strip_prefix("http://")
                .ok_or_else(|| ::anyhow::__private::must_use({
                    let error = ::anyhow::__private::format_err(
                        format_args!("expected http:// URL, got: {0}", target),
                    );
                    error
                }))?;
            let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
            let (host, port) = if let Some((h, p)) = authority.rsplit_once(':') {
                (h.to_string(), p.parse::<u16>()?)
            } else {
                (authority.to_string(), 80u16)
            };
            Ok((host, port, headers))
        }
    }
    /// Read bytes from `stream` until the end of the HTTP headers (`\r\n\r\n`).
    async fn read_headers(stream: &mut TcpStream) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(1024);
        let mut tmp = [0u8; 1];
        loop {
            stream.read_exact(&mut tmp).await?;
            buf.push(tmp[0]);
            if buf.ends_with(b"\r\n\r\n") {
                return Ok(buf);
            }
            if buf.len() > 64 * 1024 {
                return ::anyhow::__private::Err({
                    let error = ::anyhow::__private::format_err(
                        format_args!("HTTP headers exceed 64 KiB"),
                    );
                    error
                });
            }
        }
    }
}
mod protocols {
    pub mod codec {
        use anyhow::Result;
        use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
        pub use proxy_rs_derive::StreamCodec;
        pub trait StreamCodec: Sized {
            async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()>;
            async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self>;
        }
        impl StreamCodec for u8 {
            async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
                Ok(w.write_u8(*self).await?)
            }
            async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
                Ok(r.read_u8().await?)
            }
        }
        impl StreamCodec for u16 {
            async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
                Ok(w.write_u16(*self).await?)
            }
            async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
                Ok(r.read_u16().await?)
            }
        }
        impl StreamCodec for u64 {
            async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
                Ok(w.write_u64(*self).await?)
            }
            async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
                Ok(r.read_u64().await?)
            }
        }
        impl StreamCodec for String {
            async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> Result<()> {
                let bytes = self.as_bytes();
                w.write_u16(bytes.len() as u16).await?;
                Ok(w.write_all(bytes).await?)
            }
            async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> Result<Self> {
                let len = r.read_u16().await? as usize;
                let mut bytes = ::alloc::vec::from_elem(0u8, len);
                r.read_exact(&mut bytes).await?;
                Ok(String::from_utf8(bytes)?)
            }
        }
    }
    pub mod proxy {
        pub mod proxy_helpers {
            use anyhow::Result;
            use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
            use crate::protocols::proxy::proxy_header::ProxyHeader;
            const FLAG_CAN_READ: u8 = 0b0000_0001;
            const FLAG_CAN_WRITE: u8 = 0b0000_0010;
            const FLAG_CAN_EXECUTE: u8 = 0b0000_0100;
            /// Read a target host and port written by `write_proxy_header`.
            pub async fn read_proxy_header<R: AsyncRead + Unpin>(
                stream: &mut R,
            ) -> Result<ProxyHeader> {
                let version = stream.read_u8().await?;
                match version {
                    1 => read_target_v1(stream).await,
                    _ => {
                        return ::anyhow::__private::Err({
                            let error = ::anyhow::__private::format_err(
                                format_args!(
                                    "unsupported proxy protocol version: {0}",
                                    version,
                                ),
                            );
                            error
                        });
                    }
                }
            }
            pub async fn read_target_v1<R: AsyncRead + Unpin>(
                stream: &mut R,
            ) -> Result<ProxyHeader> {
                {
                    ::std::io::_print(
                        format_args!("Reading proxy header (version 1)\n"),
                    );
                };
                let host_len = stream.read_u16().await? as usize;
                let mut host_bytes = ::alloc::vec::from_elem(0u8, host_len);
                stream.read_exact(&mut host_bytes).await?;
                let host = String::from_utf8(host_bytes)?;
                let port = stream.read_u16().await?;
                let flags = stream.read_u8().await?;
                Ok(ProxyHeader {
                    host,
                    port,
                    version: 1,
                    can_read: (flags & FLAG_CAN_READ) != 0,
                    can_write: (flags & FLAG_CAN_WRITE) != 0,
                    can_execute: (flags & FLAG_CAN_EXECUTE) != 0,
                })
            }
            /// Bidirectionally proxy between two async read/write halves.
            /// Returns when either direction closes or errors.
            pub async fn proxy_streams<A, B, C, D>(
                mut a_read: A,
                mut a_write: B,
                mut b_read: C,
                mut b_write: D,
            ) -> Result<()>
            where
                A: AsyncRead + Unpin + Send + 'static,
                B: AsyncWrite + Unpin + Send + 'static,
                C: AsyncRead + Unpin + Send + 'static,
                D: AsyncWrite + Unpin + Send + 'static,
            {
                let a_to_b = tokio::io::copy(&mut a_read, &mut b_write);
                let b_to_a = tokio::io::copy(&mut b_read, &mut a_write);
                {
                    #[doc(hidden)]
                    mod __tokio_select_util {
                        pub(super) enum Out<_0, _1> {
                            _0(_0),
                            _1(_1),
                            Disabled,
                        }
                        pub(super) type Mask = u8;
                    }
                    use ::tokio::macros::support::Pin;
                    const BRANCHES: u32 = 2;
                    let mut disabled: __tokio_select_util::Mask = Default::default();
                    if !true {
                        let mask: __tokio_select_util::Mask = 1 << 0;
                        disabled |= mask;
                    }
                    if !true {
                        let mask: __tokio_select_util::Mask = 1 << 1;
                        disabled |= mask;
                    }
                    let mut output = {
                        let futures_init = (a_to_b, b_to_a);
                        let mut futures = (
                            ::tokio::macros::support::IntoFuture::into_future(
                                futures_init.0,
                            ),
                            ::tokio::macros::support::IntoFuture::into_future(
                                futures_init.1,
                            ),
                        );
                        let mut futures = &mut futures;
                        ::tokio::macros::support::poll_fn(|cx| {
                                match ::tokio::macros::support::poll_budget_available(cx) {
                                    ::core::task::Poll::Ready(t) => t,
                                    ::core::task::Poll::Pending => {
                                        return ::core::task::Poll::Pending;
                                    }
                                };
                                let mut is_pending = false;
                                let start = {
                                    ::tokio::macros::support::thread_rng_n(BRANCHES)
                                };
                                for i in 0..BRANCHES {
                                    let branch;
                                    #[allow(clippy::modulo_one)]
                                    {
                                        branch = (start + i) % BRANCHES;
                                    }
                                    match branch {
                                        #[allow(unreachable_code)]
                                        0 => {
                                            let mask = 1 << branch;
                                            if disabled & mask == mask {
                                                continue;
                                            }
                                            let (fut, ..) = &mut *futures;
                                            let mut fut = unsafe {
                                                ::tokio::macros::support::Pin::new_unchecked(fut)
                                            };
                                            let out = match ::tokio::macros::support::Future::poll(
                                                fut,
                                                cx,
                                            ) {
                                                ::tokio::macros::support::Poll::Ready(out) => out,
                                                ::tokio::macros::support::Poll::Pending => {
                                                    is_pending = true;
                                                    continue;
                                                }
                                            };
                                            disabled |= mask;
                                            #[allow(unused_variables)] #[allow(unused_mut)]
                                            match &out {
                                                result => {}
                                                _ => continue,
                                            }
                                            return ::tokio::macros::support::Poll::Ready(
                                                __tokio_select_util::Out::_0(out),
                                            );
                                        }
                                        #[allow(unreachable_code)]
                                        1 => {
                                            let mask = 1 << branch;
                                            if disabled & mask == mask {
                                                continue;
                                            }
                                            let (_, fut, ..) = &mut *futures;
                                            let mut fut = unsafe {
                                                ::tokio::macros::support::Pin::new_unchecked(fut)
                                            };
                                            let out = match ::tokio::macros::support::Future::poll(
                                                fut,
                                                cx,
                                            ) {
                                                ::tokio::macros::support::Poll::Ready(out) => out,
                                                ::tokio::macros::support::Poll::Pending => {
                                                    is_pending = true;
                                                    continue;
                                                }
                                            };
                                            disabled |= mask;
                                            #[allow(unused_variables)] #[allow(unused_mut)]
                                            match &out {
                                                result => {}
                                                _ => continue,
                                            }
                                            return ::tokio::macros::support::Poll::Ready(
                                                __tokio_select_util::Out::_1(out),
                                            );
                                        }
                                        _ => {
                                            ::core::panicking::panic_fmt(
                                                format_args!(
                                                    "internal error: entered unreachable code: {0}",
                                                    format_args!(
                                                        "reaching this means there probably is an off by one bug",
                                                    ),
                                                ),
                                            );
                                        }
                                    }
                                }
                                if is_pending {
                                    ::tokio::macros::support::Poll::Pending
                                } else {
                                    ::tokio::macros::support::Poll::Ready(
                                        __tokio_select_util::Out::Disabled,
                                    )
                                }
                            })
                            .await
                    };
                    match output {
                        __tokio_select_util::Out::_0(result) => {
                            result?;
                        }
                        __tokio_select_util::Out::_1(result) => {
                            result?;
                        }
                        __tokio_select_util::Out::Disabled => {
                            ::core::panicking::panic_fmt(
                                format_args!(
                                    "all branches are disabled and there is no else branch",
                                ),
                            );
                        }
                        _ => {
                            ::core::panicking::panic_fmt(
                                format_args!(
                                    "internal error: entered unreachable code: {0}",
                                    format_args!("failed to match bind"),
                                ),
                            );
                        }
                    }
                }
                Ok(())
            }
        }
        pub mod proxy_protocol_handler {
            use anyhow::Context;
            use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
            use tokio::net::TcpStream;
            use tracing::{info, warn};
            use crate::protocols::proxy::proxy_helpers::{
                proxy_streams, read_proxy_header,
            };
            pub struct ProxyServerProtocolV1;
            #[automatically_derived]
            impl ::core::fmt::Debug for ProxyServerProtocolV1 {
                #[inline]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    ::core::fmt::Formatter::write_str(f, "ProxyServerProtocolV1")
                }
            }
            #[automatically_derived]
            impl ::core::clone::Clone for ProxyServerProtocolV1 {
                #[inline]
                fn clone(&self) -> ProxyServerProtocolV1 {
                    ProxyServerProtocolV1
                }
            }
            impl ProtocolHandler for ProxyServerProtocolV1 {
                async fn accept(
                    &self,
                    connection: Connection,
                ) -> anyhow::Result<(), AcceptError> {
                    let alpn = String::from_utf8(connection.alpn().to_vec()).unwrap();
                    let (iroh_send, mut iroh_recv) = connection.accept_bi().await?;
                    let proxy_header = read_proxy_header(&mut iroh_recv)
                        .await
                        .with_context(|| "")
                        .unwrap();
                    let tcp_target = ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!("{0}:{1}", proxy_header.host, proxy_header.port),
                        )
                    });
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/proxy/proxy_protocol_handler.rs:18",
                                    "proxy_rs::protocols::proxy::proxy_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/proxy/proxy_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(18u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::proxy::proxy_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("Connecting to TCP target {0}", tcp_target)
                                                    as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("Connecting to TCP target {0}", tcp_target)
                                                                            as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    let tcp = TcpStream::connect(&tcp_target).await?;
                    let (tcp_read, tcp_write) = tcp.into_split();
                    proxy_streams(iroh_recv, iroh_send, tcp_read, tcp_write)
                        .await
                        .unwrap_or_default();
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/proxy/proxy_protocol_handler.rs:25",
                                    "proxy_rs::protocols::proxy::proxy_protocol_handler",
                                    ::tracing::Level::WARN,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/proxy/proxy_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(25u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::proxy::proxy_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::WARN
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::WARN
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::WARN {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::WARN {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!(
                                                    "Connection from {0} to {1} closed",
                                                    alpn,
                                                    tcp_target,
                                                ) as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::WARN {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::WARN {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!(
                                                                            "Connection from {0} to {1} closed",
                                                                            alpn,
                                                                            tcp_target,
                                                                        ) as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    Ok(())
                }
            }
        }
        pub mod alpn {
            pub const TCP_PROXY_ALPN_V1: &[u8] = b"proxy-rs/tcp-proxy/1";
        }
        pub mod proxy_header {
            use tokio::io::{AsyncWrite, AsyncWriteExt};
            pub struct ProxyHeader {
                pub version: u8,
                pub host: String,
                pub port: u16,
                pub can_read: bool,
                pub can_write: bool,
                pub can_execute: bool,
            }
            impl ProxyHeader {
                pub async fn write_to_stream<T>(
                    &self,
                    stream: &mut T,
                ) -> anyhow::Result<()>
                where
                    T: AsyncWrite + Unpin,
                {
                    stream.write_u8(self.version).await?;
                    let host_bytes = self.host.as_bytes();
                    stream.write_u16(host_bytes.len() as u16).await?;
                    stream.write_all(host_bytes).await?;
                    stream.write_u16(self.port).await?;
                    stream
                        .write_u8(
                            pack_bits([
                                self.can_read,
                                self.can_write,
                                self.can_execute,
                                false,
                                false,
                                false,
                                false,
                                false,
                            ]),
                        )
                        .await?;
                    Ok(())
                }
            }
            fn pack_bits(bits: [bool; 8]) -> u8 {
                let mut byte = 0;
                for i in 0..8 {
                    if bits[i] {
                        byte |= 1 << i;
                    }
                }
                byte
            }
        }
    }
    pub mod file_send {
        pub mod file_send_protocol_handler {
            use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
            use tokio::io::AsyncWriteExt;
            use tracing::info;
            use crate::server::copy_bytes;
            use crate::protocols::{
                ack::Ack, file_send::file_send_header::FileSendHeader,
            };
            pub struct FileServerProtocolV1;
            #[automatically_derived]
            impl ::core::fmt::Debug for FileServerProtocolV1 {
                #[inline]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    ::core::fmt::Formatter::write_str(f, "FileServerProtocolV1")
                }
            }
            #[automatically_derived]
            impl ::core::clone::Clone for FileServerProtocolV1 {
                #[inline]
                fn clone(&self) -> FileServerProtocolV1 {
                    FileServerProtocolV1
                }
            }
            impl ProtocolHandler for FileServerProtocolV1 {
                async fn accept(
                    &self,
                    connection: Connection,
                ) -> Result<(), AcceptError> {
                    self.handle(connection)
                        .await
                        .map_err(|e| AcceptError::from_boxed(e.into()))
                }
            }
            impl FileServerProtocolV1 {
                async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/file_send/file_send_protocol_handler.rs:20",
                                    "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/file_send/file_send_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(20u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!(
                                                    "Accepted file_send connection from {0}",
                                                    connection.remote_id(),
                                                ) as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!(
                                                                            "Accepted file_send connection from {0}",
                                                                            connection.remote_id(),
                                                                        ) as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    let _alpn = String::from_utf8(connection.alpn().to_vec())?;
                    let (mut iroh_send, mut iroh_recv) = connection.accept_bi().await?;
                    let file_send_header = FileSendHeader::from_stream(&mut iroh_recv)
                        .await?;
                    match &file_send_header {
                        tmp => {
                            {
                                ::std::io::_eprint(
                                    format_args!(
                                        "[{0}:{1}:{2}] {3} = {4:#?}\n",
                                        "src/protocols/file_send/file_send_protocol_handler.rs",
                                        25u32,
                                        9u32,
                                        "&file_send_header",
                                        &&tmp as &dyn ::std::fmt::Debug,
                                    ),
                                );
                            };
                            tmp
                        }
                    };
                    if file_send_header.version != 1 {
                        Ack::new(
                                1,
                                Some("unsupported file_send protocol version".to_string()),
                            )
                            .write_ack(&mut iroh_send)
                            .await?;
                        return ::anyhow::__private::Err(
                            ::anyhow::Error::msg(
                                ::alloc::__export::must_use({
                                    ::alloc::fmt::format(
                                        format_args!(
                                            "unsupported file_send protocol version: {0}",
                                            file_send_header.version,
                                        ),
                                    )
                                }),
                            ),
                        );
                    }
                    let file_path = ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!("{0}", &file_send_header.file_name),
                        )
                    });
                    let file_exists = tokio::fs::metadata(&file_path).await.is_ok();
                    if file_exists && !file_send_header.can_overwrite {
                        Ack::new(
                                1,
                                Some(
                                    ::alloc::__export::must_use({
                                        ::alloc::fmt::format(
                                            format_args!(
                                                "file {0} already exists",
                                                file_send_header.file_name,
                                            ),
                                        )
                                    }),
                                ),
                            )
                            .write_ack(&mut iroh_send)
                            .await?;
                        return ::anyhow::__private::Err({
                            let error = ::anyhow::__private::format_err(
                                format_args!(
                                    "file already exists and overwrite not allowed",
                                ),
                            );
                            error
                        });
                    }
                    Ack::new(0, None).write_ack(&mut iroh_send).await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/file_send/file_send_protocol_handler.rs:42",
                                    "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/file_send/file_send_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(42u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!(
                                                    "Creating file: {0}",
                                                    file_send_header.file_name,
                                                ) as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!(
                                                                            "Creating file: {0}",
                                                                            file_send_header.file_name,
                                                                        ) as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    let mut file = tokio::fs::File::create(&file_path).await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/file_send/file_send_protocol_handler.rs:45",
                                    "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/file_send/file_send_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(45u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!(
                                                    "Copying {0} bytes",
                                                    file_send_header.file_size,
                                                ) as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!(
                                                                            "Copying {0} bytes",
                                                                            file_send_header.file_size,
                                                                        ) as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    copy_bytes(
                            &mut iroh_recv,
                            &mut file,
                            file_send_header.file_size as usize,
                        )
                        .await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/file_send/file_send_protocol_handler.rs:48",
                                    "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/file_send/file_send_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(48u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("flushing") as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("flushing") as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    file.flush().await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/file_send/file_send_protocol_handler.rs:51",
                                    "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/file_send/file_send_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(51u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("sending ack") as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("sending ack") as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    Ack::new(0, None).write_ack(&mut iroh_send).await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/file_send/file_send_protocol_handler.rs:54",
                                    "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/file_send/file_send_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(54u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::file_send::file_send_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("Finished writing file: {0}", _alpn)
                                                    as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("Finished writing file: {0}", _alpn)
                                                                            as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    Ok(())
                }
            }
        }
        pub mod alpn {
            pub const FILE_ALPN_V1: &[u8] = b"proxy-rs/file/1";
        }
        pub mod file_send_header {
            use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
            use anyhow::Result;
            use crate::protocols::codec::StreamCodec;
            const FLAG_CAN_OVERWRITE: u8 = 0b0000_0001;
            pub struct FileSendHeader {
                pub version: u8,
                pub file_name: String,
                pub file_size: u64,
                #[codec(bitpack)]
                pub can_overwrite: bool,
            }
            #[automatically_derived]
            impl ::core::fmt::Debug for FileSendHeader {
                #[inline]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    ::core::fmt::Formatter::debug_struct_field4_finish(
                        f,
                        "FileSendHeader",
                        "version",
                        &self.version,
                        "file_name",
                        &self.file_name,
                        "file_size",
                        &self.file_size,
                        "can_overwrite",
                        &&self.can_overwrite,
                    )
                }
            }
            impl crate::protocols::codec::StreamCodec for FileSendHeader {
                async fn encode<W: tokio::io::AsyncWrite + Unpin>(
                    &self,
                    w: &mut W,
                ) -> anyhow::Result<()> {
                    use tokio::io::AsyncWriteExt;
                    <u8 as crate::protocols::codec::StreamCodec>::encode(
                            &self.version,
                            w,
                        )
                        .await?;
                    <String as crate::protocols::codec::StreamCodec>::encode(
                            &self.file_name,
                            w,
                        )
                        .await?;
                    <u64 as crate::protocols::codec::StreamCodec>::encode(
                            &self.file_size,
                            w,
                        )
                        .await?;
                    let __flags_0: u8 = 0u8 | ((self.can_overwrite as u8) << 0u32);
                    w.write_u8(__flags_0).await?;
                    Ok(())
                }
                async fn decode<R: tokio::io::AsyncRead + Unpin>(
                    r: &mut R,
                ) -> anyhow::Result<Self> {
                    use tokio::io::AsyncReadExt;
                    let version = <u8 as crate::protocols::codec::StreamCodec>::decode(r)
                        .await?;
                    let file_name = <String as crate::protocols::codec::StreamCodec>::decode(
                            r,
                        )
                        .await?;
                    let file_size = <u64 as crate::protocols::codec::StreamCodec>::decode(
                            r,
                        )
                        .await?;
                    let __flags_0 = r.read_u8().await?;
                    let can_overwrite = (__flags_0 & (1u8 << 0u32)) != 0;
                    Ok(Self {
                        version,
                        file_name,
                        file_size,
                        can_overwrite,
                    })
                }
            }
            impl FileSendHeader {
                pub async fn from_stream<T>(stream: &mut T) -> Result<Self>
                where
                    T: AsyncRead + Unpin,
                {
                    let version = stream.read_u8().await?;
                    let len = stream.read_u16().await? as usize;
                    let mut file_name_bytes = ::alloc::vec::from_elem(0u8, len);
                    stream.read_exact(&mut file_name_bytes).await?;
                    let file_name = String::from_utf8(file_name_bytes)?;
                    let file_size = stream.read_u64().await?;
                    let flags = stream.read_u8().await?;
                    Ok(FileSendHeader {
                        version,
                        file_name,
                        file_size,
                        can_overwrite: (flags & FLAG_CAN_OVERWRITE) != 0,
                    })
                }
                pub async fn write_to_stream<T>(&self, stream: &mut T) -> Result<()>
                where
                    T: AsyncWrite + Unpin,
                {
                    stream.write_u8(self.version).await?;
                    let file_name_bytes = self.file_name.as_bytes();
                    stream.write_u16(file_name_bytes.len() as u16).await?;
                    stream.write_all(file_name_bytes).await?;
                    stream.write_u64(self.file_size).await?;
                    stream
                        .write_u8(
                            pack_bits([
                                self.can_overwrite,
                                false,
                                false,
                                false,
                                false,
                                false,
                                false,
                                false,
                            ]),
                        )
                        .await?;
                    Ok(())
                }
            }
            fn pack_bits(bits: [bool; 8]) -> u8 {
                let mut byte = 0;
                for i in 0..8 {
                    if bits[i] {
                        byte |= 1 << i;
                    }
                }
                byte
            }
        }
    }
    pub mod ack {
        use anyhow::Result;
        use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, AsyncReadExt};
        pub struct Ack {
            pub ack: u8,
            pub msg: Option<String>,
        }
        impl Ack {
            pub fn new(ack: u8, msg: Option<String>) -> Self {
                Ack { ack, msg }
            }
            pub async fn write_ack<W: AsyncWrite + Unpin>(
                &self,
                stream: &mut W,
            ) -> Result<()> {
                stream.write_u8(self.ack).await?;
                match &self.msg {
                    Some(msg) => {
                        let message_as_bytes = msg.as_bytes();
                        stream.write_u16(message_as_bytes.len() as u16).await?;
                        stream.write_all(message_as_bytes).await?;
                    }
                    _ => {}
                }
                Ok(())
            }
            pub async fn read_ack<W: AsyncRead + Unpin>(stream: &mut W) -> Result<Self> {
                let ack = stream.read_u8().await?;
                {
                    ::std::io::_print(format_args!("Received ack: {0}\n", ack));
                };
                match ack {
                    0 => Ok(Self { ack, msg: None }),
                    _ => {
                        {
                            ::std::io::_print(format_args!("bad ack"));
                        };
                        let msg_len = stream.read_u16().await? as usize;
                        let mut msg_bytes = ::alloc::vec::from_elem(0u8, msg_len);
                        stream.read_exact(&mut msg_bytes).await?;
                        Ok(Self {
                            ack,
                            msg: Some(String::from_utf8_lossy(&msg_bytes).to_string()),
                        })
                    }
                }
            }
        }
    }
    pub mod ping {
        pub mod alpn {
            pub const PING_ALPN_V1: &[u8] = b"proxy-rs/ping/1";
        }
        pub mod ping_header {
            use anyhow::{Context, Result};
            use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
            use tracing::info;
            pub struct PingHeader {
                pub version: u8,
                pub msg: String,
            }
            impl PingHeader {
                pub async fn write_to_stream<W: AsyncWrite + Unpin>(
                    &self,
                    stream: &mut W,
                ) -> Result<()> {
                    stream.write_u8(self.version).await?;
                    let msg_bytes = self.msg.as_bytes();
                    stream.write_u16(msg_bytes.len() as u16).await?;
                    stream.write_all(msg_bytes).await?;
                    Ok(())
                }
                pub async fn from_stream<R: AsyncRead + Unpin>(
                    stream: &mut R,
                ) -> Result<Self> {
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/ping/ping_header.rs:20",
                                    "proxy_rs::protocols::ping::ping_header",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/ping/ping_header.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(20u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::ping::ping_header",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("reading ping header from stream")
                                                    as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("reading ping header from stream")
                                                                            as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    let version = stream
                        .read_u8()
                        .await
                        .with_context(|| "something isn't working")?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/ping/ping_header.rs:22",
                                    "proxy_rs::protocols::ping::ping_header",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/ping/ping_header.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(22u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::ping::ping_header",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("Reading ping header: version {0}", version)
                                                    as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("Reading ping header: version {0}", version)
                                                                            as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    let msg_len = stream.read_u16().await? as usize;
                    let mut msg_bytes = ::alloc::vec::from_elem(0u8, msg_len);
                    stream.read_exact(&mut msg_bytes).await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/ping/ping_header.rs:26",
                                    "proxy_rs::protocols::ping::ping_header",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/ping/ping_header.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(26u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::ping::ping_header",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!(
                                                    "Received ping header: version {0}, msg_len {1}",
                                                    version,
                                                    msg_len,
                                                ) as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!(
                                                                            "Received ping header: version {0}, msg_len {1}",
                                                                            version,
                                                                            msg_len,
                                                                        ) as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    Ok(Self {
                        version,
                        msg: String::from_utf8(msg_bytes)?,
                    })
                }
            }
        }
        pub mod ping_protocol_handler {
            use iroh::{endpoint::Connection, protocol::{AcceptError, ProtocolHandler}};
            use tracing::info;
            use crate::protocols::ping::ping_header::PingHeader;
            pub struct PingServerProtocolV1;
            #[automatically_derived]
            impl ::core::fmt::Debug for PingServerProtocolV1 {
                #[inline]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    ::core::fmt::Formatter::write_str(f, "PingServerProtocolV1")
                }
            }
            #[automatically_derived]
            impl ::core::clone::Clone for PingServerProtocolV1 {
                #[inline]
                fn clone(&self) -> PingServerProtocolV1 {
                    PingServerProtocolV1
                }
            }
            impl ProtocolHandler for PingServerProtocolV1 {
                async fn accept(
                    &self,
                    connection: Connection,
                ) -> Result<(), AcceptError> {
                    self.handle(connection)
                        .await
                        .map_err(|e| AcceptError::from_boxed(e.into()))
                }
            }
            impl PingServerProtocolV1 {
                async fn handle(&self, connection: Connection) -> anyhow::Result<()> {
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/ping/ping_protocol_handler.rs:18",
                                    "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/ping/ping_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(18u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!(
                                                    "Accepted ping connection from {0}",
                                                    connection.remote_id(),
                                                ) as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!(
                                                                            "Accepted ping connection from {0}",
                                                                            connection.remote_id(),
                                                                        ) as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    let (mut send, mut recv) = connection.accept_bi().await?;
                    let ping = PingHeader::from_stream(&mut recv).await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/ping/ping_protocol_handler.rs:21",
                                    "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/ping/ping_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(21u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("Ping received: \"{0}\"", ping.msg)
                                                    as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("Ping received: \"{0}\"", ping.msg)
                                                                            as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    ping.write_to_stream(&mut send).await?;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/ping/ping_protocol_handler.rs:23",
                                    "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/ping/ping_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(23u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!("Pong sent: \"{0}\"", ping.msg)
                                                    as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!("Pong sent: \"{0}\"", ping.msg)
                                                                            as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    send.finish()?;
                    connection.closed().await;
                    {
                        use ::tracing::__macro_support::Callsite as _;
                        static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                            static META: ::tracing::Metadata<'static> = {
                                ::tracing_core::metadata::Metadata::new(
                                    "event src/protocols/ping/ping_protocol_handler.rs:26",
                                    "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ::tracing::Level::INFO,
                                    ::tracing_core::__macro_support::Option::Some(
                                        "src/protocols/ping/ping_protocol_handler.rs",
                                    ),
                                    ::tracing_core::__macro_support::Option::Some(26u32),
                                    ::tracing_core::__macro_support::Option::Some(
                                        "proxy_rs::protocols::ping::ping_protocol_handler",
                                    ),
                                    ::tracing_core::field::FieldSet::new(
                                        &["message"],
                                        ::tracing_core::callsite::Identifier(&__CALLSITE),
                                    ),
                                    ::tracing::metadata::Kind::EVENT,
                                )
                            };
                            ::tracing::callsite::DefaultCallsite::new(&META)
                        };
                        let enabled = ::tracing::Level::INFO
                            <= ::tracing::level_filters::STATIC_MAX_LEVEL
                            && ::tracing::Level::INFO
                                <= ::tracing::level_filters::LevelFilter::current()
                            && {
                                let interest = __CALLSITE.interest();
                                !interest.is_never()
                                    && ::tracing::__macro_support::__is_enabled(
                                        __CALLSITE.metadata(),
                                        interest,
                                    )
                            };
                        if enabled {
                            (|value_set: ::tracing::field::ValueSet| {
                                let meta = __CALLSITE.metadata();
                                ::tracing::Event::dispatch(meta, &value_set);
                                if match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                } <= ::tracing::log::STATIC_MAX_LEVEL
                                {
                                    if !::tracing::dispatcher::has_been_set() {
                                        {
                                            use ::tracing::log;
                                            let level = match ::tracing::Level::INFO {
                                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                                _ => ::tracing::log::Level::Trace,
                                            };
                                            if level <= log::max_level() {
                                                let meta = __CALLSITE.metadata();
                                                let log_meta = log::Metadata::builder()
                                                    .level(level)
                                                    .target(meta.target())
                                                    .build();
                                                let logger = log::logger();
                                                if logger.enabled(&log_meta) {
                                                    ::tracing::__macro_support::__tracing_log(
                                                        meta,
                                                        logger,
                                                        log_meta,
                                                        &value_set,
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        {}
                                    }
                                } else {
                                    {}
                                };
                            })({
                                #[allow(unused_imports)]
                                use ::tracing::field::{debug, display, Value};
                                __CALLSITE
                                    .metadata()
                                    .fields()
                                    .value_set_all(
                                        &[
                                            (::tracing::__macro_support::Option::Some(
                                                &format_args!(
                                                    "Ping connection with {0} completed",
                                                    connection.remote_id(),
                                                ) as &dyn ::tracing::field::Value,
                                            )),
                                        ],
                                    )
                            });
                        } else {
                            if match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            } <= ::tracing::log::STATIC_MAX_LEVEL
                            {
                                if !::tracing::dispatcher::has_been_set() {
                                    {
                                        use ::tracing::log;
                                        let level = match ::tracing::Level::INFO {
                                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                            _ => ::tracing::log::Level::Trace,
                                        };
                                        if level <= log::max_level() {
                                            let meta = __CALLSITE.metadata();
                                            let log_meta = log::Metadata::builder()
                                                .level(level)
                                                .target(meta.target())
                                                .build();
                                            let logger = log::logger();
                                            if logger.enabled(&log_meta) {
                                                ::tracing::__macro_support::__tracing_log(
                                                    meta,
                                                    logger,
                                                    log_meta,
                                                    &{
                                                        #[allow(unused_imports)]
                                                        use ::tracing::field::{debug, display, Value};
                                                        __CALLSITE
                                                            .metadata()
                                                            .fields()
                                                            .value_set_all(
                                                                &[
                                                                    (::tracing::__macro_support::Option::Some(
                                                                        &format_args!(
                                                                            "Ping connection with {0} completed",
                                                                            connection.remote_id(),
                                                                        ) as &dyn ::tracing::field::Value,
                                                                    )),
                                                                ],
                                                            )
                                                    },
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    {}
                                }
                            } else {
                                {}
                            };
                        }
                    };
                    Ok(())
                }
            }
        }
    }
}
mod server {
    use anyhow::{Context, Result};
    use iroh::{
        Endpoint, SecretKey, address_lookup::{self, PkarrPublisher},
        endpoint::presets, protocol::Router,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use std::{path::Path, str::FromStr};
    use tracing::info;
    use crate::protocols::{
        file_send::{
            alpn::FILE_ALPN_V1, file_send_protocol_handler::FileServerProtocolV1,
        },
        ping::{alpn::PING_ALPN_V1, ping_protocol_handler::PingServerProtocolV1},
        proxy::{alpn::TCP_PROXY_ALPN_V1, proxy_protocol_handler::ProxyServerProtocolV1},
    };
    fn load_or_create_secret_key() -> Result<SecretKey> {
        let path = ".server-key";
        if Path::new(path).exists() {
            let hex = std::fs::read_to_string(path)
                .with_context(|| ::alloc::__export::must_use({
                    ::alloc::fmt::format(
                        format_args!("failed to read key file: {0}", path),
                    )
                }))?;
            let key = SecretKey::from_str(&hex).with_context(|| "")?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/server.rs:15",
                            "proxy_rs::server",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/server.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(15u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::server",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!("Loaded secret key from {0}", path)
                                            as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!("Loaded secret key from {0}", path)
                                                                    as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            Ok(key)
        } else {
            let key = SecretKey::generate();
            let ss: String = key
                .to_bytes()
                .iter()
                .map(|b| ::alloc::__export::must_use({
                    ::alloc::fmt::format(format_args!("{0:02x}", b))
                }))
                .collect();
            std::fs::write(path, ss)
                .with_context(|| ::alloc::__export::must_use({
                    ::alloc::fmt::format(
                        format_args!("failed to write key file: {0}", path),
                    )
                }))?;
            {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "event src/server.rs:22",
                            "proxy_rs::server",
                            ::tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "src/server.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(22u32),
                            ::tracing_core::__macro_support::Option::Some(
                                "proxy_rs::server",
                            ),
                            ::tracing_core::field::FieldSet::new(
                                &["message"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::EVENT,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let enabled = ::tracing::Level::INFO
                    <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && ::tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        let interest = __CALLSITE.interest();
                        !interest.is_never()
                            && ::tracing::__macro_support::__is_enabled(
                                __CALLSITE.metadata(),
                                interest,
                            )
                    };
                if enabled {
                    (|value_set: ::tracing::field::ValueSet| {
                        let meta = __CALLSITE.metadata();
                        ::tracing::Event::dispatch(meta, &value_set);
                        if match ::tracing::Level::INFO {
                            ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                            ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                            ::tracing::Level::INFO => ::tracing::log::Level::Info,
                            ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                            _ => ::tracing::log::Level::Trace,
                        } <= ::tracing::log::STATIC_MAX_LEVEL
                        {
                            if !::tracing::dispatcher::has_been_set() {
                                {
                                    use ::tracing::log;
                                    let level = match ::tracing::Level::INFO {
                                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                        _ => ::tracing::log::Level::Trace,
                                    };
                                    if level <= log::max_level() {
                                        let meta = __CALLSITE.metadata();
                                        let log_meta = log::Metadata::builder()
                                            .level(level)
                                            .target(meta.target())
                                            .build();
                                        let logger = log::logger();
                                        if logger.enabled(&log_meta) {
                                            ::tracing::__macro_support::__tracing_log(
                                                meta,
                                                logger,
                                                log_meta,
                                                &value_set,
                                            )
                                        }
                                    }
                                }
                            } else {
                                {}
                            }
                        } else {
                            {}
                        };
                    })({
                        #[allow(unused_imports)]
                        use ::tracing::field::{debug, display, Value};
                        __CALLSITE
                            .metadata()
                            .fields()
                            .value_set_all(
                                &[
                                    (::tracing::__macro_support::Option::Some(
                                        &format_args!(
                                            "Generated new secret key, saved to {0}",
                                            path,
                                        ) as &dyn ::tracing::field::Value,
                                    )),
                                ],
                            )
                    });
                } else {
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &{
                                                #[allow(unused_imports)]
                                                use ::tracing::field::{debug, display, Value};
                                                __CALLSITE
                                                    .metadata()
                                                    .fields()
                                                    .value_set_all(
                                                        &[
                                                            (::tracing::__macro_support::Option::Some(
                                                                &format_args!(
                                                                    "Generated new secret key, saved to {0}",
                                                                    path,
                                                                ) as &dyn ::tracing::field::Value,
                                                            )),
                                                        ],
                                                    )
                                            },
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                }
            };
            Ok(key)
        }
    }
    pub async fn run_server() -> Result<()> {
        {
            use ::tracing::__macro_support::Callsite as _;
            static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "event src/server.rs:28",
                        "proxy_rs::server",
                        ::tracing::Level::INFO,
                        ::tracing_core::__macro_support::Option::Some("src/server.rs"),
                        ::tracing_core::__macro_support::Option::Some(28u32),
                        ::tracing_core::__macro_support::Option::Some(
                            "proxy_rs::server",
                        ),
                        ::tracing_core::field::FieldSet::new(
                            &["message"],
                            ::tracing_core::callsite::Identifier(&__CALLSITE),
                        ),
                        ::tracing::metadata::Kind::EVENT,
                    )
                };
                ::tracing::callsite::DefaultCallsite::new(&META)
            };
            let enabled = ::tracing::Level::INFO
                <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::INFO
                    <= ::tracing::level_filters::LevelFilter::current()
                && {
                    let interest = __CALLSITE.interest();
                    !interest.is_never()
                        && ::tracing::__macro_support::__is_enabled(
                            __CALLSITE.metadata(),
                            interest,
                        )
                };
            if enabled {
                (|value_set: ::tracing::field::ValueSet| {
                    let meta = __CALLSITE.metadata();
                    ::tracing::Event::dispatch(meta, &value_set);
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &value_set,
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                })({
                    #[allow(unused_imports)]
                    use ::tracing::field::{debug, display, Value};
                    __CALLSITE
                        .metadata()
                        .fields()
                        .value_set_all(
                            &[
                                (::tracing::__macro_support::Option::Some(
                                    &format_args!("Server mode") as &dyn ::tracing::field::Value,
                                )),
                            ],
                        )
                });
            } else {
                if match ::tracing::Level::INFO {
                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                    _ => ::tracing::log::Level::Trace,
                } <= ::tracing::log::STATIC_MAX_LEVEL
                {
                    if !::tracing::dispatcher::has_been_set() {
                        {
                            use ::tracing::log;
                            let level = match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            };
                            if level <= log::max_level() {
                                let meta = __CALLSITE.metadata();
                                let log_meta = log::Metadata::builder()
                                    .level(level)
                                    .target(meta.target())
                                    .build();
                                let logger = log::logger();
                                if logger.enabled(&log_meta) {
                                    ::tracing::__macro_support::__tracing_log(
                                        meta,
                                        logger,
                                        log_meta,
                                        &{
                                            #[allow(unused_imports)]
                                            use ::tracing::field::{debug, display, Value};
                                            __CALLSITE
                                                .metadata()
                                                .fields()
                                                .value_set_all(
                                                    &[
                                                        (::tracing::__macro_support::Option::Some(
                                                            &format_args!("Server mode") as &dyn ::tracing::field::Value,
                                                        )),
                                                    ],
                                                )
                                        },
                                    )
                                }
                            }
                        }
                    } else {
                        {}
                    }
                } else {
                    {}
                };
            }
        };
        let secret_key = load_or_create_secret_key()?;
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .address_lookup(PkarrPublisher::n0_dns())
            .address_lookup(address_lookup::DnsAddressLookup::n0_dns())
            .bind()
            .await?;
        let router = Router::builder(endpoint)
            .accept(PING_ALPN_V1, PingServerProtocolV1)
            .accept(FILE_ALPN_V1, FileServerProtocolV1)
            .accept(TCP_PROXY_ALPN_V1, ProxyServerProtocolV1)
            .spawn();
        {
            use ::tracing::__macro_support::Callsite as _;
            static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "event src/server.rs:44",
                        "proxy_rs::server",
                        ::tracing::Level::INFO,
                        ::tracing_core::__macro_support::Option::Some("src/server.rs"),
                        ::tracing_core::__macro_support::Option::Some(44u32),
                        ::tracing_core::__macro_support::Option::Some(
                            "proxy_rs::server",
                        ),
                        ::tracing_core::field::FieldSet::new(
                            &["message"],
                            ::tracing_core::callsite::Identifier(&__CALLSITE),
                        ),
                        ::tracing::metadata::Kind::EVENT,
                    )
                };
                ::tracing::callsite::DefaultCallsite::new(&META)
            };
            let enabled = ::tracing::Level::INFO
                <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::INFO
                    <= ::tracing::level_filters::LevelFilter::current()
                && {
                    let interest = __CALLSITE.interest();
                    !interest.is_never()
                        && ::tracing::__macro_support::__is_enabled(
                            __CALLSITE.metadata(),
                            interest,
                        )
                };
            if enabled {
                (|value_set: ::tracing::field::ValueSet| {
                    let meta = __CALLSITE.metadata();
                    ::tracing::Event::dispatch(meta, &value_set);
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &value_set,
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                })({
                    #[allow(unused_imports)]
                    use ::tracing::field::{debug, display, Value};
                    __CALLSITE
                        .metadata()
                        .fields()
                        .value_set_all(
                            &[
                                (::tracing::__macro_support::Option::Some(
                                    &format_args!("Server NodeId: {0}", router.endpoint().id())
                                        as &dyn ::tracing::field::Value,
                                )),
                            ],
                        )
                });
            } else {
                if match ::tracing::Level::INFO {
                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                    _ => ::tracing::log::Level::Trace,
                } <= ::tracing::log::STATIC_MAX_LEVEL
                {
                    if !::tracing::dispatcher::has_been_set() {
                        {
                            use ::tracing::log;
                            let level = match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            };
                            if level <= log::max_level() {
                                let meta = __CALLSITE.metadata();
                                let log_meta = log::Metadata::builder()
                                    .level(level)
                                    .target(meta.target())
                                    .build();
                                let logger = log::logger();
                                if logger.enabled(&log_meta) {
                                    ::tracing::__macro_support::__tracing_log(
                                        meta,
                                        logger,
                                        log_meta,
                                        &{
                                            #[allow(unused_imports)]
                                            use ::tracing::field::{debug, display, Value};
                                            __CALLSITE
                                                .metadata()
                                                .fields()
                                                .value_set_all(
                                                    &[
                                                        (::tracing::__macro_support::Option::Some(
                                                            &format_args!("Server NodeId: {0}", router.endpoint().id())
                                                                as &dyn ::tracing::field::Value,
                                                        )),
                                                    ],
                                                )
                                        },
                                    )
                                }
                            }
                        }
                    } else {
                        {}
                    }
                } else {
                    {}
                };
            }
        };
        {
            use ::tracing::__macro_support::Callsite as _;
            static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "event src/server.rs:45",
                        "proxy_rs::server",
                        ::tracing::Level::INFO,
                        ::tracing_core::__macro_support::Option::Some("src/server.rs"),
                        ::tracing_core::__macro_support::Option::Some(45u32),
                        ::tracing_core::__macro_support::Option::Some(
                            "proxy_rs::server",
                        ),
                        ::tracing_core::field::FieldSet::new(
                            &["message"],
                            ::tracing_core::callsite::Identifier(&__CALLSITE),
                        ),
                        ::tracing::metadata::Kind::EVENT,
                    )
                };
                ::tracing::callsite::DefaultCallsite::new(&META)
            };
            let enabled = ::tracing::Level::INFO
                <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::INFO
                    <= ::tracing::level_filters::LevelFilter::current()
                && {
                    let interest = __CALLSITE.interest();
                    !interest.is_never()
                        && ::tracing::__macro_support::__is_enabled(
                            __CALLSITE.metadata(),
                            interest,
                        )
                };
            if enabled {
                (|value_set: ::tracing::field::ValueSet| {
                    let meta = __CALLSITE.metadata();
                    ::tracing::Event::dispatch(meta, &value_set);
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &value_set,
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                })({
                    #[allow(unused_imports)]
                    use ::tracing::field::{debug, display, Value};
                    __CALLSITE
                        .metadata()
                        .fields()
                        .value_set_all(
                            &[
                                (::tracing::__macro_support::Option::Some(
                                    &format_args!("Listening for iroh connections")
                                        as &dyn ::tracing::field::Value,
                                )),
                            ],
                        )
                });
            } else {
                if match ::tracing::Level::INFO {
                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                    _ => ::tracing::log::Level::Trace,
                } <= ::tracing::log::STATIC_MAX_LEVEL
                {
                    if !::tracing::dispatcher::has_been_set() {
                        {
                            use ::tracing::log;
                            let level = match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            };
                            if level <= log::max_level() {
                                let meta = __CALLSITE.metadata();
                                let log_meta = log::Metadata::builder()
                                    .level(level)
                                    .target(meta.target())
                                    .build();
                                let logger = log::logger();
                                if logger.enabled(&log_meta) {
                                    ::tracing::__macro_support::__tracing_log(
                                        meta,
                                        logger,
                                        log_meta,
                                        &{
                                            #[allow(unused_imports)]
                                            use ::tracing::field::{debug, display, Value};
                                            __CALLSITE
                                                .metadata()
                                                .fields()
                                                .value_set_all(
                                                    &[
                                                        (::tracing::__macro_support::Option::Some(
                                                            &format_args!("Listening for iroh connections")
                                                                as &dyn ::tracing::field::Value,
                                                        )),
                                                    ],
                                                )
                                        },
                                    )
                                }
                            }
                        }
                    } else {
                        {}
                    }
                } else {
                    {}
                };
            }
        };
        tokio::signal::ctrl_c().await?;
        {
            use ::tracing::__macro_support::Callsite as _;
            static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "event src/server.rs:48",
                        "proxy_rs::server",
                        ::tracing::Level::INFO,
                        ::tracing_core::__macro_support::Option::Some("src/server.rs"),
                        ::tracing_core::__macro_support::Option::Some(48u32),
                        ::tracing_core::__macro_support::Option::Some(
                            "proxy_rs::server",
                        ),
                        ::tracing_core::field::FieldSet::new(
                            &["message"],
                            ::tracing_core::callsite::Identifier(&__CALLSITE),
                        ),
                        ::tracing::metadata::Kind::EVENT,
                    )
                };
                ::tracing::callsite::DefaultCallsite::new(&META)
            };
            let enabled = ::tracing::Level::INFO
                <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::INFO
                    <= ::tracing::level_filters::LevelFilter::current()
                && {
                    let interest = __CALLSITE.interest();
                    !interest.is_never()
                        && ::tracing::__macro_support::__is_enabled(
                            __CALLSITE.metadata(),
                            interest,
                        )
                };
            if enabled {
                (|value_set: ::tracing::field::ValueSet| {
                    let meta = __CALLSITE.metadata();
                    ::tracing::Event::dispatch(meta, &value_set);
                    if match ::tracing::Level::INFO {
                        ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                        ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                        ::tracing::Level::INFO => ::tracing::log::Level::Info,
                        ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                        _ => ::tracing::log::Level::Trace,
                    } <= ::tracing::log::STATIC_MAX_LEVEL
                    {
                        if !::tracing::dispatcher::has_been_set() {
                            {
                                use ::tracing::log;
                                let level = match ::tracing::Level::INFO {
                                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                    _ => ::tracing::log::Level::Trace,
                                };
                                if level <= log::max_level() {
                                    let meta = __CALLSITE.metadata();
                                    let log_meta = log::Metadata::builder()
                                        .level(level)
                                        .target(meta.target())
                                        .build();
                                    let logger = log::logger();
                                    if logger.enabled(&log_meta) {
                                        ::tracing::__macro_support::__tracing_log(
                                            meta,
                                            logger,
                                            log_meta,
                                            &value_set,
                                        )
                                    }
                                }
                            }
                        } else {
                            {}
                        }
                    } else {
                        {}
                    };
                })({
                    #[allow(unused_imports)]
                    use ::tracing::field::{debug, display, Value};
                    __CALLSITE
                        .metadata()
                        .fields()
                        .value_set_all(
                            &[
                                (::tracing::__macro_support::Option::Some(
                                    &format_args!("Shutting down server")
                                        as &dyn ::tracing::field::Value,
                                )),
                            ],
                        )
                });
            } else {
                if match ::tracing::Level::INFO {
                    ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                    ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                    ::tracing::Level::INFO => ::tracing::log::Level::Info,
                    ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                    _ => ::tracing::log::Level::Trace,
                } <= ::tracing::log::STATIC_MAX_LEVEL
                {
                    if !::tracing::dispatcher::has_been_set() {
                        {
                            use ::tracing::log;
                            let level = match ::tracing::Level::INFO {
                                ::tracing::Level::ERROR => ::tracing::log::Level::Error,
                                ::tracing::Level::WARN => ::tracing::log::Level::Warn,
                                ::tracing::Level::INFO => ::tracing::log::Level::Info,
                                ::tracing::Level::DEBUG => ::tracing::log::Level::Debug,
                                _ => ::tracing::log::Level::Trace,
                            };
                            if level <= log::max_level() {
                                let meta = __CALLSITE.metadata();
                                let log_meta = log::Metadata::builder()
                                    .level(level)
                                    .target(meta.target())
                                    .build();
                                let logger = log::logger();
                                if logger.enabled(&log_meta) {
                                    ::tracing::__macro_support::__tracing_log(
                                        meta,
                                        logger,
                                        log_meta,
                                        &{
                                            #[allow(unused_imports)]
                                            use ::tracing::field::{debug, display, Value};
                                            __CALLSITE
                                                .metadata()
                                                .fields()
                                                .value_set_all(
                                                    &[
                                                        (::tracing::__macro_support::Option::Some(
                                                            &format_args!("Shutting down server")
                                                                as &dyn ::tracing::field::Value,
                                                        )),
                                                    ],
                                                )
                                        },
                                    )
                                }
                            }
                        }
                    } else {
                        {}
                    }
                } else {
                    {}
                };
            }
        };
        router.shutdown().await?;
        Ok(())
    }
    pub async fn copy_bytes<A, B>(
        a: &mut A,
        b: &mut B,
        size: usize,
    ) -> anyhow::Result<()>
    where
        A: tokio::io::AsyncRead + Unpin,
        B: tokio::io::AsyncWrite + Unpin,
    {
        let mut buf = ::alloc::vec::from_elem(0u8, size);
        a.read_exact(&mut buf).await?;
        b.write_all(&buf).await?;
        Ok(())
    }
}
mod socks5 {
    use anyhow::{bail, Result};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    /// Perform a SOCKS5 handshake on the given stream.
    /// Returns the requested target host and port, with the stream
    /// left positioned at the start of proxied data.
    pub async fn handshake(stream: &mut TcpStream) -> Result<(String, u16)> {
        let version = stream.read_u8().await?;
        if version != 5 {
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("unsupported SOCKS version: {0}", version),
                );
                error
            });
        }
        let nmethods = stream.read_u8().await? as usize;
        let mut methods = ::alloc::vec::from_elem(0u8, nmethods);
        stream.read_exact(&mut methods).await?;
        if !methods.contains(&0x00) {
            stream.write_all(&[0x05, 0xFF]).await?;
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("client offered no acceptable auth methods"),
                );
                error
            });
        }
        stream.write_all(&[0x05, 0x00]).await?;
        let version = stream.read_u8().await?;
        if version != 5 {
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("expected SOCKS5 in request, got version {0}", version),
                );
                error
            });
        }
        let cmd = stream.read_u8().await?;
        let _rsv = stream.read_u8().await?;
        let atyp = stream.read_u8().await?;
        if cmd != 0x01 {
            stream.write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!(
                        "unsupported SOCKS5 command: {0:#04x} (only CONNECT supported)",
                        cmd,
                    ),
                );
                error
            });
        }
        let host = match atyp {
            0x01 => {
                let mut addr = [0u8; 4];
                stream.read_exact(&mut addr).await?;
                ::alloc::__export::must_use({
                    ::alloc::fmt::format(
                        format_args!(
                            "{0}.{1}.{2}.{3}",
                            addr[0],
                            addr[1],
                            addr[2],
                            addr[3],
                        ),
                    )
                })
            }
            0x03 => {
                let len = stream.read_u8().await? as usize;
                let mut name = ::alloc::vec::from_elem(0u8, len);
                stream.read_exact(&mut name).await?;
                String::from_utf8(name)?
            }
            0x04 => {
                let mut addr = [0u8; 16];
                stream.read_exact(&mut addr).await?;
                let v6 = std::net::Ipv6Addr::from(addr);
                ::alloc::__export::must_use({
                    ::alloc::fmt::format(format_args!("{0}", v6))
                })
            }
            _ => {
                return ::anyhow::__private::Err({
                    let error = ::anyhow::__private::format_err(
                        format_args!("unsupported SOCKS5 address type: {0:#04x}", atyp),
                    );
                    error
                });
            }
        };
        let port = stream.read_u16().await?;
        stream.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await?;
        Ok((host, port))
    }
}
use client::client::{run_send_file, run_tcp_client};
#[command(about = "Iroh proxy (SOCKS5 + HTTP) — server and client modes")]
struct Args {
    /// iroh ticket to connect to (client mode)
    #[arg(short, long)]
    node_id: Option<String>,
    #[arg(short, long)]
    listen: Option<String>,
    #[arg(short, long)]
    file: Option<String>,
    /// allow overwriting existing files
    #[arg(short, long)]
    overwrite: bool,
}
#[automatically_derived]
#[allow(unused_qualifications, clippy::redundant_locals)]
impl clap::Parser for Args {}
#[allow(
    dead_code,
    unreachable_code,
    unused_variables,
    unused_braces,
    unused_qualifications,
)]
#[allow(
    clippy::style,
    clippy::complexity,
    clippy::pedantic,
    clippy::restriction,
    clippy::perf,
    clippy::deprecated,
    clippy::nursery,
    clippy::cargo,
    clippy::suspicious_else_formatting,
    clippy::almost_swapped,
    clippy::redundant_locals,
)]
#[automatically_derived]
impl clap::CommandFactory for Args {
    fn command<'b>() -> clap::Command {
        let __clap_app = clap::Command::new({
            let _ = "proxy-rs";
            "proxy-rs"
        });
        <Self as clap::Args>::augment_args(__clap_app)
    }
    fn command_for_update<'b>() -> clap::Command {
        let __clap_app = clap::Command::new({
            let _ = "proxy-rs";
            "proxy-rs"
        });
        <Self as clap::Args>::augment_args_for_update(__clap_app)
    }
}
#[allow(
    dead_code,
    unreachable_code,
    unused_variables,
    unused_braces,
    unused_qualifications,
)]
#[allow(
    clippy::style,
    clippy::complexity,
    clippy::pedantic,
    clippy::restriction,
    clippy::perf,
    clippy::deprecated,
    clippy::nursery,
    clippy::cargo,
    clippy::suspicious_else_formatting,
    clippy::almost_swapped,
    clippy::redundant_locals,
)]
#[automatically_derived]
impl clap::FromArgMatches for Args {
    fn from_arg_matches(
        __clap_arg_matches: &clap::ArgMatches,
    ) -> ::std::result::Result<Self, clap::Error> {
        Self::from_arg_matches_mut(&mut __clap_arg_matches.clone())
    }
    fn from_arg_matches_mut(
        __clap_arg_matches: &mut clap::ArgMatches,
    ) -> ::std::result::Result<Self, clap::Error> {
        #![allow(deprecated)]
        let v = Args {
            node_id: __clap_arg_matches.remove_one::<String>("node_id"),
            listen: __clap_arg_matches.remove_one::<String>("listen"),
            file: __clap_arg_matches.remove_one::<String>("file"),
            overwrite: __clap_arg_matches
                .remove_one::<bool>("overwrite")
                .ok_or_else(|| clap::Error::raw(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "the following required argument was not provided: overwrite",
                ))?,
        };
        ::std::result::Result::Ok(v)
    }
    fn update_from_arg_matches(
        &mut self,
        __clap_arg_matches: &clap::ArgMatches,
    ) -> ::std::result::Result<(), clap::Error> {
        self.update_from_arg_matches_mut(&mut __clap_arg_matches.clone())
    }
    fn update_from_arg_matches_mut(
        &mut self,
        __clap_arg_matches: &mut clap::ArgMatches,
    ) -> ::std::result::Result<(), clap::Error> {
        #![allow(deprecated)]
        if __clap_arg_matches.contains_id("node_id") {
            #[allow(non_snake_case)]
            let node_id = &mut self.node_id;
            *node_id = __clap_arg_matches.remove_one::<String>("node_id");
        }
        if __clap_arg_matches.contains_id("listen") {
            #[allow(non_snake_case)]
            let listen = &mut self.listen;
            *listen = __clap_arg_matches.remove_one::<String>("listen");
        }
        if __clap_arg_matches.contains_id("file") {
            #[allow(non_snake_case)]
            let file = &mut self.file;
            *file = __clap_arg_matches.remove_one::<String>("file");
        }
        if __clap_arg_matches.contains_id("overwrite") {
            #[allow(non_snake_case)]
            let overwrite = &mut self.overwrite;
            *overwrite = __clap_arg_matches
                .remove_one::<bool>("overwrite")
                .ok_or_else(|| clap::Error::raw(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "the following required argument was not provided: overwrite",
                ))?;
        }
        ::std::result::Result::Ok(())
    }
}
#[allow(
    dead_code,
    unreachable_code,
    unused_variables,
    unused_braces,
    unused_qualifications,
)]
#[allow(
    clippy::style,
    clippy::complexity,
    clippy::pedantic,
    clippy::restriction,
    clippy::perf,
    clippy::deprecated,
    clippy::nursery,
    clippy::cargo,
    clippy::suspicious_else_formatting,
    clippy::almost_swapped,
    clippy::redundant_locals,
)]
#[automatically_derived]
impl clap::Args for Args {
    fn group_id() -> Option<clap::Id> {
        Some(clap::Id::from("Args"))
    }
    fn augment_args<'b>(__clap_app: clap::Command) -> clap::Command {
        {
            let __clap_app = __clap_app
                .group(
                    clap::ArgGroup::new("Args")
                        .multiple(true)
                        .args({
                            let members: [clap::Id; 4usize] = [
                                clap::Id::from("node_id"),
                                clap::Id::from("listen"),
                                clap::Id::from("file"),
                                clap::Id::from("overwrite"),
                            ];
                            members
                        }),
                );
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("node_id")
                        .value_name("NODE_ID")
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                String,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::Set);
                    let arg = arg
                        .help("iroh ticket to connect to (client mode)")
                        .long_help(None)
                        .short('n')
                        .long("node-id");
                    let arg = arg;
                    arg
                });
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("listen")
                        .value_name("LISTEN")
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                String,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::Set);
                    let arg = arg.short('l').long("listen");
                    let arg = arg;
                    arg
                });
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("file")
                        .value_name("FILE")
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                String,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::Set);
                    let arg = arg.short('f').long("file");
                    let arg = arg;
                    arg
                });
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("overwrite")
                        .value_name("OVERWRITE")
                        .required(true && clap::ArgAction::SetTrue.takes_values())
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                bool,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::SetTrue);
                    let arg = arg
                        .help("allow overwriting existing files")
                        .long_help(None)
                        .short('o')
                        .long("overwrite");
                    let arg = arg;
                    arg
                });
            __clap_app.about("Iroh proxy (SOCKS5 + HTTP) — server and client modes")
        }
    }
    fn augment_args_for_update<'b>(__clap_app: clap::Command) -> clap::Command {
        {
            let __clap_app = __clap_app
                .group(
                    clap::ArgGroup::new("Args")
                        .multiple(true)
                        .args({
                            let members: [clap::Id; 4usize] = [
                                clap::Id::from("node_id"),
                                clap::Id::from("listen"),
                                clap::Id::from("file"),
                                clap::Id::from("overwrite"),
                            ];
                            members
                        }),
                );
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("node_id")
                        .value_name("NODE_ID")
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                String,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::Set);
                    let arg = arg
                        .help("iroh ticket to connect to (client mode)")
                        .long_help(None)
                        .short('n')
                        .long("node-id");
                    let arg = arg.required(false);
                    arg
                });
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("listen")
                        .value_name("LISTEN")
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                String,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::Set);
                    let arg = arg.short('l').long("listen");
                    let arg = arg.required(false);
                    arg
                });
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("file")
                        .value_name("FILE")
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                String,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::Set);
                    let arg = arg.short('f').long("file");
                    let arg = arg.required(false);
                    arg
                });
            let __clap_app = __clap_app
                .arg({
                    #[allow(deprecated)]
                    let arg = clap::Arg::new("overwrite")
                        .value_name("OVERWRITE")
                        .required(true && clap::ArgAction::SetTrue.takes_values())
                        .value_parser({
                            use ::clap_builder::builder::impl_prelude::*;
                            let auto = ::clap_builder::builder::_infer_ValueParser_for::<
                                bool,
                            >::new();
                            (&&&&&&auto).value_parser()
                        })
                        .action(clap::ArgAction::SetTrue);
                    let arg = arg
                        .help("allow overwriting existing files")
                        .long_help(None)
                        .short('o')
                        .long("overwrite");
                    let arg = arg.required(false);
                    arg
                });
            __clap_app.about("Iroh proxy (SOCKS5 + HTTP) — server and client modes")
        }
    }
}
fn main() -> Result<()> {
    let body = async {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::new("proxy_rs=warn,proxy_rs=info,error"),
            )
            .init();
        let args = Args::parse();
        if let Some(listen) = args.listen {
            return run_tcp_client(listen, args.node_id)
                .await
                .or_else(|e: anyhow::Error| {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Failed to run TCP client: {0:#}", e),
                        );
                        error
                    });
                });
        }
        if let Some(file) = args.file {
            return run_send_file(file, args.node_id, args.overwrite)
                .await
                .or_else(|e: anyhow::Error| {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Failed to run file send: {0:#}", e),
                        );
                        error
                    });
                });
        }
        server::run_server().await
    };
    let body = {
        if false {
            let _: &dyn ::core::future::Future<Output = Result<()>> = &body;
        }
        body
    };
    #[allow(
        clippy::expect_used,
        clippy::diverging_sub_expression,
        clippy::needless_return,
        clippy::unwrap_in_result
    )]
    {
        use tokio::runtime::Builder;
        return Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed building the Runtime")
            .block_on(body);
    }
}
