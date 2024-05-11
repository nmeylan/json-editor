use std::hash::{Hash, Hasher};
use std::time::Instant;
use egui::ahash::{HashSet, HashSetExt};
use crate::parser::lexer::Lexer;
use crate::parser::parser::Parser;
use crate::table::Column;

pub mod parser;
pub mod lexer;

pub struct JSONParser<'a> {
    pub parser: Parser<'a>,
}

#[derive(Clone)]
pub struct ParseOptions {
    pub parse_array: bool,
    pub max_depth: usize,
    pub start_parse_at: Option<String>,
    pub prefix: Option<String>,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            parse_array: true,
            max_depth: 10,
            start_parse_at: None,
            prefix: None,
        }
    }
}

impl ParseOptions {
    pub fn parse_array(mut self, parse_array: bool) -> Self {
        self.parse_array = parse_array;
        self
    }

    pub fn start_parse_at(mut self, pointer: String) -> Self {
        self.start_parse_at = Some(pointer);
        self
    }
    pub fn max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }
    pub fn prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }
}

#[derive(Debug, Clone)]
pub struct JsonArrayEntries {
    entries: FlatJsonValue,
    index: usize,
}

impl JsonArrayEntries {
    pub fn entries(&self) -> &FlatJsonValue {
        &self.entries
    }
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn find_node_at(&self, pointer: &str) -> Option<&(PointerKey, Option<String>)> {
        self.entries().iter().find(|(p, _)| p.pointer.eq(pointer))
    }
}


#[derive(Debug, Default, Clone)]
pub struct PointerKey {
    pub pointer: String,
    pub value_type: ValueType,
    pub depth: u8,
    pub index: usize,
}

impl PartialEq<Self> for PointerKey {
    fn eq(&self, other: &Self) -> bool {
        self.pointer.eq(&other.pointer)
    }
}

impl Eq for PointerKey {}

impl Hash for PointerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pointer.hash(state);
    }
}

impl PointerKey {
    pub fn parent(&self) -> PointerKey {
        let index = self.pointer.rfind('/').unwrap_or(0);
        Self {
            pointer: self.pointer[0..index].to_string(),
            value_type: ValueType::Object,
            depth: self.depth.max(0),
            index: 0,
        }
    }
}

macro_rules! concat_string {
    () => { String::with_capacity(0) };
    ($($s:expr),+) => {{
        use std::ops::AddAssign;
        let mut len = 0;
        $(len.add_assign(AsRef::<str>::as_ref(&$s).len());)+
        let mut buf = String::with_capacity(len);
        $(buf.push_str($s.as_ref());)+
        buf
    }};
}

impl PointerKey {
    pub fn from_pointer(pointer: String, value_type: ValueType, depth: u8) -> Self {
        Self {
            pointer,
            value_type,
            depth,
            index: 0,
        }
    }
    pub fn from_pointer_and_index(pointer: String, value_type: ValueType, depth: u8, index: usize) -> Self {
        Self {
            pointer,
            value_type,
            depth,
            index,
        }
    }
}

#[derive(Eq, Hash, PartialEq, Debug, Clone, Copy)]
#[derive(Default)]
pub enum ValueType {
    Array,
    Object,
    Number,
    String,
    Bool,
    Null,
    #[default]
    None,
}


type PointerFragment = Vec<String>;

pub type FlatJsonValue = Vec<(PointerKey, Option<String>)>;

#[derive(Clone)]
pub struct ParseResult {
    pub json: FlatJsonValue,
    pub max_json_depth: usize,
    pub parsing_max_depth: usize,
    pub root_value_type: ValueType,
    pub started_parsing_at: Option<String>,
    pub parsing_prefix: Option<String>,
    pub root_array_len: usize,
}

impl ParseResult {
    pub fn clone_except_json(&self) -> Self {
        Self {
            json: Default::default(),
            max_json_depth: self.max_json_depth,
            parsing_max_depth: self.parsing_max_depth,
            root_value_type: Default::default(),
            started_parsing_at: self.started_parsing_at.clone(),
            parsing_prefix: self.parsing_prefix.clone(),
            root_array_len: self.root_array_len,
        }
    }
}

#[macro_export]
macro_rules! concat_string {
    () => { String::with_capacity(0) };
    ($($s:expr),+) => {{
        use std::ops::AddAssign;
        let mut len = 0;
        $(len.add_assign(AsRef::<str>::as_ref(&$s).len());)+
        let mut buf = String::with_capacity(len);
        $(buf.push_str($s.as_ref());)+
        buf
    }};
}


impl<'a> JSONParser<'a> {
    pub fn new(input: &'a str) -> Self {
        let lexer = Lexer::new(input.as_bytes());
        let parser = Parser::new(lexer);

        Self { parser }
    }
    pub fn parse(&mut self, options: ParseOptions) -> Result<ParseResult, String> {
        self.parser.parse(&options, 1)
    }

    pub fn change_depth_array(previous_parse_result: ParseResult, mut json_array: Vec<JsonArrayEntries>, depth: usize) -> Result<(Vec<JsonArrayEntries>, Vec<Column>), String> {
        let len = json_array.len();
        let mut new_json_array = Vec::with_capacity(json_array.len());
        let mut unique_keys: Vec<Column> = Vec::with_capacity(1000);
        let start = Instant::now();
        for i in (0..len).rev() {
            let mut parse_result = previous_parse_result.clone_except_json();
            parse_result.json = json_array.pop().unwrap().entries;
            let mut options = ParseOptions::default().parse_array(false).max_depth(depth);
            let result = Self::change_depth(parse_result, options)?;
            let mut vec = result.json;

            for j in 0..vec.len() {
                let (k, _v) = &mut vec[j];
                let _i = i.to_string();
                let (prefix_len) = if let Some(ref started_parsing_at) = previous_parse_result.started_parsing_at {
                    let prefix = concat_string!(started_parsing_at, "/", _i);
                    prefix.len()
                } else if let Some(ref prefix) = previous_parse_result.parsing_prefix {
                    let prefix = concat_string!(prefix, "/", _i);
                    prefix.len()
                } else {
                    let prefix = concat_string!("/", _i);
                    prefix.len()
                };
                if !k.pointer.is_empty() {
                    if k.pointer.len() <= prefix_len {
                        // panic!("ERROR, depth {} out of bounds of {}, expected to have a prefix of len {}", depth, k.pointer, prefix_len);
                        continue;
                    }
                    let key = &k.pointer[prefix_len..k.pointer.len()];
                    let column = Column {
                        name: key.to_string(),
                        depth: k.depth,
                        value_type: k.value_type,
                        seen_count: 0,
                        order: unique_keys.len(),
                    };
                    if let Some(column) = unique_keys.iter_mut().find(|c| c.eq(&&column)) {
                        column.seen_count += 1;
                    } else {
                        if !column.name.contains("#") {
                            unique_keys.push(column);
                        }
                    }
                }
                k.index = i;
            }
            new_json_array.push(JsonArrayEntries { entries: vec, index: i });
        }
        new_json_array.reverse();
        unique_keys.sort();
        println!("took {}ms to change depth", start.elapsed().as_millis());
        Ok((new_json_array, unique_keys))
    }

    pub fn change_depth(previous_parse_result: ParseResult, mut parse_options: ParseOptions) -> Result<ParseResult, String> {
        if previous_parse_result.parsing_max_depth < parse_options.max_depth {
            let previous_len = previous_parse_result.json.len();
            let mut new_flat_json_structure = FlatJsonValue::with_capacity(previous_len + (parse_options.max_depth - previous_parse_result.parsing_max_depth) * (previous_len / 3));
            for (k, v) in previous_parse_result.json {
                if !matches!(k.value_type, ValueType::Object) {
                    new_flat_json_structure.push((k, v));
                } else {
                    if k.depth == previous_parse_result.parsing_max_depth as u8 {
                        if let Some(mut v) = v {
                            new_flat_json_structure.push((k.clone(), Some(v.clone())));
                            let lexer = Lexer::new(v.as_bytes());
                            let mut parser = Parser::new(lexer);
                            parse_options.prefix = Some(k.pointer);
                            let res = parser.parse(&parse_options, k.depth + 1)?;
                            new_flat_json_structure.extend(res.json);
                        }
                    } else {
                        new_flat_json_structure.push((k, v));
                    }

                }
            }
            Ok(ParseResult {
                json: new_flat_json_structure,
                max_json_depth: previous_parse_result.max_json_depth,
                parsing_max_depth: parse_options.max_depth,
                root_value_type: previous_parse_result.root_value_type,
                started_parsing_at: previous_parse_result.started_parsing_at,
                parsing_prefix: previous_parse_result.parsing_prefix,
                root_array_len: previous_parse_result.root_array_len,
            })
        } else if previous_parse_result.parsing_max_depth > parse_options.max_depth {
            // serialization
            todo!("");
        } else {
            Ok(previous_parse_result)
        }
    }

    pub fn as_array(mut previous_parse_result: ParseResult) -> Result<(Vec<JsonArrayEntries>, Vec<Column>), String> {
        if !matches!(previous_parse_result.root_value_type, ValueType::Array) {
            return Err("Parsed json root is not an array".to_string());
        }
        let mut unique_keys: Vec<Column> = Vec::with_capacity(1000);
        let mut res: Vec<JsonArrayEntries> = Vec::with_capacity(previous_parse_result.root_array_len);
        let mut j = previous_parse_result.json.len() - 1;
        let mut estimated_capacity = 100;
        for i in (0..previous_parse_result.root_array_len).rev() {
            let mut flat_json_values = FlatJsonValue::with_capacity(estimated_capacity);
            let mut is_first_entry = true;
            let _i = i.to_string();
            loop {
                if j >= 0 && !previous_parse_result.json.is_empty() {
                    let (k, _v) = &previous_parse_result.json[j];
                    let (match_prefix, prefix_len) = if let Some(ref started_parsing_at) = previous_parse_result.started_parsing_at {
                        let prefix = concat_string!(started_parsing_at, "/", _i);
                        // println!("else if {}", prefix);
                        (k.pointer.starts_with(&prefix), prefix.len())
                    } else if let Some(ref prefix) = previous_parse_result.parsing_prefix {
                        let prefix = concat_string!(prefix, "/", _i);
                        // println!("else if {}", prefix);
                        (k.pointer.starts_with(&prefix), prefix.len())
                    } else {
                        let prefix = concat_string!("/", _i);
                        // println!("else {}", prefix);
                        (k.pointer.starts_with(&prefix), prefix.len())
                    };

                    if match_prefix {
                        if !k.pointer.is_empty() {
                            if k.pointer.len() < prefix_len{
                                panic!("{} len is < {}", k.pointer, prefix_len);
                            }
                            let key = &k.pointer[prefix_len..k.pointer.len()];
                            let column = Column {
                                name: key.to_string(),
                                depth: k.depth,
                                value_type: k.value_type,
                                seen_count: 1,
                                order: unique_keys.len(),
                            };
                            if let Some(column) = unique_keys.iter_mut().find(|c| c.eq(&&column)) {
                                column.seen_count += 1;
                            } else {
                                unique_keys.push(column);
                            }
                        }
                        if is_first_entry {
                            is_first_entry = false;
                            let prefix = &k.pointer[0..prefix_len];
                            flat_json_values.push((PointerKey::from_pointer_and_index(concat_string!(prefix, "/#"), ValueType::Number, k.depth, i), Some(i.to_string())));
                        }
                        let (mut k, v) = previous_parse_result.json.pop().unwrap();
                        k.index = i;
                        flat_json_values.push((k, v));
                    } else {
                        break;
                    }
                    if j == 0 {
                        break;
                    }
                    j -= 1;
                } else {
                    break;
                }
            }
            res.push(JsonArrayEntries { entries: flat_json_values, index: i });
        }
        res.reverse();
        unique_keys.sort();
        Ok((res, unique_keys))
    }

    pub fn filter_non_null_column(previous_parse_result: &Vec<JsonArrayEntries>, prefix: &str, non_null_columns: &Vec<String>) -> Vec<JsonArrayEntries> {
        let mut res: Vec<JsonArrayEntries> = Vec::with_capacity(previous_parse_result.len());
        for row in previous_parse_result {
            let mut should_add_row = true;
            for pointer in non_null_columns {
                let pointer_to_find = concat_string!(prefix, "/", row.index().to_string(), pointer);
                if let Some((_, value)) = row.find_node_at(&pointer_to_find) {
                    if value.is_none() {
                        should_add_row = false;
                        break;
                    }
                } else {
                    should_add_row = false;
                    break;
                }
            }

            if should_add_row {
                res.push(row.clone());
            }
        }
        res
    }
}


#[derive(Debug)]
pub enum Token<'a> {
    CurlyOpen,
    CurlyClose,
    SquareOpen,
    SquareClose,
    Colon,
    Comma,
    String(&'a str),
    Number(&'a str),
    Boolean(bool),
    Null,
}