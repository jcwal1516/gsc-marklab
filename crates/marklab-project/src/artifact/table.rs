use std::collections::BTreeMap;

use super::{
    identity::is_identifier, TableColumn, TableColumnType, TableFormat, TableManifest,
    TableManifestError, TableScalarType, MAX_IDENTIFIER_BYTES, MAX_TABLE_COLUMNS,
};

impl TableFormat {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::ArrowIpcFile => "arrow_ipc_file",
            Self::ParquetFile => "parquet_file",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Result<Self, TableManifestError> {
        match value {
            "arrow_ipc_file" => Ok(Self::ArrowIpcFile),
            "parquet_file" => Ok(Self::ParquetFile),
            _ => Err(TableManifestError::InvalidFormat {
                value: value.to_owned(),
            }),
        }
    }
}

impl TableScalarType {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Utf8 => "utf8",
            Self::Binary => "binary",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Result<Self, TableManifestError> {
        match value {
            "boolean" => Ok(Self::Boolean),
            "i64" => Ok(Self::I64),
            "u64" => Ok(Self::U64),
            "f32" => Ok(Self::F32),
            "f64" => Ok(Self::F64),
            "utf8" => Ok(Self::Utf8),
            "binary" => Ok(Self::Binary),
            _ => Err(TableManifestError::InvalidColumnType {
                value: value.to_owned(),
            }),
        }
    }

    fn stable_primary_key(self) -> bool {
        !matches!(self, Self::F32 | Self::F64)
    }
}

impl TableColumn {
    /// Build a column after validating its stable name and type bounds.
    pub fn new(
        name: impl Into<String>,
        column_type: TableColumnType,
        nullable: bool,
    ) -> Result<Self, TableManifestError> {
        let name = name.into();
        if !is_identifier(&name) {
            return Err(TableManifestError::InvalidColumnName { value: name });
        }
        if matches!(
            column_type,
            TableColumnType::FixedSizeList { length: 0, .. }
        ) {
            return Err(TableManifestError::InvalidFixedListLength { column: name });
        }
        Ok(Self {
            name,
            column_type,
            nullable,
        })
    }

    /// Column name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declared column type.
    pub fn column_type(&self) -> &TableColumnType {
        &self.column_type
    }

    /// Whether null values are permitted.
    pub fn nullable(&self) -> bool {
        self.nullable
    }
}

impl TableManifest {
    /// Build and validate an exact ordered table declaration.
    pub fn new(
        format: TableFormat,
        encoding_version: impl Into<String>,
        row_count: u64,
        columns: Vec<TableColumn>,
        primary_key: Vec<String>,
    ) -> Result<Self, TableManifestError> {
        let encoding_version = encoding_version.into();
        if encoding_version.is_empty()
            || encoding_version.len() > MAX_IDENTIFIER_BYTES
            || !encoding_version.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(TableManifestError::InvalidEncodingVersion {
                value: encoding_version,
            });
        }
        if columns.is_empty() || columns.len() > MAX_TABLE_COLUMNS {
            return Err(TableManifestError::InvalidColumnCount {
                observed: columns.len(),
                maximum: MAX_TABLE_COLUMNS,
            });
        }
        let mut by_name = BTreeMap::new();
        for (index, column) in columns.iter().enumerate() {
            if by_name.insert(column.name.clone(), index).is_some() {
                return Err(TableManifestError::DuplicateColumn {
                    column: column.name.clone(),
                });
            }
        }
        if primary_key.is_empty() {
            return Err(TableManifestError::EmptyPrimaryKey);
        }
        let mut seen_keys = BTreeMap::new();
        for key in &primary_key {
            if seen_keys.insert(key.clone(), ()).is_some() {
                return Err(TableManifestError::DuplicatePrimaryKey {
                    column: key.clone(),
                });
            }
            let Some(index) = by_name.get(key) else {
                return Err(TableManifestError::MissingPrimaryKeyColumn {
                    column: key.clone(),
                });
            };
            let column = &columns[*index];
            if column.nullable {
                return Err(TableManifestError::NullablePrimaryKey {
                    column: key.clone(),
                });
            }
            let stable = match column.column_type {
                TableColumnType::Scalar(scalar) => scalar.stable_primary_key(),
                TableColumnType::FixedSizeList { .. } => false,
            };
            if !stable {
                return Err(TableManifestError::UnstablePrimaryKeyType {
                    column: key.clone(),
                });
            }
        }
        Ok(Self {
            format,
            encoding_version,
            row_count,
            columns,
            primary_key,
        })
    }

    /// Declared file format.
    pub fn format(&self) -> TableFormat {
        self.format
    }

    /// Explicit writer/encoding contract version.
    pub fn encoding_version(&self) -> &str {
        &self.encoding_version
    }

    /// Exact row count.
    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    /// Ordered columns.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Ordered non-null stable primary-key columns.
    pub fn primary_key(&self) -> &[String] {
        &self.primary_key
    }

    /// Require an observed declaration to match every semantic field exactly.
    pub fn require_exact(&self, observed: &Self) -> Result<(), TableManifestError> {
        let field = if self.format != observed.format {
            "format"
        } else if self.encoding_version != observed.encoding_version {
            "encoding_version"
        } else if self.row_count != observed.row_count {
            "row_count"
        } else if self.columns != observed.columns {
            "columns"
        } else if self.primary_key != observed.primary_key {
            "primary_key"
        } else {
            return Ok(());
        };
        Err(TableManifestError::Mismatch { field })
    }
}
