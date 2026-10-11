use crate::effectscript::{
    EFFECT_ALIAS, EFFECT_MODULE, PRELUDE, UTILS_MODULE, YIELD_WRAP_ALIAS, diag as es_diag,
    is_effectscript_file_name,
};
use crate::frontend::prelude::*;

#[derive(Clone, Copy, Default)]
struct EfRegion {
    finally: bool,
    loops: u32,
    switches: u32,
    has_return: bool,
    breaks: bool,
    continues: bool,
}

#[derive(Clone, Default)]
pub struct EfFrame {
    effect: bool,
    arrow: bool,
    body_started: bool,
    success_type: Node,
    regions: Vec<EfRegion>,
    loops: u32,
    switches: u32,
}

#[derive(Default)]
pub struct EfState {
    pub enabled: bool,
    frames: Vec<EfFrame>,
    pending_effect_body: bool,
    pending_success_type: Node,
    completions: u32,
    return_only_tries: Vec<(Node, Node, String)>,
    uses_effect: bool,
    uses_yield_wrap: bool,
    anchor: i32,
    signature_aliases: Vec<Node>,
    body_aliases: Vec<Node>,
}

impl EfState {
    pub fn reset(&mut self, file_name: &str) {
        *self = EfState {
            enabled: is_effectscript_file_name(file_name),
            ..EfState::default()
        };
    }
}

impl<'a> Parser<'a> {
    fn ef_in_effect_body(&self) -> bool {
        self.ef.enabled && self.ef.frames.last().is_some_and(|frame| frame.effect)
    }

    pub fn ef_enter_function_body(&mut self) -> bool {
        if !self.ef.enabled {
            return false;
        }
        if let Some(top) = self.ef.frames.last_mut()
            && top.arrow
            && !top.body_started
        {
            top.body_started = true;
            return false;
        }
        let effect = std::mem::take(&mut self.ef.pending_effect_body);
        let success_type = std::mem::take(&mut self.ef.pending_success_type);
        self.ef.frames.push(EfFrame {
            effect,
            success_type,
            body_started: true,
            ..EfFrame::default()
        });
        true
    }

    pub fn ef_exit_function_body(&mut self, entered: bool) {
        if entered {
            self.ef.frames.pop();
        }
    }

    pub fn ef_enter_arrow(&mut self) -> bool {
        if !self.ef.enabled {
            return false;
        }
        self.ef.frames.push(EfFrame {
            arrow: true,
            ..EfFrame::default()
        });
        true
    }

    pub fn ef_enter_barrier(&mut self) -> bool {
        if !self.ef.enabled {
            return false;
        }
        self.ef.frames.push(EfFrame {
            body_started: true,
            ..EfFrame::default()
        });
        true
    }

    fn ef_this_is_effect_function(&self) -> bool {
        self.ef
            .frames
            .iter()
            .rev()
            .find(|frame| !frame.arrow)
            .is_some_and(|frame| frame.effect)
    }

    pub fn ef_check_unsupported_reference(&mut self, name: &str) {
        if self.ef.enabled && self.ef_this_is_effect_function() {
            let range = self.scanner.token_range();
            self.parse_error_at_range(
                range,
                es_diag::X_0_is_not_supported_in_effect_functions_yet,
                vec![name.to_string()],
            );
        }
    }

    fn ef_region_mut(&mut self) -> Option<&mut EfRegion> {
        if !self.ef_in_effect_body() {
            return None;
        }
        self.ef.frames.last_mut()?.regions.last_mut()
    }

    fn ef_push_region(&mut self, finally: bool) {
        if let Some(frame) = self.ef.frames.last_mut() {
            frame.regions.push(EfRegion {
                finally,
                ..EfRegion::default()
            });
        }
    }

    fn ef_pop_region(&mut self) -> EfRegion {
        self.ef
            .frames
            .last_mut()
            .and_then(|frame| frame.regions.pop())
            .unwrap_or_default()
    }

    fn ef_in_region(&self) -> bool {
        self.ef_in_effect_body()
            && self
                .ef
                .frames
                .last()
                .is_some_and(|frame| !frame.regions.is_empty())
    }

    fn ef_jump_counts(&mut self) -> Option<(&mut u32, &mut u32)> {
        if !self.ef_in_effect_body() {
            return None;
        }
        let frame = self.ef.frames.last_mut()?;
        Some(match frame.regions.last_mut() {
            Some(region) => (&mut region.loops, &mut region.switches),
            None => (&mut frame.loops, &mut frame.switches),
        })
    }

    pub fn ef_enter_jump_target(&mut self, is_loop: bool) -> bool {
        let Some((loops, switches)) = self.ef_jump_counts() else {
            return false;
        };
        if is_loop {
            *loops += 1;
        } else {
            *switches += 1;
        }
        true
    }

    pub fn ef_exit_jump_target(&mut self, entered: bool, is_loop: bool) {
        if !entered {
            return;
        }
        if let Some((loops, switches)) = self.ef_jump_counts() {
            if is_loop {
                *loops -= 1;
            } else {
                *switches -= 1;
            }
        }
    }

    fn ef_jump_leaves_region(&self, keyword: SyntaxKind) -> Option<bool> {
        if !self.ef_in_region() {
            return None;
        }
        let frame = self.ef.frames.last()?;
        let targets = |loops: u32, switches: u32| {
            if keyword == SyntaxKind::ContinueKeyword {
                loops > 0
            } else {
                loops + switches > 0
            }
        };
        let region = frame.regions.last()?;
        if targets(region.loops, region.switches) {
            return None;
        }
        let has_target = frame.regions[..frame.regions.len() - 1]
            .iter()
            .any(|outer| targets(outer.loops, outer.switches))
            || targets(frame.loops, frame.switches);
        Some(has_target && !region.finally)
    }

    pub fn ef_lower_jump(&mut self, keyword: SyntaxKind, label: Node, pos: i32) -> Option<Node> {
        let leaves = self.ef_jump_leaves_region(keyword);
        let text = if keyword == SyntaxKind::ContinueKeyword {
            "continue"
        } else {
            "break"
        };
        let in_region = self.ef_in_region();
        if !in_region || (label.is_nil() && leaves.is_none()) {
            return None;
        }
        if label.is_some() || leaves == Some(false) {
            let start = skip_trivia(self.source_text, pos);
            self.parse_error_at(
                start,
                start + text.len() as i32,
                es_diag::X_0_cannot_jump_out_of_a_try_catch_or_finally_block_in_an_effect_function_yet,
                vec![text.to_string()],
            );
            return None;
        }
        Some(self.ef_jump_completion(keyword, TextRange::new(pos, self.node_pos())))
    }

    fn ef_jump_completion(&mut self, keyword: SyntaxKind, loc: TextRange) -> Node {
        let region = self
            .ef
            .frames
            .last_mut()
            .and_then(|frame| frame.regions.last_mut())
            .expect("a try region");
        let key = if keyword == SyntaxKind::ContinueKeyword {
            region.continues = true;
            "k"
        } else {
            region.breaks = true;
            "b"
        };
        let name = self.ef_identifier(key);
        let value = self.factory.new_keyword_expression(SyntaxKind::TrueKeyword);
        let value = self.ef_finish_synthesized(value);
        let property = self.factory.new_property_assignment(
            ModifierList::NIL,
            name,
            Node::NIL,
            Node::NIL,
            value,
        );
        let property = self.ef_finish_synthesized(property);
        let properties = self.ef_node_list(&[property]);
        let object = self
            .factory
            .new_object_literal_expression(properties, false);
        let object = self.ef_finish_synthesized(object);
        let statement = self.factory.new_return_statement(object);
        self.ef_finish_at(statement, loc, NodeFlags::NONE)
    }

    fn ef_dispatch_jump(&mut self, keyword: SyntaxKind) -> Node {
        if self.ef_jump_leaves_region(keyword) == Some(true) {
            let statement = self.ef_jump_completion(keyword, TextRange::undefined());
            set_node_flags(statement, statement.flags() | NodeFlags::REPARSED);
            return statement;
        }
        let statement = if keyword == SyntaxKind::ContinueKeyword {
            self.factory.new_continue_statement(Node::NIL)
        } else {
            self.factory.new_break_statement(Node::NIL)
        };
        self.ef_finish_synthesized(statement)
    }

    fn ef_completion_has(&mut self, completion: &str, key: &str) -> Node {
        let key = self.factory.new_string_literal(key, TokenFlags::NONE);
        let key = self.ef_finish_synthesized(key);
        let operator = self.factory.new_token(SyntaxKind::InKeyword);
        let operator = self.ef_finish_synthesized(operator);
        let object = self.ef_identifier(completion);
        let test =
            self.factory
                .new_binary_expression(ModifierList::NIL, key, Node::NIL, operator, object);
        self.ef_finish_synthesized(test)
    }

    fn ef_dispatch(&mut self, completion: &str, region: EfRegion) -> Node {
        let mut statements = Vec::new();
        let kinds = usize::from(region.has_return)
            + usize::from(region.breaks)
            + usize::from(region.continues);
        let mut push = |parser: &mut Self, key: &str, statement: Node| {
            let statement = if kinds == 1 {
                statement
            } else {
                let test = parser.ef_completion_has(completion, key);
                let guarded = parser.factory.new_if_statement(test, statement, Node::NIL);
                parser.ef_finish_synthesized(guarded)
            };
            statements.push(statement);
        };
        if region.has_return {
            let result = if self.ef_in_region() {
                self.ef_identifier(completion)
            } else {
                let object = self.ef_identifier(completion);
                let member = self.ef_identifier("v");
                let access = self.factory.new_property_access_expression(
                    object,
                    Node::NIL,
                    member,
                    NodeFlags::NONE,
                );
                self.ef_finish_synthesized(access)
            };
            if self.ef_in_region() {
                let frame = self.ef.frames.last_mut().expect("an effect frame");
                for outer in &mut frame.regions {
                    outer.has_return = true;
                }
            }
            let statement = self.factory.new_return_statement(result);
            let statement = self.ef_finish_synthesized(statement);
            push(self, "v", statement);
        }
        if region.breaks {
            let statement = self.ef_dispatch_jump(SyntaxKind::BreakKeyword);
            push(self, "b", statement);
        }
        if region.continues {
            let statement = self.ef_dispatch_jump(SyntaxKind::ContinueKeyword);
            push(self, "k", statement);
        }
        let body = if statements.len() == 1 {
            statements[0]
        } else {
            let list = self.ef_node_list(&statements);
            let block = self.factory.new_block(list, true);
            self.ef_finish_synthesized(block)
        };
        let condition = self.ef_identifier(completion);
        let dispatch = self.factory.new_if_statement(condition, body, Node::NIL);
        self.ef_finish_synthesized(dispatch)
    }

    pub fn ef_lower_return_value(&mut self, pos: i32, expression: Node) -> Node {
        if !self.ef_in_region() {
            return expression;
        }
        let frame = self.ef.frames.last_mut().expect("an effect frame");
        if frame.regions.last().is_some_and(|region| region.finally) {
            let start = skip_trivia(self.source_text, pos);
            self.parse_error_at(
                start,
                start + "return".len() as i32,
                es_diag::A_return_statement_cannot_be_used_in_a_finally_block_of_an_effect_function,
                vec![],
            );
            return expression;
        }
        for region in &mut frame.regions {
            region.has_return = true;
        }
        let success_type = frame.success_type;
        let value = if expression.is_nil() {
            self.ef_identifier("undefined")
        } else {
            expression
        };
        let loc = if expression.is_nil() {
            TextRange::undefined()
        } else {
            expression.loc()
        };
        let declared = if success_type.is_some() {
            let declared = self.factory.deep_clone_with_location(success_type);
            set_node_flags(declared, declared.flags() | NodeFlags::REPARSED);
            declared
        } else {
            Node::NIL
        };
        let name = self.ef_identifier("v");
        let property = self.factory.new_property_assignment(
            ModifierList::NIL,
            name,
            Node::NIL,
            declared,
            value,
        );
        let property = self.ef_finish_at(property, loc, NodeFlags::NONE);
        let properties = self.factory.new_node_list_with_loc(&[property], loc);
        let object = self
            .factory
            .new_object_literal_expression(properties, false);
        self.ef_finish_at(object, loc, NodeFlags::NONE)
    }

    fn ef_gen_call(&mut self, block: Node) -> Node {
        let loc = TextRange::new(skip_trivia(self.source_text, block.pos()), block.end());
        let star = self.factory.new_token(SyntaxKind::AsteriskToken);
        let star = self.ef_finish_synthesized(star);
        let parameters = self
            .factory
            .new_node_list_with_loc(&[], TextRange::new(loc.pos(), loc.pos()));
        let generator = self.factory.new_function_expression(
            ModifierList::NIL,
            star,
            Node::NIL,
            NodeList::NIL,
            parameters,
            Node::NIL,
            Node::NIL,
            block,
        );
        let generator = self.ef_finish_at(generator, loc, NodeFlags::NONE);
        self.ef_effect_call("gen", &[generator], loc)
    }

    fn ef_effect_call(&mut self, member: &str, arguments: &[Node], loc: TextRange) -> Node {
        let callee = self.ef_effect_member(member);
        let arguments = self.factory.new_node_list_with_loc(arguments, loc);
        let call = self.factory.new_call_expression(
            callee,
            Node::NIL,
            NodeList::NIL,
            arguments,
            NodeFlags::NONE,
        );
        self.ef_finish_at(call, loc, NodeFlags::NONE)
    }

    pub fn ef_try_statement(&mut self) -> bool {
        self.ef_in_effect_body()
    }

    pub fn parse_effect_try_statement(&mut self, pos: i32, jsdoc: JsdocScannerInfo) -> Node {
        self.parse_expected(SyntaxKind::TryKeyword);
        self.ef_push_region(false);
        let try_block = self.parse_block(false, None);
        let mut region = self.ef_pop_region();
        let mut catch_clause = Node::NIL;
        if self.token == SyntaxKind::CatchKeyword {
            self.ef_push_region(false);
            catch_clause = self.parse_catch_clause();
            let catch_region = self.ef_pop_region();
            region.has_return |= catch_region.has_return;
            region.breaks |= catch_region.breaks;
            region.continues |= catch_region.continues;
        }
        let mut finally_block = Node::NIL;
        if catch_clause.is_nil() || self.token == SyntaxKind::FinallyKeyword {
            self.parse_expected_with_diagnostic(
                SyntaxKind::FinallyKeyword,
                Some(diag::X_catch_or_finally_expected),
                true,
            );
            self.ef_push_region(true);
            finally_block = self.parse_block(false, None);
            self.ef_pop_region();
        }
        let loc = TextRange::new(pos, self.node_pos());
        let try_start = skip_trivia(self.source_text, pos);
        let mut effect = self.ef_gen_call(try_block);
        if catch_clause.is_some() {
            effect = self.ef_attach_catch(effect, catch_clause, try_start);
        }
        if finally_block.is_some() {
            let finalizer = self.ef_gen_call(finally_block);
            let finalizer_loc = finalizer.loc();
            let finalizer = self.ef_effect_call("orDie", &[finalizer], finalizer_loc);
            let span = TextRange::new(try_start, finalizer.end());
            effect = self.ef_effect_call("ensuring", &[effect, finalizer], span);
        }
        let yielded = self.ef_yield_star(effect, loc);
        if !(region.has_return || region.breaks || region.continues) {
            let statement = self.factory.new_expression_statement(yielded);
            let statement = self.ef_finish_at(statement, loc, NodeFlags::NONE);
            self.with_js_doc(statement, jsdoc);
            return statement;
        }
        self.ef.completions += 1;
        let completion = format!("__es_c{}", self.ef.completions);
        let name = self.ef_identifier(&completion);
        let declaration =
            self.factory
                .new_variable_declaration(name, Node::NIL, Node::NIL, yielded);
        let declaration = self.ef_finish_at(declaration, loc, NodeFlags::NONE);
        let declarations = self.factory.new_node_list_with_loc(&[declaration], loc);
        let list = self
            .factory
            .new_variable_declaration_list(declarations, NodeFlags::CONST);
        let list = self.ef_finish_at(list, loc, NodeFlags::CONST);
        let variable = self.factory.new_variable_statement(ModifierList::NIL, list);
        let variable = self.ef_finish_at(variable, loc, NodeFlags::NONE);
        let return_only = region.has_return && !region.breaks && !region.continues;
        let at_function_level = !self.ef_in_region();
        let dispatch = self.ef_dispatch(&completion, region);
        let statements = self
            .factory
            .new_node_list_with_loc(&[variable, dispatch], loc);
        let block = self.factory.new_block(statements, true);
        let block = self.ef_finish_at(block, loc, NodeFlags::NONE);
        self.with_js_doc(block, jsdoc);
        if return_only && at_function_level {
            self.ef
                .return_only_tries
                .push((block, variable, completion));
        }
        block
    }

    fn ef_return_final_try(&mut self, body: Node) -> Node {
        let tries = std::mem::take(&mut self.ef.return_only_tries);
        let statements: Vec<Node> = body.statements().into_iter().collect();
        let Some(&last) = statements.last() else {
            return body;
        };
        let Some((_, variable, completion)) =
            tries.into_iter().find(|(block, _, _)| *block == last)
        else {
            return body;
        };
        let object = self.ef_identifier(&completion);
        let question_dot = self.factory.new_token(SyntaxKind::QuestionDotToken);
        let question_dot = self.ef_finish_synthesized(question_dot);
        let member = self.ef_identifier("v");
        let access = self.factory.new_property_access_expression(
            object,
            question_dot,
            member,
            NodeFlags::OPTIONAL_CHAIN,
        );
        let access = self.ef_finish_synthesized(access);
        let return_statement = self.factory.new_return_statement(access);
        let return_statement = self.ef_finish_synthesized(return_statement);
        let inner = self
            .factory
            .new_node_list_with_loc(&[variable, return_statement], last.loc());
        let replacement = self.factory.new_block(inner, true);
        let replacement = self.ef_finish_at(replacement, last.loc(), NodeFlags::NONE);
        let mut rewritten = statements;
        *rewritten.last_mut().expect("a last statement") = replacement;
        let list = self
            .factory
            .new_node_list_with_loc(&rewritten, body.statement_list().loc());
        let block = self.factory.new_block(list, true);
        self.ef_finish_at(block, body.loc(), NodeFlags::NONE)
    }

    fn ef_attach_catch(&mut self, effect: Node, catch_clause: Node, try_start: i32) -> Node {
        let declaration = catch_clause.variable_declaration();
        let block = catch_clause.block();
        let (name, type_node, parameter_loc) = if declaration.is_some() {
            (declaration.name(), declaration.type_(), declaration.loc())
        } else {
            let name = self.ef_identifier("__es_error");
            (name, Node::NIL, TextRange::undefined())
        };
        let parameter = self.factory.new_parameter_declaration(
            ModifierList::NIL,
            Node::NIL,
            name,
            Node::NIL,
            type_node,
            Node::NIL,
        );
        let parameter = self.ef_finish_at(parameter, parameter_loc, NodeFlags::NONE);
        let block_start = skip_trivia(self.source_text, block.pos());
        let list_loc = if parameter_loc.pos() < 0 {
            TextRange::new(block_start, block_start)
        } else {
            parameter_loc
        };
        let parameters = self.factory.new_node_list_with_loc(&[parameter], list_loc);
        let body = self.ef_gen_call(block);
        let arrow_token = self.factory.new_token(SyntaxKind::EqualsGreaterThanToken);
        let arrow_position = skip_trivia(self.source_text, block.pos());
        let arrow_token = self.ef_finish_at(
            arrow_token,
            TextRange::new(arrow_position, arrow_position),
            NodeFlags::REPARSED,
        );
        let catch_loc = TextRange::new(
            skip_trivia(self.source_text, catch_clause.pos()),
            catch_clause.end(),
        );
        let handler = self.factory.new_arrow_function(
            ModifierList::NIL,
            NodeList::NIL,
            parameters,
            Node::NIL,
            Node::NIL,
            arrow_token,
            body,
        );
        let handler = self.ef_finish_at(handler, catch_loc, NodeFlags::NONE);
        let span = TextRange::new(try_start, handler.end());
        self.ef_effect_call("catchAll", &[effect, handler], span)
    }

    pub fn ef_check_for_await(&mut self, await_token: Node) {
        if await_token.is_some() && self.ef_in_effect_body() {
            self.parse_error_at(
                skip_trivia(self.source_text, await_token.pos()),
                await_token.end(),
                es_diag::X_0_is_not_supported_in_effect_functions_yet,
                vec!["for await".to_string()],
            );
        }
    }

    fn ef_token_is(&self, text: &str) -> bool {
        self.token == SyntaxKind::Identifier && self.scanner.token_value() == text
    }

    pub fn is_effect_function_start(&mut self) -> bool {
        self.ef.enabled
            && self.ef_token_is("effect")
            && self.look_ahead(Parser::ef_next_token_starts_function)
    }

    pub fn ef_next_token_starts_function(&mut self) -> bool {
        self.next_token();
        !self.has_preceding_line_break()
            && (self.token == SyntaxKind::AsyncKeyword || self.token == SyntaxKind::FunctionKeyword)
    }

    pub fn scan_effect_function_start(&mut self) -> bool {
        self.ef.enabled && self.ef_token_is("effect") && self.ef_next_token_starts_function()
    }

    pub fn is_use_expression(&mut self) -> bool {
        self.ef.enabled
            && self.ef_token_is("use")
            && self.look_ahead(|p| {
                p.next_token();
                p.token == SyntaxKind::Identifier && !p.has_preceding_line_break()
            })
    }

    fn ef_anchor_at(&mut self, pos: i32) {
        self.ef.anchor = skip_trivia(self.source_text, pos);
    }

    fn ef_anchor_range(&self) -> TextRange {
        TextRange::undefined()
    }

    fn ef_node_list(&mut self, nodes: &[Node]) -> NodeList {
        let loc = match (nodes.first(), nodes.last()) {
            (Some(first), Some(last)) => {
                TextRange::new(first.pos().min(self.ef.anchor), last.end())
            }
            _ => self.ef_anchor_range(),
        };
        self.factory.new_node_list_with_loc(nodes, loc)
    }

    fn ef_finish_synthesized(&mut self, node: Node) -> Node {
        let loc = self.ef_anchor_range();
        let flags = self.context_flags | NodeFlags::REPARSED;
        if !finish_store_node(node, loc, flags) {
            set_node_loc(node, loc);
            set_node_flags(node, node.flags() | flags);
        }
        self.override_parent_in_immediate_children(node);
        node
    }

    fn ef_finish_at(&mut self, node: Node, loc: TextRange, extra: NodeFlags) -> Node {
        let mut flags = self.context_flags | extra;
        if loc.pos() < 0 {
            flags |= NodeFlags::REPARSED;
        }
        if !finish_store_node(node, loc, flags) {
            set_node_loc(node, loc);
            set_node_flags(node, node.flags() | flags);
        }
        self.override_parent_in_immediate_children(node);
        node
    }

    fn ef_identifier(&mut self, text: &str) -> Node {
        let node = self.factory.new_identifier(text);
        let node = self.ef_finish_synthesized(node);
        if text == EFFECT_ALIAS {
            self.ef.body_aliases.push(node);
        }
        node
    }

    fn ef_effect_member(&mut self, member: &str) -> Node {
        let alias = self.ef_identifier(EFFECT_ALIAS);
        let name = self.ef_identifier(member);
        let node =
            self.factory
                .new_property_access_expression(alias, Node::NIL, name, NodeFlags::NONE);
        self.ef_finish_synthesized(node)
    }

    fn ef_never(&mut self) -> Node {
        let node = self.factory.new_keyword_type_node(SyntaxKind::NeverKeyword);
        self.ef_finish_synthesized(node)
    }

    fn ef_type_reference(&mut self, name: Node, arguments: &[Node]) -> Node {
        let arguments = self.ef_node_list(arguments);
        let node = self.factory.new_type_reference_node(name, arguments);
        self.ef_finish_synthesized(node)
    }

    fn ef_effect_type(&mut self, success: Node, error: Node, requirements: Node) -> Node {
        let alias = self.ef_identifier(EFFECT_ALIAS);
        let member = self.ef_identifier("Effect");
        let name = self.factory.new_qualified_name(alias, member);
        let name = self.ef_finish_synthesized(name);
        self.ef_type_reference(name, &[success, error, requirements])
    }

    fn ef_clone_or_never(&mut self, node: Node) -> Node {
        if node.is_nil() {
            self.ef_never()
        } else {
            self.factory.deep_clone_with_location(node)
        }
    }

    fn ef_yield_star(&mut self, operand: Node, loc: TextRange) -> Node {
        self.ef_anchor_at(loc.pos());
        let star = self.factory.new_token(SyntaxKind::AsteriskToken);
        let star = self.ef_finish_synthesized(star);
        let node = self.factory.new_yield_expression(star, operand);
        self.ef_finish_at(node, loc, NodeFlags::YIELD_CONTEXT)
    }

    pub fn ef_lower_await(&mut self, pos: i32, operand: Node) -> Option<Node> {
        if !self.ef_in_effect_body() || !self.in_await_context() {
            return None;
        }
        let loc = TextRange::new(pos, self.node_pos());
        let yield_node = self.ef_yield_star(operand, loc);
        let node = self.factory.new_parenthesized_expression(yield_node);
        Some(self.ef_finish_at(node, loc, NodeFlags::NONE))
    }

    pub fn parse_use_expression(&mut self) -> Node {
        let pos = self.node_pos();
        let keyword = self.scanner.token_range();
        self.next_token();
        let operand = self.parse_simple_unary_expression();
        if !self.ef_in_effect_body() {
            self.parse_error_at_range(
                keyword,
                es_diag::A_use_expression_is_only_allowed_in_the_body_of_an_effect_function,
                vec![],
            );
        }
        let loc = TextRange::new(pos, self.node_pos());
        let yield_node = self.ef_yield_star(operand, loc);
        let node = self.factory.new_parenthesized_expression(yield_node);
        self.ef_finish_at(node, loc, NodeFlags::NONE)
    }

    pub fn ef_lower_throw(&mut self, pos: i32, expression: Node) -> Option<Node> {
        if !self.ef_in_effect_body() || expression.is_nil() {
            return None;
        }
        self.ef_anchor_at(expression.pos());
        let callee = self.ef_effect_member("fail");
        let arguments = self
            .factory
            .new_node_list_with_loc(&[expression], expression.loc());
        let call = self.factory.new_call_expression(
            callee,
            Node::NIL,
            NodeList::NIL,
            arguments,
            NodeFlags::NONE,
        );
        let call = self.ef_finish_at(call, expression.loc(), NodeFlags::NONE);
        let loc = TextRange::new(pos, self.node_pos());
        let yield_node = self.ef_yield_star(call, loc);
        let node = self.factory.new_return_statement(yield_node);
        Some(self.ef_finish_at(node, loc, NodeFlags::NONE))
    }

    pub fn parse_effect_function_declaration(
        &mut self,
        pos: i32,
        jsdoc: JsdocScannerInfo,
        modifiers: ModifierList,
    ) -> Node {
        let effect_keyword = self.scanner.token_range();
        self.next_token();
        if !self.parse_optional(SyntaxKind::AsyncKeyword) {
            self.parse_error_at_range(
                effect_keyword,
                es_diag::An_effect_function_must_be_declared_async,
                vec![],
            );
        }
        self.parse_expected(SyntaxKind::FunctionKeyword);
        let asterisk_token = self.parse_optional_token(SyntaxKind::AsteriskToken);
        if asterisk_token.is_some() {
            self.parse_error_at(
                skip_trivia(self.source_text, asterisk_token.pos()),
                asterisk_token.end(),
                es_diag::An_effect_function_cannot_be_a_generator,
                vec![],
            );
        }
        let mut name = Node::NIL;
        if modifiers.is_nil()
            || !modifiers
                .modifier_flags()
                .intersects(ModifierFlags::DEFAULT)
            || self.is_binding_identifier()
        {
            name = self.parse_binding_identifier();
        }
        let type_parameters = self.parse_type_parameters();
        let save_context_flags = self.context_flags;
        if modifiers.is_some() && modifiers.modifier_flags().intersects(ModifierFlags::EXPORT) {
            self.set_context_flags(NodeFlags::AWAIT_CONTEXT, true);
        }
        let parameters = self.parse_parameters(ParseFlags::AWAIT);
        let success_type = self.parse_return_type(SyntaxKind::ColonToken, false);
        let mut clause_keyword = None;
        let mut requirements = Node::NIL;
        if self.ef_token_is("requires") {
            clause_keyword.get_or_insert(self.scanner.token_range());
            self.next_token();
            requirements = self.parse_type();
        }
        let mut errors = Node::NIL;
        if self.ef_token_is("throws") {
            clause_keyword.get_or_insert(self.scanner.token_range());
            self.next_token();
            errors = self.parse_type();
        }
        if success_type.is_nil()
            && let Some(range) = clause_keyword
        {
            self.parse_error_at_range(
                range,
                es_diag::An_effect_function_with_a_requires_or_throws_clause_must_declare_its_success_type,
                vec![],
            );
        }
        self.ef.uses_effect = true;
        self.ef.pending_effect_body = true;
        self.ef.pending_success_type = success_type;
        let body =
            self.parse_function_block_or_semicolon(ParseFlags::AWAIT, Some(diag::X_or_expected));
        self.ef.pending_effect_body = false;
        self.ef.pending_success_type = Node::NIL;
        self.context_flags = save_context_flags;

        let body = if body.is_some() {
            self.ef_return_final_try(body)
        } else {
            body
        };
        let explicit = success_type.is_some();
        let generator_type = if explicit && body.is_some() {
            self.ef_anchor_at(body.pos());
            self.ef.uses_yield_wrap = true;
            let unknown = self
                .factory
                .new_keyword_type_node(SyntaxKind::UnknownKeyword);
            let unknown = self.ef_finish_synthesized(unknown);
            let channel_errors = self.ef_clone_or_never(errors);
            let channel_requirements = self.ef_clone_or_never(requirements);
            let yielded = self.ef_effect_type(unknown, channel_errors, channel_requirements);
            let wrap_name = self.ef_identifier(YIELD_WRAP_ALIAS);
            let wrapped = self.ef_type_reference(wrap_name, &[yielded]);
            let success = self.factory.deep_clone_with_location(success_type);
            let next = self.ef_never();
            let generator_name = self.ef_identifier("Generator");
            self.ef_type_reference(generator_name, &[wrapped, success, next])
        } else {
            Node::NIL
        };
        let return_type = if explicit {
            let start = success_type.pos();
            let end = [success_type, errors, requirements]
                .iter()
                .filter(|node| node.is_some())
                .map(|node| node.end())
                .max()
                .unwrap_or(success_type.end());
            self.ef_anchor_at(start);
            let alias = self.ef_identifier(EFFECT_ALIAS);
            if self.ef.frames.is_empty() {
                self.ef.signature_aliases.push(alias);
            }
            let member = self.ef_identifier("Effect");
            let name = self.factory.new_qualified_name(alias, member);
            let name = self.ef_finish_synthesized(name);
            self.ef.anchor = success_type.end();
            let errors = if errors.is_nil() {
                self.ef_never()
            } else {
                errors
            };
            if errors.is_some() {
                self.ef.anchor = self.ef.anchor.max(errors.end());
            }
            let requirements = if requirements.is_nil() {
                self.ef_never()
            } else {
                requirements
            };
            let loc = TextRange::new(start, end);
            let arguments = self.factory.new_node_list_with_loc(
                &[success_type, errors, requirements],
                TextRange::new(start, requirements.end()),
            );
            let node = self.factory.new_type_reference_node(name, arguments);
            self.ef_finish_at(node, loc, NodeFlags::NONE)
        } else {
            Node::NIL
        };
        let outer_body = if body.is_some() {
            let loc = TextRange::new(skip_trivia(self.source_text, body.pos()), body.end());
            self.ef.anchor = loc.pos();
            let star = self.factory.new_token(SyntaxKind::AsteriskToken);
            let star = self.ef_finish_synthesized(star);
            let generator_parameters = self
                .factory
                .new_node_list_with_loc(&[], TextRange::new(loc.pos(), loc.pos()));
            let generator = self.factory.new_function_expression(
                ModifierList::NIL,
                star,
                Node::NIL,
                NodeList::NIL,
                generator_parameters,
                generator_type,
                Node::NIL,
                body,
            );
            let generator = self.ef_finish_at(generator, loc, NodeFlags::NONE);
            self.ef.anchor = loc.pos();
            let callee = self.ef_effect_member("gen");
            let arguments = self.factory.new_node_list_with_loc(&[generator], loc);
            let call = self.factory.new_call_expression(
                callee,
                Node::NIL,
                NodeList::NIL,
                arguments,
                NodeFlags::NONE,
            );
            let call = self.ef_finish_at(call, loc, NodeFlags::NONE);
            let ret = self.factory.new_return_statement(call);
            let ret = self.ef_finish_at(ret, loc, NodeFlags::NONE);
            let statements = self.factory.new_node_list_with_loc(&[ret], loc);
            let block = self.factory.new_block(statements, true);
            self.ef_finish_at(block, loc, NodeFlags::NONE)
        } else {
            Node::NIL
        };
        let node = self.factory.new_function_declaration(
            modifiers,
            Node::NIL,
            name,
            type_parameters,
            parameters,
            return_type,
            Node::NIL,
            outer_body,
        );
        let result = self.finish_node(node, pos);
        self.with_js_doc(result, jsdoc);
        result
    }

    pub fn ef_injected_imports(&mut self, statements: &[Node]) -> Vec<Node> {
        let mut imports = Vec::new();
        if !self.ef.enabled {
            return imports;
        }
        let mut prelude = prelude_names_to_import(statements);
        let mut needs_alias_import = self.ef.uses_effect;
        if self.ef.uses_effect
            && let Some(import_effect) = effect_name_available(statements)
            && !declares_effect_below_top_level(statements)
        {
            for alias in std::mem::take(&mut self.ef.body_aliases) {
                rename_identifier(alias, "Effect");
            }
            self.ef.signature_aliases.clear();
            if import_effect && !prelude.contains(&"Effect") {
                prelude.insert(0, "Effect");
            }
            needs_alias_import = false;
        }
        if !self.ef.signature_aliases.is_empty()
            && let Some(import_effect) = effect_name_available(statements)
        {
            for alias in std::mem::take(&mut self.ef.signature_aliases) {
                rename_identifier(alias, "Effect");
            }
            if import_effect && !prelude.contains(&"Effect") {
                prelude.insert(0, "Effect");
            }
        }
        if !prelude.is_empty() {
            let pairs: Vec<(&str, &str)> = prelude.iter().map(|name| (*name, *name)).collect();
            imports.push(self.ef_import(false, &pairs, EFFECT_MODULE));
        }
        if needs_alias_import {
            imports.push(self.ef_import(false, &[("Effect", EFFECT_ALIAS)], EFFECT_MODULE));
        }
        if self.ef.uses_yield_wrap {
            imports.push(self.ef_import(true, &[("YieldWrap", YIELD_WRAP_ALIAS)], UTILS_MODULE));
        }
        imports
    }

    fn ef_import(&mut self, type_only: bool, names: &[(&str, &str)], module: &str) -> Node {
        let mut specifiers = Vec::with_capacity(names.len());
        for (export, alias) in names {
            let property_name = if export == alias {
                Node::NIL
            } else {
                self.ef_identifier(export)
            };
            let name = self.ef_identifier(alias);
            let specifier = self
                .factory
                .new_import_specifier(false, property_name, name);
            specifiers.push(self.ef_finish_synthesized(specifier));
        }
        let elements = self.ef_node_list(&specifiers);
        let named_imports = self.factory.new_named_imports(elements);
        let named_imports = self.ef_finish_synthesized(named_imports);
        let phase = if type_only {
            SyntaxKind::TypeKeyword
        } else {
            SyntaxKind::Unknown
        };
        let clause = self
            .factory
            .new_import_clause(phase, Node::NIL, named_imports);
        let clause = self.ef_finish_synthesized(clause);
        let module_specifier = self.factory.new_string_literal(module, TokenFlags::NONE);
        let module_specifier = self.ef_finish_synthesized(module_specifier);
        let declaration = self.factory.new_import_declaration(
            ModifierList::NIL,
            clause,
            module_specifier,
            Node::NIL,
        );
        self.ef_finish_synthesized(declaration)
    }
}

fn prelude_names_to_import(statements: &[Node]) -> Vec<&'static str> {
    let mut used = Vec::new();
    for &statement in statements {
        collect_prelude_references(statement, &mut used);
    }
    if used.is_empty() {
        return used;
    }
    let mut declared = Vec::new();
    for &statement in statements {
        collect_top_level_names(statement, &mut declared);
    }
    used.retain(|name| !declared.iter().any(|d| d == name));
    used.sort_by_key(|name| PRELUDE.iter().position(|p| p == name));
    used
}

fn collect_prelude_references(node: Node, used: &mut Vec<&'static str>) {
    if node.kind() == SyntaxKind::Identifier {
        if node.pos() >= 0
            && let Some(&name) = PRELUDE.iter().find(|&&name| name == node.text())
            && !used.contains(&name)
            && is_reference_position(node)
        {
            used.push(name);
        }
        return;
    }
    node.for_each_child(|child| {
        collect_prelude_references(child, used);
        false
    });
}

fn is_reference_position(node: Node) -> bool {
    let parent = node.parent();
    match parent.kind() {
        SyntaxKind::PropertyAccessExpression
        | SyntaxKind::PropertyAssignment
        | SyntaxKind::PropertyDeclaration
        | SyntaxKind::PropertySignature
        | SyntaxKind::MethodDeclaration
        | SyntaxKind::MethodSignature
        | SyntaxKind::GetAccessor
        | SyntaxKind::SetAccessor
        | SyntaxKind::EnumMember
        | SyntaxKind::JsxAttribute => parent.name() != node,
        SyntaxKind::QualifiedName => parent.right() != node,
        SyntaxKind::ImportSpecifier
        | SyntaxKind::ExportSpecifier
        | SyntaxKind::LabeledStatement
        | SyntaxKind::BreakStatement
        | SyntaxKind::ContinueStatement => false,
        _ => true,
    }
}

fn collect_top_level_names(statement: Node, declared: &mut Vec<String>) {
    match statement.kind() {
        SyntaxKind::ImportDeclaration => {
            let clause = statement.import_clause();
            if clause.is_nil() {
                return;
            }
            push_identifier(clause.name(), declared);
            let bindings = clause.named_bindings();
            if bindings.is_nil() {
                return;
            }
            if bindings.kind() == SyntaxKind::NamespaceImport {
                push_identifier(bindings.name(), declared);
            } else {
                for element in bindings.elements() {
                    push_identifier(element.name(), declared);
                }
            }
        }
        SyntaxKind::ImportEqualsDeclaration
        | SyntaxKind::FunctionDeclaration
        | SyntaxKind::ClassDeclaration
        | SyntaxKind::EnumDeclaration
        | SyntaxKind::ModuleDeclaration
        | SyntaxKind::InterfaceDeclaration
        | SyntaxKind::TypeAliasDeclaration => push_identifier(statement.name(), declared),
        SyntaxKind::VariableStatement => {
            for declaration in statement.declaration_list().declarations().nodes() {
                push_binding_names(declaration.name(), declared);
            }
        }
        _ => {}
    }
}

fn push_binding_names(name: Node, declared: &mut Vec<String>) {
    if name.is_nil() {
        return;
    }
    match name.kind() {
        SyntaxKind::ObjectBindingPattern | SyntaxKind::ArrayBindingPattern => {
            for element in name.elements() {
                if element.kind() == SyntaxKind::BindingElement {
                    push_binding_names(element.name(), declared);
                }
            }
        }
        _ => push_identifier(name, declared),
    }
}

fn push_identifier(name: Node, declared: &mut Vec<String>) {
    if name.is_some() && name.kind() == SyntaxKind::Identifier {
        declared.push(name.text().to_string());
    }
}

fn effect_name_available(statements: &[Node]) -> Option<bool> {
    let mut declared = Vec::new();
    for &statement in statements {
        collect_top_level_names(statement, &mut declared);
    }
    if !declared.iter().any(|name| name == "Effect") {
        return Some(true);
    }
    let imported_from_effect = statements.iter().any(|&statement| {
        if statement.kind() != SyntaxKind::ImportDeclaration
            || statement.module_specifier().text() != EFFECT_MODULE
        {
            return false;
        }
        let clause = statement.import_clause();
        if clause.is_nil() || clause.phase_modifier() == SyntaxKind::TypeKeyword {
            return false;
        }
        let bindings = clause.named_bindings();
        bindings.is_some()
            && bindings.kind() == SyntaxKind::NamedImports
            && bindings.elements().into_iter().any(|element: Node| {
                element.name().text() == "Effect"
                    && (element.property_name().is_nil()
                        || element.property_name().text() == "Effect")
            })
    });
    imported_from_effect.then_some(false)
}

fn rename_identifier(identifier: Node, text: &str) {
    let mut data = with_ast_data(identifier, Clone::clone);
    if let crate::astdata::NodeData::Identifier(d) = &mut data {
        d.text = text.to_string();
    }
    replace_node_data(identifier, data);
}

fn declares_effect_below_top_level(statements: &[Node]) -> bool {
    fn visit(node: Node) -> bool {
        let declares = matches!(
            node.kind(),
            SyntaxKind::VariableDeclaration
                | SyntaxKind::Parameter
                | SyntaxKind::BindingElement
                | SyntaxKind::FunctionDeclaration
                | SyntaxKind::FunctionExpression
                | SyntaxKind::ClassDeclaration
                | SyntaxKind::ClassExpression
                | SyntaxKind::EnumDeclaration
                | SyntaxKind::ModuleDeclaration
                | SyntaxKind::TypeParameter
        ) && node.pos() >= 0
            && node.name().is_some()
            && node.name().kind() == SyntaxKind::Identifier
            && node.name().text() == "Effect";
        declares || node.for_each_child(visit)
    }
    statements.iter().any(|&statement| visit(statement))
}
