use indexmap::IndexMap;
use tree_sitter::{Node, Parser, Query, QueryCursor};
use tree_sitter_python::language;

use crate::ast::ast_structs::{AstDefinition, AstUsage};
use crate::ast::treesitter::structs::SymbolType;
use crate::ast::parse_common::{ContextAnyParser, Thing, type_deindex, type_deindex_n, type_call, type_zerolevel_comma_split};


pub struct ContextPy<'a> {
    pub ap: ContextAnyParser<'a>,
    pub pass2: Vec<(Node<'a>, Vec<String>)>,
    pub class1: Query,
}

fn py_import_save<'a>(cx: &mut ContextPy<'a>, path: &Vec<String>, dotted_from: String, import_what: String, import_as: String)
{
    let save_as = format!("{}::{}", path.join("::"), import_as);
    let mut from_list = dotted_from.split(".").map(|x| { String::from(x.trim()) }).filter(|x| { !x.is_empty() }).collect::<Vec<String>>();
    from_list.push(import_what);
    cx.ap.alias.insert(save_as, from_list.join("::"));
}

fn debug_helper(cx: &ContextPy, args: std::fmt::Arguments) {
    cx.ap.indented_println(args);
}

macro_rules! debug {
    ($cx:expr, $($arg:tt)*) => {
        debug_helper($cx, format_args!($($arg)*));
    }
}

fn resolved_type(type_str: &String) -> bool {
    type_str != "?" && !type_str.is_empty() && type_str != "!?"
}

fn py_import<'a>(cx: &mut ContextPy<'a>, node: &Node<'a>, path: &Vec<String>)
{
    let mut dotted_from = String::new();
    let mut just_do_it = false;
    let mut from_clause = false;
    for i in 0 .. node.child_count() {
        let child = node.child(i).unwrap();
        let child_text = cx.ap.code[child.byte_range()].to_string();
        match child.kind() {
            "import" => { just_do_it = true; },
            "from" => { from_clause = true; },
            "dotted_name" => {
                if just_do_it {
                    py_import_save(cx, path, dotted_from.clone(), child_text.clone(), child_text.clone());
                } else if from_clause {
                    dotted_from = child_text.clone();
                }
            },
            "aliased_import" => {
                let mut import_what = String::new();
                for i in 0..child.child_count() {
                    let subch = child.child(i).unwrap();
                    let subch_text = cx.ap.code[subch.byte_range()].to_string();
                    // dotted_name[identifier[os]]·as[as]·identifier[ooooos]]
                    match subch.kind() {
                        "dotted_name" => { import_what = subch_text; },
                        "as" => { },
                        "identifier" => { py_import_save(cx, path, dotted_from.clone(), import_what.clone(), subch_text); },
                        _ => {},
                    }
                }
            },
            "," => {},
            _ => {
                debug!(cx, "IMPORT {:?} {:?}", child.kind(), child_text);
            }
        }
    }
}

fn py_is_trivial(potential_usage: &str) -> bool {
    match potential_usage {
        "int" | "float" | "str" | "bool" => true,
        _ if potential_usage.ends_with("::self") => true,
        _ => false,
    }
}

fn py_simple_resolve(cx: &mut ContextPy, path: &Vec<String>, look_for: &String) -> Option<String>
{
    match look_for.as_str() {
        "Any" => { return Some("*".to_string()); },
        "print" | "int" | "float" | "str" | "bool" => { return Some(look_for.clone()); },
        _ => {},
    }
    let mut current_path = path.clone();
    while !current_path.is_empty() {
        let mut hypothetical = current_path.clone();
        hypothetical.push(look_for.clone());
        let hypothtical_str = hypothetical.join("::");
        let thing_maybe = cx.ap.things.get(&hypothtical_str);
        if thing_maybe.is_some() {
            return Some(hypothtical_str);
        }
        if let Some(an_alias) = cx.ap.alias.get(&hypothtical_str) {
            return Some(an_alias.clone());
        }
        current_path.pop();
    }
    return None;
}

fn py_resolve_dotted_creating_usages(cx: &mut ContextPy, node: &Node, path: &Vec<String>, allow_creation: bool) -> Option<AstUsage>
{
    let node_text = cx.ap.code[node.byte_range()].to_string();
    debug!(cx, "DOTTED {:?}", node_text);
    // debug!(cx, "DOTTED {}", cx.ap.recursive_print_with_red_brackets(&node));

    // identifier[Goat]
    // attribute[identifier[my_module].identifier[Animal]]
    match node.kind() {
        "identifier" => {
            if let Some(success) = py_simple_resolve(cx, path, &node_text) {
                let u = AstUsage {
                    targets_for_guesswork: vec![],
                    resolved_as: success.clone(),
                    debug_hint: format!("simple_id"),
                    uline: node.range().start_point.row,
                };
                if !py_is_trivial(u.resolved_as.as_str()) && !cx.ap.suppress_adding {
                    cx.ap.usages.push((path.join("::"), u.clone()));
                    debug!(cx, "ADD_USAGE ID {:?}", u);
                }
                debug!(cx, "DOTTED simple_id u={:?}", u);
                return Some(u);
            }
            if allow_creation {
                debug!(cx, "DOTTED {} local_var_create", node_text);
                return Some(AstUsage {
                    targets_for_guesswork: vec![],
                    resolved_as: format!("{}::{}", path.join("::"), node_text),
                    debug_hint: format!("local_var_create"),
                    uline: node.range().start_point.row,
                });
            }
        },
        "attribute" => {
            let object = node.child_by_field_name("object").unwrap();
            let attrib = node.child_by_field_name("attribute").unwrap();
            let object_type = py_type_of_expr_creating_usages(cx, Some(object), path);
            let attrib_text = cx.ap.code[attrib.byte_range()].to_string();
            let attrib_path = format!("{}::{}", object_type, attrib_text);
            let mut u = AstUsage {
                targets_for_guesswork: vec![],
                resolved_as: attrib_path.clone(),
                debug_hint: format!("attr"),
                uline: attrib.range().start_point.row,
            };
            if let Some(existing_attr) = cx.ap.things.get(&attrib_path) {
                if !cx.ap.suppress_adding {
                    cx.ap.usages.push((path.join("::"), u.clone()));
                    debug!(cx, "ADD_USAGE ATTR {:?}", u);
                }
                return Some(u);
            }
            if let Some(existing_object) = cx.ap.things.get(&object_type) {
                if allow_creation {
                    u.debug_hint = format!("attr_create");
                    return Some(u);
                }
            }
        },
        _ => {
            debug!(cx, "DOTTED syntax {}", cx.ap.recursive_print_with_red_brackets(node));
        }
    }
    None
}

fn py_lhs_tuple<'a>(cx: &mut ContextPy<'a>, left: &Node<'a>, type_node: Option<Node>, path: &Vec<String>) -> (Vec<(Node<'a>, String)>, bool)
{
    let mut lhs_tuple: Vec<(Node, String)> = Vec::new();
    let mut is_list = false;
    match left.kind() {
        "pattern_list" | "tuple_pattern" => {
            is_list = true;
            for j in 0 .. left.child_count() {
                let child = left.child(j).unwrap();
                match child.kind() {
                    "identifier" | "attribute" => {
                        lhs_tuple.push((child, "?".to_string()));
                    },
                    "," | "(" | ")" => { },
                    _ => {
                        debug!(cx, "PY_LHS PATTERN SYNTAX {:?}", child.kind());
                    }
                }
            }
        },
        "identifier" | "attribute" => {
            lhs_tuple.push((*left, py_type_generic(cx, type_node, path, 0)));
        },
        _ => {
            debug!(cx, "py_lhs syntax {}", cx.ap.recursive_print_with_red_brackets(left));
        },
    }
    (lhs_tuple, is_list)
}

fn py_assignment<'a>(cx: &mut ContextPy<'a>, node: &Node<'a>, path: &Vec<String>, is_for_loop: bool)
{
    let left_node = node.child_by_field_name("left");
    let right_node = node.child_by_field_name("right");
    let mut rhs_type = py_type_of_expr_creating_usages(cx, right_node, path);
    if is_for_loop {
        rhs_type = type_deindex(rhs_type);
    }
    if left_node.is_none() {
        return;
    }
    let (lhs_tuple, is_list) = py_lhs_tuple(cx, &left_node.unwrap(), node.child_by_field_name("type"), path);
    // debug!(cx, "ASSIGNMENT {:?} {:?} {:?} {:?}", lhs_tuple, is_list, rhs_type, path);
    for n in 0 .. lhs_tuple.len() {
        let (lhs_lvalue, lvalue_type) = &lhs_tuple[n];
        if is_list {
            py_var_add(cx, lhs_lvalue, lvalue_type.clone(), type_deindex_n(rhs_type.clone(), n), path);
        } else {
            py_var_add(cx, lhs_lvalue, lvalue_type.clone(), rhs_type.clone(), path);
        }
    }
}

fn py_var_add(cx: &mut ContextPy, lhs_lvalue: &Node, lvalue_type: String, rhs_type: String, path: &Vec<String>)
{
    let lvalue_usage = if let Some(u) = py_resolve_dotted_creating_usages(cx, lhs_lvalue, path, true) {
        debug!(cx, "VAR_ADD u={:?}", u);
        u
    } else {
        // debug!(cx, "syntax error or something");
        return; // syntax error or something
    };
    let lvalue_path;
    if lvalue_usage.targets_for_guesswork.is_empty() { // no guessing, exact location
        lvalue_path = lvalue_usage.resolved_as.clone();
    } else {
        // never mind can't create anything, for example a.b.c = 5 if b doesn't exit
        return;
    }
    let mut good_idea_to_write = true;
    let potential_new_type = if !resolved_type(&lvalue_type) || lvalue_type.starts_with("ERR") { rhs_type.clone() } else { lvalue_type.clone() };
    // debug!(cx, "VAR_ADD {:?} {:?} {:?} potential_new_type={:?} good_idea_to_write={:?}", lvalue_path, lvalue_type, rhs_type, potential_new_type, good_idea_to_write);
    if let Some(existing_thing) = cx.ap.things.get(&lvalue_path) {
        good_idea_to_write = !resolved_type(&existing_thing.type_resolved) && resolved_type(&potential_new_type);
        if good_idea_to_write {
            debug!(cx, "VAR_ADD {} UPDATE TYPE {:?} -> {:?}", lvalue_path, existing_thing.type_resolved, potential_new_type);
            cx.ap.resolved_anything = true;
        }
    }
    if good_idea_to_write {
        let thing = Thing {
            thing_kind: 'v',
            type_resolved: potential_new_type,
            tline: lhs_lvalue.range().start_point.row,
        };
        debug!(cx, "VAR_ADD {} {:?}", lvalue_path, thing);
        cx.ap.things.insert(lvalue_path, thing);
    }
}

fn py_type_generic(cx: &mut ContextPy, node: Option<Node>, path: &Vec<String>, level: usize) -> String {
    if node.is_none() {
        return format!("?")
    }
    // type[generic_type[identifier[List]type_parameter[[type[identifier[Goat]]]]]]]
    // type[generic_type[identifier[List]type_parameter[[type[generic_type[identifier[Optional]type_parameter[[type[identifier[Goat]]]]]]]]
    let node = node.unwrap();
    match node.kind() {
        "type" => { py_type_generic(cx, node.child(0), path, level+1) },
        "identifier" | "attribute" => {
            if let Some(a_type) = py_resolve_dotted_creating_usages(cx, &node, path, false) {
                if !a_type.resolved_as.is_empty() {
                    return a_type.resolved_as;
                } else if !a_type.targets_for_guesswork.is_empty() {
                    return a_type.targets_for_guesswork.first().unwrap().clone();
                }
            }
            format!("UNK/id/{}", cx.ap.code[node.byte_range()].to_string())
        },
        "list" => { format!("CALLABLE_ARGLIST") },
        "generic_type" => {
            let mut inside_type = String::new();
            let mut todo = "";
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                let child_text = cx.ap.code[child.byte_range()].to_string();
                // debug!(cx, "{}GENERIC_LOOP {:?} {:?}", spaces, child.kind(), child_text);
                match (child.kind(), child_text.as_str()) {
                    // ("identifier", "Any") => todo = "give_up",
                    ("identifier", "List") => todo = "List",
                    ("identifier", "Set") => todo = "Set",
                    ("identifier", "Dict") => todo = "Dict",
                    ("identifier", "Tuple") => todo = "Tuple",
                    ("identifier", "Callable") => todo = "Callable",
                    ("identifier", "Optional") => todo = "Optional",
                    ("identifier", _) | ("attribute", _) => inside_type = format!("ERR/ID/{}", child_text),
                    ("type_parameter", _) => inside_type = py_type_generic(cx, Some(child), path, level+1),
                    (_, _) => inside_type = format!("ERR/GENERIC/{:?}", child.kind()),
                }
            }
            let result = match todo {
                "give_up" => format!(""),
                "List" => format!("[{}]", inside_type),
                "Set" => format!("[{}]", inside_type),
                "Tuple" => format!("({})", inside_type),
                "Optional" => format!("{}", inside_type),
                "Callable" => {
                    if let Some(return_type_only) = inside_type.strip_prefix("CALLABLE_ARGLIST,") {
                        format!("!{}", return_type_only)
                    } else {
                        format!("!")
                    }
                },
                "Dict" => {
                    let split = type_zerolevel_comma_split(inside_type.as_str());
                    if split.len() == 2 {
                        format!("[{}]", split[1])
                    } else {
                        format!("BADDICT[{}]", inside_type)
                    }
                },
                _ => format!("NOTHING_TODO/{}", inside_type)
            };
            // debug!(cx, "{}=> TODO {}", spaces, result);
            result
        }
        "type_parameter" => {
            // type_parameter[ "[" "type" "," "type" "]" ]
            let mut comma_sep_types = String::new();
            for i in 0 .. node.child_count() {
                let child = node.child(i).unwrap();
                comma_sep_types.push_str(match child.kind() {
                    "[" | "]" => "".to_string(),
                    "type" | "identifier" => py_type_generic(cx, Some(child), path, level+1),
                    "," => ",".to_string(),
                    _ => format!("SOMETHING/{:?}/{}", child.kind(), cx.ap.code[child.byte_range()].to_string())
                }.as_str());
            }
            comma_sep_types
        }
        _ => {
            format!("UNK/{:?}/{}", node.kind(), cx.ap.code[node.byte_range()].to_string())
        }
    }
}


// my_list1 = [1,2,3]
// my_list2: List[int] = [3,2,1]
// # assignment[ field_name="left" identifier[my_list1] field_name="" ·= field_name="right" ·list[ field_name="" [ field_name="" integer[1] field_name="" , field_name="" integer[2] field_name="" , field_name="" integer[3] field_name="" ]]]py_lhs_tuple

fn py_type_of_expr_creating_usages(cx: &mut ContextPy, node: Option<Node>, path: &Vec<String>) -> String
{
    if node.is_none() {
        return "".to_string();
    }
    let node = node.unwrap();
    let node_text = cx.ap.code[node.byte_range()].to_string();
    debug!(cx, "EXPR {}", node_text);
    cx.ap.reclevel += 1;
    let type_of = match node.kind() {
        "expression_list" | "argument_list" => {
            let mut elements = vec![];
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                match child.kind() {
                    "(" | "," |")" => { continue; }
                    _ => {}
                }
                elements.push(py_type_of_expr_creating_usages(cx, Some(child), path));
            }
            format!("({})", elements.join(","))
        },
        "tuple" => {
            let mut elements = vec![];
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                match child.kind() {
                    "(" | "," |")" => { continue; }
                    _ => {}
                }
                elements.push(py_type_of_expr_creating_usages(cx, Some(child), path));
            }
            format!("({})", elements.join(","))
        },
        "comparison_operator" => {
            for i in 0 .. node.child_count() {
                let child = node.child(i).unwrap();
                match child.kind() {
                    "is" | "is not" | ">" | "<" | "<=" | "==" | "!=" | ">=" => { continue; }
                    _ => {}
                }
                py_type_of_expr_creating_usages(cx, Some(child), path);
            }
            "bool".to_string()
        },
        "integer" => { "int".to_string() },
        "float" => { "float".to_string() },
        "string" => { "str".to_string() },
        "false" => { "bool".to_string() },
        "true" => { "bool".to_string() },
        "none" => { "void".to_string() },
        "call" => {
            let fname = node.child_by_field_name("function");
            if fname.is_none() {
                debug!(cx, "ERR/CALL/NAMELESS {}", cx.ap.recursive_print_with_red_brackets(&node));
                format!("ERR/CALL/NAMELESS")
            } else {
                let ftype = if let Some(u) = py_resolve_dotted_creating_usages(cx, &fname.unwrap(), path, false) {
                    if !u.resolved_as.is_empty() {
                        if let Some(resolved_thing) = cx.ap.things.get(&u.resolved_as) {
                            resolved_thing.type_resolved.clone()
                        } else {
                            format!("ERR/CALL/NOT_A_THING/{}", u.resolved_as.clone())
                        }
                    } else {
                        "?".to_string()  // something outside of this file :/
                    }
                } else {
                    format!("ERR/FUNC_NOT_FOUND/{}", cx.ap.code[fname.unwrap().byte_range()].to_string())
                };
                let arg_types = py_type_of_expr_creating_usages(cx, node.child_by_field_name("arguments"), path);
                let ret_type = type_call(ftype.clone(), arg_types.clone());
                // debug!(cx, "\nCALL ftype={:?} arg_types={:?} => ret_type={:?}", ftype, arg_types, ret_type);
                ret_type
            }
        },
        "identifier" | "dotted_name" | "attribute" => {
            let dotted_type = if let Some(u) = py_resolve_dotted_creating_usages(cx, &node, path, false) {
                if let Some(resolved_thing) = cx.ap.things.get(&u.resolved_as) {
                    resolved_thing.type_resolved.clone()
                } else {
                    format!("ERR/DOTTED/NOT_A_THING/{}", u.resolved_as.clone())
                }
            } else {
                format!("ERR/DOTTED_NOT_FOUND/{}", node_text)
            };
            dotted_type
        },
        "subscript" => {
            debug!(cx, "subscript {}", cx.ap.recursive_print_with_red_brackets(&node));
            let typeof_value = py_type_of_expr_creating_usages(cx, node.child_by_field_name("value"), path);
            py_type_of_expr_creating_usages(cx, node.child_by_field_name("subscript"), path);
            type_deindex(typeof_value)
        },
        _ => {
            debug!(cx, "py_type_of_expr syntax {}", cx.ap.recursive_print_with_red_brackets(&node));
            format!("ERR/EXPR/{:?}/{}", node.kind(), node_text)
        }
    };
    cx.ap.reclevel -= 1;
    debug!(cx, "/EXPR type={}", type_of);
    type_of
}

fn py_class<'a>(cx: &mut ContextPy<'a>, node: &Node<'a>, path: &Vec<String>)
{
    let mut derived_from = vec![];
    let mut query_cursor = QueryCursor::new();
    for m in query_cursor.matches(&cx.class1, *node, cx.ap.code.as_bytes()) {
        for capture in m.captures {
            let capture_name = cx.class1.capture_names()[capture.index as usize];
            if capture_name == "dfrom" {
                derived_from.push(format!("py🔎{}", cx.ap.code[capture.node.byte_range()].to_string()));
            }
        }
    }

    let mut class_name = "".to_string();
    let mut body = None;
    let mut body_line1 = usize::MAX;
    let mut body_line2 = 0;
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        match child.kind() {
            "identifier" => class_name = cx.ap.code[child.byte_range()].to_string(),
            "block" => {
                body_line1 = body_line1.min(child.range().start_point.row + 1);
                body_line2 = body_line2.max(child.range().end_point.row + 1);
                body = Some(child);
                break;
            },
            _ => {}
        }
    }

    if class_name == "" {
        return;
    }
    if body.is_none() {
        return;
    }

    let class_path = [path.clone(), vec![class_name.clone()]].concat();
    cx.ap.defs.insert(class_path.join("::"), AstDefinition {
        official_path: class_path.clone(),
        symbol_type: SymbolType::StructDeclaration,
        usages: vec![],
        this_is_a_class: format!("py🔎{}", class_name),
        this_class_derived_from: derived_from,
        cpath: "".to_string(),
        decl_line1: node.range().start_point.row + 1,
        decl_line2: (node.range().start_point.row + 1).max(body_line1 - 1),
        body_line1,
        body_line2,
    });

    cx.ap.things.insert(class_path.join("::"), Thing {
        thing_kind: 's',
        type_resolved: format!("!{}", class_path.join("::")),   // this is about constructor in python, name of the class() is used as constructor, return type is the class
        tline: node.range().start_point.row,
    });

    py_body(cx, &body.unwrap(), &class_path);
    // debug!(cx, "\nCLASS {:?}", cx.ap.defs.get(&class_path.join("::")).unwrap());
}


fn py_function<'a>(cx: &mut ContextPy<'a>, node: &Node<'a>, path: &Vec<String>) {
    // function_definition[def·identifier[jump_around]parameters[(identifier[self])]·->[->]·type[identifier[Animal]]
    // function_definition[def·identifier[jump_around]parameters[(typed_parameter[identifier[v1]:·type[identifier[Goat]]]
    let mut body_line1 = usize::MAX;
    let mut body_line2 = 0;
    let mut func_name = "".to_string();
    let mut params_node = None;
    let mut body = None;
    let mut returns = None;
    for i in 0 .. node.child_count() {
        let child = node.child(i).unwrap();
        match child.kind() {
            "identifier" => func_name = cx.ap.code[child.byte_range()].to_string(),
            "block" => {
                body_line1 = body_line1.min(child.range().start_point.row + 1);
                body_line2 = body_line2.max(child.range().end_point.row + 1);
                body = Some(child);
                break;
            },
            "parameters" => params_node = Some(child),
            "type" => returns = Some(child),
            "def" | "->" | ":" => {},
            _ => {
                debug!(cx, "\nFUNCTION STRANGE NODE {:?}", child.kind());
            }
        }
    }
    if func_name == "" {
        // XXX make error
        return;
    }
    if body.is_none() {
        // XXX make error
        return;
    }
    if params_node.is_none() {
        // XXX make error
        return;
    }

    let mut func_path = path.clone();
    func_path.push(func_name.clone());

    cx.ap.defs.insert(func_path.join("::"), AstDefinition {
        official_path: func_path.clone(),
        symbol_type: SymbolType::FunctionDeclaration,
        usages: vec![],
        this_is_a_class: "".to_string(),
        this_class_derived_from: vec![],
        cpath: "".to_string(),
        decl_line1: node.range().start_point.row + 1,
        decl_line2: (node.range().start_point.row + 1).max(body_line1 - 1),
        body_line1,
        body_line2,
    });

    let returns_type = py_type_generic(cx, returns, path, 0);

    cx.ap.things.insert(func_path.join("::"), Thing {
        thing_kind: 'f',
        type_resolved: format!("!{}", returns_type),
        tline: node.range().start_point.row,
    });

    // All types in type annotations must be already visible in python
    let params = params_node.unwrap();
    for i in 0..params.child_count() {
        let param_node = params.child(i).unwrap();
        let mut param_name = "".to_string();
        let mut type_resolved = "".to_string();
        match param_node.kind() {
            "identifier" => {
                param_name = cx.ap.code[param_node.byte_range()].to_string();
                if param_name == "self" {
                    type_resolved = path.join("::");
                }
            },
            "typed_parameter" => {
                if let Some(param_name_node) = param_node.child(0) {
                    param_name = cx.ap.code[param_name_node.byte_range()].to_string();
                }
                type_resolved = py_type_generic(cx, param_node.child_by_field_name("type"), &func_path, 0);
            },
            // "list_splat_pattern" for *args
            // "dictionary_splat_pattern" for **kwargs
            _ => {
                continue;
            }
        }
        if param_name.is_empty() {
            // XXX make error
            continue;
        }
        let param_path = [func_path.clone(), vec![param_name.clone()]].concat();
        cx.ap.things.insert(param_path.join("::"), Thing {
            thing_kind: 'p',
            type_resolved,
            tline: param_node.range().start_point.row,
        });
    }

    cx.pass2.push( (body.unwrap(), func_path.clone()) );
}


fn py_save_func_return_type(cx: &mut ContextPy, ret_type: String, fpath: &Vec<String>)
{
    let func_path = fpath.join("::");
    if let Some(func_exists) = cx.ap.things.get(&func_path) {
        let good_idea_to_write = !resolved_type(&func_exists.type_resolved) && resolved_type(&ret_type) && func_exists.thing_kind == 'f';
        if good_idea_to_write {
            let ret_type = format!("!{}", ret_type);
            debug!(cx, "\nUPDATE RETURN TYPE {:?} for {}", ret_type, fpath.join("::"));
            cx.ap.things.insert(func_path, Thing {
                thing_kind: 'f',
                type_resolved: ret_type,
                tline: func_exists.tline,
            });
            cx.ap.resolved_anything = true;
        }
    }
}

fn py_body<'a>(cx: &mut ContextPy<'a>, node: &Node<'a>, path: &Vec<String>) -> String
{
    let mut ret_type = "void".to_string();  // if there's no return clause, then it's None aka void
    debug!(cx, "{}", node.kind());
    cx.ap.reclevel += 1;
    match node.kind() {
        "import_statement" | "import_from_statement" => py_import(cx, node, path),
        "if" | "else" | "elif" => { },
        "module" | "block" | "expression_statement" | "else_clause" | "if_statement" | "elif_clause" => {
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                // debug!(cx, "CHILD {}", cx.ap.recursive_print_with_red_brackets(&child));
                // debug!(cx, "CHILD {}", child.kind());
                match child.kind() {
                    "if" | "elif" | "else" | ":" | "integer" | "float" | "string" | "false" | "true" => { continue; }
                    "return_statement" => { ret_type = py_type_of_expr_creating_usages(cx, child.child(1), path); }
                    _ => { let _ = py_body(cx, &child, path); }
                }
                // let alt_ret_type = ;
                // debug!(cx, "ret_type child.kind()=={} ret_type={} alt_ret_type={}", child.kind(), ret_type, alt_ret_type);
                // if child.kind() == "block" && ret_type == "void" && resolved_type(&alt_ret_type) {
                //     ret_type = alt_ret_type;
                // }
            }
        },
        "class_definition" => py_class(cx, node, path),  // class recursively calls py_body
        "function_definition" => py_function(cx, node, path),  // function adds body to pass2, that calls py_body later
        "assignment" => py_assignment(cx, node, path, false),
        "for_statement" => py_assignment(cx, node, path, true),
        "call" | "comparison_operator" => { py_type_of_expr_creating_usages(cx, Some(node.clone()), path); }
        _ => {
            debug!(cx, "py_body syntax {}", cx.ap.recursive_print_with_red_brackets(node));
        }
    }
    cx.ap.reclevel -= 1;
    debug!(cx, "/{} func_returns={:?}", node.kind(), ret_type);
    return ret_type;
}

pub fn py_make_cx(code: &str) -> ContextPy
{
    let mut sitter = Parser::new();
    sitter.set_language(&language()).unwrap();
    let cx = ContextPy {
        ap: ContextAnyParser {
            sitter,
            reclevel: 0,
            code,
            suppress_adding: false,
            resolved_anything: false,
            defs: IndexMap::new(),
            things: IndexMap::new(),
            usages: vec![],
            alias: IndexMap::new(),
            star_imports: vec![],
        },
        pass2: vec![],
        // class_definition[class·identifier[Goat]argument_list[(identifier[Animal])]:
        class1: Query::new(&language(), "(class_definition name: (_) superclasses: (argument_list (_) @dfrom))").unwrap(),
    };
    cx
}

#[allow(dead_code)]
pub fn parse(code: &str) -> String
{
    let mut cx = py_make_cx(code);
    let tree = cx.ap.sitter.parse(code, None).unwrap();
    let path = vec!["file".to_string()];

    // pass1
    py_body(&mut cx, &tree.root_node(), &path);

    // pass2
    cx.ap.suppress_adding = true;
    let my_pass2 = cx.pass2.clone();
    loop {
        cx.ap.resolved_anything = false;
        for (body, func_path) in my_pass2.iter() {
            debug!(&cx, "\n\x1b[31mPASS2 RESOLVE {:?}\x1b[0m", func_path.join("::"));
            let ret_type = py_body(&mut cx, body, func_path);
            debug!(&cx, "\n\x1b[31mPASS2 RESOLVE {:?} new return type {}\x1b[0m", func_path.join("::"), ret_type);
            py_save_func_return_type(&mut cx, ret_type, func_path);
        }
        if !cx.ap.resolved_anything {
            break;
        }
    }
    cx.ap.suppress_adding = false;
    for (body, func_path) in my_pass2.iter() {
        debug!(&cx, "\n\x1b[31mPASS2 SAVE USAGES\x1b[0m {:?}", func_path.join("::"));
        py_body(&mut cx, body, func_path);
    }

    cx.ap.dump();
    cx.ap.annotate_code("#")
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_py_tort1() {
        let code = include_str!("alt_testsuite/py_torture1_attr.py");
        let annotated = parse(code);
        std::fs::write("src/ast/alt_testsuite/py_torture1_attr_annotated.py", annotated).expect("Unable to write file");
    }

    #[test]
    fn test_parse_py_tort2() {
        let code = include_str!("alt_testsuite/py_torture2_resolving.py");
        let annotated = parse(code);
        std::fs::write("src/ast/alt_testsuite/py_torture2_resolving_annotated.py", annotated).expect("Unable to write file");
    }
}
