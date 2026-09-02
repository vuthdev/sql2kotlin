use std::{collections::{HashSet}, fmt::Write};

use heck::{ToLowerCamelCase, ToPascalCase};
use sqlparser::{ast::{ColumnOption, DataType::{self}, GeneratedAs, ObjectNamePart, Statement::{self}, TableConstraint, TimezoneInfo}, dialect::GenericDialect, parser::Parser};

fn main() {
    let sql = r#"
        create table public."Otp" (
           	id uuid not null,
           	"createdAt" timestamptz default CURRENT_TIMESTAMP null,
           	"createdById" int8 null,
           	"deletedAt" timestamptz null,
           	"updatedAt" timestamptz default CURRENT_TIMESTAMP null,
           	"updatedById" int8 null,
           	"version" int8 default 0 null,
           	app int2 null,
           	code varchar(50) null,
           	"customerId" uuid null,
           	device int2 null,
           	"deviceModel" varchar(255) null,
           	email varchar(100) null,
           	"expiredAt" timestamptz null,
           	hash varchar(1000) null,
           	"otpHash" varchar(50) null,
           	"phoneNumber" varchar(50) null,
           	"sendOtpOption" int2 default 1 null,
           	status int2 null,
           	"type" int2 null,
           	udid varchar(100) null,
           	"userId" int8 null,
           	constraint "Otp_id_not_null" not null id,
           	constraint "Otp_pkey" primary key (id)
        );
    "#;

    let cleaned_sql = strip_unsupported_not_null_constraints(sql);

    let dialect = GenericDialect {};
    let statements = Parser::parse_sql(&dialect, &cleaned_sql).expect("Failed to parse SQL");

    for statement in statements {
        if let Statement::CreateTable(stmt_create_table) = statement {
            let column_names: Vec<String> = stmt_create_table
                .columns
                .iter()
                .map(|col| col.name.value.clone())
                .collect();

            let entity_type = check_base_entity(&column_names);

            let excluded_fields: HashSet<&str> = match entity_type {
                Some("BaseWithDeleteEntity") => [
                    "createdAt",
                    "createdById",
                    "updatedAt",
                    "updatedById",
                    "version",
                    "deletedAt",
                ]
                .into_iter()
                .collect(),
                Some("BaseEntity") => [
                    "createdAt",
                    "version",
                    "createdById",
                    "updatedAt",
                    "updatedById",
                ]
                .into_iter()
                .collect(),
                _ => HashSet::new(),
            };

            let table_name: Option<String> = stmt_create_table
                .name.0.last().and_then(|part| match part {
                    ObjectNamePart::Identifier(ident) => Some(ident.value.clone()),
                    ObjectNamePart::Function(f) => Some(f.to_string()),
                });

            let pk_columns: HashSet<String> = stmt_create_table
                .constraints
                .iter()
                .filter_map(|c| match c {
                    TableConstraint::PrimaryKey(pk) => Some(
                        pk.columns
                            .iter()
                            .map(|idx_col| idx_col.column.to_string())
                            .collect::<Vec<_>>()
                    ),
                    _ => None,
                })
                .flatten()
                .collect();

            if let Some(table_name) = table_name {
                println!();
                println!("--- Parsed Table ---");
                println!("@Entity");
                println!("@Table(name = \"{}\")", format_identifier(&table_name));
                println!("class {}(", table_name);
                
                for column in stmt_create_table.columns {
                    let col_name = &column.name.value;
                    
                    if excluded_fields.contains(col_name.as_str()) {
                        continue;
                    }

                    let mut is_pk = pk_columns.contains(col_name.as_str());
                    let mut generated_annotation: Option<String> = None;
                    let mut more_column_options: Vec<String> = Vec::new();
                    
                    for column_option in &column.options {
                        match &column_option.option {
                            ColumnOption::PrimaryKey(_) => is_pk = true,
                            ColumnOption::NotNull => more_column_options.push("nullable = false".to_string()),
                            ColumnOption::Unique(_) => more_column_options.push("unique = true".to_string()),
                            ColumnOption::Generated {
                                generated_as,
                                generation_expr,
                                ..
                            } => {
                                generated_annotation = Some(match (generated_as, generation_expr) {
                                    // identity/sequence-backed generation -> DB assigns the value
                                    (GeneratedAs::Always, None) => {
                                        "  @GeneratedValue(strategy = GenerationType.IDENTITY)".to_string()
                                    }
                                    (GeneratedAs::ByDefault, None) => {
                                        "  @GeneratedValue(strategy = GenerationType.IDENTITY)".to_string()
                                    }
                                    // has an actual expression -> it's a computed column, not an auto-increment
                                    (_, Some(expr)) => {
                                        format!("  @Generated(GenerationTime.INSERT) // computed: {}", expr)
                                    }
                                    _ => "  @GeneratedValue".to_string(),
                                });
                            }
                            _ => {}
                        }
                    }

                    if matches!(column.data_type, DataType::Text) {
                        more_column_options.push("columnDefinition = \"TEXT\"".to_string());
                    }
                    
                    let mut column_annotation = String::new();
                    if is_pk {
                        column_annotation.push_str("  @Id\n");
                        column_annotation.push_str(
                            if column.data_type.eq(&DataType::Uuid) {
                                "  @GeneratedValue(strategy = GenerationType.UUID)"
                            } else {
                                generated_annotation
                                    .as_deref()
                                    .unwrap_or("  @GeneratedValue(strategy = GenerationType.AUTO)")
                            }
                        );
                    } else {
                        let extra = if more_column_options.is_empty() {
                            String::new()
                        } else {
                            format!(", {}", more_column_options.join(", "))
                        };
                        write!(
                            column_annotation,
                            "  @Column(name = \"{}\"{})",
                            format_identifier(col_name),
                            extra
                        ).unwrap();
                        if let Some(r#gen) = &generated_annotation {
                            write!(column_annotation, "\n{}", r#gen).unwrap();
                        }
                    }

                    println!("{}", column_annotation);

                    let variable_pk: &str = if is_pk { "val" }  else { "var" };
                    
                    println!(
                        "  {} {}: {}? = null,",
                        variable_pk,
                        col_name,
                        format_data_type(&column.data_type)
                    );
                    println!();
                }
                if let Some(base_class) = entity_type {
                    println!(") : {}()", base_class);
                } else {
                    println!(")");
                }
            }
        } else {
            println!("Invalid create table sql query!")
        }
    }
}

fn format_data_type(data_type: &DataType) -> String {
    match data_type {
        DataType::Uuid => "UUID".to_string(),
        DataType::Int2(_) => "Short".to_string(),
        DataType::Bool => "Boolean".to_string(),
        DataType::Text => "String".to_string(),
        DataType::Int4(_) => "Int".to_string(),
        DataType::Int8(_) => "Int".to_string(),
        DataType::Int16 => "Int".to_string(),
        DataType::Int32 => "Int".to_string(),
        DataType::Int64 => "Int".to_string(),
        DataType::Int128 => "Int".to_string(),
        DataType::Int256 => "Int".to_string(),
        DataType::Integer(_) => "Int".to_string(),
        DataType::SmallInt(_) => "Short".to_string(),
        DataType::Varchar(_) => "String".to_string(),
        DataType::DoublePrecision => "Double".to_string(),
        DataType::Double(_) => "Double".to_string(),
        DataType::Float8 => "Double".to_string(),
        DataType::Decimal(_) => "BigDecimal".to_string(),
        DataType::Timestamp(_precision, TimezoneInfo::WithTimeZone | TimezoneInfo::Tz) => {
                    "OffsetDateTime".to_string() // TIMESTAMPTZ
                }
                DataType::Timestamp(_precision, TimezoneInfo::None) => {
                    "LocalDateTime".to_string()  // TIMESTAMP without timezone
                }
        DataType::BigInt(_) => "Long".to_string(),
        DataType::Date => "Date".to_string(),
        _ => data_type.to_string()
    }
}

fn check_base_entity(column_names: &[String]) -> Option<&'static str> {
    let actual: HashSet<&str> = column_names.iter().map(|s| s.as_str()).collect();

    let base_audit: HashSet<&str> = [
        "createdAt",
        "updatedAt",
        "createdById",
        "updatedById",
        "version",
    ]
    .into_iter()
    .collect();

    let has_base_audit = base_audit.is_subset(&actual);
    let has_delete = actual.contains("deletedAt");

    if has_base_audit && has_delete {
        Some("BaseWithDeleteEntity")
    } else if has_base_audit {
        Some("BaseEntity")
    } else {
        None
    }
}

fn format_identifier(s: &str) -> String {
    let needs_quoting = s.chars().any(|c| c.is_uppercase());

    if needs_quoting {
        format!("`{}`", s)
    } else {
        s.to_string()
    }
}

fn strip_unsupported_not_null_constraints(sql: &str) -> String {

    let filtered: Vec<&str> = sql
        .lines()
        .filter(|line| {
            let trimmed = line.trim().to_lowercase();
            !(trimmed.starts_with("constraint") && trimmed.contains("not null"))
        })
        .collect();

    let joined = filtered.join("\n");
    // collapse ",\n    );" -> "\n    );" so we don't leave a dangling comma
    let re_comma_before_close = joined.replace(",\n        );", "\n        );");
    re_comma_before_close
}
