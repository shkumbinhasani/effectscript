#![allow(non_upper_case_globals)]

use crate::diagnostics::{EFFECTSCRIPT_CATALOG, Message};

pub static Effect_can_fail_with_0_which_is_not_declared_in_the_throws_clause_of_1: &Message =
    &EFFECTSCRIPT_CATALOG[0];
pub static Effect_requires_0_which_is_not_declared_in_the_requires_clause_of_1: &Message =
    &EFFECTSCRIPT_CATALOG[1];
pub static Thrown_value_of_type_0_is_not_declared_in_the_throws_clause_of_1: &Message =
    &EFFECTSCRIPT_CATALOG[2];
pub static A_Promise_cannot_be_awaited_in_an_effect_function_Convert_it_with_Effect_promise_or_Effect_tryPromise: &Message = &EFFECTSCRIPT_CATALOG[3];
pub static Expression_of_type_0_is_not_an_Effect_and_cannot_be_awaited_in_an_effect_function:
    &Message = &EFFECTSCRIPT_CATALOG[4];
pub static A_use_expression_is_only_allowed_in_the_body_of_an_effect_function: &Message =
    &EFFECTSCRIPT_CATALOG[5];
pub static X_0_is_not_supported_in_effect_functions_yet: &Message = &EFFECTSCRIPT_CATALOG[6];
pub static An_effect_function_must_be_declared_async: &Message = &EFFECTSCRIPT_CATALOG[7];
pub static An_effect_function_with_a_requires_or_throws_clause_must_declare_its_success_type:
    &Message = &EFFECTSCRIPT_CATALOG[8];
pub static An_effect_function_cannot_be_a_generator: &Message = &EFFECTSCRIPT_CATALOG[9];
pub static X_0_cannot_jump_out_of_a_try_catch_or_finally_block_in_an_effect_function_yet: &Message =
    &EFFECTSCRIPT_CATALOG[10];
pub static Expression_of_type_0_is_not_an_Effect_service_and_cannot_be_used_with_use: &Message =
    &EFFECTSCRIPT_CATALOG[11];
pub static A_finally_block_in_an_effect_function_cannot_fail_Handle_0_inside_the_block: &Message =
    &EFFECTSCRIPT_CATALOG[12];
pub static A_return_statement_cannot_be_used_in_a_finally_block_of_an_effect_function: &Message =
    &EFFECTSCRIPT_CATALOG[13];
