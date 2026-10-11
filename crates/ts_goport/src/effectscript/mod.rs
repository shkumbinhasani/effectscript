pub mod diag;

use crate::prelude::{Node, source_file_file_name};

pub const EXTENSION: &str = ".ef";
pub const EFFECT_ALIAS: &str = "__es_Effect";
pub const YIELD_WRAP_ALIAS: &str = "__es_YieldWrap";
pub const EFFECT_MODULE: &str = "effect";
pub const UTILS_MODULE: &str = "effect/Utils";

pub const PRELUDE: &[&str] = &[
    "Effect",
    "Context",
    "Layer",
    "Data",
    "Option",
    "Either",
    "Exit",
    "Cause",
    "Schedule",
    "Duration",
    "Stream",
    "Schema",
    "Fiber",
    "Chunk",
    "HashMap",
    "HashSet",
    "Deferred",
    "PubSub",
    "Redacted",
    "DateTime",
    "Brand",
    "Metric",
    "ManagedRuntime",
    "Runtime",
    "STM",
    "Micro",
    "Match",
    "Console",
    "pipe",
];

#[must_use]
pub fn is_effectscript_file_name(file_name: &str) -> bool {
    file_name.len() >= EXTENSION.len()
        && file_name[file_name.len() - EXTENSION.len()..].eq_ignore_ascii_case(EXTENSION)
}

#[must_use]
pub fn is_effectscript_source_file(file: Node) -> bool {
    is_effectscript_file_name(source_file_file_name(file))
}
