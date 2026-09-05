use std::{collections::{HashSet}, fmt::Write};

use heck::{ToLowerCamelCase, ToPascalCase};
use sqlparser::{ast::{ColumnOption, DataType::{self}, GeneratedAs, ObjectNamePart, Statement::{self}, TableConstraint, TimezoneInfo}, dialect::GenericDialect, parser::Parser};

fn main() {
    let sql = r#"
        -- SQLINES FOR EVALUATION USE ONLY
        CREATE TABLE users (
            -- Sy... SQLINES DEMO ***
            user_id          NUMBER(19) GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            uuid             CHAR(36) DEFAULT gen_random_uuid() UNIQUE NOT NULL,
            
            -- SQLINES DEMO ***  & Security
            username         VARCHAR2(50 CHAR) UNIQUE NOT NULL,
            email            VARCHAR2(255 CHAR) UNIQUE NOT NULL,
            password_hash    VARCHAR2(255 CHAR) NOT NULL,
            two_factor_enabled CHAR(1) DEFAULT FALSE,
            security_pin     VARCHAR2(6 CHAR),
            
            -- SQLINES DEMO *** Information
            first_name       VARCHAR2(50 CHAR) NOT NULL,
            last_name        VARCHAR2(50 CHAR) NOT NULL,
            middle_name      VARCHAR2(50 CHAR),
            display_name     VARCHAR2(100 CHAR),
            gender           VARCHAR2(20 CHAR),
            date_of_birth    DATE,
            avatar_url       VARCHAR2(4000),
            bio              VARCHAR2(4000),
            
            -- Co... SQLINES DEMO ***
            phone_number     VARCHAR2(20 CHAR),
            secondary_email  VARCHAR2(255 CHAR),
            website_url      VARCHAR2(4000),
            
            -- Ad... SQLINES DEMO ***
            street_address_1 VARCHAR2(255 CHAR),
            street_address_2 VARCHAR2(100 CHAR),
            city             VARCHAR2(100 CHAR),
            state_province   VARCHAR2(100 CHAR),
            postal_code      VARCHAR2(20 CHAR),
            country_code     CHAR(2),
            
            -- SQLINES DEMO *** ings
            language_code    VARCHAR2(10 CHAR) DEFAULT 'en',
            timezone         VARCHAR2(50 CHAR) DEFAULT 'UTC',
            currency_code    CHAR(3) DEFAULT 'USD',
            
            -- SQLINES DEMO *** Role
            role             VARCHAR2(30 CHAR) DEFAULT 'user',
            status           VARCHAR2(20 CHAR) DEFAULT 'active',
            is_email_verified CHAR(1) DEFAULT FALSE,
            is_phone_verified CHAR(1) DEFAULT FALSE,
            
            -- Au... SQLINES DEMO ***
            created_at       TIMESTAMP WITH TIME ZONE DEFAULT SYSTIMESTAMP NOT NULL,
            updated_at       TIMESTAMP WITH TIME ZONE DEFAULT SYSTIMESTAMP NOT NULL,
            last_login_at    TIMESTAMP WITH TIME ZONE,
            deleted_at       TIMESTAMP WITH TIME ZONE,
            created_by_id      number(19) null,
            updated_by_id      number(19) null,
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
                    "deletedAt",
                ]
                .into_iter()
                .collect(),
                Some("BaseEntity") => [
                    "createdAt",
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
                        col_name.to_lower_camel_case(),
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
        DataType::Boolean => "Boolean".to_string(),
        DataType::Text => "String".to_string(),
        DataType::Int(_) => "Int".to_string(),
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
        DataType::Float8 => "Float".to_string(),
        DataType::Float(_) => "Float".to_string(),
        DataType::Decimal(_) => "BigDecimal".to_string(),
        DataType::Numeric(_) => "BigDecimal".to_string(),
        DataType::Char(_) => "String".to_string(),
        DataType::JSON => "MutableMap<String, String>".to_string(),
        DataType::JSONB => "MutableMap<String, String>".to_string(),
        DataType::Timestamp(_precision, TimezoneInfo::WithTimeZone | TimezoneInfo::Tz) => {
                    "OffsetDateTime".to_string() // TIMESTAMPTZ
                }
                DataType::Timestamp(_precision, TimezoneInfo::None) => {
                    "LocalDateTime".to_string()  // TIMESTAMP without timezone
                }
        DataType::BigInt(_) => "Long".to_string(),
        DataType::Date => "Date".to_string(),
        DataType::Date32 => "LocalDate".to_string(), 
        _ => data_type.to_string()
    }
}

fn check_base_entity(column_names: &[String]) -> Option<&'static str> {
    let actual: HashSet<String> = column_names
        .iter()
        .flat_map(|s| [s.to_string(), s.to_lower_camel_case()])
        .collect();
    
    // Minimum required base fields for audit
    let base_audit: HashSet<String> = [
        "createdAt",
        "updatedAt",
        "createdById",
        "updatedById",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    
    let has_base_audit = base_audit.is_subset(&actual);
    let has_delete = actual.contains("deletedAt") || actual.contains("deleted_at");
    
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
