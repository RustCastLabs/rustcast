//! Unit conversion parsing and calculation.

use crate::{
    app::{
        ToApp,
        apps::{App, AppCommand},
    },
    clipboard::ClipBoardContentType,
    commands::Function,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitCategory {
    Length,
    Mass,
    Volume,
    Temperature,
}

#[derive(Debug, Clone, Copy)]
pub struct UnitDef {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub category: UnitCategory,
    pub scale: f64,
    pub offset: f64,
}

#[derive(Debug, Clone)]
pub struct ConversionResult {
    pub source_value: f64,
    pub source_unit: &'static UnitDef,
    pub target_value: f64,
    pub target_unit: &'static UnitDef,
}

#[derive(Debug, Clone)]
struct ParsedQuery {
    value: f64,
    source_unit: &'static UnitDef,
    target_unit: Option<&'static UnitDef>,
}

const UNITS: &[UnitDef] = &[
    // Length (base: meter)
    UnitDef {
        name: "mm",
        aliases: &[
            "mm",
            "millimeter",
            "millimetre",
            "millimeters",
            "millimetres",
        ],
        category: UnitCategory::Length,
        scale: 0.001,
        offset: 0.0,
    },
    UnitDef {
        name: "cm",
        aliases: &[
            "cm",
            "centimeter",
            "centimetre",
            "centimeters",
            "centimetres",
        ],
        category: UnitCategory::Length,
        scale: 0.01,
        offset: 0.0,
    },
    UnitDef {
        name: "m",
        aliases: &["m", "meter", "metre", "meters", "metres"],
        category: UnitCategory::Length,
        scale: 1.0,
        offset: 0.0,
    },
    UnitDef {
        name: "km",
        aliases: &["km", "kilometer", "kilometre", "kilometers", "kilometres"],
        category: UnitCategory::Length,
        scale: 1000.0,
        offset: 0.0,
    },
    UnitDef {
        name: "in",
        aliases: &["in", "inch", "inches"],
        category: UnitCategory::Length,
        scale: 0.0254,
        offset: 0.0,
    },
    UnitDef {
        name: "ft",
        aliases: &["ft", "foot", "feet"],
        category: UnitCategory::Length,
        scale: 0.3048,
        offset: 0.0,
    },
    UnitDef {
        name: "yd",
        aliases: &["yd", "yard", "yards"],
        category: UnitCategory::Length,
        scale: 0.9144,
        offset: 0.0,
    },
    UnitDef {
        name: "mi",
        aliases: &["mi", "mile", "miles"],
        category: UnitCategory::Length,
        scale: 1609.344,
        offset: 0.0,
    },
    // Mass (base: gram)
    UnitDef {
        name: "mg",
        aliases: &["mg", "milligram", "milligrams"],
        category: UnitCategory::Mass,
        scale: 0.001,
        offset: 0.0,
    },
    UnitDef {
        name: "g",
        aliases: &["g", "gram", "grams"],
        category: UnitCategory::Mass,
        scale: 1.0,
        offset: 0.0,
    },
    UnitDef {
        name: "kg",
        aliases: &["kg", "kilogram", "kilograms"],
        category: UnitCategory::Mass,
        scale: 1000.0,
        offset: 0.0,
    },
    UnitDef {
        name: "oz",
        aliases: &["oz", "ounce", "ounces"],
        category: UnitCategory::Mass,
        scale: 28.349_523_125,
        offset: 0.0,
    },
    UnitDef {
        name: "lb",
        aliases: &["lb", "lbs", "pound", "pounds"],
        category: UnitCategory::Mass,
        scale: 453.592_37,
        offset: 0.0,
    },
    // Volume (base: liter)
    UnitDef {
        name: "ml",
        aliases: &[
            "ml",
            "milliliter",
            "millilitre",
            "milliliters",
            "millilitres",
        ],
        category: UnitCategory::Volume,
        scale: 0.001,
        offset: 0.0,
    },
    UnitDef {
        name: "l",
        aliases: &["l", "liter", "litre", "liters", "litres"],
        category: UnitCategory::Volume,
        scale: 1.0,
        offset: 0.0,
    },
    UnitDef {
        name: "tsp",
        aliases: &["tsp", "teaspoon", "teaspoons"],
        category: UnitCategory::Volume,
        scale: 0.004_928_921_593_75,
        offset: 0.0,
    },
    UnitDef {
        name: "tbsp",
        aliases: &["tbsp", "tablespoon", "tablespoons"],
        category: UnitCategory::Volume,
        scale: 0.014_786_764_781_25,
        offset: 0.0,
    },
    UnitDef {
        name: "floz",
        aliases: &["floz", "fl-oz", "fl_oz", "fluidounce", "fluidounces"],
        category: UnitCategory::Volume,
        scale: 0.029_573_529_562_5,
        offset: 0.0,
    },
    UnitDef {
        name: "cup",
        aliases: &["cup", "cups"],
        category: UnitCategory::Volume,
        scale: 0.236_588_236_5,
        offset: 0.0,
    },
    UnitDef {
        name: "pt",
        aliases: &["pt", "pint", "pints"],
        category: UnitCategory::Volume,
        scale: 0.473_176_473,
        offset: 0.0,
    },
    UnitDef {
        name: "qt",
        aliases: &["qt", "quart", "quarts"],
        category: UnitCategory::Volume,
        scale: 0.946_352_946,
        offset: 0.0,
    },
    UnitDef {
        name: "gal",
        aliases: &["gal", "gallon", "gallons"],
        category: UnitCategory::Volume,
        scale: 3.785_411_784,
        offset: 0.0,
    },
    // Temperature (base: celsius)
    UnitDef {
        name: "c",
        aliases: &["c", "celsius", "centigrade"],
        category: UnitCategory::Temperature,
        scale: 1.0,
        offset: 0.0,
    },
    UnitDef {
        name: "f",
        aliases: &["f", "fahrenheit"],
        category: UnitCategory::Temperature,
        scale: 5.0 / 9.0,
        offset: -32.0,
    },
    UnitDef {
        name: "k",
        aliases: &["k", "kelvin", "kelvins"],
        category: UnitCategory::Temperature,
        scale: 1.0,
        offset: -273.15,
    },
];

impl ToApp for ConversionResult {
    fn to_app(&self) -> crate::app::apps::App {
        let source = format!(
            "{} {}",
            format_number(self.source_value),
            self.source_unit.name
        );
        let target = format!(
            "{} {}",
            format_number(self.target_value),
            self.target_unit.name
        );
        App {
            ranking: 0,
            open_command: AppCommand::Function(Function::CopyToClipboard(
                ClipBoardContentType::Text(target.clone()),
            )),
            desc: source,
            icons: None,
            display_name: target,
            search_name: String::new(),
        }
    }
}

pub fn convert_query(query: &str) -> Option<Vec<ConversionResult>> {
    let parsed = parse_query(query)?;
    let base_value = to_base(parsed.value, parsed.source_unit);

    let mut results = Vec::new();
    let targets: Vec<&UnitDef> = match parsed.target_unit {
        Some(target) => vec![target],
        None => UNITS
            .iter()
            .filter(|unit| unit.category == parsed.source_unit.category)
            .collect(),
    };

    for target_unit in targets {
        if target_unit.name == parsed.source_unit.name {
            continue;
        }
        let target_value = from_base(base_value, target_unit);
        results.push(ConversionResult {
            source_value: parsed.value,
            source_unit: parsed.source_unit,
            target_value,
            target_unit,
        });
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

pub fn format_number(value: f64) -> String {
    let value = if value.abs() < 1e-9 { 0.0 } else { value };
    let mut s = format!("{value:.6}");
    if let Some(dot_pos) = s.find('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') && dot_pos == s.len() - 1 {
            s.pop();
        }
    }
    s
}

fn parse_query(query: &str) -> Option<ParsedQuery> {
    let (value_str, rest) = parse_number_prefix(query)?;
    let value: f64 = value_str.parse().ok()?;
    let rest = rest.trim_start();
    if rest.is_empty() {
        return None;
    }

    let rest_lc = rest.to_lowercase();
    let tokens: Vec<&str> = rest_lc.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let source_unit = find_unit(tokens[0])?;
    match tokens.len() {
        1 => Some(ParsedQuery {
            value,
            source_unit,
            target_unit: None,
        }),
        2 => {
            let target_unit = find_unit(tokens[1])?;
            if target_unit.category != source_unit.category {
                return None;
            }
            Some(ParsedQuery {
                value,
                source_unit,
                target_unit: Some(target_unit),
            })
        }
        3 if tokens[1] == "to" || tokens[1] == "in" => {
            let target_unit = find_unit(tokens[2])?;
            if target_unit.category != source_unit.category {
                return None;
            }
            Some(ParsedQuery {
                value,
                source_unit,
                target_unit: Some(target_unit),
            })
        }
        _ => None,
    }
}

fn parse_number_prefix(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }

    let mut chars = s.char_indices().peekable();
    if let Some((_, c)) = chars.peek()
        && (*c == '+' || *c == '-')
    {
        chars.next();
    }

    let mut end = 0;
    let mut has_digit = false;
    while let Some((idx, c)) = chars.peek().cloned() {
        if c.is_ascii_digit() {
            has_digit = true;
            end = idx + c.len_utf8();
            chars.next();
        } else if c == '.' {
            end = idx + c.len_utf8();
            chars.next();
        } else {
            break;
        }
    }

    if !has_digit || end == 0 {
        return None;
    }

    let (num, rest) = s.split_at(end);
    Some((num, rest))
}

fn find_unit(token: &str) -> Option<&'static UnitDef> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    UNITS
        .iter()
        .find(|unit| unit.name == token || unit.aliases.contains(&token))
}

fn to_base(value: f64, unit: &UnitDef) -> f64 {
    (value + unit.offset) * unit.scale
}

fn from_base(value: f64, unit: &UnitDef) -> f64 {
    value / unit.scale - unit.offset
}
