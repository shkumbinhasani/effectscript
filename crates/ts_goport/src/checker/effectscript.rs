use crate::effectscript::{diag as es_diag, is_effectscript_source_file};
use crate::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EffectYieldKind {
    Await,
    Use,
    Throw,
    Try,
}

pub struct EffectChannels {
    pub errors: TypeId,
    pub requirements: TypeId,
}

#[must_use]
pub fn effect_yield_kind(node: Node) -> Option<EffectYieldKind> {
    if node.asterisk_token().is_nil() || node.pos() < 0 {
        return None;
    }
    let file = get_source_file_of_node(node);
    if file.is_nil() || !is_effectscript_source_file(file) {
        return None;
    }
    let text = source_file_text(file);
    let start = skip_trivia(&text, node.pos()) as usize;
    let rest = text.get(start..)?;
    let keyword_is = |keyword: &str| {
        rest.starts_with(keyword)
            && !rest[keyword.len()..]
                .chars()
                .next()
                .is_some_and(|c| c == '_' || c == '$' || c.is_alphanumeric())
    };
    if keyword_is("await") {
        Some(EffectYieldKind::Await)
    } else if keyword_is("use") {
        Some(EffectYieldKind::Use)
    } else if keyword_is("throw") {
        Some(EffectYieldKind::Throw)
    } else if keyword_is("try") {
        Some(EffectYieldKind::Try)
    } else {
        None
    }
}

fn effect_function_name(generator: Node) -> String {
    let function = get_containing_function(generator);
    if function.is_some() && function.name().is_some() {
        function.name().text().to_string()
    } else {
        "effect function".to_string()
    }
}

impl Checker {
    pub fn check_effect_yield_operand(
        &mut self,
        node: Node,
        kind: EffectYieldKind,
        operand_type: TypeId,
    ) -> bool {
        if kind == EffectYieldKind::Try || self.is_type_any(operand_type) {
            return true;
        }
        if kind == EffectYieldKind::Throw {
            self.check_effect_finally(node, operand_type);
            return true;
        }
        if kind == EffectYieldKind::Await && self.is_thenable_type(operand_type) {
            self.error(
                node.expression(),
                es_diag::A_Promise_cannot_be_awaited_in_an_effect_function_Convert_it_with_Effect_promise_or_Effect_tryPromise,
                args![],
            );
            return false;
        }
        if self.get_effect_channels(operand_type).is_none() {
            let message = if kind == EffectYieldKind::Use {
                es_diag::Expression_of_type_0_is_not_an_Effect_service_and_cannot_be_used_with_use
            } else {
                es_diag::Expression_of_type_0_is_not_an_Effect_and_cannot_be_awaited_in_an_effect_function
            };
            let type_text = self.type_to_string(operand_type);
            self.error(node.expression(), message, args![type_text]);
            return false;
        }
        self.check_effect_finally(node, operand_type);
        true
    }

    fn check_effect_finally(&mut self, node: Node, operand_type: TypeId) {
        if !is_in_effect_finally(get_containing_function(node)) {
            return;
        }
        let Some(channels) = self.get_effect_channels(operand_type) else {
            return;
        };
        let never = self.never_type;
        let errors = self.undeclared_members(channels.errors, never);
        if errors.is_empty() {
            return;
        }
        let text = self.join_types(&errors);
        let mut error_node = node.expression();
        if effect_yield_kind(node) == Some(EffectYieldKind::Throw)
            && let Some(argument) = error_node.arguments().into_iter().next()
        {
            error_node = argument;
        }
        self.error(
            error_node,
            es_diag::A_finally_block_in_an_effect_function_cannot_fail_Handle_0_inside_the_block,
            args![text],
        );
    }

    pub fn get_effect_channels(&mut self, t: TypeId) -> Option<EffectChannels> {
        let apparent = self.get_apparent_type(t);
        for property in self.get_properties_of_type(apparent).iter().copied() {
            let property_type = self.get_type_of_symbol(property);
            let errors = self.get_variance_channel(property_type, "_E");
            let requirements = self.get_variance_channel(property_type, "_R");
            if let (Some(errors), Some(requirements)) = (errors, requirements)
                && self.get_variance_channel(property_type, "_A").is_some()
            {
                return Some(EffectChannels {
                    errors,
                    requirements,
                });
            }
        }
        None
    }

    fn get_variance_channel(&mut self, t: TypeId, name: &str) -> Option<TypeId> {
        if !self.ty(t).flags.intersects(TypeFlags::OBJECT) {
            return None;
        }
        let symbol = self.get_property_of_type(t, name);
        if symbol.is_nil() {
            return None;
        }
        let channel = self.get_type_of_symbol(symbol);
        let signatures = self.get_signatures_of_type(channel, SignatureKind::CALL);
        let signature = *signatures.first()?;
        Some(self.get_return_type_of_signature(signature))
    }

    fn undeclared_members(&mut self, actual: TypeId, declared: TypeId) -> Vec<TypeId> {
        let members: Vec<TypeId> = if self.ty(actual).flags.intersects(TypeFlags::UNION) {
            self.ty(actual).types().to_vec()
        } else {
            vec![actual]
        };
        members
            .into_iter()
            .filter(|&member| {
                !self.ty(member).flags.intersects(TypeFlags::NEVER)
                    && !self.is_type_assignable_to(member, declared)
            })
            .collect()
    }

    fn join_types(&mut self, types: &[TypeId]) -> String {
        types
            .iter()
            .map(|&t| self.type_to_string(t))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    pub fn check_effect_yield_contract(
        &mut self,
        node: Node,
        kind: EffectYieldKind,
        operand_type: TypeId,
        signature_yield_type: TypeId,
    ) -> bool {
        let Some(declared) = self.get_declared_effect_channels(signature_yield_type) else {
            return false;
        };
        let Some(actual) = self.get_effect_channels(operand_type) else {
            return false;
        };
        let generator = get_containing_function(node);
        let function_name = effect_function_name(generator);
        let undeclared_errors = self.undeclared_members(actual.errors, declared.errors);
        let undeclared_requirements =
            self.undeclared_members(actual.requirements, declared.requirements);
        if undeclared_errors.is_empty() && undeclared_requirements.is_empty() {
            return true;
        }
        let mut error_node = node.expression();
        if kind == EffectYieldKind::Throw {
            let arguments = error_node.arguments();
            if let Some(argument) = arguments.into_iter().next() {
                error_node = argument;
            }
        }
        if !undeclared_errors.is_empty() {
            let text = self.join_types(&undeclared_errors);
            let message = if kind == EffectYieldKind::Throw {
                es_diag::Thrown_value_of_type_0_is_not_declared_in_the_throws_clause_of_1
            } else {
                es_diag::Effect_can_fail_with_0_which_is_not_declared_in_the_throws_clause_of_1
            };
            self.error(error_node, message, args![text, function_name]);
        }
        if !undeclared_requirements.is_empty() {
            let text = self.join_types(&undeclared_requirements);
            self.error(
                error_node,
                es_diag::Effect_requires_0_which_is_not_declared_in_the_requires_clause_of_1,
                args![text, function_name],
            );
        }
        true
    }

    fn get_declared_effect_channels(
        &mut self,
        signature_yield_type: TypeId,
    ) -> Option<EffectChannels> {
        if !self
            .ty(signature_yield_type)
            .object_flags()
            .intersects(ObjectFlags::REFERENCE)
        {
            return None;
        }
        let arguments = self.get_type_arguments(signature_yield_type);
        let wrapped = *arguments.first()?;
        self.get_effect_channels(wrapped)
    }
}

fn is_effect_call(call: Node, member: &str) -> bool {
    if call.kind() != SyntaxKind::CallExpression {
        return false;
    }
    let callee = call.expression();
    callee.kind() == SyntaxKind::PropertyAccessExpression
        && callee.pos() < 0
        && callee.name().text() == member
}

fn is_in_effect_finally(generator: Node) -> bool {
    if generator.is_nil() {
        return false;
    }
    let gen_call = generator.parent();
    is_effect_call(gen_call, "gen") && is_effect_call(gen_call.parent(), "orDie")
}
